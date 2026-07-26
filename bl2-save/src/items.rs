//! Read the backpack/bank item & weapon lists out of the save protobuf.
//!
//! Top-level fields: 41 = Bank, 53 = backpack Items, 54 = backpack Weapons. Each
//! occurrence is a small "item entry" message whose field 1 is the item serial
//! (see [`crate::serial`]) and field 2 is the quantity.

use crate::error::Result;
use crate::proto;
use crate::serial::{self, ItemSerial, PartRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Backpack,
    Bank,
}

/// One decoded inventory entry.
#[derive(Clone, Debug)]
pub struct Item {
    /// Stable index of this entry among all item entries (walk order), used to
    /// target a single item for editing. Independent of decode success.
    pub id: usize,
    pub location: Location,
    pub serial: ItemSerial,
    pub quantity: Option<i64>,
}

/// Decode every item/weapon in backpack and bank. Entries whose serial can't be
/// parsed (empty/unknown version) are skipped rather than failing the whole list.
pub fn read_items(protobuf: &[u8]) -> Result<Vec<Item>> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::new();
    let mut id = 0usize;
    for f in &fields {
        let location = match f.number {
            41 => Location::Bank,
            53 | 54 => Location::Backpack,
            _ => continue,
        };
        if f.wire_type != 2 {
            continue;
        }
        let this_id = id;
        id += 1; // every item entry gets an id, even if its serial won't decode
        let entry = proto::wire2_content(protobuf, f)?;
        let ifields = proto::parse_fields(entry)?;
        let Some(sf) = ifields.iter().find(|x| x.number == 1 && x.wire_type == 2) else {
            continue;
        };
        let serial_bytes = proto::wire2_content(entry, sf)?;
        let serial = match serial::unwrap(serial_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let quantity = ifields
            .iter()
            .find(|x| x.number == 2 && x.wire_type == 0)
            .and_then(|q| proto::read_varint_value(entry, q.val_start));
        out.push(Item {
            id: this_id,
            location,
            serial,
            quantity,
        });
    }
    Ok(out)
}

/// Set every backpack + bank item/weapon to `level` (grade + game_stage),
/// rebuilding the protobuf and preserving all other bytes. Returns the new
/// protobuf and the number of items changed. Items that shouldn't be leveled
/// (grade absent or <= 1) are left untouched unless `force` is set.
pub fn relevel_all(protobuf: &[u8], level: i64, force: bool) -> Result<(Vec<u8>, usize)> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::with_capacity(protobuf.len());
    let mut changed = 0;
    for f in &fields {
        let is_item_field = matches!(f.number, 41 | 53 | 54) && f.wire_type == 2;
        if !is_item_field {
            out.extend_from_slice(&protobuf[f.tag_start..f.end]);
            continue;
        }
        let entry = proto::wire2_content(protobuf, f)?;
        let ifields = proto::parse_fields(entry)?;
        let new_entry = match ifields.iter().find(|x| x.number == 1 && x.wire_type == 2) {
            Some(sf) => {
                let serial = proto::wire2_content(entry, sf)?;
                match serial::releveled(serial, level, force)? {
                    Some(new_serial) => {
                        changed += 1;
                        proto::replace_field_content(entry, &ifields, 1, &new_serial)
                    }
                    None => entry.to_vec(),
                }
            }
            None => entry.to_vec(),
        };
        proto::emit_wire2_field(&mut out, f.number, &new_entry);
    }
    Ok((out, changed))
}

/// Set a single item (by its [`Item::id`]) to `level`, rebuilding the protobuf.
/// Returns the new protobuf and whether it changed (false if the id doesn't
/// exist or the item is a protected "no-level" item). Never levels grade-≤1.
pub fn set_one_level(protobuf: &[u8], target: usize, level: i64) -> Result<(Vec<u8>, bool)> {
    let (out, changed) = set_levels(protobuf, &[(target, level)])?;
    Ok((out, changed > 0))
}

/// Set the level of many items in a single rebuild. `targets` pairs an
/// [`Item::id`] with the level it should end up at; ids not listed are copied
/// through untouched. Returns the new protobuf and how many items changed.
///
/// One pass for the whole batch — levelling a full bank one item at a time means
/// re-parsing and rebuilding the entire protobuf per item.
pub fn set_levels(protobuf: &[u8], targets: &[(usize, i64)]) -> Result<(Vec<u8>, usize)> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::with_capacity(protobuf.len());
    let mut id = 0usize;
    let mut changed = 0usize;
    for f in &fields {
        let is_item_field = matches!(f.number, 41 | 53 | 54) && f.wire_type == 2;
        if !is_item_field {
            out.extend_from_slice(&protobuf[f.tag_start..f.end]);
            continue;
        }
        let this_id = id;
        id += 1;
        let entry = proto::wire2_content(protobuf, f)?;
        let want = targets
            .iter()
            .find(|(tid, _)| *tid == this_id)
            .map(|(_, lvl)| *lvl);
        let new_entry = match want {
            Some(level) => {
                let ifields = proto::parse_fields(entry)?;
                match ifields.iter().find(|x| x.number == 1 && x.wire_type == 2) {
                    Some(sf) => {
                        let serial = proto::wire2_content(entry, sf)?;
                        match serial::releveled(serial, level, false)? {
                            Some(new_serial) => {
                                changed += 1;
                                proto::replace_field_content(entry, &ifields, 1, &new_serial)
                            }
                            None => entry.to_vec(),
                        }
                    }
                    None => entry.to_vec(),
                }
            }
            None => entry.to_vec(),
        };
        proto::emit_wire2_field(&mut out, f.number, &new_entry);
    }
    Ok((out, changed))
}

/// Set part `slot` of the item with [`Item::id`] `target` to `part`, rebuilding
/// the protobuf. Returns the new protobuf and whether it changed.
pub fn set_item_part(
    protobuf: &[u8],
    target: usize,
    slot: usize,
    part: PartRef,
) -> Result<(Vec<u8>, bool)> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::with_capacity(protobuf.len());
    let mut id = 0usize;
    let mut changed = false;
    for f in &fields {
        let is_item_field = matches!(f.number, 41 | 53 | 54) && f.wire_type == 2;
        if !is_item_field {
            out.extend_from_slice(&protobuf[f.tag_start..f.end]);
            continue;
        }
        let this_id = id;
        id += 1;
        let entry = proto::wire2_content(protobuf, f)?;
        let new_entry = if this_id == target {
            let ifields = proto::parse_fields(entry)?;
            match ifields.iter().find(|x| x.number == 1 && x.wire_type == 2) {
                Some(sf) => {
                    let serial = proto::wire2_content(entry, sf)?;
                    match serial::with_part(serial, slot, part)? {
                        Some(new_serial) => {
                            changed = true;
                            proto::replace_field_content(entry, &ifields, 1, &new_serial)
                        }
                        None => entry.to_vec(),
                    }
                }
                None => entry.to_vec(),
            }
        } else {
            entry.to_vec()
        };
        proto::emit_wire2_field(&mut out, f.number, &new_entry);
    }
    Ok((out, changed))
}

// ---- Overpower level (stored as a "virtual item" in field 53) ----
//
// The game hides some DLC state in fake backpack items: a placeholder serial
// (set == 255) whose field 2 ("quantity") holds a tagged, negated int64. The low
// byte is an id — 4 = "Overpower levels unlocked" — and the level is the rest:
//   stored = wrapping_neg(4 | (op_level << 8)).
// Ported from apocalyptech's `_set_overpowered_level` / Gibbed's SaveExpansion.
const OP_ID: u64 = 4;

/// The 40-byte placeholder serial Gibbed uses when creating a fresh OP virtual
/// item: version 7, zero key, then a body that decodes to set == 255, rest 0.
const OP_BASE_SERIAL: [u8; 40] = [
    0x07, 0x00, 0x00, 0x00, 0x00, 0x39, 0x2a, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Is this item entry the OP-level virtual item (placeholder serial + field-2 tag 4)?
/// Returns the decoded OP level if so.
fn op_entry_level(entry: &[u8]) -> Option<i64> {
    let ifields = proto::parse_fields(entry).ok()?;
    let f2 = ifields.iter().find(|x| x.number == 2 && x.wire_type == 0)?;
    let raw = proto::read_varint_value(entry, f2.val_start)? as u64;
    let magnitude = raw.wrapping_neg();
    if magnitude & 0xFF != OP_ID {
        return None;
    }
    // Confirm the serial is a real placeholder (belt-and-suspenders vs a stray value).
    let sf = ifields.iter().find(|x| x.number == 1 && x.wire_type == 2)?;
    let serial = proto::wire2_content(entry, sf).ok()?;
    if !serial::unwrap(serial).ok()?.is_placeholder() {
        return None;
    }
    Some((magnitude >> 8) as i64)
}

/// Read the unlocked Overpower level, or None if the character has no OP data.
pub fn read_op_level(protobuf: &[u8]) -> Result<Option<i64>> {
    let fields = proto::parse_fields(protobuf)?;
    for f in &fields {
        if f.number != 53 || f.wire_type != 2 {
            continue;
        }
        let entry = proto::wire2_content(protobuf, f)?;
        if let Some(op) = op_entry_level(entry) {
            return Ok(Some(op));
        }
    }
    Ok(None)
}

/// Set the unlocked Overpower level, updating the existing virtual item or
/// appending a fresh one (mirroring Gibbed). Returns the new protobuf.
pub fn set_op_level(protobuf: &[u8], op: i64) -> Result<Vec<u8>> {
    let magnitude: u64 = OP_ID | ((op.max(0) as u64 & 0x7F_FFFF) << 8);
    let field2 = magnitude.wrapping_neg();

    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::with_capacity(protobuf.len() + 48);
    let mut done = false;
    for f in &fields {
        if !done && f.number == 53 && f.wire_type == 2 {
            let entry = proto::wire2_content(protobuf, f)?;
            if op_entry_level(entry).is_some() {
                let ifields = proto::parse_fields(entry)?;
                let new_entry = proto::upsert_varint_field(entry, &ifields, 2, field2 as i64);
                proto::emit_wire2_field(&mut out, 53, &new_entry);
                done = true;
                continue;
            }
        }
        out.extend_from_slice(&protobuf[f.tag_start..f.end]);
    }
    if !done {
        let mut entry = Vec::new();
        proto::emit_wire2_field(&mut entry, 1, &OP_BASE_SERIAL);
        proto::emit_varint_field(&mut entry, 2, field2);
        proto::emit_varint_field(&mut entry, 3, 0);
        proto::emit_varint_field(&mut entry, 4, 0);
        proto::emit_wire2_field(&mut out, 53, &entry);
    }
    Ok(out)
}

/// The raw serial bytes of the item with [`Item::id`] `target`, if present.
pub fn serial_by_id(protobuf: &[u8], target: usize) -> Result<Option<Vec<u8>>> {
    let fields = proto::parse_fields(protobuf)?;
    let mut id = 0usize;
    for f in &fields {
        if !matches!(f.number, 41 | 53 | 54) || f.wire_type != 2 {
            continue;
        }
        let this_id = id;
        id += 1;
        if this_id != target {
            continue;
        }
        let entry = proto::wire2_content(protobuf, f)?;
        let ifields = proto::parse_fields(entry)?;
        return match ifields.iter().find(|x| x.number == 1 && x.wire_type == 2) {
            Some(sf) => Ok(Some(proto::wire2_content(entry, sf)?.to_vec())),
            None => Ok(None),
        };
    }
    Ok(None)
}

/// Append a new inventory entry carrying `serial` to the save, mirroring how
/// Gibbed/apocalyptech build imported items: bank → field 41 (serial only),
/// backpack weapon → field 54 `{1:serial, 2:0, 3:1}`, backpack item → field 53
/// `{1:serial, 2:1, 3:0, 4:1}`. Returns the new protobuf.
pub fn add_item(protobuf: &[u8], serial: &[u8], is_weapon: bool, to_bank: bool) -> Vec<u8> {
    let mut entry = Vec::new();
    proto::emit_wire2_field(&mut entry, 1, serial);
    let field = if to_bank {
        41
    } else if is_weapon {
        proto::emit_varint_field(&mut entry, 2, 0);
        proto::emit_varint_field(&mut entry, 3, 1);
        54
    } else {
        proto::emit_varint_field(&mut entry, 2, 1);
        proto::emit_varint_field(&mut entry, 3, 0);
        proto::emit_varint_field(&mut entry, 4, 1);
        53
    };
    let mut out = protobuf.to_vec();
    proto::emit_wire2_field(&mut out, field, &entry);
    out
}

/// The raw serial blobs (for round-trip testing against a real save).
#[cfg(test)]
pub(crate) fn raw_serials(protobuf: &[u8]) -> Result<Vec<Vec<u8>>> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::new();
    for f in &fields {
        if !matches!(f.number, 41 | 53 | 54) || f.wire_type != 2 {
            continue;
        }
        let entry = proto::wire2_content(protobuf, f)?;
        for sf in proto::parse_fields(entry)?
            .iter()
            .filter(|x| x.number == 1 && x.wire_type == 2)
        {
            out.push(proto::wire2_content(entry, sf)?.to_vec());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_item_then_read_it_back() {
        // Take a real code from the library, add it to an empty backpack.
        let code = &crate::code_library()[0].code;
        let (serial, is_weapon) = crate::serial::from_code(code).unwrap();
        let proto = add_item(&[], &serial, is_weapon, false);
        let items = read_items(&proto).unwrap();
        assert_eq!(items.len(), 1, "one item added");
        let got = serial_by_id(&proto, 0).unwrap().expect("serial by id");
        // re-decode: same weapon/item kind as the source code
        assert_eq!(serial::unwrap(&got).unwrap().is_weapon, is_weapon);
        assert!(
            serial_by_id(&proto, 9).unwrap().is_none(),
            "missing id -> None"
        );
    }

    #[test]
    fn op_level_virtual_item_roundtrips() {
        // No OP data on an empty save; set 10 -> read 10; update in place to 7.
        assert_eq!(read_op_level(&[]).unwrap(), None);
        let p = set_op_level(&[], 10).unwrap();
        assert_eq!(read_op_level(&p).unwrap(), Some(10));
        let p2 = set_op_level(&p, 7).unwrap();
        assert_eq!(read_op_level(&p2).unwrap(), Some(7));
        // updating didn't append a second virtual item
        assert_eq!(
            read_items(&p2).unwrap().len(),
            read_items(&p).unwrap().len()
        );
    }
}
