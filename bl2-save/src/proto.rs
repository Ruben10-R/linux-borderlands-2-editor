//! Minimal protobuf wire-format walker.
//!
//! We do NOT model the full `WillowTwoPlayerSaveGame` schema. Instead we walk the
//! top-level fields, remembering each field's exact byte range, so we can:
//!   - read the fields we care about, and
//!   - rewrite ONLY those fields while copying every other field byte-for-byte.
//!
//! This guarantees unknown fields survive an edit untouched — the property that
//! keeps the game from rejecting our saves.

use crate::error::{Result, SaveError};

// --- verified top-level field numbers (confirmed against a real save) ---
pub const FIELD_CLASS: u64 = 1; // PlayerClassDefinition path (string)
pub const FIELD_LEVEL: u64 = 2; // experience level (varint)
pub const FIELD_XP: u64 = 3; // experience points (varint)
pub const FIELD_SKILL_POINTS: u64 = 4; // available (general) skill points (varint)
pub const FIELD_SPECIALIST_SKILL_POINTS: u64 = 5; // specialist skill points (varint)
pub const FIELD_CURRENCY: u64 = 6; // currency_on_hand (packed repeated int32)
pub const FIELD_PLAYTHROUGHS_COMPLETED: u64 = 7; // playthroughs finished (varint)
pub const FIELD_APPEARANCE: u64 = 19; // appearance message; sub-field 1 = name (string)
pub const FIELD_SAVE_GAME_ID: u64 = 20; // save_game_id (varint)
pub const FIELD_TIME_PLAYED: u64 = 25; // seconds played (varint)
pub const FIELD_ACTIVE_PLAYTHROUGH: u64 = 49; // current playthrough 0/1/2 (varint)

pub const IDX_MONEY: usize = 0;
pub const IDX_ERIDIUM: usize = 1;
pub const IDX_SERAPH: usize = 2;
pub const IDX_TORGUE: usize = 4;

/// Name of a top-level save field, for the Raw inspector. "" if unknown.
pub fn field_name(number: u64) -> &'static str {
    // Complete map from Gibbed's WillowTwoPlayerSaveGame proto (field numbers +
    // names are facts; readable snake_case is ours).
    match number {
        1 => "class",
        2 => "level",
        3 => "experience",
        4 => "general_skill_points",
        5 => "specialist_skill_points",
        6 => "currency_on_hand",
        7 => "playthroughs_completed",
        8 => "skills",
        9 => "unknown_9",
        10 => "unknown_10",
        11 => "resources",
        12 => "item_data",
        13 => "inventory_slots",
        14 => "weapon_data",
        15 => "stats",
        16 => "visited_teleporters",
        17 => "last_teleporter",
        18 => "mission_playthroughs",
        19 => "ui_preferences",
        20 => "save_game_id",
        21 => "plot_mission_number",
        22 => "unknown_22",
        23 => "used_marketing_codes",
        24 => "marketing_codes_notification",
        25 => "total_play_time",
        26 => "last_saved_date",
        27 => "dlc_expansion_data",
        28 => "unknown_28",
        29 => "region_game_stages",
        30 => "world_discovery_list",
        31 => "is_badass_mode_save",
        32 => "weapon_mementos",
        33 => "item_mementos",
        34 => "save_guid",
        35 => "applied_customizations",
        36 => "black_market_upgrades",
        37 => "active_mission_number",
        38 => "challenges",
        39 => "level_challenge_unlocks",
        40 => "one_off_level_challenges",
        41 => "bank",
        42 => "num_challenge_prestiges",
        43 => "lockouts",
        44 => "is_dlc_player_class",
        45 => "dlc_player_class_package_id",
        46 => "explored_areas",
        47 => "unknown_47",
        48 => "golden_keys_notified",
        49 => "active_playthrough",
        50 => "show_new_playthrough_notification",
        51 => "received_default_weapon",
        52 => "queued_training_messages",
        53 => "items",
        54 => "weapons",
        55 => "awesome_skill_disabled",
        56 => "bank_size",
        57 => "vehicle_customizations",
        58 => "vehicle_steering_mode",
        _ => "",
    }
}

/// A plain-language explanation of a top-level field: what it is and what
/// changing it does. "" for fields whose purpose we haven't documented.
pub fn field_help(number: u64) -> &'static str {
    match number {
        1 => "The character's class (Vault Hunter). Change it in the Character tab.",
        2 => "Character level (1–72). Use Sync to keep XP consistent with it.",
        3 => "Total experience points. Use Sync to derive the right amount from the level.",
        4 => "Unspent skill points for the main skill trees.",
        5 => "Unspent specialist skill points (a second pool used by some mechanics).",
        6 => "Money, Eridium, Seraph crystals and Torgue tokens — edit these in the Currency tab.",
        7 => "Playthroughs completed (0–3): 1 unlocks TVHM, 2 unlocks UVHM.",
        8 => "Your skill tree: which skills are picked and their points.",
        11 => "Ammo pools and their SDU upgrade levels.",
        13 => "Backpack/equipped inventory slot counts (driven by SDU upgrades).",
        15 => "Challenge and statistics tracking (a large opaque blob).",
        16 => "Fast-travel stations you've unlocked — edit in the Fast Travel tab.",
        17 => "The fast-travel station you'll spawn at / last used.",
        18 => "Per-playthrough mission progress and status.",
        19 => "Character name and colour choices (name is on the Character tab).",
        20 => "Internal id of this save slot. Rarely needs changing.",
        21 => "Plot mission counter. Changing it can desync story progress.",
        12 => "Legacy/unpacked item data (the live items are in field 53).",
        14 => "Legacy/unpacked weapon data (the live weapons are in field 54).",
        23 => "SHiFT / marketing codes you've redeemed.",
        24 => "SHiFT codes still needing an in-game notification.",
        25 => "Total seconds played (shown in the General tab).",
        26 => "When the save was last written (YYYYMMDDHHMMSS).",
        27 => "Which DLC/expansions this save has data for.",
        29 => "Per-region enemy level scaling.",
        30 => "Discovered areas / world-discovery entries.",
        31 => "Whether this is a Badass-mode save.",
        32 => "\u{201c}Weapon mementos\u{201d} — favourited/kept weapons.",
        33 => "\u{201c}Item mementos\u{201d} — favourited/kept items.",
        34 => "Unique save identifier (GUID). Leave alone unless you know why.",
        35 => "Equipped head and skin — edit in the Character tab.",
        36 => "Black Market (Eridium) upgrades purchased: ammo/backpack/bank capacity.",
        37 => "The currently-tracked mission index.",
        38 => "Challenge progress entries (Badass challenges).",
        39 => "Which level-specific challenges are unlocked.",
        40 => "One-off level-challenge completion flags.",
        41 => "Items and weapons stored in your bank — edit in the Items tab.",
        42 => "How many times you've reset (prestiged) challenges.",
        43 => "Raid-boss / area lockout timers.",
        44 => "Whether the class is a DLC class (Mechromancer/Psycho).",
        45 => "Package id of the DLC class, if any.",
        46 => "Areas whose map fog you've revealed.",
        48 => "Golden-key notification counter — NOT the key count (that's in profile.bin).",
        49 => "Current playthrough you load into: 0 = Normal, 1 = TVHM, 2 = UVHM.",
        50 => "Flag: show the \u{201c}new playthrough unlocked\u{201d} notification.",
        51 => "Flag: whether you've received the starter weapon.",
        52 => "Queued in-game training/tutorial messages.",
        53 => "Non-weapon backpack items (shields, grenades, relics, class mods).",
        54 => "Weapons in your backpack.",
        55 => "Flag: the \u{201c}awesome skill\u{201d} (badass) toggle disabled.",
        56 => {
            "Bank slot count — edit via Bank slots in the General tab (kept in sync with the SDU)."
        }
        57 => "Equipped vehicle skins per family — edit in the Vehicle tab.",
        58 => "Vehicle steering mode (0 = default, 1 = alternate).",
        _ => "",
    }
}

#[derive(Clone, Copy)]
pub struct Field {
    pub number: u64,
    pub wire_type: u8,
    pub tag_start: usize, // start of the tag varint
    pub val_start: usize, // start of the value (for wire type 2 this is the length varint)
    pub end: usize,       // exclusive end of the value
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *pos >= buf.len() {
            return Err(SaveError::Proto("varint runs past end of buffer".into()));
        }
        let b = buf[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err(SaveError::Proto("varint too long".into()));
        }
    }
    Ok(result)
}

fn encode_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

/// Walk every top-level field of a protobuf message.
pub fn parse_fields(buf: &[u8]) -> Result<Vec<Field>> {
    let mut fields = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let tag_start = pos;
        let tag = read_varint(buf, &mut pos)?;
        let number = tag >> 3;
        let wire_type = (tag & 7) as u8;
        let val_start = pos;
        match wire_type {
            0 => {
                read_varint(buf, &mut pos)?;
            }
            1 => pos += 8,
            2 => {
                // Compare in u64 against what's left. Casting the length to
                // usize first would truncate on a 32-bit target (wasm32) and a
                // crafted length could then wrap past the check below.
                let len = read_varint(buf, &mut pos)?;
                if len > (buf.len() - pos) as u64 {
                    return Err(SaveError::Proto(
                        "length-delimited field runs past end of buffer".into(),
                    ));
                }
                pos += len as usize;
            }
            5 => pos += 4,
            other => return Err(SaveError::Proto(format!("unsupported wire type {other}"))),
        }
        if pos > buf.len() {
            return Err(SaveError::Proto(
                "field value runs past end of buffer".into(),
            ));
        }
        fields.push(Field {
            number,
            wire_type,
            tag_start,
            val_start,
            end: pos,
        });
    }
    Ok(fields)
}

/// Content bytes of a length-delimited (wire type 2) field.
pub(crate) fn wire2_content<'a>(buf: &'a [u8], f: &Field) -> Result<&'a [u8]> {
    if f.wire_type != 2 {
        return Err(SaveError::Proto("expected a length-delimited field".into()));
    }
    let mut p = f.val_start;
    let len = read_varint(buf, &mut p)?;
    // `p + len` in usize could overflow on a 32-bit target, so range-check first.
    let end = usize::try_from(len)
        .ok()
        .and_then(|l| p.checked_add(l))
        .ok_or_else(|| SaveError::Proto("length-delimited field length overflows".into()))?;
    buf.get(p..end)
        .ok_or_else(|| SaveError::Proto("length-delimited field runs past end".into()))
}

/// Append a length-delimited (wire type 2) field: tag + length + content.
pub(crate) fn emit_wire2_field(out: &mut Vec<u8>, number: u64, content: &[u8]) {
    encode_varint(out, (number << 3) | 2);
    encode_varint(out, content.len() as u64);
    out.extend_from_slice(content);
}

/// Append a wire-type-0 (varint) field to `out`.
pub(crate) fn emit_varint_field(out: &mut Vec<u8>, number: u64, value: u64) {
    encode_varint(out, number << 3);
    encode_varint(out, value);
}

/// Decode one varint at `pos`, returning (value, bytes consumed).
pub(crate) fn decode_varint_at(buf: &[u8], pos: usize) -> Option<(u64, usize)> {
    let mut p = pos;
    let v = read_varint(buf, &mut p).ok()?;
    Some((v, p - pos))
}

/// Append a bare varint (no tag) to `out`.
pub(crate) fn push_varint(out: &mut Vec<u8>, v: u64) {
    encode_varint(out, v);
}

/// Replace every occurrence of the repeated string field `number` with `values`,
/// copying all other fields verbatim in order (new values are appended at the
/// end). Order among other fields is preserved, so `only_fields_changed` with
/// `number` allowed will pass.
pub(crate) fn set_repeated_string_field(
    buf: &[u8],
    fields: &[Field],
    number: u64,
    values: &[String],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    for f in fields {
        if f.number == number {
            continue;
        }
        out.extend_from_slice(&buf[f.tag_start..f.end]);
    }
    for v in values {
        emit_wire2_field(&mut out, number, v.as_bytes());
    }
    out
}

/// Rebuild a message with the first wire-2 field `number`'s content replaced,
/// copying every other field byte-for-byte.
pub(crate) fn replace_field_content(
    msg: &[u8],
    fields: &[Field],
    number: u64,
    new_content: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(msg.len());
    let mut done = false;
    for f in fields {
        if !done && f.number == number && f.wire_type == 2 {
            emit_wire2_field(&mut out, number, new_content);
            done = true;
        } else {
            out.extend_from_slice(&msg[f.tag_start..f.end]);
        }
    }
    out
}

/// Read a varint value at a byte offset (the value of a wire-0 field).
pub(crate) fn read_varint_value(buf: &[u8], start: usize) -> Option<i64> {
    let mut p = start;
    read_varint(buf, &mut p).ok().map(|v| v as i64)
}

/// Read the first varint field with the given number (e.g. level=2, xp=3).
pub fn read_varint_field(buf: &[u8], fields: &[Field], number: u64) -> Option<i64> {
    let f = fields
        .iter()
        .find(|f| f.number == number && f.wire_type == 0)?;
    let mut p = f.val_start;
    read_varint(buf, &mut p).ok().map(|v| v as i64)
}

/// Read the first length-delimited field as a UTF-8 string (e.g. class=1).
pub fn read_string_field(buf: &[u8], fields: &[Field], number: u64) -> Option<String> {
    let f = fields
        .iter()
        .find(|f| f.number == number && f.wire_type == 2)?;
    // Bounds-checked rather than sliced directly: the length comes from the file.
    let content = wire2_content(buf, f).ok()?;
    Some(String::from_utf8_lossy(content).into_owned())
}

/// Map a class-definition asset path to the character's name.
pub fn class_name(class_def: &str) -> &'static str {
    if class_def.contains("Assassin") {
        "Zer0 (Assassin)"
    } else if class_def.contains("Mercenary") {
        "Salvador (Gunzerker)"
    } else if class_def.contains("Soldier") {
        "Axton (Commando)"
    } else if class_def.contains("Siren") {
        "Maya (Siren)"
    } else if class_def.contains("LilacPlayerClass") {
        "Krieg (Psycho)"
    } else if class_def.contains("Mechromancer") {
        "Gaige (Mechromancer)"
    } else {
        "Unknown"
    }
}

/// The `currency_on_hand` values, in order. Handles both packed (wire type 2)
/// and unpacked (repeated wire type 0) encodings.
pub fn read_currency(buf: &[u8], fields: &[Field]) -> Result<Vec<i64>> {
    let mut out = Vec::new();
    for f in fields.iter().filter(|f| f.number == FIELD_CURRENCY) {
        match f.wire_type {
            0 => {
                let mut p = f.val_start;
                out.push(read_varint(buf, &mut p)? as i64);
            }
            2 => {
                // Walk the packed block via its bounds-checked content slice, so
                // a bogus length can't run the reader on into later fields.
                let content = wire2_content(buf, f)?;
                let mut p = 0usize;
                while p < content.len() {
                    out.push(read_varint(content, &mut p)? as i64);
                }
            }
            other => {
                return Err(SaveError::Proto(format!(
                    "currency field has odd wire type {other}"
                )))
            }
        }
    }
    Ok(out)
}

fn currency_is_packed(fields: &[Field]) -> bool {
    fields
        .iter()
        .find(|f| f.number == FIELD_CURRENCY)
        .map(|f| f.wire_type == 2)
        .unwrap_or(true)
}

fn encode_currency(values: &[i64], packed: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if packed {
        let mut payload = Vec::new();
        for &v in values {
            encode_varint(&mut payload, v as u64);
        }
        encode_varint(&mut out, (FIELD_CURRENCY << 3) | 2);
        encode_varint(&mut out, payload.len() as u64);
        out.extend_from_slice(&payload);
    } else {
        for &v in values {
            encode_varint(&mut out, FIELD_CURRENCY << 3);
            encode_varint(&mut out, v as u64);
        }
    }
    out
}

/// Rebuild the message with a new currency list, copying every other field
/// byte-for-byte. The new block is emitted where the first currency field
/// appeared (protobuf field order is not significant, but this keeps the diff
/// minimal).
pub fn rewrite_currency(buf: &[u8], fields: &[Field], new_values: &[i64]) -> Result<Vec<u8>> {
    let packed = currency_is_packed(fields);
    let new_block = encode_currency(new_values, packed);

    let mut out = Vec::with_capacity(buf.len());
    let mut emitted = false;
    for f in fields {
        if f.number == FIELD_CURRENCY {
            if !emitted {
                out.extend_from_slice(&new_block);
                emitted = true;
            }
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    if !emitted {
        out.extend_from_slice(&new_block);
    }
    Ok(out)
}

/// Rebuild the message with a new value for a single top-level varint field
/// (e.g. level=2, xp=3), copying every other field byte-for-byte. Errors if the
/// field is absent (we only ever edit fields the save already has).
pub fn rewrite_varint_field(
    buf: &[u8],
    fields: &[Field],
    number: u64,
    new_value: i64,
) -> Result<Vec<u8>> {
    if !fields
        .iter()
        .any(|f| f.number == number && f.wire_type == 0)
    {
        return Err(SaveError::Proto(format!(
            "varint field {number} not present in save"
        )));
    }
    let mut new_block = Vec::new();
    encode_varint(&mut new_block, number << 3);
    encode_varint(&mut new_block, new_value as u64);

    let mut out = Vec::with_capacity(buf.len());
    let mut emitted = false;
    for f in fields {
        if f.number == number && f.wire_type == 0 {
            if !emitted {
                out.extend_from_slice(&new_block);
                emitted = true;
            }
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    Ok(out)
}

/// Rebuild the message with the first wire-2 field `number` set to `new_value`
/// (a UTF-8 string), copying every other field byte-for-byte. Errors if absent.
pub fn rewrite_string_field(
    buf: &[u8],
    fields: &[Field],
    number: u64,
    new_value: &str,
) -> Result<Vec<u8>> {
    if !fields
        .iter()
        .any(|f| f.number == number && f.wire_type == 2)
    {
        return Err(SaveError::Proto(format!(
            "string field {number} not present"
        )));
    }
    let mut out = Vec::with_capacity(buf.len());
    let mut done = false;
    for f in fields {
        if !done && f.number == number && f.wire_type == 2 {
            emit_wire2_field(&mut out, number, new_value.as_bytes());
            done = true;
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    Ok(out)
}

/// Set a top-level varint field, or append it if absent (protobuf order is not
/// significant). Copies every other field byte-for-byte.
pub fn upsert_varint_field(buf: &[u8], fields: &[Field], number: u64, value: i64) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() + 4);
    let mut done = false;
    for f in fields {
        if !done && f.number == number && f.wire_type == 0 {
            encode_varint(&mut out, number << 3);
            encode_varint(&mut out, value as u64);
            done = true;
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    if !done {
        encode_varint(&mut out, number << 3);
        encode_varint(&mut out, value as u64);
    }
    out
}

/// Confirm the only top-level fields that differ between two messages are those
/// in `allowed`. Guards every edit against accidental collateral changes.
pub fn only_fields_changed(old: &[u8], new: &[u8], allowed: &[u64]) -> Result<()> {
    let of = parse_fields(old)?;
    let nf = parse_fields(new)?;
    let others = |buf: &[u8], fs: &[Field]| -> Vec<Vec<u8>> {
        fs.iter()
            .filter(|f| !allowed.contains(&f.number))
            .map(|f| buf[f.tag_start..f.end].to_vec())
            .collect()
    };
    if others(old, &of) != others(new, &nf) {
        return Err(SaveError::SelfVerify(
            "edit altered fields outside the intended set".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrips() {
        for v in [0u64, 1, 127, 128, 300, 16_384, u32::MAX as u64, u64::MAX] {
            let mut b = Vec::new();
            encode_varint(&mut b, v);
            let (dv, n) = decode_varint_at(&b, 0).expect("decode");
            assert_eq!(dv, v);
            assert_eq!(n, b.len());
        }
    }

    #[test]
    fn field_name_covers_all_58() {
        for n in 1..=58u64 {
            assert!(!field_name(n).is_empty(), "field {n} has no name");
        }
        assert_eq!(field_name(999), "");
    }

    #[test]
    fn upsert_rewrites_present_and_appends_absent() {
        let mut m = Vec::new();
        emit_varint_field(&mut m, 2, 5);
        emit_varint_field(&mut m, 9, 7);
        let fs = parse_fields(&m).unwrap();

        let m2 = upsert_varint_field(&m, &fs, 2, 42);
        let fs2 = parse_fields(&m2).unwrap();
        assert_eq!(read_varint_field(&m2, &fs2, 2), Some(42));
        assert_eq!(
            read_varint_field(&m2, &fs2, 9),
            Some(7),
            "other field preserved"
        );

        let m3 = upsert_varint_field(&m, &fs, 3, 99);
        let fs3 = parse_fields(&m3).unwrap();
        assert_eq!(
            read_varint_field(&m3, &fs3, 3),
            Some(99),
            "absent field appended"
        );
    }

    #[test]
    fn repeated_string_field_replaces_all() {
        let mut m = Vec::new();
        emit_varint_field(&mut m, 2, 5);
        emit_wire2_field(&mut m, 16, b"AAA");
        emit_wire2_field(&mut m, 16, b"BBB");
        let fs = parse_fields(&m).unwrap();
        let vals = ["X".to_string(), "Y".to_string(), "Z".to_string()];
        let m2 = set_repeated_string_field(&m, &fs, 16, &vals);
        only_fields_changed(&m, &m2, &[16]).expect("only field 16 changed");
        let fs2 = parse_fields(&m2).unwrap();
        let got: Vec<String> = fs2
            .iter()
            .filter(|f| f.number == 16 && f.wire_type == 2)
            .map(|f| String::from_utf8(wire2_content(&m2, f).unwrap().to_vec()).unwrap())
            .collect();
        assert_eq!(got, vals);
        assert_eq!(
            read_varint_field(&m2, &fs2, 2),
            Some(5),
            "field 2 untouched"
        );
    }

    #[test]
    fn only_fields_changed_detects_stray_edits() {
        let mut a = Vec::new();
        emit_varint_field(&mut a, 2, 5);
        emit_varint_field(&mut a, 3, 7);
        let mut b = Vec::new();
        emit_varint_field(&mut b, 2, 5);
        emit_varint_field(&mut b, 3, 8);
        assert!(only_fields_changed(&a, &b, &[3]).is_ok());
        assert!(
            only_fields_changed(&a, &b, &[2]).is_err(),
            "field 3 changed but not allowed"
        );
    }

    #[test]
    fn parse_fields_rejects_malformed_messages() {
        // Length-delimited field claiming more bytes than remain.
        assert!(parse_fields(&[(1 << 3) | 2, 200, 1, 2, 3]).is_err());
        // Varint with the continuation bit set but no following byte.
        assert!(parse_fields(&[1 << 3, 0x80]).is_err());
        // Tag varint itself truncated.
        assert!(parse_fields(&[0x80]).is_err());
        // Wire types we don't model (3/4 = deprecated groups, 6/7 = invalid).
        for wt in [3u8, 4, 6, 7] {
            assert!(
                parse_fields(&[(1 << 3) | wt, 0]).is_err(),
                "wire type {wt} must be refused"
            );
        }
        // fixed64 / fixed32 running past the end.
        assert!(parse_fields(&[(1 << 3) | 1, 0, 0]).is_err());
        assert!(parse_fields(&[(1 << 3) | 5, 0]).is_err());
    }

    /// Truncating a well-formed message at any point must return, not panic.
    #[test]
    fn truncated_messages_never_panic() {
        let mut m = Vec::new();
        emit_varint_field(&mut m, 2, 300);
        emit_wire2_field(&mut m, 1, b"a string value");
        emit_varint_field(&mut m, 49, 1);
        for len in 0..m.len() {
            if let Ok(fs) = parse_fields(&m[..len]) {
                // Readers must also stay in bounds on a short-but-parsable prefix.
                let _ = read_varint_field(&m[..len], &fs, 2);
                let _ = read_string_field(&m[..len], &fs, 1);
                let _ = read_currency(&m[..len], &fs);
            }
        }
    }

    #[test]
    fn rewrite_string_field_edits_and_errors_when_absent() {
        let mut m = Vec::new();
        emit_wire2_field(&mut m, 1, b"old");
        let fs = parse_fields(&m).unwrap();
        let m2 = rewrite_string_field(&m, &fs, 1, "newer").unwrap();
        let fs2 = parse_fields(&m2).unwrap();
        assert_eq!(read_string_field(&m2, &fs2, 1).as_deref(), Some("newer"));
        assert!(rewrite_string_field(&m, &fs, 99, "x").is_err());
    }
}
