// Minimal protobuf wire-format walker.
//
// We do NOT model the full WillowTwoPlayerSaveGame schema. Instead we walk the
// top-level fields, remembering each field's exact byte range, so we can:
//   - read the fields we care about (currency_on_hand, field 6), and
//   - rewrite ONLY those fields while copying every other field byte-for-byte.
// This guarantees unknown fields survive an edit untouched.

use std::error::Error;

pub const CURRENCY_FIELD: u64 = 6; // WillowTwoPlayerSaveGame.currency_on_hand
pub const IDX_MONEY: usize = 0;
pub const IDX_ERIDIUM: usize = 1;

#[derive(Clone, Copy)]
pub struct Field {
    pub number: u64,
    pub wire_type: u8,
    pub tag_start: usize, // start of the tag varint
    pub val_start: usize, // start of the value (for wire type 2 this is the length varint)
    pub end: usize,       // exclusive end of the value
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64, Box<dyn Error>> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *pos >= buf.len() {
            return Err("varint runs past end of buffer".into());
        }
        let b = buf[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err("varint too long".into());
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
pub fn parse_fields(buf: &[u8]) -> Result<Vec<Field>, Box<dyn Error>> {
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
            other => return Err(format!("unsupported wire type {other}").into()),
        }
        if pos > buf.len() {
            return Err("field value runs past end of buffer".into());
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

/// The currency_on_hand values, in order. Handles both packed (wire type 2) and
/// unpacked (repeated wire type 0) encodings.
pub fn read_currency(buf: &[u8], fields: &[Field]) -> Result<Vec<i64>, Box<dyn Error>> {
    let mut out = Vec::new();
    for f in fields.iter().filter(|f| f.number == CURRENCY_FIELD) {
        match f.wire_type {
            0 => {
                let mut p = f.val_start;
                out.push(read_varint(buf, &mut p)? as i64);
            }
            2 => {
                // length-delimited packed block of varints
                let mut p = f.val_start;
                let len = read_varint(buf, &mut p)? as usize;
                let content_end = p + len;
                while p < content_end {
                    out.push(read_varint(buf, &mut p)? as i64);
                }
            }
            other => return Err(format!("currency field has odd wire type {other}").into()),
        }
    }
    Ok(out)
}

/// True if currency is stored packed (a single length-delimited field).
pub fn currency_is_packed(fields: &[Field]) -> bool {
    fields
        .iter()
        .find(|f| f.number == CURRENCY_FIELD)
        .map(|f| f.wire_type == 2)
        .unwrap_or(false)
}

fn encode_currency(values: &[i64], packed: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if packed {
        let mut payload = Vec::new();
        for &v in values {
            encode_varint(&mut payload, v as u64);
        }
        encode_varint(&mut out, (CURRENCY_FIELD << 3) | 2); // tag
        encode_varint(&mut out, payload.len() as u64); // length
        out.extend_from_slice(&payload);
    } else {
        for &v in values {
            encode_varint(&mut out, CURRENCY_FIELD << 3); // tag
            encode_varint(&mut out, v as u64);
        }
    }
    out
}

/// Rebuild the message with a new currency list, copying every other field
/// byte-for-byte. The new currency block is emitted where the first currency
/// field appeared (protobuf field order is not significant, but this keeps the
/// diff minimal).
pub fn rewrite_currency(
    buf: &[u8],
    fields: &[Field],
    new_values: &[i64],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let packed = currency_is_packed(fields);
    let new_block = encode_currency(new_values, packed);

    let mut out = Vec::with_capacity(buf.len());
    let mut emitted_currency = false;
    for f in fields {
        if f.number == CURRENCY_FIELD {
            if !emitted_currency {
                out.extend_from_slice(&new_block);
                emitted_currency = true;
            }
            // skip original currency field bytes
        } else {
            out.extend_from_slice(&buf[f.tag_start..f.end]);
        }
    }
    if !emitted_currency {
        // no currency field existed; append one
        out.extend_from_slice(&new_block);
    }
    Ok(out)
}
