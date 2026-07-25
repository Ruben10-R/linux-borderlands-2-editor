//! Read the backpack/bank item & weapon lists out of the save protobuf.
//!
//! Top-level fields: 41 = Bank, 53 = backpack Items, 54 = backpack Weapons. Each
//! occurrence is a small "item entry" message whose field 1 is the item serial
//! (see [`crate::serial`]) and field 2 is the quantity.

use crate::error::Result;
use crate::proto;
use crate::serial::{self, ItemSerial};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Backpack,
    Bank,
}

/// One decoded inventory entry.
#[derive(Clone, Debug)]
pub struct Item {
    pub location: Location,
    pub serial: ItemSerial,
    pub quantity: Option<i64>,
}

/// Decode every item/weapon in backpack and bank. Entries whose serial can't be
/// parsed (empty/unknown version) are skipped rather than failing the whole list.
pub fn read_items(protobuf: &[u8]) -> Result<Vec<Item>> {
    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::new();
    for f in &fields {
        let location = match f.number {
            41 => Location::Bank,
            53 | 54 => Location::Backpack,
            _ => continue,
        };
        if f.wire_type != 2 {
            continue;
        }
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
        out.push(Item { location, serial, quantity });
    }
    Ok(out)
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
        for sf in proto::parse_fields(entry)?.iter().filter(|x| x.number == 1 && x.wire_type == 2) {
            out.push(proto::wire2_content(entry, sf)?.to_vec());
        }
    }
    Ok(out)
}
