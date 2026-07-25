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

// --- verified top-level field numbers (see PLAN.md; confirmed vs a real save) ---
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

/// Human name for a top-level save field number (from apocalyptech's save
/// structure), for the Raw inspector. "" if unknown.
pub fn field_name(number: u64) -> &'static str {
    match number {
        1 => "class",
        2 => "level",
        3 => "experience",
        4 => "general_skill_points",
        5 => "specialist_skill_points",
        6 => "currency_on_hand",
        7 => "playthroughs_completed",
        8 => "skills",
        11 => "resources",
        13 => "inventory_sizes",
        15 => "stats",
        16 => "active_fast_travel",
        17 => "last_fast_travel",
        18 => "missions",
        19 => "appearance",
        20 => "save_game_id",
        21 => "mission_number",
        23 => "unlocks",
        24 => "unlock_notifications",
        25 => "time_played",
        26 => "save_timestamp",
        29 => "game_stages",
        30 => "areas",
        34 => "save_guid",
        35 => "wearing",
        36 => "black_market",
        37 => "active_mission",
        38 => "challenges",
        41 => "bank",
        43 => "lockouts",
        46 => "explored_areas",
        49 => "active_playthrough",
        53 => "items",
        54 => "weapons",
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
        23 => "Unlock flags (opaque). Editing may have unpredictable effects.",
        24 => "Unlock-notification flags (opaque).",
        25 => "Total seconds played (shown in the General tab).",
        26 => "When the save was last written (YYYYMMDDHHMMSS).",
        29 => "Per-region enemy level scaling.",
        30 => "Discovered areas / world-discovery entries.",
        34 => "Unique save identifier (GUID). Leave alone unless you know why.",
        35 => "Equipped head and skin — edit in the Character tab.",
        36 => "Black Market (Eridium) upgrades purchased: ammo/backpack/bank capacity.",
        37 => "The currently-tracked mission index.",
        38 => "Challenge progress entries (Badass challenges).",
        41 => "Items and weapons stored in your bank — edit in the Items tab.",
        43 => "Raid-boss / area lockout timers.",
        46 => "Areas whose map fog you've revealed.",
        49 => "Current playthrough you load into: 0 = Normal, 1 = TVHM, 2 = UVHM.",
        53 => "Non-weapon backpack items (shields, grenades, relics, class mods).",
        54 => "Weapons in your backpack.",
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
                let len = read_varint(buf, &mut pos)? as usize;
                pos += len;
            }
            5 => pos += 4,
            other => {
                return Err(SaveError::Proto(format!("unsupported wire type {other}")))
            }
        }
        if pos > buf.len() {
            return Err(SaveError::Proto("field value runs past end of buffer".into()));
        }
        fields.push(Field { number, wire_type, tag_start, val_start, end: pos });
    }
    Ok(fields)
}

/// Content bytes of a length-delimited (wire type 2) field.
pub(crate) fn wire2_content<'a>(buf: &'a [u8], f: &Field) -> Result<&'a [u8]> {
    if f.wire_type != 2 {
        return Err(SaveError::Proto("expected a length-delimited field".into()));
    }
    let mut p = f.val_start;
    let len = read_varint(buf, &mut p)? as usize;
    buf.get(p..p + len)
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
    let f = fields.iter().find(|f| f.number == number && f.wire_type == 0)?;
    let mut p = f.val_start;
    read_varint(buf, &mut p).ok().map(|v| v as i64)
}

/// Read the first length-delimited field as a UTF-8 string (e.g. class=1).
pub fn read_string_field(buf: &[u8], fields: &[Field], number: u64) -> Option<String> {
    let f = fields.iter().find(|f| f.number == number && f.wire_type == 2)?;
    let mut p = f.val_start;
    let len = read_varint(buf, &mut p).ok()? as usize;
    Some(String::from_utf8_lossy(&buf[p..p + len]).into_owned())
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
                let mut p = f.val_start;
                let len = read_varint(buf, &mut p)? as usize;
                let content_end = p + len;
                while p < content_end {
                    out.push(read_varint(buf, &mut p)? as i64);
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
            encode_varint(&mut out, (FIELD_CURRENCY << 3) | 0);
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
    if !fields.iter().any(|f| f.number == number && f.wire_type == 0) {
        return Err(SaveError::Proto(format!(
            "varint field {number} not present in save"
        )));
    }
    let mut new_block = Vec::new();
    encode_varint(&mut new_block, (number << 3) | 0);
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
    if !fields.iter().any(|f| f.number == number && f.wire_type == 2) {
        return Err(SaveError::Proto(format!("string field {number} not present")));
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
            encode_varint(&mut out, (number << 3) | 0);
            encode_varint(&mut out, value as u64);
            done = true;
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    if !done {
        encode_varint(&mut out, (number << 3) | 0);
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
