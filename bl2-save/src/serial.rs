//! Borderlands 2 item/weapon **serial** de/obfuscation and bit-packing.
//!
//! Ported verbatim from apocalyptech's proven Python (which mirrors Gibbed):
//! a serial is `version_type(1) | key(int32 BE) | obfuscated-body`. The body is
//! an XOR keystream + a byte rotation over a 2-byte BogoCRC followed by a
//! little-endian bit-packed list of database references (type/balance/…/parts).
//!
//! The references are indices into the GameInfo database (mapping them to human
//! names is a separate, larger task — see PLAN.md §8/§15). This module gives the
//! structured numbers and round-trips them byte-for-byte.

use crate::error::{Result, SaveError};

const ITEM_STRUCT_VERSION: u8 = 7; // BL2

// Bit widths of each packed field, indexed by is_weapon (0 = item, 1 = weapon):
// [set, type, balance, manufacturer, grade, game_stage, then 11 parts].
const SIZES: [[u32; 17]; 2] = [
    [8, 17, 20, 11, 7, 7, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16],
    [8, 13, 20, 11, 7, 7, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17],
];
// (type, balance, manufacturer) "lib" bit widths, indexed by is_weapon.
const HEADER_BITS: [[u32; 3]; 2] = [[8, 10, 7], [6, 10, 7]];

/// A database reference split into its library id and asset index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartRef {
    pub lib: u32,
    pub asset: u32,
}

/// A decoded item/weapon serial.
#[derive(Clone, Debug)]
pub struct ItemSerial {
    pub is_weapon: bool,
    pub seed: i32,
    pub set: u32,
    pub item_type: PartRef,
    pub balance: PartRef,
    pub manufacturer: PartRef,
    pub grade: Option<i64>,
    /// Game stage — effectively the item's level.
    pub stage: Option<i64>,
    pub parts: Vec<Option<PartRef>>,
}

impl ItemSerial {
    /// A "virtual" placeholder (e.g. the OP-level marker), not a real item.
    pub fn is_placeholder(&self) -> bool {
        self.set == 255
    }

    /// Manufacturer name (e.g. "Jakobs"), if resolvable from the GameInfo slice.
    pub fn manufacturer_name(&self) -> Option<String> {
        crate::gameinfo::name("Manufacturers", self.set, self.manufacturer.lib, self.manufacturer.asset)
    }

    /// Weapon/item type name (e.g. "Jakobs Pistol"), if resolvable.
    pub fn type_name(&self) -> Option<String> {
        let category = if self.is_weapon { "WeaponTypes" } else { "ItemTypes" };
        crate::gameinfo::name(category, self.set, self.item_type.lib, self.item_type.asset)
    }
}

// ---- keystream / rotation (symmetric XOR; rotate_right ↔ rotate_left) ----

fn xor_data(data: &[u8], key: u32) -> Vec<u8> {
    let mut key = key;
    let mut out = Vec::with_capacity(data.len());
    for &c in data {
        key = ((key as u64 * 279_470_273) % 4_294_967_291) as u32;
        out.push(c ^ (key & 0xFF) as u8);
    }
    out
}

fn rotate_right(data: &[u8], steps: usize) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let s = steps % data.len();
    let split = data.len() - s;
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[split..]);
    out.extend_from_slice(&data[..split]);
    out
}

#[allow(dead_code)] // write side: exercised by tests now; drives item editing (M7)
fn rotate_left(data: &[u8], steps: usize) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let s = steps % data.len();
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[s..]);
    out.extend_from_slice(&data[..s]);
    out
}

// ---- bit-packing of the value list ----

fn unpack_values(is_weapon: bool, data: &[u8]) -> Vec<Option<u64>> {
    let mut buf = Vec::with_capacity(data.len() + 1);
    buf.push(0x20); // leading pad byte, as in the reference
    buf.extend_from_slice(data);
    let end = buf.len() * 8;

    let sizes = &SIZES[is_weapon as usize];
    let mut i = 8usize;
    let mut result = Vec::with_capacity(sizes.len());
    for &size in sizes {
        let size = size as usize;
        let j = i + size;
        if j > end {
            result.push(None);
            continue;
        }
        let lo = i >> 3;
        let hi = (j >> 3).min(buf.len() - 1);
        let mut value: u64 = 0;
        let mut idx = hi as isize;
        while idx >= lo as isize {
            value = (value << 8) | buf[idx as usize] as u64;
            idx -= 1;
        }
        let field = (value >> (i & 7)) & !(0xFFu64 << size);
        result.push(Some(field));
        i = j;
    }
    result
}

#[allow(dead_code)] // write side: exercised by tests now; drives item editing (M7)
fn pack_values(is_weapon: bool, values: &[Option<u64>]) -> Vec<u8> {
    let mut i = 0usize;
    let mut item = vec![0u8; 48]; // generous; truncated below (reference uses 32)
    for (value_opt, &size) in values.iter().zip(SIZES[is_weapon as usize].iter()) {
        let Some(v) = value_opt else { break };
        let mut value = v << (i & 7);
        let mut j = i >> 3;
        while value != 0 {
            item[j] |= (value & 0xFF) as u8;
            value >>= 8;
            j += 1;
        }
        i += size as usize;
    }
    if (i & 7) != 0 {
        let value = 0xFFu64 << (i & 7);
        item[i >> 3] |= (value & 0xFF) as u8;
    }
    item.truncate((i + 7) >> 3);
    item
}

#[allow(dead_code)] // write side: exercised by tests now; drives item editing (M7)
fn create_body(item: &[u8], header: &[u8], key: i32) -> Vec<u8> {
    let mut crc_input = Vec::with_capacity(header.len() + 2 + 33);
    crc_input.extend_from_slice(header);
    crc_input.extend_from_slice(&[0xFF, 0xFF]);
    crc_input.extend_from_slice(item);
    crc_input.resize(header.len() + 2 + 33, 0xFF); // pad item region to 33 with 0xFF
    let h = crc32fast::hash(&crc_input);
    let checksum = (((h >> 16) ^ h) & 0xFFFF) as u16;

    let mut cb = Vec::with_capacity(2 + item.len());
    cb.extend_from_slice(&checksum.to_be_bytes());
    cb.extend_from_slice(item);
    let rotated = rotate_left(&cb, (key & 31) as usize);
    xor_data(&rotated, (key >> 5) as u32)
}

// ---- raw unwrap / wrap (the faithful, round-trippable core) ----

fn unwrap_raw(data: &[u8]) -> Result<(bool, Vec<Option<u64>>, i32)> {
    if data.len() < 5 {
        return Err(SaveError::Proto("item serial too short".into()));
    }
    let version_type = data[0];
    if version_type & 0x7F != ITEM_STRUCT_VERSION {
        return Err(SaveError::Proto(format!(
            "unsupported item serial version {}",
            version_type & 0x7F
        )));
    }
    let key = i32::from_be_bytes([data[1], data[2], data[3], data[4]]);
    let is_weapon = (version_type >> 7) & 1 == 1;

    let xored = xor_data(&data[5..], (key >> 5) as u32);
    let raw = rotate_right(&xored, (key & 31) as usize);
    if raw.len() < 2 {
        return Err(SaveError::Proto("item body too short".into()));
    }
    Ok((is_weapon, unpack_values(is_weapon, &raw[2..]), key))
}

#[allow(dead_code)] // write side: exercised by tests now; drives item editing (M7)
fn wrap_raw(is_weapon: bool, values: &[Option<u64>], key: i32) -> Vec<u8> {
    let item = pack_values(is_weapon, values);
    let mut header = Vec::with_capacity(5);
    header.push(((is_weapon as u8) << 7) | ITEM_STRUCT_VERSION);
    header.extend_from_slice(&key.to_be_bytes());
    let body = create_body(&item, &header, key);
    let mut out = header;
    out.extend_from_slice(&body);
    out
}

/// Decode then re-encode a serial (round-trip). Used to prove byte-fidelity
/// against real serials in tests.
#[cfg(test)]
pub(crate) fn reencode(data: &[u8]) -> Result<Vec<u8>> {
    let (is_weapon, values, key) = unwrap_raw(data)?;
    Ok(wrap_raw(is_weapon, &values, key))
}

/// Return the serial re-leveled to `level` (both grade and game_stage), or None
/// if the item shouldn't be leveled: no grade field, or grade <= 1 (a "no level"
/// item like some class mods/relics) unless `force` is set. Mirrors apocalyptech.
pub(crate) fn releveled(serial: &[u8], level: i64, force: bool) -> Result<Option<Vec<u8>>> {
    let (is_weapon, mut values, key) = unwrap_raw(serial)?;
    if values.first().and_then(|o| *o) == Some(255) {
        return Ok(None); // virtual placeholder (e.g. OP-level marker) — never level
    }
    let grade = values.get(4).and_then(|o| *o);
    match grade {
        Some(g) if force || g > 1 => {
            let lvl = level.clamp(0, 127) as u64; // grade/game_stage are 7-bit fields
            values[4] = Some(lvl);
            values[5] = Some(lvl);
            Ok(Some(wrap_raw(is_weapon, &values, key)))
        }
        _ => Ok(None),
    }
}

// ---- structured decode ----

fn split(x: u64, bits: u32) -> PartRef {
    PartRef { lib: (x >> bits) as u32, asset: (x & ((1u64 << bits) - 1)) as u32 }
}

/// Decode a serial byte blob into structured fields.
pub fn unwrap(data: &[u8]) -> Result<ItemSerial> {
    let (is_weapon, v, key) = unwrap_raw(data)?;
    let hdr = HEADER_BITS[is_weapon as usize];
    let get = |idx: usize| -> u64 { v.get(idx).and_then(|o| *o).unwrap_or(0) };
    let part_bits = 10 + is_weapon as u32;
    let parts = v
        .get(6..)
        .unwrap_or(&[])
        .iter()
        .map(|o| o.map(|x| split(x, part_bits)))
        .collect();
    Ok(ItemSerial {
        is_weapon,
        seed: key,
        set: get(0) as u32,
        item_type: split(get(1), hdr[0]),
        balance: split(get(2), hdr[1]),
        manufacturer: split(get(3), hdr[2]),
        grade: v.get(4).and_then(|o| *o).map(|x| x as i64),
        stage: v.get(5).and_then(|o| *o).map(|x| x as i64),
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_roundtrip_reproduces_serial() {
        // A plausible item value list (17 fields). wrap(unwrap(x)) must equal x.
        let values: Vec<Option<u64>> = vec![
            Some(0),     // set
            Some(0x1234), // type
            Some(0x2ABC), // balance
            Some(0x055),  // manufacturer
            Some(30),     // grade
            Some(30),     // stage
            Some(0x101), Some(0x202), None, None, None, None, None, None, None, None, None,
        ];
        for is_weapon in [false, true] {
            let key: i32 = -0x2BADC0DE;
            let serial = wrap_raw(is_weapon, &values, key);
            let (w2, v2, k2) = unwrap_raw(&serial).unwrap();
            assert_eq!(w2, is_weapon);
            assert_eq!(k2, key);
            // Re-wrap the decoded values → identical bytes (proves both directions).
            let serial2 = wrap_raw(w2, &v2, k2);
            assert_eq!(serial, serial2, "serial must round-trip byte-for-byte");
        }
    }

    #[test]
    fn structured_decode_reads_fields() {
        let values: Vec<Option<u64>> = vec![
            Some(0), Some(0x1234), Some(0x2ABC), Some(0x055), Some(28), Some(31),
            Some(0x101), None, None, None, None, None, None, None, None, None, None,
        ];
        let serial = wrap_raw(true, &values, 12345);
        let it = unwrap(&serial).unwrap();
        assert!(it.is_weapon);
        assert_eq!(it.stage, Some(31));
        assert_eq!(it.grade, Some(28));
        // type with weapon header bits (6): lib = 0x1234 >> 6, asset = 0x1234 & 0x3F
        assert_eq!(it.item_type, PartRef { lib: 0x1234 >> 6, asset: 0x1234 & 0x3F });
    }
}
