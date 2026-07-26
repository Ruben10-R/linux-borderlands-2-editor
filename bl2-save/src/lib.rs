//! `bl2-save` — Borderlands 2 save-file core.
//!
//! The product: decode a `.sav` to its inner protobuf, edit specific fields
//! surgically (every untouched byte is preserved), and re-encode to a file the
//! game accepts. A CLI and (later) a GUI sit on top of this crate.
//!
//! ```no_run
//! use bl2_save::SaveFile;
//! let mut save = SaveFile::load("save0001.sav")?;
//! println!("{} — level {}", save.class_name().unwrap_or_default(), save.level().unwrap_or(0));
//! save.set_money(99_999_999)?;
//! save.save("save0001.sav", /* backup = */ true)?;
//! # Ok::<(), bl2_save::SaveError>(())
//! ```

mod capacity;
mod catalog;
mod codec;
mod create;
mod customizations;
mod error;
mod gameinfo;
mod item_codes;
mod items;
mod levels;
mod profile;
mod proto;
mod serial;
mod stations;
mod vehicles;

pub use levels::{level_for_xp, xp_for_level};
pub use profile::ProfileFile;
pub use vehicles::{VehicleFamily, VehicleSkin, FAMILIES as VEHICLE_FAMILIES};

pub use catalog::*;
pub use create::ImportGroup;
pub use customizations::Customization;
pub use error::{Result, SaveError};
pub use item_codes::{code_library, library_categories, LibraryItem};
pub use items::{Item, Location};
pub use serial::{ItemSerial, PartRef};
pub use stations::Station;

use std::fs;
use std::path::Path;

/// Highest Overpower level the game supports: OP1–OP8, each earned by clearing
/// Digistruct Peak again at level 72 with UVHM finished. 0 means no OP level.
///
/// The setter itself doesn't clamp — it stays faithful to whatever it's given —
/// so frontends clamp to this when offering the value to a user.
pub const MAX_OP_LEVEL: i64 = 8;

/// A loaded Borderlands 2 save, held as its decoded inner protobuf.
///
/// All edits mutate the protobuf surgically; call [`SaveFile::save`] to re-encode.
pub struct SaveFile {
    proto: Vec<u8>,
}

impl SaveFile {
    /// Decode a full `.sav` byte buffer, validating SHA1 + CRC.
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        // Check the SHA1 before decompressing. Nothing is lost — a file whose
        // hash is wrong was going to be rejected anyway — and it keeps arbitrary
        // bytes out of the LZO decompressor, which panics on some malformed
        // streams. That panic is contained on native but NOT on wasm32, where
        // panics abort, so gating here is what protects the web build.
        // (`ProfileFile::from_bytes` has always done this.)
        if raw.len() < 24 {
            return Err(SaveError::TooShort(raw.len()));
        }
        if !codec::sha1_matches(raw) {
            return Err(SaveError::Sha1Mismatch);
        }
        let dec = codec::decode(raw)?;
        // The hash is already known good, so the CRC over the protobuf is the
        // only checksum left that can disagree.
        if dec.crc_stored != dec.crc_calc {
            return Err(SaveError::CrcMismatch {
                stored: dec.crc_stored,
                computed: dec.crc_calc,
            });
        }
        Ok(Self { proto: dec.proto })
    }

    /// Read and decode a `.sav` from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_bytes(&fs::read(path)?)
    }

    /// Re-encode to full `.sav` bytes. Runs a self-check: the bytes are decoded
    /// again and must reproduce the exact protobuf, or we refuse (returns Err).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let bytes = codec::encode(&self.proto)?;
        let check = codec::decode(&bytes)?;
        if !check.is_valid() {
            return Err(SaveError::SelfVerify("re-decoded checksums invalid".into()));
        }
        if check.proto != self.proto {
            return Err(SaveError::SelfVerify("re-decoded protobuf differs".into()));
        }
        Ok(bytes)
    }

    /// Encode and write to disk. When `backup` is true and the target exists, the
    /// current file is copied to `<path>.bak` first (never overwrites an existing
    /// `.bak`'s value silently — it's a plain copy of what's there now).
    pub fn save(&self, path: impl AsRef<Path>, backup: bool) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.to_bytes()?; // self-verify before we touch the disk
        if backup && path.exists() {
            let mut bak = path.as_os_str().to_owned();
            bak.push(".bak");
            fs::copy(path, &bak)?;
        }
        fs::write(path, &bytes)?;
        Ok(())
    }

    // ---------- reads ----------

    fn fields(&self) -> Result<Vec<proto::Field>> {
        proto::parse_fields(&self.proto)
    }

    /// A named, grouped summary of every top-level protobuf field, in first-seen
    /// order — for the Raw inspector. Single-occurrence varint/string fields are
    /// exposed as editable `value`/`text`; repeated or nested fields are summarised.
    pub fn raw_fields(&self) -> Result<Vec<RawField>> {
        let fields = self.fields()?;
        let mut out = Vec::new();
        let mut seen: Vec<u64> = Vec::new();
        for f in &fields {
            if seen.contains(&f.number) {
                continue;
            }
            seen.push(f.number);
            let occ: Vec<&proto::Field> = fields.iter().filter(|x| x.number == f.number).collect();
            let name = proto::field_name(f.number);
            let mut rf = RawField {
                number: f.number,
                name,
                help: proto::field_help(f.number),
                kind: "bytes",
                count: occ.len(),
                value: None,
                text: None,
                preview: String::new(),
            };
            if occ.len() > 1 {
                rf.kind = "collection";
                rf.preview = format!("{} entries", occ.len());
            } else {
                match f.wire_type {
                    0 => {
                        let v = proto::read_varint_value(&self.proto, f.val_start).unwrap_or(0);
                        rf.kind = "varint";
                        rf.value = Some(v);
                        rf.preview = v.to_string();
                    }
                    2 => {
                        let content = proto::wire2_content(&self.proto, f)?;
                        let printable = !content.is_empty()
                            && content
                                .iter()
                                .all(|&b| b == b'\t' || b == b'\n' || (0x20..0x7f).contains(&b));
                        if printable {
                            let s = String::from_utf8_lossy(content).into_owned();
                            rf.kind = "string";
                            rf.preview = format!("{s:?}");
                            rf.text = Some(s);
                        } else {
                            rf.kind = "message";
                            rf.preview = format!("(message, {} bytes)", content.len());
                        }
                    }
                    1 => {
                        rf.kind = "fixed64";
                        rf.preview = "(8 bytes)".into();
                    }
                    5 => {
                        rf.kind = "fixed32";
                        rf.preview = "(4 bytes)".into();
                    }
                    _ => {}
                }
            }
            out.push(rf);
        }
        Ok(out)
    }

    /// Set a single top-level varint field (Raw editor). Only that field changes.
    pub fn set_raw_varint(&mut self, number: u64, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::upsert_varint_field(&self.proto, &fields, number, value);
        proto::only_fields_changed(&self.proto, &new, &[number])?;
        self.proto = new;
        Ok(())
    }

    /// Set a single top-level UTF-8 string field (Raw editor). Only that field changes.
    pub fn set_raw_string(&mut self, number: u64, value: &str) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::rewrite_string_field(&self.proto, &fields, number, value)?;
        proto::only_fields_changed(&self.proto, &new, &[number])?;
        self.proto = new;
        Ok(())
    }

    /// The character's class as the game shows it (e.g. "Zer0 (Assassin)").
    pub fn class_name(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        let def = proto::read_string_field(&self.proto, &fields, proto::FIELD_CLASS)?;
        Some(proto::class_name(&def).to_string())
    }

    /// Experience level.
    pub fn level(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_LEVEL)
    }

    /// Experience points.
    pub fn xp(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_XP)
    }

    /// Available (general) skill points.
    pub fn skill_points(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_SKILL_POINTS)
    }

    /// Specialist skill points (the second pool, used by some class mechanics).
    pub fn specialist_skill_points(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_SPECIALIST_SKILL_POINTS)
    }

    /// Playthroughs completed (0–3). 1 = TVHM unlocked, 2 = UVHM unlocked.
    pub fn playthroughs_completed(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_PLAYTHROUGHS_COMPLETED)
    }

    /// Current playthrough index (0 = Normal, 1 = TVHM, 2 = UVHM). Absent ⇒ 0.
    pub fn active_playthrough(&self) -> i64 {
        self.fields()
            .ok()
            .and_then(|fs| {
                proto::read_varint_field(&self.proto, &fs, proto::FIELD_ACTIVE_PLAYTHROUGH)
            })
            .unwrap_or(0)
    }

    /// Save-game id (identifier, shown for reference).
    pub fn save_game_id(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_SAVE_GAME_ID)
    }

    /// Total seconds of play time recorded on this save.
    pub fn time_played(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_TIME_PLAYED)
    }

    /// Raw class-definition asset path (e.g. "GD_Siren.Character.CharClass_Siren").
    pub fn class_def(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        proto::read_string_field(&self.proto, &fields, proto::FIELD_CLASS)
    }

    /// Character name (from the appearance message, sub-field 1).
    pub fn name(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        let f = fields
            .iter()
            .find(|f| f.number == proto::FIELD_APPEARANCE && f.wire_type == 2)?;
        let inner = proto::wire2_content(&self.proto, f).ok()?;
        let ifields = proto::parse_fields(inner).ok()?;
        proto::read_string_field(inner, &ifields, 1)
    }

    /// The fast-travel stations this character has unlocked (protobuf field 16,
    /// `repeated string visited_teleporters`), e.g. "SouthernShelfTown".
    pub fn visited_stations(&self) -> Vec<String> {
        let Ok(fields) = self.fields() else {
            return Vec::new();
        };
        fields
            .iter()
            .filter(|f| f.number == 16 && f.wire_type == 2)
            .filter_map(|f| {
                let c = proto::wire2_content(&self.proto, f).ok()?;
                std::str::from_utf8(c).ok().map(str::to_string)
            })
            .collect()
    }

    /// The "wearing" list (field 35): index 0 = head path, index 4 = skin path,
    /// with "0" placeholders between. Returns the raw strings.
    pub fn wearing(&self) -> Vec<String> {
        let Ok(fields) = self.fields() else {
            return Vec::new();
        };
        fields
            .iter()
            .filter(|f| f.number == 35 && f.wire_type == 2)
            .filter_map(|f| {
                let c = proto::wire2_content(&self.proto, f).ok()?;
                std::str::from_utf8(c).ok().map(str::to_string)
            })
            .collect()
    }

    /// The station the character last fast-travelled to (field 17).
    pub fn last_station(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        let f = fields.iter().find(|f| f.number == 17 && f.wire_type == 2)?;
        let c = proto::wire2_content(&self.proto, f).ok()?;
        std::str::from_utf8(c).ok().map(str::to_string)
    }

    /// The chosen skin paths for a vehicle family (from field 57).
    pub fn vehicle_family_skins(&self, family_path: &str) -> Vec<String> {
        vehicles::family_skins(&self.proto, family_path)
    }

    /// Set a vehicle family's chosen skins (empty/"None" entries are dropped).
    /// Only field 57 changes.
    pub fn set_vehicle_skins(&mut self, family_path: &str, skins: &[String]) -> Result<()> {
        let new = vehicles::set_family_skins(&self.proto, family_path, skins)?;
        proto::only_fields_changed(&self.proto, &new, &[57])?;
        self.proto = new;
        Ok(())
    }

    /// Set the equipped head and skin paths (field 35 "wearing", indices 0 and 4),
    /// preserving the "0" placeholders. Only field 35 changes.
    pub fn set_wearing(&mut self, head: &str, skin: &str) -> Result<()> {
        let fields = self.fields()?;
        let mut list = self.wearing();
        while list.len() < 5 {
            list.push("0".to_string());
        }
        list[0] = head.to_string();
        list[4] = skin.to_string();
        let new = proto::set_repeated_string_field(&self.proto, &fields, 35, &list);
        proto::only_fields_changed(&self.proto, &new, &[35])?;
        self.proto = new;
        Ok(())
    }

    /// Replace the unlocked fast-travel stations (field 16) with `resource_names`.
    /// Pass values from [`stations_catalog`]'s `rn`. Only field 16 changes.
    pub fn set_visited_stations(&mut self, resource_names: &[String]) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::set_repeated_string_field(&self.proto, &fields, 16, resource_names);
        proto::only_fields_changed(&self.proto, &new, &[16])?;
        self.proto = new;
        Ok(())
    }

    /// Full `currency_on_hand` array (index 0 = money, 1 = eridium, …).
    pub fn currency(&self) -> Result<Vec<i64>> {
        let fields = self.fields()?;
        proto::read_currency(&self.proto, &fields)
    }

    /// Money (dollars) — `currency_on_hand[0]`.
    pub fn money(&self) -> i64 {
        self.currency()
            .ok()
            .and_then(|c| c.get(proto::IDX_MONEY).copied())
            .unwrap_or(0)
    }

    /// Eridium — `currency_on_hand[1]`.
    pub fn eridium(&self) -> i64 {
        self.currency()
            .ok()
            .and_then(|c| c.get(proto::IDX_ERIDIUM).copied())
            .unwrap_or(0)
    }

    /// Seraph crystals — `currency_on_hand[2]`.
    pub fn seraph(&self) -> i64 {
        self.currency()
            .ok()
            .and_then(|c| c.get(proto::IDX_SERAPH).copied())
            .unwrap_or(0)
    }

    /// Torgue tokens — `currency_on_hand[4]`.
    pub fn torgue(&self) -> i64 {
        self.currency()
            .ok()
            .and_then(|c| c.get(proto::IDX_TORGUE).copied())
            .unwrap_or(0)
    }

    /// All decoded backpack + bank items and weapons.
    pub fn items(&self) -> Result<Vec<Item>> {
        items::read_items(&self.proto)
    }

    /// Unlocked Overpower level (0–8 in game), or None if the character has no
    /// OP data yet. Stored as a hidden "virtual item" (see
    /// [`SaveFile::set_op_level`]).
    pub fn op_level(&self) -> Option<i64> {
        items::read_op_level(&self.proto).ok().flatten()
    }

    /// Backpack slot count, if present (min 12, +3 per SDU).
    pub fn backpack_size(&self) -> Option<i64> {
        capacity::backpack_size(&self.proto)
    }

    /// Bank slot count (min 6, +2 per SDU).
    pub fn bank_size(&self) -> i64 {
        capacity::bank_size(&self.proto)
    }

    /// Set backpack capacity (snapped to the SDU grid: 12 + 3·n). Touches only
    /// the inventory-sizes and black-market fields.
    pub fn set_backpack_size(&mut self, slots: i64) -> Result<()> {
        let new = capacity::set_backpack_size(&self.proto, slots)?;
        proto::only_fields_changed(&self.proto, &new, &[13, 36])?;
        self.proto = new;
        Ok(())
    }

    /// Set bank capacity (snapped to the SDU grid: 6 + 2·n). Touches only the
    /// bank-size and black-market fields.
    pub fn set_bank_size(&mut self, slots: i64) -> Result<()> {
        let new = capacity::set_bank_size(&self.proto, slots)?;
        proto::only_fields_changed(&self.proto, &new, &[36, 56])?;
        self.proto = new;
        Ok(())
    }

    // ---------- edits (each guarded so only the intended field changes) ----------

    fn set_currency_index(&mut self, index: usize, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let mut currency = proto::read_currency(&self.proto, &fields)?;
        while currency.len() <= index {
            currency.push(0);
        }
        currency[index] = value;
        let new = proto::rewrite_currency(&self.proto, &fields, &currency)?;
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_CURRENCY])?;
        self.proto = new;
        Ok(())
    }

    /// Set money (`currency_on_hand[0]`).
    pub fn set_money(&mut self, value: i64) -> Result<()> {
        self.set_currency_index(proto::IDX_MONEY, value)
    }

    /// Set eridium (`currency_on_hand[1]`).
    pub fn set_eridium(&mut self, value: i64) -> Result<()> {
        self.set_currency_index(proto::IDX_ERIDIUM, value)
    }

    /// Set seraph crystals (`currency_on_hand[2]`).
    pub fn set_seraph(&mut self, value: i64) -> Result<()> {
        self.set_currency_index(proto::IDX_SERAPH, value)
    }

    /// Set torgue tokens (`currency_on_hand[4]`).
    pub fn set_torgue(&mut self, value: i64) -> Result<()> {
        self.set_currency_index(proto::IDX_TORGUE, value)
    }

    fn set_varint(&mut self, number: u64, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::rewrite_varint_field(&self.proto, &fields, number, value)?;
        proto::only_fields_changed(&self.proto, &new, &[number])?;
        self.proto = new;
        Ok(())
    }

    /// Set experience level. (Note: does not adjust XP or skill points.)
    pub fn set_level(&mut self, value: i64) -> Result<()> {
        self.set_varint(proto::FIELD_LEVEL, value)
    }

    /// Set experience points.
    pub fn set_xp(&mut self, value: i64) -> Result<()> {
        self.set_varint(proto::FIELD_XP, value)
    }

    /// Set available skill points (added if the field is absent).
    pub fn set_skill_points(&mut self, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new =
            proto::upsert_varint_field(&self.proto, &fields, proto::FIELD_SKILL_POINTS, value);
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_SKILL_POINTS])?;
        self.proto = new;
        Ok(())
    }

    /// Set specialist skill points (added if the field is absent).
    pub fn set_specialist_skill_points(&mut self, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::upsert_varint_field(
            &self.proto,
            &fields,
            proto::FIELD_SPECIALIST_SKILL_POINTS,
            value,
        );
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_SPECIALIST_SKILL_POINTS])?;
        self.proto = new;
        Ok(())
    }

    /// Set playthroughs completed (0–3). 1 unlocks TVHM, 2 unlocks UVHM.
    pub fn set_playthroughs_completed(&mut self, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::upsert_varint_field(
            &self.proto,
            &fields,
            proto::FIELD_PLAYTHROUGHS_COMPLETED,
            value,
        );
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_PLAYTHROUGHS_COMPLETED])?;
        self.proto = new;
        Ok(())
    }

    /// Set the current playthrough (0 = Normal, 1 = TVHM, 2 = UVHM). Added if absent.
    pub fn set_active_playthrough(&mut self, value: i64) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::upsert_varint_field(
            &self.proto,
            &fields,
            proto::FIELD_ACTIVE_PLAYTHROUGH,
            value,
        );
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_ACTIVE_PLAYTHROUGH])?;
        self.proto = new;
        Ok(())
    }

    /// Change class (pass a def path from [`CLASSES`]). Note: does not reset skills.
    pub fn set_class(&mut self, class_def: &str) -> Result<()> {
        let fields = self.fields()?;
        let new = proto::rewrite_string_field(&self.proto, &fields, proto::FIELD_CLASS, class_def)?;
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_CLASS])?;
        self.proto = new;
        Ok(())
    }

    /// Set the character name (the appearance message's sub-field 1).
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        let fields = self.fields()?;
        let f = *fields
            .iter()
            .find(|f| f.number == proto::FIELD_APPEARANCE && f.wire_type == 2)
            .ok_or_else(|| SaveError::Proto("appearance field missing".into()))?;
        let inner = proto::wire2_content(&self.proto, &f)?;
        let ifields = proto::parse_fields(inner)?;
        let new_inner = if ifields.iter().any(|x| x.number == 1 && x.wire_type == 2) {
            proto::rewrite_string_field(inner, &ifields, 1, name)?
        } else {
            let mut v = inner.to_vec();
            proto::emit_wire2_field(&mut v, 1, name.as_bytes());
            v
        };
        let new =
            proto::replace_field_content(&self.proto, &fields, proto::FIELD_APPEARANCE, &new_inner);
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_APPEARANCE])?;
        self.proto = new;
        Ok(())
    }

    /// Set every backpack + bank item and weapon to `level` (grade + game stage).
    /// Items that shouldn't be leveled (grade absent or ≤ 1, e.g. some class
    /// mods/relics) are skipped unless `force` is set. Returns the count changed.
    /// Guarded so nothing outside the item fields (41/53/54) can change.
    pub fn set_all_item_levels(&mut self, level: i64, force: bool) -> Result<usize> {
        let (new, changed) = items::relevel_all(&self.proto, level, force)?;
        proto::only_fields_changed(&self.proto, &new, &[41, 53, 54])?;
        self.proto = new;
        Ok(changed)
    }

    /// Set one item (by its [`Item::id`]) to `level`. Returns whether it changed
    /// (false if the id doesn't exist or the item is a protected no-level item).
    pub fn set_item_level(&mut self, id: usize, level: i64) -> Result<bool> {
        let (new, changed) = items::set_one_level(&self.proto, id, level)?;
        proto::only_fields_changed(&self.proto, &new, &[41, 53, 54])?;
        self.proto = new;
        Ok(changed)
    }

    /// Swap part `slot` (0-based) of the item with [`Item::id`] `id` to the part
    /// `(lib, asset)`. Only changes slots that already hold a part. Returns whether
    /// it changed. See [`parts_catalog`] for valid `(lib, asset)` values.
    pub fn set_item_part(&mut self, id: usize, slot: usize, lib: u32, asset: u32) -> Result<bool> {
        let (new, changed) = items::set_item_part(&self.proto, id, slot, PartRef { lib, asset })?;
        proto::only_fields_changed(&self.proto, &new, &[41, 53, 54])?;
        self.proto = new;
        Ok(changed)
    }

    /// A shareable `BL2(...)` item code for the item with [`Item::id`] `id`
    /// (re-keyed to 0 so it's deterministic and matches Gibbed/apocalyptech).
    /// Returns `None` if the id doesn't exist or its serial can't be decoded.
    pub fn item_code(&self, id: usize) -> Result<Option<String>> {
        match items::serial_by_id(&self.proto, id)? {
            Some(serial) => Ok(Some(serial::to_code(&serial)?)),
            None => Ok(None),
        }
    }

    /// Set the unlocked Overpower level (0 clears it; see [`MAX_OP_LEVEL`] for
    /// the game's ceiling — this does not clamp). Requires level 72 + a
    /// completed UVHM to be meaningful in-game; the level is then selected at the
    /// character screen. Stored as a hidden virtual item; only field 53 changes.
    pub fn set_op_level(&mut self, op: i64) -> Result<()> {
        let new = items::set_op_level(&self.proto, op)?;
        proto::only_fields_changed(&self.proto, &new, &[53])?;
        self.proto = new;
        Ok(())
    }

    /// Import a `BL2(...)` item code as a new backpack (or bank) entry. The
    /// serial is re-keyed so it's a fresh copy. Returns an error if the code
    /// isn't a valid `BL2(...)` string.
    pub fn add_item_from_code(&mut self, code: &str, to_bank: bool) -> Result<()> {
        let (serial, is_weapon) = serial::from_code(code)?;
        let new = items::add_item(&self.proto, &serial, is_weapon, to_bank);
        proto::only_fields_changed(&self.proto, &new, &[41, 53, 54])?;
        self.proto = new;
        Ok(())
    }

    /// Import every `BL2(...)` code found in `text` (any separators — commas,
    /// pipes, slashes, newlines — are fine, since each code is scanned as
    /// `BL2(` … `)`). Returns (imported, failed).
    pub fn add_items_from_codes(&mut self, text: &str, to_bank: bool) -> (usize, usize) {
        let (mut ok, mut failed) = (0, 0);
        for code in extract_codes(text) {
            match self.add_item_from_code(&code, to_bank) {
                Ok(()) => ok += 1,
                Err(_) => failed += 1,
            }
        }
        (ok, failed)
    }
}

#[cfg(test)]
mod tests;
