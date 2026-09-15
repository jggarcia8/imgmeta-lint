// Minimal RFC 1950/1951 (zlib/DEFLATE) decoder. zTXt and compressed iTXt
// chunks in PNG are zlib streams, and pulling in a crate just to unwrap a
// handful of kilobytes of text metadata would defeat the point of a
// dependency-free linter.

use std::collections::HashMap;

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, byte_pos: 0, bit_pos: 0 }
    }

    fn read_bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.byte_pos)?;
        let bit = (byte >> self.bit_pos) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit as u32)
    }

    // Header fields and extra-bit values are packed LSB-first.
    fn read_bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.read_bit()? << i;
        }
        Some(v)
    }

    fn align_to_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }
}

// Canonical Huffman decode table keyed by (code length, code value).
type HuffTable = HashMap<(u8, u32), u16>;

fn build_huffman(lengths: &[u8]) -> HuffTable {
    let max_bits = lengths.iter().copied().max().unwrap_or(0) as usize;
    let mut bl_count = vec![0u32; max_bits + 1];
    for &l in lengths {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = vec![0u32; max_bits + 2];
    let mut code = 0u32;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    let mut table = HashMap::new();
    for (sym, &len) in lengths.iter().enumerate() {
        if len > 0 {
            let c = next_code[len as usize];
            next_code[len as usize] += 1;
            table.insert((len, c), sym as u16);
        }
    }
    table
}

// Huffman codes are packed most-significant-bit first, unlike everything
// else in the stream, so this builds the code up one bit at a time.
fn decode_symbol(br: &mut BitReader, table: &HuffTable) -> Option<u16> {
    let mut code = 0u32;
    for len in 1..=15u8 {
        code = (code << 1) | br.read_bit()?;
        if let Some(&sym) = table.get(&(len, code)) {
            return Some(sym);
        }
    }
    None
}

fn fixed_tables() -> (HuffTable, HuffTable) {
    let mut lit_lengths = [0u8; 288];
    for (i, len) in lit_lengths.iter_mut().enumerate() {
        *len = if i < 144 {
            8
        } else if i < 256 {
            9
        } else if i < 280 {
            7
        } else {
            8
        };
    }
    let dist_lengths = [5u8; 30];
    (build_huffman(&lit_lengths), build_huffman(&dist_lengths))
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate_block(
    br: &mut BitReader,
    lit_table: &HuffTable,
    dist_table: &HuffTable,
    out: &mut Vec<u8>,
) -> Option<()> {
    loop {
        let sym = decode_symbol(br, lit_table)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Some(());
        } else {
            let idx = (sym - 257) as usize;
            let extra = *LEN_EXTRA.get(idx)?;
            let length = *LEN_BASE.get(idx)? as usize + br.read_bits(extra as u32)? as usize;

            let dist_sym = decode_symbol(br, dist_table)? as usize;
            let dextra = *DIST_EXTRA.get(dist_sym)?;
            let distance =
                *DIST_BASE.get(dist_sym)? as usize + br.read_bits(dextra as u32)? as usize;

            if distance == 0 || distance > out.len() {
                return None;
            }
            let start = out.len() - distance;
            for i in 0..length {
                out.push(out[start + i]);
            }
        }
    }
}

fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut br = BitReader::new(data);
    let mut out = Vec::new();

    loop {
        let bfinal = br.read_bit()?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_to_byte();
                let len = br.read_bits(16)? as usize;
                let nlen = br.read_bits(16)? as usize;
                if (len ^ 0xFFFF) != nlen {
                    return None;
                }
                for _ in 0..len {
                    out.push(br.read_bits(8)? as u8);
                }
            }
            1 => {
                let (lit_table, dist_table) = fixed_tables();
                inflate_block(&mut br, &lit_table, &dist_table, &mut out)?;
            }
            2 => {
                let hlit = br.read_bits(5)? as usize + 257;
                let hdist = br.read_bits(5)? as usize + 1;
                let hclen = br.read_bits(4)? as usize + 4;
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl_lengths = [0u8; 19];
                for &slot in ORDER.iter().take(hclen) {
                    cl_lengths[slot] = br.read_bits(3)? as u8;
                }
                let cl_table = build_huffman(&cl_lengths);

                let mut lengths = Vec::with_capacity(hlit + hdist);
                while lengths.len() < hlit + hdist {
                    match decode_symbol(&mut br, &cl_table)? {
                        sym @ 0..=15 => lengths.push(sym as u8),
                        16 => {
                            let repeat = br.read_bits(2)? + 3;
                            let prev = lengths.last().copied()?;
                            for _ in 0..repeat {
                                lengths.push(prev);
                            }
                        }
                        17 => {
                            let repeat = br.read_bits(3)? + 3;
                            for _ in 0..repeat {
                                lengths.push(0);
                            }
                        }
                        18 => {
                            let repeat = br.read_bits(7)? + 11;
                            for _ in 0..repeat {
                                lengths.push(0);
                            }
                        }
                        _ => return None,
                    }
                }
                if lengths.len() != hlit + hdist {
                    return None;
                }
                let lit_table = build_huffman(&lengths[..hlit]);
                let dist_table = build_huffman(&lengths[hlit..]);
                inflate_block(&mut br, &lit_table, &dist_table, &mut out)?;
            }
            _ => return None,
        }
        if bfinal == 1 {
            break;
        }
    }

    Some(out)
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

/// Decodes a zlib stream (2-byte header, DEFLATE payload, 4-byte Adler-32
/// trailer) as used by PNG's zTXt and compressed iTXt chunks.
pub fn zlib_decompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 6 {
        return None;
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0F != 8 {
        return None; // not the DEFLATE compression method
    }
    if (cmf as u16 * 256 + flg as u16) % 31 != 0 {
        return None; // header check bits don't match
    }
    if flg & 0x20 != 0 {
        return None; // preset dictionary not supported
    }

    let trailer = data.len() - 4;
    let compressed = &data[2..trailer];
    let expected = u32::from_be_bytes([data[trailer], data[trailer + 1], data[trailer + 2], data[trailer + 3]]);

    let out = inflate(compressed)?;
    if adler32(&out) != expected {
        return None;
    }
    Some(out)
}
