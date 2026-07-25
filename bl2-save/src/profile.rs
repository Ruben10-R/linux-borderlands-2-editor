//! Borderlands 2 `profile.bin` — account-wide data (Golden Keys, Badass Rank,
//! customization unlocks, SHiFT codes).
//!
//! Container is the same as a save: `SHA1(20) + uncompressedSize(4 BE) + LZO1x`
//! (SHA1 over `size + compressed`). Unlike saves there is **no WSG header and no
//! Huffman** — the decompressed payload is a flat big-endian **typed-entry (TLV)
//! list**:
//! ```text
//! numEntries: u32
//! repeat: startByte u8 · id u32 · dataType u8 · value · endByte u8
//! ```
//! `dataType`: 1=Int32, 4=String, 5=Single, 6=Binary, 8=Int8. String/Binary
//! values are `len u32` then bytes. Format understood from the (unlicensed)
//! `withmorten/B2Profile` — reference only; this is our own implementation.

use sha1::{Digest, Sha1};

use crate::error::{Result, SaveError};

const DT_INT32: u8 = 1;
const DT_STRING: u8 = 4;
const DT_SINGLE: u8 = 5;
const DT_BINARY: u8 = 6;
const DT_INT8: u8 = 8;

const ID_BADASS_RANK1: u32 = 136; // Int32 (displayed rank = (136 + 137) / 10)
const ID_BADASS_RANK2: u32 = 137; // Int32
const ID_BADASS_TOKENS_AVAILABLE: u32 = 138; // Int32
const ID_BADASS_TOKENS_EARNED: u32 = 139; // Int32
const MAX_BADASS_TOKENS: i64 = 62530;

/// Badass rank produced by `t` earned tokens: `floor(t^1.8)`.
fn rank_from_tokens(t: i64) -> i64 {
    (t.max(0) as f64).powf(1.8).floor() as i64
}

/// Fewest earned tokens whose rank is at least `rank`.
fn tokens_for_rank(rank: i64) -> i64 {
    if rank <= 0 {
        return 0;
    }
    let mut t = ((rank as f64).powf(1.0 / 1.8).floor() as i64).max(0);
    while t < MAX_BADASS_TOKENS && rank_from_tokens(t) < rank {
        t += 1;
    }
    t.min(MAX_BADASS_TOKENS)
}
const ID_GOLDEN_KEYS: u32 = 162; // Binary: [{source u8, num u8, used u8}...]
const GOLDEN_SOURCE_SHIFT: u8 = 0;
const ID_CUSTOMIZATIONS: u32 = 300; // Binary: unlock bitmap (0xFF = unlocked)

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// One TLV entry. `value` holds the exact on-disk value bytes (for String/Binary
/// that includes the 4-byte length prefix), so re-serializing is byte-faithful.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    start_byte: u8,
    id: u32,
    data_type: u8,
    value: Vec<u8>,
    end_byte: u8,
}

/// A decoded profile, held as its list of entries.
pub struct ProfileFile {
    entries: Vec<Entry>,
}

fn parse_entries(data: &[u8]) -> Result<Vec<Entry>> {
    if data.len() < 4 {
        return Err(SaveError::Proto("profile too short".into()));
    }
    let n = be32(&data[0..4]) as usize;
    let mut pos = 4usize;
    // `n` comes straight out of the file, so don't pre-allocate from it; the
    // per-entry bounds checks below stop a bogus count from running long.
    let mut entries = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        let need = |p: usize, k: usize| -> Result<()> {
            if p + k > data.len() {
                Err(SaveError::Proto("profile entry runs past end".into()))
            } else {
                Ok(())
            }
        };
        need(pos, 6)?; // start(1) + id(4) + type(1)
        let start_byte = data[pos];
        let id = be32(&data[pos + 1..pos + 5]);
        let data_type = data[pos + 5];
        let vstart = pos + 6;
        let vlen = match data_type {
            DT_INT32 | DT_SINGLE => 4,
            DT_INT8 => 1,
            DT_STRING | DT_BINARY => {
                need(vstart, 4)?;
                4 + be32(&data[vstart..vstart + 4]) as usize
            }
            other => return Err(SaveError::Proto(format!("profile: bad data type {other}"))),
        };
        need(vstart, vlen + 1)?; // value + end byte
        let value = data[vstart..vstart + vlen].to_vec();
        let end_byte = data[vstart + vlen];
        entries.push(Entry {
            start_byte,
            id,
            data_type,
            value,
            end_byte,
        });
        pos = vstart + vlen + 1;
    }
    Ok(entries)
}

fn serialize_entries(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for e in entries {
        out.push(e.start_byte);
        out.extend_from_slice(&e.id.to_be_bytes());
        out.push(e.data_type);
        out.extend_from_slice(&e.value);
        out.push(e.end_byte);
    }
    out
}

/// Decompress the SHA1+LZO container to the inner TLV bytes.
fn decompress(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 24 {
        return Err(SaveError::TooShort(raw.len()));
    }
    let uncompressed_size = be32(&raw[20..24]) as usize;
    let inner = crate::codec::lzo_decompress(&raw[24..], uncompressed_size, "profile decompress")?;
    if inner.len() != uncompressed_size {
        return Err(SaveError::Size(format!(
            "profile LZO output {} != declared {}",
            inner.len(),
            uncompressed_size
        )));
    }
    Ok(inner)
}

/// Recompress inner TLV bytes into the SHA1+LZO container.
fn compress(inner: &[u8]) -> Result<Vec<u8>> {
    let compressed = lzokay_native::compress(inner)
        .map_err(|e| SaveError::Lzo(format!("profile compress: {e}")))?;
    let mut hashed = Vec::with_capacity(4 + compressed.len());
    hashed.extend_from_slice(&(inner.len() as u32).to_be_bytes());
    hashed.extend_from_slice(&compressed);
    let hash = Sha1::digest(&hashed);
    let mut out = Vec::with_capacity(20 + hashed.len());
    out.extend_from_slice(&hash);
    out.extend_from_slice(&hashed);
    Ok(out)
}

impl ProfileFile {
    /// Decode a `profile.bin` byte buffer (validates SHA1).
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        if raw.len() < 24 {
            return Err(SaveError::TooShort(raw.len()));
        }
        if Sha1::digest(&raw[20..]).as_slice() != &raw[0..20] {
            return Err(SaveError::Sha1Mismatch);
        }
        let inner = decompress(raw)?;
        Ok(Self {
            entries: parse_entries(&inner)?,
        })
    }

    /// Read and decode a `profile.bin` from disk.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_bytes(&std::fs::read(path)?)
    }

    /// Re-encode to `profile.bin` bytes, self-verifying that a re-decode
    /// reproduces the exact entries.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let inner = serialize_entries(&self.entries);
        let bytes = compress(&inner)?;
        let check = ProfileFile::from_bytes(&bytes)?;
        if check.entries != self.entries {
            return Err(SaveError::SelfVerify("re-decoded profile differs".into()));
        }
        Ok(bytes)
    }

    /// Encode and write to disk, backing up the existing file first when `backup`.
    pub fn save(&self, path: impl AsRef<std::path::Path>, backup: bool) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.to_bytes()?;
        if backup && path.exists() {
            let mut bak = path.as_os_str().to_owned();
            bak.push(".bak");
            std::fs::copy(path, &bak)?;
        }
        std::fs::write(path, &bytes)?;
        Ok(())
    }

    fn entry(&self, id: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn int32(&self, id: u32) -> Option<i32> {
        let e = self.entry(id)?;
        if e.data_type == DT_INT32 && e.value.len() == 4 {
            Some(i32::from_be_bytes([
                e.value[0], e.value[1], e.value[2], e.value[3],
            ]))
        } else {
            None
        }
    }

    fn set_int32(&mut self, id: u32, val: i32) -> Result<()> {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.id == id && e.data_type == DT_INT32)
            .ok_or_else(|| SaveError::Proto(format!("profile has no int32 field {id}")))?;
        e.value = val.to_be_bytes().to_vec();
        Ok(())
    }

    /// Displayed Badass Rank = (entry 136 + entry 137) / 10.
    pub fn badass_rank(&self) -> Option<i32> {
        match (self.int32(ID_BADASS_RANK1), self.int32(ID_BADASS_RANK2)) {
            (None, None) => None,
            (a, b) => Some((a.unwrap_or(0) + b.unwrap_or(0)) / 10),
        }
    }

    /// Unspent Badass tokens available to invest.
    pub fn badass_tokens(&self) -> Option<i32> {
        self.int32(ID_BADASS_TOKENS_AVAILABLE)
    }

    /// Set the Badass Rank. Raises `earned` to the tokens needed for the rank and
    /// adds the same delta to `available`, so the game's `invested + available ==
    /// earned` invariant is preserved without touching invested bonus stats. The
    /// stored rank (entries 136/137) is set from the achievable rank. Lowering the
    /// rank below already-invested tokens clamps available to 0.
    pub fn set_badass_rank(&mut self, rank: i32) -> Result<()> {
        let earned = tokens_for_rank(rank.max(0) as i64);
        let actual_rank = rank_from_tokens(earned); // >= requested (LUT granularity)
        let cur_earned = self.int32(ID_BADASS_TOKENS_EARNED).unwrap_or(0) as i64;
        let cur_avail = self.int32(ID_BADASS_TOKENS_AVAILABLE).unwrap_or(0) as i64;
        let new_avail = (cur_avail + (earned - cur_earned)).max(0);

        // Displayed rank = (136 + 137) / 10, so each holds (rank * 10) / 2.
        let half = ((actual_rank * 10) / 2) as i32;
        self.set_int32(ID_BADASS_RANK1, half)?;
        self.set_int32(ID_BADASS_RANK2, half)?;
        self.set_int32(ID_BADASS_TOKENS_EARNED, earned as i32)?;
        self.set_int32(ID_BADASS_TOKENS_AVAILABLE, new_avail as i32)?;
        Ok(())
    }

    /// SHiFT Golden Keys available, or None if the profile has no key data.
    pub fn golden_keys(&self) -> Option<u8> {
        let e = self.entry(ID_GOLDEN_KEYS)?;
        if e.data_type != DT_BINARY || e.value.len() < 4 {
            return None;
        }
        e.value[4..]
            .chunks_exact(3)
            .find(|c| c[0] == GOLDEN_SOURCE_SHIFT)
            .map(|c| c[1])
    }

    /// (unlocked_bytes, total_bytes) of the customization-unlock blob, if present.
    /// Every head/skin/vehicle-skin unlock lives here; 0xFF bytes are unlocked.
    pub fn customization_stats(&self) -> Option<(usize, usize)> {
        let e = self.entry(ID_CUSTOMIZATIONS)?;
        if e.data_type != DT_BINARY || e.value.len() < 4 {
            return None;
        }
        let data = &e.value[4..];
        Some((data.iter().filter(|&&b| b == 0xFF).count(), data.len()))
    }

    /// Unlock (`true`) or lock (`false`) every customization — heads, skins and
    /// vehicle skins — by filling the unlock blob. Mirrors Gibbed-style tools.
    pub fn set_all_customizations(&mut self, unlocked: bool) -> Result<()> {
        let fill = if unlocked { 0xFF } else { 0x00 };
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.id == ID_CUSTOMIZATIONS && e.data_type == DT_BINARY)
            .ok_or_else(|| SaveError::Proto("profile has no customizations entry".into()))?;
        if e.value.len() < 4 {
            return Err(SaveError::Proto("customizations entry malformed".into()));
        }
        for b in &mut e.value[4..] {
            *b = fill;
        }
        Ok(())
    }

    /// Set the SHiFT Golden Key count (0–255). Adds a SHiFT record if absent.
    pub fn set_golden_keys(&mut self, n: u8) -> Result<()> {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.id == ID_GOLDEN_KEYS && e.data_type == DT_BINARY)
            .ok_or_else(|| SaveError::Proto("profile has no Golden Keys entry".into()))?;
        if e.value.len() < 4 {
            return Err(SaveError::Proto("Golden Keys entry malformed".into()));
        }
        // Find the SHiFT (source 0) record and set its NumKeys byte.
        let mut found = false;
        for c in e.value[4..].chunks_exact_mut(3) {
            if c[0] == GOLDEN_SOURCE_SHIFT {
                c[1] = n;
                found = true;
                break;
            }
        }
        if !found {
            // Append a new SHiFT record {source 0, num n, used 0} and grow the len.
            e.value.extend_from_slice(&[GOLDEN_SOURCE_SHIFT, n, 0]);
            let new_len = (e.value.len() - 4) as u32;
            e.value[0..4].copy_from_slice(&new_len.to_be_bytes());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<Entry> {
        vec![
            Entry {
                start_byte: 1,
                id: 2,
                data_type: DT_INT32,
                value: vec![0, 0, 0, 5],
                end_byte: 0,
            },
            Entry {
                start_byte: 2,
                id: ID_GOLDEN_KEYS,
                data_type: DT_BINARY,
                // len=6, two records: SHiFT{0,3,0}, Tulip{173,1,0}
                value: vec![0, 0, 0, 6, 0, 3, 0, 173, 1, 0],
                end_byte: 0,
            },
            Entry {
                start_byte: 2,
                id: ID_BADASS_RANK1,
                data_type: DT_INT32,
                value: vec![0, 0, 0, 200],
                end_byte: 0,
            },
            Entry {
                start_byte: 2,
                id: ID_BADASS_RANK2,
                data_type: DT_INT32,
                value: vec![0, 0, 0, 100],
                end_byte: 0,
            },
            Entry {
                start_byte: 2,
                id: ID_BADASS_TOKENS_AVAILABLE,
                data_type: DT_INT32,
                value: vec![0, 0, 0, 0],
                end_byte: 0,
            },
            Entry {
                start_byte: 2,
                id: ID_BADASS_TOKENS_EARNED,
                data_type: DT_INT32,
                value: vec![0, 0, 0, 30],
                end_byte: 0,
            },
        ]
    }

    #[test]
    fn badass_rank_set_preserves_sanity() {
        // rank/tokens helpers are inverse-ish.
        assert!(rank_from_tokens(tokens_for_rank(500)) >= 500);
        assert_eq!(tokens_for_rank(0), 0);

        let bytes = compress(&serialize_entries(&synthetic())).unwrap();
        let mut p = ProfileFile::from_bytes(&bytes).unwrap();
        // start: earned 30, available 0 → invested = 30.
        let invested = p.int32(ID_BADASS_TOKENS_EARNED).unwrap() - p.badass_tokens().unwrap();
        assert_eq!(invested, 30);
        p.set_badass_rank(1000).unwrap();
        let out = p.to_bytes().expect("badass self-verify");
        let re = ProfileFile::from_bytes(&out).unwrap();
        // Sanity invariant preserved: invested + available == earned.
        let earned = re.int32(ID_BADASS_TOKENS_EARNED).unwrap();
        let avail = re.badass_tokens().unwrap();
        assert_eq!(invested + avail, earned, "invested + available == earned");
        assert!(re.badass_rank().unwrap() >= 1000, "rank raised to >= 1000");
        assert_eq!(re.golden_keys(), Some(3), "golden keys untouched");
    }

    #[test]
    fn tlv_roundtrips_byte_exact() {
        let entries = synthetic();
        let bytes = serialize_entries(&entries);
        assert_eq!(
            parse_entries(&bytes).unwrap(),
            entries,
            "TLV parse∘serialize is identity"
        );
    }

    #[test]
    fn container_and_edits_roundtrip() {
        let bytes = compress(&serialize_entries(&synthetic())).unwrap();
        let mut p = ProfileFile::from_bytes(&bytes).unwrap();
        assert_eq!(p.golden_keys(), Some(3));
        assert_eq!(p.badass_rank(), Some(30)); // (200 + 100) / 10
        p.set_golden_keys(200).unwrap();
        let out = p.to_bytes().expect("profile self-verify");
        let re = ProfileFile::from_bytes(&out).unwrap();
        assert_eq!(re.golden_keys(), Some(200), "golden keys persisted");
        assert_eq!(re.badass_rank(), Some(30), "badass untouched");
    }

    // ---- malformed input: must return Err, never panic or over-allocate ----

    #[test]
    fn rejects_short_and_truncated_containers() {
        assert!(matches!(
            ProfileFile::from_bytes(&[]),
            Err(SaveError::TooShort(0))
        ));
        assert!(ProfileFile::from_bytes(&[0u8; 40]).is_err());
        // Truncating a valid profile at every length must never panic.
        let good = compress(&serialize_entries(&synthetic())).unwrap();
        for len in 0..good.len() {
            let _ = ProfileFile::from_bytes(&good[..len]);
        }
    }

    #[test]
    fn rejects_absurd_entry_count() {
        // Claims 4 billion entries but carries none — must not try to allocate
        // for them, and must fail on the first missing entry.
        let mut data = u32::MAX.to_be_bytes().to_vec();
        data.extend_from_slice(&[1, 0, 0, 0, 2, DT_INT32, 0, 0, 0, 5, 0]);
        assert!(matches!(parse_entries(&data), Err(SaveError::Proto(_))));
    }

    #[test]
    fn rejects_bad_type_and_truncated_value() {
        // Unknown data type.
        let mut data = 1u32.to_be_bytes().to_vec();
        data.extend_from_slice(&[1, 0, 0, 0, 2, 99, 0]);
        assert!(matches!(parse_entries(&data), Err(SaveError::Proto(_))));

        // Binary entry whose declared length runs past the buffer.
        let mut data = 1u32.to_be_bytes().to_vec();
        data.extend_from_slice(&[1, 0, 0, 0, 2, DT_BINARY]);
        data.extend_from_slice(&9999u32.to_be_bytes());
        data.extend_from_slice(&[1, 2, 3]);
        assert!(matches!(parse_entries(&data), Err(SaveError::Proto(_))));

        // Int32 entry cut off mid-value.
        let mut data = 1u32.to_be_bytes().to_vec();
        data.extend_from_slice(&[1, 0, 0, 0, 2, DT_INT32, 0, 0]);
        assert!(matches!(parse_entries(&data), Err(SaveError::Proto(_))));
    }

    #[test]
    fn edits_on_a_profile_missing_the_entry_error_cleanly() {
        // A profile with only an unrelated int32: every setter must report a
        // missing entry rather than silently doing nothing.
        let bare = vec![Entry {
            start_byte: 1,
            id: 2,
            data_type: DT_INT32,
            value: vec![0, 0, 0, 5],
            end_byte: 0,
        }];
        let bytes = compress(&serialize_entries(&bare)).unwrap();
        let mut p = ProfileFile::from_bytes(&bytes).unwrap();
        assert!(p.golden_keys().is_none());
        assert!(p.badass_rank().is_none());
        assert!(p.customization_stats().is_none());
        assert!(p.set_golden_keys(10).is_err());
        assert!(p.set_all_customizations(true).is_err());
        assert!(p.set_badass_rank(100).is_err());
    }

    /// `load` reads from disk and `save` writes a `.bak` before overwriting.
    #[test]
    fn load_and_save_roundtrip_on_disk_with_backup() {
        let dir = std::env::temp_dir().join(format!("bl2prof_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.bin");

        let bytes = compress(&serialize_entries(&synthetic())).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        let mut p = ProfileFile::load(&path).expect("load from disk");
        assert_eq!(p.golden_keys(), Some(3));
        p.set_golden_keys(123).unwrap();
        p.save(&path, true).expect("save with backup");

        let bak = dir.join("profile.bin.bak");
        assert!(bak.exists(), "backup written");
        assert_eq!(
            ProfileFile::load(&bak).unwrap().golden_keys(),
            Some(3),
            "backup holds the pre-edit value"
        );
        assert_eq!(
            ProfileFile::load(&path).unwrap().golden_keys(),
            Some(123),
            "target holds the edit"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Byte-exact round-trip of a real profile.bin, if one is present.
    #[test]
    fn golden_real_profile_if_present() {
        let raw = match ["../samples/profile.bin", "samples/profile.bin"]
            .iter()
            .find_map(|p| std::fs::read(p).ok())
        {
            Some(r) => r,
            None => return,
        };
        let inner = decompress(&raw).unwrap();
        let entries = parse_entries(&inner).unwrap();
        assert_eq!(
            serialize_entries(&entries),
            inner,
            "TLV must reproduce the real inner bytes"
        );
        let mut p = ProfileFile::from_bytes(&raw).unwrap();
        eprintln!(
            "golden profile: {} entries, golden keys {:?}, badass rank {:?}, tokens {:?}",
            p.entries.len(),
            p.golden_keys(),
            p.badass_rank(),
            p.badass_tokens()
        );
        // Editing golden keys on the real profile must succeed, self-verify, and
        // read back — without changing the entry count or badass rank.
        let rank_before = p.badass_rank();
        p.set_golden_keys(80)
            .expect("set golden keys on real profile");
        let out = p.to_bytes().expect("real profile self-verify");
        let re = ProfileFile::from_bytes(&out).unwrap();
        assert_eq!(
            re.golden_keys(),
            Some(80),
            "golden keys persisted on real profile"
        );
        assert_eq!(re.entries.len(), p.entries.len(), "entry count preserved");
        assert_eq!(re.badass_rank(), rank_before, "badass rank untouched");
        eprintln!("golden profile: set golden keys -> {:?}", re.golden_keys());

        // Customizations unlock-all on the real profile, if it has the entry.
        eprintln!(
            "golden profile: customizations {:?}",
            p.customization_stats()
        );
        if p.customization_stats().is_some() {
            p.set_all_customizations(true)
                .expect("unlock all customizations");
            let out2 = p.to_bytes().expect("customization self-verify");
            let re2 = ProfileFile::from_bytes(&out2).unwrap();
            let (unlocked, total) = re2.customization_stats().unwrap();
            assert_eq!(unlocked, total, "all {total} customizations unlocked");
            eprintln!("golden profile: unlocked all {total} customizations");
        }
    }
}
