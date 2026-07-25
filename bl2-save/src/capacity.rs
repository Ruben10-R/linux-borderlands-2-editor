//! Backpack & bank capacity (SDU) editing.
//!
//! Capacity is stored in two places that must agree (per apocalyptech/Gibbed):
//!  - the *size* itself — backpack in field 13's sub-field 1, bank in top-level
//!    field 56;
//!  - the *SDU upgrade count* in the black-market array (field 36, a packed list
//!    of varints; index 7 = backpack, index 8 = bank).
//! Sizes snap to the SDU grid: backpack = 12 + 3·sdu, bank = 6 + 2·sdu.

use crate::error::Result;
use crate::proto;

const BACKPACK_BASE: i64 = 12;
const BANK_BASE: i64 = 6;
const BACKPACK_STEP: i64 = 3;
const BANK_STEP: i64 = 2;
const BACKPACK_SDU_IDX: usize = 7;
const BANK_SDU_IDX: usize = 8;
const FIELD_SIZES: u64 = 13;
const FIELD_BLACK_MARKET: u64 = 36;
const FIELD_BANK_SIZE: u64 = 56;

fn read_packed(content: &[u8]) -> Vec<u64> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < content.len() {
        match proto::decode_varint_at(content, pos) {
            Some((v, n)) if n > 0 => {
                out.push(v);
                pos += n;
            }
            _ => break,
        }
    }
    out
}

fn write_packed(values: &[u64]) -> Vec<u8> {
    let mut out = Vec::new();
    for &v in values {
        proto::push_varint(&mut out, v);
    }
    out
}

/// Current backpack slot count (field 13 → sub-field 1), if present.
pub fn backpack_size(protobuf: &[u8]) -> Option<i64> {
    let fields = proto::parse_fields(protobuf).ok()?;
    let f = fields.iter().find(|f| f.number == FIELD_SIZES && f.wire_type == 2)?;
    let content = proto::wire2_content(protobuf, f).ok()?;
    let sub = proto::parse_fields(content).ok()?;
    proto::read_varint_field(content, &sub, 1)
}

/// Current bank slot count (top-level field 56); the base if absent.
pub fn bank_size(protobuf: &[u8]) -> i64 {
    proto::parse_fields(protobuf)
        .ok()
        .and_then(|fs| proto::read_varint_field(protobuf, &fs, FIELD_BANK_SIZE))
        .unwrap_or(BANK_BASE)
}

/// Set the black-market SDU array index `idx` to `sdu`, padding as needed.
/// No-op (returns the input) if field 36 is absent.
fn set_black_market_sdu(protobuf: &[u8], idx: usize, sdu: u64) -> Result<Vec<u8>> {
    let fields = proto::parse_fields(protobuf)?;
    let Some(f) = fields.iter().find(|f| f.number == FIELD_BLACK_MARKET && f.wire_type == 2) else {
        return Ok(protobuf.to_vec());
    };
    let content = proto::wire2_content(protobuf, f)?;
    let mut packed = read_packed(content);
    while packed.len() <= idx {
        packed.push(0);
    }
    packed[idx] = sdu;
    Ok(proto::replace_field_content(protobuf, &fields, FIELD_BLACK_MARKET, &write_packed(&packed)))
}

/// Set backpack capacity to (at least) `slots`, snapped to the SDU grid.
/// Returns the new protobuf; only fields 13 and 36 change.
pub fn set_backpack_size(protobuf: &[u8], slots: i64) -> Result<Vec<u8>> {
    let sdu = (((slots - BACKPACK_BASE).max(0) + BACKPACK_STEP - 1) / BACKPACK_STEP).min(255);
    let new_size = BACKPACK_BASE + sdu * BACKPACK_STEP;

    // 1. field 13 sub-field 1 = new_size
    let fields = proto::parse_fields(protobuf)?;
    let out = if let Some(f) = fields.iter().find(|f| f.number == FIELD_SIZES && f.wire_type == 2) {
        let content = proto::wire2_content(protobuf, f)?;
        let sub = proto::parse_fields(content)?;
        let new_content = proto::upsert_varint_field(content, &sub, 1, new_size);
        proto::replace_field_content(protobuf, &fields, FIELD_SIZES, &new_content)
    } else {
        protobuf.to_vec()
    };
    // 2. black-market backpack SDU
    set_black_market_sdu(&out, BACKPACK_SDU_IDX, sdu as u64)
}

/// Set bank capacity to (at least) `slots`, snapped to the SDU grid.
/// Returns the new protobuf; only fields 36 and 56 change.
pub fn set_bank_size(protobuf: &[u8], slots: i64) -> Result<Vec<u8>> {
    let sdu = (((slots - BANK_BASE).max(0) + BANK_STEP - 1) / BANK_STEP).min(255);
    let new_size = BANK_BASE + sdu * BANK_STEP;

    let fields = proto::parse_fields(protobuf)?;
    let out = proto::upsert_varint_field(protobuf, &fields, FIELD_BANK_SIZE, new_size);
    let _ = fields;
    set_black_market_sdu(&out, BANK_SDU_IDX, sdu as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        // field 13 { sub1 = 12 }, field 36 = 8 packed varint zeros, field 56 = 6
        let mut sizes = Vec::new();
        proto::emit_varint_field(&mut sizes, 1, 12);
        let mut m = Vec::new();
        proto::emit_wire2_field(&mut m, FIELD_SIZES, &sizes);
        proto::emit_wire2_field(&mut m, FIELD_BLACK_MARKET, &vec![0u8; 9]);
        proto::emit_varint_field(&mut m, FIELD_BANK_SIZE, 6);
        m
    }

    #[test]
    fn backpack_snaps_to_sdu_grid() {
        let m = synthetic();
        assert_eq!(backpack_size(&m), Some(12));
        let m2 = set_backpack_size(&m, 39).unwrap();
        assert_eq!(backpack_size(&m2), Some(39)); // 12 + 3*9
        // a non-grid request snaps up to the next SDU
        let m3 = set_backpack_size(&m, 20).unwrap();
        assert_eq!(backpack_size(&m3), Some(21)); // 12 + 3*3
    }

    #[test]
    fn bank_snaps_to_sdu_grid() {
        let m = synthetic();
        assert_eq!(bank_size(&m), 6);
        let m2 = set_bank_size(&m, 24).unwrap();
        assert_eq!(bank_size(&m2), 24); // 6 + 2*9
        let m3 = set_bank_size(&m, 21).unwrap();
        assert_eq!(bank_size(&m3), 22); // 6 + 2*8
    }
}
