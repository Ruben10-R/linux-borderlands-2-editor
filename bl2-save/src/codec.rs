//! Borderlands 2 `.sav` codec: SHA1 + LZO1x + WSG + custom Huffman + CRC32.
//!
//! Decodes a `.sav` down to raw protobuf bytes and re-encodes it back.
//! Outer sizes are big-endian; inner WSG fields are little-endian (PC platform).
//!
//! Proven byte-correct against real saves, and accepted in-game.

use sha1::{Digest, Sha1};

use crate::error::{Result, SaveError};

// ---------- bit I/O (MSB-first) ----------

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize, // bit index
}
impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    /// Reads are bounds-checked: the payload length comes from the file, so a
    /// corrupt header can ask for more symbols than the payload encodes.
    fn read_bit(&mut self) -> Result<u8> {
        let byte = *self
            .data
            .get(self.pos >> 3)
            .ok_or_else(|| SaveError::Huffman("payload ended mid-symbol".into()))?;
        let bit = (byte >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Ok(bit)
    }
    fn read_byte(&mut self) -> Result<u8> {
        let mut v = 0u8;
        for _ in 0..8 {
            v = (v << 1) | self.read_bit()?;
        }
        Ok(v)
    }
}

struct BitWriter {
    bytes: Vec<u8>,
    cur: u8,
    nbits: u8,
}
impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            cur: 0,
            nbits: 0,
        }
    }
    fn write_bit(&mut self, bit: u8) {
        self.cur = (self.cur << 1) | (bit & 1);
        self.nbits += 1;
        if self.nbits == 8 {
            self.bytes.push(self.cur);
            self.cur = 0;
            self.nbits = 0;
        }
    }
    fn write_byte(&mut self, b: u8) {
        for i in (0..8).rev() {
            self.write_bit((b >> i) & 1);
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.cur <<= 8 - self.nbits; // left-align the final partial byte
            self.bytes.push(self.cur);
        }
        self.bytes
    }
}

// ---------- Huffman (Gibbed/apocalyptech format) ----------

enum Node {
    Leaf(u8),
    Internal(Box<Node>, Box<Node>),
}

/// A Huffman tree over 256 symbols can't legitimately be deeper than 255
/// levels. The cap stops crafted input (a long run of zero bits) from
/// recursing until the stack overflows — which aborts, and can't be caught.
const MAX_TREE_DEPTH: u32 = 256;

fn read_tree(br: &mut BitReader, depth: u32) -> Result<Node> {
    if depth > MAX_TREE_DEPTH {
        return Err(SaveError::Huffman(format!(
            "tree deeper than {MAX_TREE_DEPTH} levels"
        )));
    }
    if br.read_bit()? == 1 {
        Ok(Node::Leaf(br.read_byte()?))
    } else {
        let left = read_tree(br, depth + 1)?;
        let right = read_tree(br, depth + 1)?;
        Ok(Node::Internal(Box::new(left), Box::new(right)))
    }
}

fn write_tree(node: &Node, bw: &mut BitWriter) {
    match node {
        Node::Leaf(v) => {
            bw.write_bit(1);
            bw.write_byte(*v);
        }
        Node::Internal(l, r) => {
            bw.write_bit(0);
            write_tree(l, bw);
            write_tree(r, bw);
        }
    }
}

fn huffman_decode(data: &[u8], out_len: usize) -> Result<Vec<u8>> {
    let mut br = BitReader::new(data);
    let root = read_tree(&mut br, 0)?;
    // Don't pre-allocate from `out_len` — it comes straight out of the file.
    let mut out = Vec::with_capacity(out_len.min(1 << 20));
    while out.len() < out_len {
        let mut node = &root;
        loop {
            match node {
                Node::Leaf(v) => {
                    out.push(*v);
                    break;
                }
                Node::Internal(l, r) => {
                    node = if br.read_bit()? == 0 { l } else { r };
                }
            }
        }
    }
    Ok(out)
}

fn build_codes(node: &Node, prefix: &mut Vec<u8>, table: &mut [Vec<u8>]) {
    match node {
        Node::Leaf(v) => {
            table[*v as usize] = if prefix.is_empty() {
                vec![0]
            } else {
                prefix.clone()
            };
        }
        Node::Internal(l, r) => {
            prefix.push(0);
            build_codes(l, prefix, table);
            prefix.pop();
            prefix.push(1);
            build_codes(r, prefix, table);
            prefix.pop();
        }
    }
}

fn huffman_encode(data: &[u8]) -> Vec<u8> {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;

    let mut freq = [0usize; 256];
    for &b in data {
        freq[b as usize] += 1;
    }

    struct Entry {
        freq: usize,
        seq: usize,
        node: Box<Node>,
    }
    impl PartialEq for Entry {
        fn eq(&self, o: &Self) -> bool {
            self.freq == o.freq && self.seq == o.seq
        }
    }
    impl Eq for Entry {}
    impl Ord for Entry {
        fn cmp(&self, o: &Self) -> Ordering {
            o.freq.cmp(&self.freq).then(o.seq.cmp(&self.seq))
        }
    }
    impl PartialOrd for Entry {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }

    let mut heap = BinaryHeap::new();
    let mut seq = 0usize;
    for (b, &count) in freq.iter().enumerate() {
        if count > 0 {
            heap.push(Entry {
                freq: count,
                seq,
                node: Box::new(Node::Leaf(b as u8)),
            });
            seq += 1;
        }
    }

    let root = if heap.len() == 1 {
        *heap.pop().unwrap().node
    } else {
        while heap.len() > 1 {
            let a = heap.pop().unwrap();
            let b = heap.pop().unwrap();
            let combined = a.freq + b.freq;
            heap.push(Entry {
                freq: combined,
                seq,
                node: Box::new(Node::Internal(a.node, b.node)),
            });
            seq += 1;
        }
        *heap.pop().unwrap().node
    };

    let mut table: Vec<Vec<u8>> = vec![Vec::new(); 256];
    build_codes(&root, &mut Vec::new(), &mut table);

    let mut bw = BitWriter::new();
    write_tree(&root, &mut bw);
    for &b in data {
        for &bit in &table[b as usize] {
            bw.write_bit(bit);
        }
    }
    bw.finish()
}

// ---------- checksums / endian helpers ----------

/// Decompress an LZO1x stream, containing the decompressor's panics.
///
/// `lzokay-native` 0.1.0 (the newest release) panics with an arithmetic
/// overflow when a back-reference points before the start of the output — i.e.
/// on data that simply isn't a valid LZO stream. Since every input here is a
/// file the user picked, that has to be an error, not a crash.
///
/// Caveat: on wasm32 panics abort rather than unwind, so this cannot catch
/// there. The lasting fix is a bounds-checked decompressor (e.g. the `lzo`
/// crate) — a codec swap worth doing deliberately, not as a drive-by.
pub(crate) fn lzo_decompress(src: &[u8], expected: usize, what: &str) -> Result<Vec<u8>> {
    match std::panic::catch_unwind(|| lzokay_native::decompress_all(src, Some(expected))) {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(SaveError::Lzo(format!("{what}: {e}"))),
        Err(_) => Err(SaveError::Lzo(format!(
            "{what}: not a valid LZO stream (decompressor bailed out)"
        ))),
    }
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize().into()
}

fn crc32(data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    h.finalize()
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

// ---------- public decode / encode ----------

/// Bytes preceding the Huffman payload inside the decompressed block:
/// innerSize(4) + "WSG"(3) + version(4) + crc(4) + protoSize(4).
const WSG_HEADER_LEN: usize = 19;

/// Sanity cap on the declared protobuf size. Real saves are well under a
/// megabyte; this exists only so a corrupt header can't make us materialise
/// gigabytes — a degenerate single-leaf tree costs zero bits per output byte,
/// so the payload length alone doesn't bound the output.
const MAX_PROTO_SIZE: usize = 64 << 20; // 64 MiB

/// A decoded save: the raw inner protobuf plus the checksums we validated.
pub struct Decoded {
    pub proto: Vec<u8>,
    pub crc_stored: u32,
    pub crc_calc: u32,
    pub sha_ok: bool,
}
impl Decoded {
    pub fn is_valid(&self) -> bool {
        self.sha_ok && self.crc_stored == self.crc_calc
    }
}

/// Decode raw `.sav` bytes into the inner protobuf, validating SHA1 + CRC.
pub fn decode(raw: &[u8]) -> Result<Decoded> {
    if raw.len() < 24 {
        return Err(SaveError::TooShort(raw.len()));
    }

    let sha_ok = sha1(&raw[20..]) == raw[0..20];

    let outer_size = be32(&raw[20..24]) as usize;
    let outer = lzo_decompress(&raw[24..], outer_size, "decompress")?;
    if outer.len() != outer_size {
        return Err(SaveError::Size(format!(
            "LZO output {} != declared outer size {}",
            outer.len(),
            outer_size
        )));
    }
    // Every header read below is a fixed offset into `outer`, so bound it once.
    if outer.len() < WSG_HEADER_LEN {
        return Err(SaveError::Size(format!(
            "decompressed block is {} bytes — too short for a {WSG_HEADER_LEN}-byte WSG header",
            outer.len()
        )));
    }

    let inner_size = be32(&outer[0..4]) as usize;
    if inner_size != outer.len() - 4 {
        return Err(SaveError::Size(format!(
            "innerSize {} != outer-4 {}",
            inner_size,
            outer.len() - 4
        )));
    }
    if &outer[4..7] != b"WSG" {
        return Err(SaveError::BadMagic);
    }
    let version = le32(&outer[7..11]);
    if version != 2 {
        return Err(SaveError::BadVersion(version));
    }
    let crc_stored = le32(&outer[11..15]);
    let proto_size = le32(&outer[15..19]) as usize;
    if proto_size > MAX_PROTO_SIZE {
        return Err(SaveError::Size(format!(
            "declared protobuf size {proto_size} exceeds the {MAX_PROTO_SIZE}-byte sanity cap"
        )));
    }

    let proto = huffman_decode(&outer[WSG_HEADER_LEN..], proto_size)?;
    if proto.len() != proto_size {
        return Err(SaveError::Size(format!(
            "decoded protobuf {} != declared {}",
            proto.len(),
            proto_size
        )));
    }
    let crc_calc = crc32(&proto);

    Ok(Decoded {
        proto,
        crc_stored,
        crc_calc,
        sha_ok,
    })
}

/// Encode protobuf bytes back into a full `.sav` (reverse pipeline).
pub fn encode(proto: &[u8]) -> Result<Vec<u8>> {
    let mut huff = huffman_encode(proto);
    // The game's Huffman decoder reads a few bits PAST the last symbol's code
    // (aligned reads), so the payload needs trailing padding or the game reads
    // off the end of the buffer and rejects the save as corrupt. Gibbed and
    // apocalyptech append exactly 4 zero bytes here; match that.
    huff.extend_from_slice(&[0, 0, 0, 0]);

    let mut wsg = Vec::new();
    wsg.extend_from_slice(b"WSG");
    wsg.extend_from_slice(&2u32.to_le_bytes());
    wsg.extend_from_slice(&crc32(proto).to_le_bytes());
    wsg.extend_from_slice(&(proto.len() as u32).to_le_bytes());
    wsg.extend_from_slice(&huff);

    let mut outer = Vec::new();
    outer.extend_from_slice(&(wsg.len() as u32).to_be_bytes());
    outer.extend_from_slice(&wsg);

    let compressed =
        lzokay_native::compress(&outer).map_err(|e| SaveError::Lzo(format!("compress: {e}")))?;

    let mut body = Vec::new();
    body.extend_from_slice(&(outer.len() as u32).to_be_bytes());
    body.extend_from_slice(&compressed);

    let mut file = Vec::new();
    file.extend_from_slice(&sha1(&body));
    file.extend_from_slice(&body);
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip_synthetic() {
        // A small but valid protobuf: field 1 (wire2) = "abc", field 2 (varint) = 5.
        let proto: Vec<u8> = vec![0x0a, 0x03, b'a', b'b', b'c', 0x10, 0x05];
        let bytes = encode(&proto).expect("encode");
        let dec = decode(&bytes).expect("decode");
        assert!(dec.is_valid(), "sha1 + crc valid");
        assert_eq!(dec.proto, proto, "round-trips to the same protobuf");
    }

    #[test]
    fn decode_rejects_corrupt_sha1() {
        let proto: Vec<u8> = vec![0x10, 0x2a];
        let mut bytes = encode(&proto).unwrap();
        bytes[0] ^= 0xFF; // flip a SHA1 byte
        let dec = decode(&bytes).expect("still decodes structurally");
        assert!(!dec.sha_ok, "sha mismatch detected");
    }

    // ---- malformed input: every one of these must return Err, never panic ----

    /// Wrap arbitrary "decompressed block" bytes in a structurally valid
    /// SHA1 + size + LZO container, so tests can target the inner parsing.
    fn container_of(outer: &[u8]) -> Vec<u8> {
        let compressed = lzokay_native::compress(outer).unwrap();
        let mut body = Vec::new();
        body.extend_from_slice(&(outer.len() as u32).to_be_bytes());
        body.extend_from_slice(&compressed);
        let mut file = Vec::new();
        file.extend_from_slice(&sha1(&body));
        file.extend_from_slice(&body);
        file
    }

    /// A decompressed block with a valid WSG header and the given payload.
    fn block_with(proto_size: u32, payload: &[u8]) -> Vec<u8> {
        let mut wsg = Vec::new();
        wsg.extend_from_slice(b"WSG");
        wsg.extend_from_slice(&2u32.to_le_bytes());
        wsg.extend_from_slice(&0u32.to_le_bytes()); // crc (unchecked on this path)
        wsg.extend_from_slice(&proto_size.to_le_bytes());
        wsg.extend_from_slice(payload);
        let mut outer = Vec::new();
        outer.extend_from_slice(&(wsg.len() as u32).to_be_bytes());
        outer.extend_from_slice(&wsg);
        outer
    }

    #[test]
    fn rejects_file_shorter_than_the_outer_header() {
        assert!(matches!(decode(&[]), Err(SaveError::TooShort(0))));
        assert!(matches!(decode(&[0u8; 23]), Err(SaveError::TooShort(23))));
    }

    #[test]
    fn rejects_decompressed_block_too_short_for_a_header() {
        // Used to index outer[0..4] unconditionally and panic.
        for len in 0..WSG_HEADER_LEN {
            let bytes = container_of(&vec![0xABu8; len]);
            assert!(
                decode(&bytes).is_err(),
                "a {len}-byte block must be rejected, not panic"
            );
        }
        // Non-degenerate lengths reach — and are caught by — our length guard.
        // (An empty block fails earlier, inside LZO.)
        for len in 1..WSG_HEADER_LEN {
            let bytes = container_of(&vec![0xABu8; len]);
            assert!(
                matches!(decode(&bytes), Err(SaveError::Size(_))),
                "a {len}-byte block must be reported as a size error"
            );
        }
    }

    #[test]
    fn rejects_proto_size_larger_than_the_payload() {
        // A real 2-leaf tree (so each symbol costs a bit), then far too few
        // bits for the 100_000 symbols the header claims. Used to run off the
        // end of the payload and panic.
        let bytes = container_of(&block_with(100_000, &[0x50, 0x68, 0x40]));
        assert!(
            matches!(decode(&bytes), Err(SaveError::Huffman(_))),
            "truncated payload must be reported, not panic"
        );
    }

    #[test]
    fn rejects_absurd_proto_size() {
        let bytes = container_of(&block_with(u32::MAX, &[0x50, 0x68, 0x40]));
        assert!(matches!(decode(&bytes), Err(SaveError::Size(_))));
    }

    #[test]
    fn rejects_pathologically_deep_tree() {
        // All-zero bits = "internal node" forever, i.e. unbounded recursion.
        let bytes = container_of(&block_with(16, &[0u8; 64]));
        assert!(
            matches!(decode(&bytes), Err(SaveError::Huffman(_))),
            "a 512-level tree must be refused before the stack blows"
        );
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let mut block = block_with(1, &[0xFF; 8]);
        block[4] = b'X'; // "WSG" -> "XSG"
        assert!(matches!(
            decode(&container_of(&block)),
            Err(SaveError::BadMagic)
        ));

        let mut block = block_with(1, &[0xFF; 8]);
        block[7] = 9; // version 2 -> 9
        assert!(matches!(
            decode(&container_of(&block)),
            Err(SaveError::BadVersion(9))
        ));
    }

    #[test]
    fn rejects_garbage_that_is_not_lzo() {
        // 24 bytes of noise: LZO decompression should fail cleanly.
        let junk: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(31)).collect();
        assert!(decode(&junk).is_err());
    }

    /// Fuzz-ish: truncating a real save at every length must never panic.
    #[test]
    fn truncations_of_a_valid_save_never_panic() {
        let proto: Vec<u8> = (0..256u32).flat_map(|i| [0x10, (i % 100) as u8]).collect();
        let good = encode(&proto).unwrap();
        for len in 0..good.len() {
            let _ = decode(&good[..len]); // must return, panicking fails the test
        }
    }
}
