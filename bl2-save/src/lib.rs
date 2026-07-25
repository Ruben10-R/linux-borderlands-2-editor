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

mod codec;
mod error;
mod gameinfo;
mod items;
mod proto;
mod serial;

pub use error::{Result, SaveError};
pub use items::{Item, Location};
pub use serial::{ItemSerial, PartRef};

use std::fs;
use std::path::Path;

/// A loaded Borderlands 2 save, held as its decoded inner protobuf.
///
/// All edits mutate the protobuf surgically; call [`SaveFile::save`] to re-encode.
pub struct SaveFile {
    proto: Vec<u8>,
}

impl SaveFile {
    /// Decode a full `.sav` byte buffer, validating SHA1 + CRC.
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        let dec = codec::decode(raw)?;
        if !dec.is_valid() {
            return Err(if !dec.sha_ok {
                SaveError::Sha1Mismatch
            } else {
                SaveError::CrcMismatch { stored: dec.crc_stored, computed: dec.crc_calc }
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

    /// Human-readable character/class name (e.g. "Zer0 (Assassin)").
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

    /// Available skill points.
    pub fn skill_points(&self) -> Option<i64> {
        let fields = self.fields().ok()?;
        proto::read_varint_field(&self.proto, &fields, proto::FIELD_SKILL_POINTS)
    }

    /// Raw class-definition asset path (e.g. "GD_Siren.Character.CharClass_Siren").
    pub fn class_def(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        proto::read_string_field(&self.proto, &fields, proto::FIELD_CLASS)
    }

    /// Character name (from the appearance message, sub-field 1).
    pub fn name(&self) -> Option<String> {
        let fields = self.fields().ok()?;
        let f = fields.iter().find(|f| f.number == proto::FIELD_APPEARANCE && f.wire_type == 2)?;
        let inner = proto::wire2_content(&self.proto, f).ok()?;
        let ifields = proto::parse_fields(inner).ok()?;
        proto::read_string_field(inner, &ifields, 1)
    }

    /// Full `currency_on_hand` array (index 0 = money, 1 = eridium, …).
    pub fn currency(&self) -> Result<Vec<i64>> {
        let fields = self.fields()?;
        proto::read_currency(&self.proto, &fields)
    }

    /// Money (dollars) — `currency_on_hand[0]`.
    pub fn money(&self) -> i64 {
        self.currency().ok().and_then(|c| c.get(proto::IDX_MONEY).copied()).unwrap_or(0)
    }

    /// Eridium — `currency_on_hand[1]`.
    pub fn eridium(&self) -> i64 {
        self.currency().ok().and_then(|c| c.get(proto::IDX_ERIDIUM).copied()).unwrap_or(0)
    }

    /// Seraph crystals — `currency_on_hand[2]`.
    pub fn seraph(&self) -> i64 {
        self.currency().ok().and_then(|c| c.get(proto::IDX_SERAPH).copied()).unwrap_or(0)
    }

    /// Torgue tokens — `currency_on_hand[4]`.
    pub fn torgue(&self) -> i64 {
        self.currency().ok().and_then(|c| c.get(proto::IDX_TORGUE).copied()).unwrap_or(0)
    }

    /// All decoded backpack + bank items and weapons.
    pub fn items(&self) -> Result<Vec<Item>> {
        items::read_items(&self.proto)
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
        let new = proto::upsert_varint_field(&self.proto, &fields, proto::FIELD_SKILL_POINTS, value);
        proto::only_fields_changed(&self.proto, &new, &[proto::FIELD_SKILL_POINTS])?;
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
        let new = proto::replace_field_content(&self.proto, &fields, proto::FIELD_APPEARANCE, &new_inner);
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
}

/// The six playable classes: (display name, class-definition asset path).
/// Paths extracted from Gibbed's GameInfo; pass the path to [`SaveFile::set_class`].
pub const CLASSES: [(&str, &str); 6] = [
    ("Axton (Commando)", "GD_Soldier.Character.CharClass_Soldier"),
    ("Maya (Siren)", "GD_Siren.Character.CharClass_Siren"),
    ("Salvador (Gunzerker)", "GD_Mercenary.Character.CharClass_Mercenary"),
    ("Zer0 (Assassin)", "GD_Assassin.Character.CharClass_Assassin"),
    ("Gaige (Mechromancer)", "GD_Tulip_Mechromancer.Character.CharClass_Mechromancer"),
    ("Krieg (Psycho)", "GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass"),
];

/// One selectable part in a parts picker.
#[derive(Clone, Debug)]
pub struct PartOption {
    pub lib: u32,
    pub asset: u32,
    pub name: String,
}

/// Every available part (with a readable name) for weapons vs items in a given
/// set — the choices for a parts picker. NOTE: not filtered by item compatibility,
/// so arbitrary combinations can produce items the game rejects.
pub fn parts_catalog(is_weapon: bool, set: u32) -> Vec<PartOption> {
    let category = if is_weapon { "WeaponParts" } else { "ItemParts" };
    gameinfo::catalog(category, set)
        .into_iter()
        .map(|(lib, asset, name)| PartOption { lib, asset, name })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny synthetic top-level protobuf: class(1,str), level(2), xp(3),
    /// packed currency(6)=[money, eridium, 0], plus an "unknown" field(9) we must
    /// never disturb.
    fn synthetic_proto() -> Vec<u8> {
        let mut p = Vec::new();
        // field 1, wire 2: class string
        let class = b"GD_Assassin.Character.CharClass_Assassin";
        p.push((1 << 3) | 2);
        p.push(class.len() as u8);
        p.extend_from_slice(class);
        // field 2, wire 0: level = 4
        p.push((2 << 3) | 0);
        p.push(4);
        // field 3, wire 0: xp = 4288
        p.push((3 << 3) | 0);
        p.extend_from_slice(&[0xC0, 0x21]); // 4288 as varint
        // field 6, wire 2: packed [608, 0, 0]
        p.push((6 << 3) | 2);
        p.push(4); // payload len: 608(2) + 0 + 0
        p.extend_from_slice(&[0xE0, 0x04, 0x00, 0x00]);
        // field 9, wire 2: opaque "unknown" bytes
        p.push((9 << 3) | 2);
        p.push(3);
        p.extend_from_slice(&[0xDE, 0xAD, 0xBE][..3]);
        p
    }

    #[test]
    fn codec_roundtrip_is_byte_identical_and_valid() {
        let proto = synthetic_proto();
        let bytes = codec::encode(&proto).unwrap();
        let dec = codec::decode(&bytes).unwrap();
        assert!(dec.is_valid(), "checksums must validate");
        assert_eq!(dec.proto, proto, "round-trip must preserve protobuf exactly");
    }

    #[test]
    fn reads_expected_scalars() {
        let s = SaveFile { proto: synthetic_proto() };
        assert_eq!(s.level(), Some(4));
        assert_eq!(s.xp(), Some(4288));
        assert_eq!(s.money(), 608);
        assert_eq!(s.eridium(), 0);
        assert_eq!(s.class_name().as_deref(), Some("Zer0 (Assassin)"));
    }

    #[test]
    fn edits_change_only_the_target_field() {
        let mut s = SaveFile { proto: synthetic_proto() };
        s.set_money(99_999_999).unwrap();
        s.set_eridium(500).unwrap();
        s.set_level(50).unwrap();
        s.set_xp(1_000_000).unwrap();
        assert_eq!(s.money(), 99_999_999);
        assert_eq!(s.eridium(), 500);
        assert_eq!(s.level(), Some(50));
        assert_eq!(s.xp(), Some(1_000_000));
        // The opaque unknown field (9) must be byte-preserved.
        let fields = s.fields().unwrap();
        let f9 = fields.iter().find(|f| f.number == 9).expect("field 9 preserved");
        assert_eq!(&s.proto[f9.val_start + 1..f9.end], &[0xDE, 0xAD, 0xBE]);
    }

    /// The game decompresses saves with a C LZO1x implementation. Prove our
    /// pure-Rust `lzokay` compressed output is decodable by that canonical C impl
    /// (`minilzo`) — i.e. it is standard LZO1x the game will accept.
    #[test]
    fn lzokay_output_is_decodable_by_c_lzo() {
        // Semi-structured data so the compressor actually emits matches + literals.
        let outer: Vec<u8> = (0..8192u32)
            .map(|i| ((i as u8).wrapping_mul(37)) ^ ((i >> 3) as u8))
            .collect();
        let comp = lzokay_native::compress(&outer).expect("lzokay compress");
        let lzo = minilzo_rs::LZO::init().expect("minilzo init");
        let back = lzo.decompress_safe(&comp, outer.len()).expect("C LZO decompress");
        assert_eq!(back, outer, "C LZO must decode lzokay output → the game will too");
    }

    #[test]
    fn currency_indices() {
        let mut s = SaveFile { proto: synthetic_proto() };
        s.set_money(1).unwrap();
        s.set_eridium(2).unwrap();
        s.set_seraph(3).unwrap();
        s.set_torgue(4).unwrap(); // pads currency_on_hand out to index 4
        assert_eq!((s.money(), s.eridium(), s.seraph(), s.torgue()), (1, 2, 3, 4));
    }

    #[test]
    fn character_edits() {
        let mut s = SaveFile { proto: synthetic_proto() };
        s.set_skill_points(7).unwrap(); // field 4 absent in synthetic → appended
        assert_eq!(s.skill_points(), Some(7));
        s.set_class("GD_Siren.Character.CharClass_Siren").unwrap();
        assert_eq!(s.class_name().as_deref(), Some("Maya (Siren)"));
        assert_eq!(s.class_def().as_deref(), Some("GD_Siren.Character.CharClass_Siren"));
        // Other fields untouched.
        assert_eq!(s.level(), Some(4));
        assert_eq!(s.money(), 608);
    }

    #[test]
    fn full_edit_still_encodes_and_self_verifies() {
        let mut s = SaveFile { proto: synthetic_proto() };
        s.set_money(12_345).unwrap();
        // to_bytes() self-verifies; if the encoded file didn't round-trip it errors.
        let bytes = s.to_bytes().unwrap();
        let reloaded = SaveFile::from_bytes(&bytes).unwrap();
        assert_eq!(reloaded.money(), 12_345);
    }

    /// Golden test against a real save if one is present (they're gitignored, so
    /// skip cleanly in CI / on machines without a sample).
    #[test]
    fn golden_real_save_if_present() {
        let candidates = ["../samples/save0001.sav", "samples/save0001.sav"];
        let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
            eprintln!("golden: no sample save present, skipping");
            return;
        };
        let save = SaveFile::load(path).expect("real save should load");
        // Re-encode must round-trip byte-identically at the protobuf level.
        let bytes = save.to_bytes().expect("real save should self-verify");
        let reloaded = SaveFile::from_bytes(&bytes).unwrap();
        assert_eq!(save.money(), reloaded.money());
        assert!(save.level().is_some());

        // Decisive game-acceptance proxy: our (lzokay) re-encoded REAL save must be
        // decompressible by the C LZO the game uses, to a valid WSG buffer.
        let outer_size = u32::from_be_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let lzo = minilzo_rs::LZO::init().unwrap();
        let outer = lzo
            .decompress_safe(&bytes[24..], outer_size)
            .expect("game's C LZO must decompress our real re-encoded save");
        assert_eq!(outer.len(), outer_size, "decompressed size must match header");
        assert_eq!(&outer[4..7], b"WSG", "decompressed buffer must be a WSG block");

        // Every REAL item serial must decode AND re-encode byte-for-byte. The
        // hand-crafted "virtual" placeholders (OP-level markers, set==255) are
        // decoded but not normally packed, so they're excluded from the byte check.
        let serials = items::raw_serials(&save.proto).expect("read serials");
        let mut real = 0;
        for (n, s) in serials.iter().enumerate() {
            let decoded = serial::unwrap(s).unwrap_or_else(|e| panic!("serial #{n} decode: {e}"));
            if decoded.is_placeholder() {
                continue;
            }
            real += 1;
            let re = serial::reencode(s).expect("reencode");
            assert_eq!(&re, s, "real serial #{n} must round-trip byte-for-byte");
        }
        eprintln!("golden: {}/{} real serials round-tripped", real, serials.len());
        // The typed list should decode without error too.
        let _ = save.items().expect("items() should succeed");

        // Re-level every item to 50: must self-verify, change only item fields,
        // and every non-placeholder item must then report level 50.
        let mut leveled = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        let n = leveled.set_all_item_levels(50, true).expect("relevel");
        let _ = leveled.to_bytes().expect("re-leveled save must self-verify");
        assert_eq!(leveled.money(), save.money(), "re-leveling must not touch money");
        for it in leveled.items().unwrap() {
            if !it.serial.is_placeholder() {
                assert_eq!(it.serial.stage, Some(50), "item should now be level 50");
            }
        }
        eprintln!("golden: re-leveled {n} items to 50");

        // Per-item leveling by id: changes iff the item is levelable; protected
        // (grade-≤1) items stay locked. All levelable items end at 42.
        let mut one = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        for it in save.items().unwrap() {
            let changed = one.set_item_level(it.id, 42).unwrap();
            assert_eq!(changed, it.serial.is_levelable(), "id {}: changed vs levelable", it.id);
        }
        let _ = one.to_bytes().expect("per-item edits must self-verify");
        for it in one.items().unwrap() {
            if it.serial.is_levelable() {
                assert_eq!(it.serial.stage, Some(42), "levelable item should be 42");
            }
        }

        // Parts editing: swap a present part on the first item that has one.
        if let Some(it) = save
            .items()
            .unwrap()
            .into_iter()
            .find(|it| !it.serial.is_placeholder() && it.serial.parts.iter().any(|p| p.is_some()))
        {
            let cat = parts_catalog(it.serial.is_weapon, it.serial.set);
            assert!(!cat.is_empty(), "parts catalog must not be empty");
            let slot = it.serial.parts.iter().position(|p| p.is_some()).unwrap();
            let choice = &cat[0];
            let mut edited = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
            assert!(edited.set_item_part(it.id, slot, choice.lib, choice.asset).unwrap());
            let _ = edited.to_bytes().expect("part edit must self-verify");
            let after = edited.items().unwrap();
            let pr = after.iter().find(|x| x.id == it.id).unwrap().serial.parts[slot].unwrap();
            assert_eq!((pr.lib, pr.asset), (choice.lib, choice.asset), "slot holds chosen part");
            eprintln!("golden: swapped part slot {slot} to {}", choice.name);
        }

        // Name editing (appearance sub-field) on the real save.
        if save.name().is_some() {
            let mut n = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
            n.set_name("Zer0edit").expect("set_name");
            let _ = n.to_bytes().expect("name edit self-verify");
            assert_eq!(n.name().as_deref(), Some("Zer0edit"));
            assert_eq!(n.money(), save.money(), "name edit must not touch money");
            eprintln!("golden: renamed to {:?}", n.name());
        }
    }
}
