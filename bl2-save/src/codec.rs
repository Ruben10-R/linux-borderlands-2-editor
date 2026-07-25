//! Borderlands 2 `.sav` codec: SHA1 + LZO1x + WSG + custom Huffman + CRC32.
//!
//! Decodes a `.sav` down to raw protobuf bytes and re-encodes it back.
//! Outer sizes are big-endian; inner WSG fields are little-endian (PC platform).
//!
//! Proven byte-correct against real saves AND accepted in-game (see PLAN.md §4.1).

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
    fn read_bit(&mut self) -> u8 {
        let byte = self.data[self.pos >> 3];
        let bit = (byte >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        bit
    }
    fn read_byte(&mut self) -> u8 {
        let mut v = 0u8;
        for _ in 0..8 {
            v = (v << 1) | self.read_bit();
        }
        v
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

fn read_tree(br: &mut BitReader) -> Node {
    if br.read_bit() == 1 {
        Node::Leaf(br.read_byte())
    } else {
        let left = read_tree(br);
        let right = read_tree(br);
        Node::Internal(Box::new(left), Box::new(right))
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

fn huffman_decode(data: &[u8], out_len: usize) -> Vec<u8> {
    let mut br = BitReader::new(data);
    let root = read_tree(&mut br);
    let mut out = Vec::with_capacity(out_len);
    while out.len() < out_len {
        let mut node = &root;
        loop {
            match node {
                Node::Leaf(v) => {
                    out.push(*v);
                    break;
                }
                Node::Internal(l, r) => {
                    node = if br.read_bit() == 0 { l } else { r };
                }
            }
        }
    }
    out
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
    let outer = lzokay_native::decompress_all(&raw[24..], Some(outer_size))
        .map_err(|e| SaveError::Lzo(format!("decompress: {e}")))?;
    if outer.len() != outer_size {
        return Err(SaveError::Size(format!(
            "LZO output {} != declared outer size {}",
            outer.len(),
            outer_size
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

    let proto = huffman_decode(&outer[19..], proto_size);
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
    // apocalyptech append exactly 4 zero bytes here; match that. (See PLAN.md §4.1.)
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
}
