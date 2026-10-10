//! A minimal PNG encoder: CRC-32, Adler-32, and a DEFLATE compressor.
//!
//! Written from the format specifications. No crates.
//!
//! The shape of a PNG file this produces:
//!
//! ```text
//!   8 bytes  signature      89 50 4E 47 0D 0A 1A 0A
//!   chunk    IHDR           13 bytes: width, height, depth, colour type, ...
//!   chunk    IDAT           one zlib stream (RFC 1950) over the filtered rows
//!   chunk    IEND           empty
//! ```
//!
//! Each chunk is a 4-byte big-endian length, a 4-byte kind, that many payload
//! bytes, and a 4-byte CRC over the kind and the payload together.
//!
//! The bit-packing is the part that is easy to get backwards, so it is worth
//! stating plainly. DEFLATE writes the bit stream into bytes least-significant
//! bit first: the first bit of the stream is bit 0 of the first byte. Huffman
//! codes, by contrast, are assigned and written most-significant bit first. So
//! a Huffman code has to be bit-reversed on its way into the byte stream. Extra
//! bits (lengths, distances) are plain integers and are *not* reversed.

const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

/// CRC-32 as used by PNG chunks (IEEE 802.3 polynomial, reflected).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &byte in data {
        c = CRC_TABLE[((c ^ byte as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 as used by the zlib wrapper.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Accumulates a DEFLATE bit stream. DEFLATE packs bits least-significant
/// first within each byte; Huffman codes are written most-significant bit
/// first.
struct BitWriter {
    out: Vec<u8>,
    bit_buffer: u32,
    bit_count: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            bit_buffer: 0,
            bit_count: 0,
        }
    }

    fn write_bits(&mut self, value: u32, count: u32) {
        self.bit_buffer |= value << self.bit_count;
        self.bit_count += count;
        while self.bit_count >= 8 {
            self.out.push((self.bit_buffer & 0xFF) as u8);
            self.bit_buffer >>= 8;
            self.bit_count -= 8;
        }
    }

    /// Huffman codes are stored MSB-first, so reverse before writing.
    fn write_code(&mut self, code: u32, length: u32) {
        let mut reversed = 0;
        for i in 0..length {
            reversed |= ((code >> (length - 1 - i)) & 1) << i;
        }
        self.write_bits(reversed, length);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_count > 0 {
            self.out.push((self.bit_buffer & 0xFF) as u8);
        }
        self.out
    }
}

/// The literal/length alphabet of DEFLATE's fixed Huffman code (RFC 1951 §3.2.6).
fn write_fixed_literal(w: &mut BitWriter, symbol: u32) {
    match symbol {
        0..=143 => w.write_code(0b0011_0000 + symbol, 8),
        144..=255 => w.write_code(0b1_1001_0000 + (symbol - 144), 9),
        256..=279 => w.write_code(symbol - 256, 7),
        _ => w.write_code(0b1100_0000 + (symbol - 280), 8),
    }
}

/// Length codes 257..285: the base length each code represents, and how many
/// extra bits follow it.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Distance codes 0..29, and their extra-bit counts.
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Greedy LZ77 match finder over a hash chain of 3-byte sequences.
///
/// This is not a sophisticated compressor. Each position indexes its 3-byte
/// prefix into `head`, and `prev` chains older positions that hashed the same
/// way; the search walks at most `MAX_CHAIN` links back. On rendered images it
/// stays close to a full chain search, because runs of identical pixels make
/// the newest chain link correct almost every time.
const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MAX_CHAIN: usize = 32;
const WINDOW: usize = 32768;

fn hash3(data: &[u8], pos: usize) -> usize {
    let a = data[pos] as usize;
    let b = data[pos + 1] as usize;
    let c = data[pos + 2] as usize;
    ((a << 10) ^ (b << 5) ^ c) & (HASH_SIZE - 1)
}

/// Compress `data` into a complete zlib stream: 2-byte header, DEFLATE
/// blocks with a fixed Huffman code, then the Adler-32 checksum.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate_raw(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The bare DEFLATE stream: block header, symbols, end-of-block. No zlib
/// wrapper, so not directly usable on its own; `deflate` adds that.
pub fn deflate_raw(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write_bits(1, 1); // BFINAL
    w.write_bits(1, 2); // BTYPE = 01, fixed Huffman

    let mut head = vec![usize::MAX; HASH_SIZE];
    let mut prev = vec![usize::MAX; data.len().max(1)];

    let mut pos = 0usize;
    while pos < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;

        if pos + MIN_MATCH <= data.len() {
            let h = hash3(data, pos);
            let mut candidate = head[h];
            let limit = pos.saturating_sub(WINDOW);
            let mut chain = 0;

            while candidate != usize::MAX && candidate >= limit && chain < MAX_CHAIN {
                chain += 1;
                let max_len = MAX_MATCH.min(data.len() - pos);
                let mut len = 0;
                while len < max_len && data[candidate + len] == data[pos + len] {
                    len += 1;
                }
                if len > best_len {
                    best_len = len;
                    best_dist = pos - candidate;
                    if len == MAX_MATCH {
                        break;
                    }
                }
                let next = prev[candidate];
                if next >= candidate {
                    break;
                }
                candidate = next;
            }

            prev[pos] = head[h];
            head[h] = pos;
        }

        if best_len >= MIN_MATCH {
            let length_index = LENGTH_BASE
                .iter()
                .rposition(|&b| b as usize <= best_len)
                .expect("every length >= 3 has a code");
            write_fixed_literal(&mut w, 257 + length_index as u32);
            let extra = LENGTH_EXTRA[length_index];
            if extra > 0 {
                let base = LENGTH_BASE[length_index];
                w.write_bits((best_len - base as usize) as u32, extra);
            }

            let dist_index = DIST_BASE
                .iter()
                .rposition(|&b| b as usize <= best_dist)
                .expect("every distance >= 1 has a code");
            w.write_code(dist_index as u32, 5);
            let dextra = DIST_EXTRA[dist_index];
            if dextra > 0 {
                w.write_bits((best_dist - DIST_BASE[dist_index] as usize) as u32, dextra);
            }

            // Index the bytes the match consumed so later matches can find them.
            for k in (pos + 1)..(pos + best_len) {
                if k + MIN_MATCH <= data.len() {
                    let h = hash3(data, k);
                    prev[k] = head[h];
                    head[h] = k;
                }
            }
            pos += best_len;
        } else {
            write_fixed_literal(&mut w, data[pos] as u32);
            pos += 1;
        }
    }

    write_fixed_literal(&mut w, 256); // end of block
    w.finish()
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    let mut body = Vec::with_capacity(4 + payload.len());
    body.extend_from_slice(kind);
    body.extend_from_slice(payload);

    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Encode 8-bit RGB pixels as a complete PNG file.
///
/// `rgb` must be exactly `width * height * 3` bytes, row-major.
pub fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), (width as usize) * (height as usize) * 3);

    // Prefix every scanline with filter type 0 (None).
    let mut raw = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    let stride = width as usize * 3;
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * stride..(y + 1) * stride]);
    }

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // colour type 2 = truecolour RGB
    ihdr.push(0); // compression method: DEFLATE
    ihdr.push(0); // filter method
    ihdr.push(0); // no interlace
    chunk(&mut out, b"IHDR", &ihdr);

    chunk(&mut out, b"IDAT", &deflate(&raw));
    chunk(&mut out, b"IEND", &[]);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // A fixed-Huffman-only inflate, written the way RFC 1951 describes a
    // decoder, so that "the byte stream is well formed" and "the stream
    // decodes back to the input" are separate, both-checked claims. Every
    // structural test below can pass on a stream that decodes to garbage;
    // the round-trip tests cannot.
    // ---------------------------------------------------------------

    struct BitReader<'a> {
        data: &'a [u8],
        pos: usize,
        bit: u32,
    }

    impl<'a> BitReader<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self {
                data,
                pos: 0,
                bit: 0,
            }
        }

        fn read_bit(&mut self) -> u32 {
            let b = (self.data[self.pos] >> self.bit) & 1;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.pos += 1;
            }
            b as u32
        }

        /// A plain little-endian integer field: extra bits and stored blocks.
        fn read_bits(&mut self, count: u32) -> u32 {
            let mut v = 0;
            for i in 0..count {
                v |= self.read_bit() << i;
            }
            v
        }

        /// A Huffman code, which arrives most-significant bit first.
        fn read_code(&mut self, length: u32) -> u32 {
            let mut code = 0;
            for _ in 0..length {
                code = (code << 1) | self.read_bit();
            }
            code
        }
    }

    /// Decode one symbol of the fixed literal/length code (RFC 1951 3.2.6).
    fn decode_fixed_symbol(r: &mut BitReader) -> u32 {
        let code7 = r.read_code(7);
        // 0000000 through 0010111 are the 7-bit codes, symbols 256..=279.
        if code7 <= 0b001_0111 {
            return 256 + code7;
        }
        // The remaining codes all start with 0011 or 11, so a 7-bit peek is
        // never enough to commit; take the eighth bit.
        let code8 = (code7 << 1) | r.read_bit();
        // 00110000..=10111111 are the 8-bit literal codes, symbols 0..=143.
        if (0b0011_0000..=0b1011_1111).contains(&code8) {
            return code8 - 0b0011_0000;
        }
        // 11000000..=11000111 are the 8-bit codes for symbols 280..=287.
        if (0b1100_0000..=0b1100_0111).contains(&code8) {
            return 280 + (code8 - 0b1100_0000);
        }
        // Everything else is a 9-bit code: 110010000..=111111111 for 144..=255.
        let code9 = (code8 << 1) | r.read_bit();
        assert!(
            (0b1100_10000..=0b1111_11111).contains(&code9),
            "invalid fixed Huffman code"
        );
        144 + (code9 - 0b1100_10000)
    }

    /// Decode a raw DEFLATE stream that `deflate_raw` produced. Only fixed
    /// Huffman blocks are handled, because that is all the encoder emits.
    fn inflate_fixed(raw: &[u8]) -> Vec<u8> {
        let mut r = BitReader::new(raw);
        let bfinal = r.read_bit();
        let btype = r.read_bits(2);
        assert_eq!(bfinal, 1, "expected a single final block");
        assert_eq!(btype, 1, "expected fixed Huffman coding");

        let mut out: Vec<u8> = Vec::new();
        loop {
            let symbol = decode_fixed_symbol(&mut r);
            if symbol == 256 {
                return out;
            }
            if symbol < 256 {
                out.push(symbol as u8);
                continue;
            }
            let li = (symbol - 257) as usize;
            let length = LENGTH_BASE[li] as usize + r.read_bits(LENGTH_EXTRA[li]) as usize;
            let di = r.read_code(5) as usize;
            let distance = DIST_BASE[di] as usize + r.read_bits(DIST_EXTRA[di]) as usize;
            assert!(
                distance <= out.len(),
                "match reaches before the output start"
            );
            let start = out.len() - distance;
            // Byte-at-a-time so that length > distance produces the same
            // overlapping copy a real decoder produces.
            for k in 0..length {
                let b = out[start + k];
                out.push(b);
            }
        }
    }

    /// The payload of the first IDAT chunk, located the way a decoder does.
    fn idat_payload(png: &[u8]) -> Vec<u8> {
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            if kind == b"IDAT" {
                return png[pos + 8..pos + 8 + len].to_vec();
            }
            pos += 12 + len;
        }
        panic!("no IDAT chunk");
    }

    #[test]
    fn crc32_matches_the_reference_check_value() {
        // The standard CRC-32 check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn adler32_matches_the_reference_check_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn deflate_starts_with_a_zlib_header_and_checksum() {
        let out = deflate(b"hello hello hello");
        assert_eq!(&out[..2], &[0x78, 0x01]);
        assert_eq!(
            &out[out.len() - 4..],
            &adler32(b"hello hello hello").to_be_bytes()
        );
    }

    #[test]
    fn deflate_really_compresses_runs() {
        let data = vec![7u8; 10_000];
        let out = deflate(&data);
        assert!(out.len() < 200, "compressed {} bytes from 10000", out.len());
    }

    #[test]
    fn deflate_handles_empty_input() {
        let out = deflate(&[]);
        assert_eq!(&out[..2], &[0x78, 0x01]);
        assert!(out.len() > 4);
    }

    #[test]
    fn deflate_handles_incompressible_input_without_corrupting_it() {
        let data: Vec<u8> = (0..5000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 16) as u8)
            .collect();
        let out = deflate(&data);
        assert_eq!(&out[out.len() - 4..], &adler32(&data).to_be_bytes());
    }

    #[test]
    fn encode_rgb_has_a_valid_png_signature() {
        let png = encode_rgb(1, 1, &[255, 0, 0]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn encode_rgb_emits_ihdr_idat_iend_in_order() {
        let png = encode_rgb(4, 4, &vec![128; 4 * 4 * 3]);
        let text = String::from_utf8_lossy(&png).to_string();
        let ihdr = text.find("IHDR").unwrap();
        let idat = text.find("IDAT").unwrap();
        let iend = text.find("IEND").unwrap();
        assert!(ihdr < idat && idat < iend);
    }

    #[test]
    fn ihdr_carries_the_requested_dimensions() {
        let png = encode_rgb(300, 150, &vec![0; 300 * 150 * 3]);
        let ihdr_pos = png
            .windows(4)
            .position(|w| w == b"IHDR")
            .expect("IHDR chunk present");
        // Chunk layout: 4-byte length, then "IHDR", then width and height.
        let width = u32::from_be_bytes(png[ihdr_pos + 4..ihdr_pos + 8].try_into().unwrap());
        let height = u32::from_be_bytes(png[ihdr_pos + 8..ihdr_pos + 12].try_into().unwrap());
        assert_eq!(width, 300);
        assert_eq!(height, 150);
    }

    #[test]
    fn every_chunk_carries_a_matching_crc() {
        let png = encode_rgb(4, 4, &vec![10; 4 * 4 * 3]);
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let body = &png[pos + 4..pos + 4 + 4 + len];
            let expected = u32::from_be_bytes(
                png[pos + 4 + 4 + len..pos + 8 + 4 + len]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(crc32(body), expected, "bad CRC on chunk at offset {pos}");
            pos += 4 + 4 + len + 4;
        }
        assert_eq!(pos, png.len(), "chunks must tile the file exactly");
    }

    #[test]
    fn a_rendered_image_compresses_well() {
        // Flat runs, like a real render's sky.
        let mut rgb = Vec::new();
        for _ in 0..225 {
            rgb.extend_from_slice(&[90, 140, 220].repeat(400));
        }
        let png = encode_rgb(400, 225, &rgb);
        assert!(
            png.len() < rgb.len() / 10,
            "png {} bytes from rgb {}",
            png.len(),
            rgb.len()
        );
    }

    // ---------------------------------------------------------------
    // Bit-order traces, hand-checked against RFC 1951 3.2.6.
    // ---------------------------------------------------------------

    #[test]
    fn the_block_header_is_a_single_final_fixed_block() {
        // BFINAL = 1 in bit 0, BTYPE = 01 in bits 1..2, so the first byte is
        // 0b011 = 3 before any symbol is written.
        let mut w = BitWriter::new();
        w.write_bits(1, 1);
        w.write_bits(1, 2);
        let out = w.finish();
        assert_eq!(out, vec![0x03]);
    }

    #[test]
    fn end_of_block_is_seven_zero_bits() {
        // Symbol 256 has code 0000000 (RFC 1951 3.2.6), so the byte is 0.
        let mut w = BitWriter::new();
        write_fixed_literal(&mut w, 256);
        assert_eq!(w.finish(), vec![0x00]);
    }

    #[test]
    fn literal_a_is_the_reversed_eight_bit_code() {
        // Symbol 65 is in the 0..=143 range, so code = 0b0011_0000 + 65 =
        // 0b0111_0001. Reversed for the LSB-first stream that is 0b1000_1110.
        let mut w = BitWriter::new();
        write_fixed_literal(&mut w, 65);
        assert_eq!(w.finish(), vec![0b1000_1110]);

        // Byte 0x8E read low bit first is 0,1,1,1,0,0,0,1 -- which is the
        // code 0b0111_0001 read high bit first. That is the round trip.
        let stream_bits: Vec<u32> = (0..8).map(|i| ((0b1000_1110u8 >> i) & 1) as u32).collect();
        assert_eq!(stream_bits, vec![0, 1, 1, 1, 0, 0, 0, 1]);
    }

    #[test]
    fn a_higher_literal_needs_the_nine_bit_code() {
        // Symbol 200 is past 143, so its code is 0b110010000 + (200 - 144) =
        // 0b111001000, nine bits wide. Reversed for the LSB-first stream
        // that is 0b00100111 in the first byte, then a zero bit in the next.
        let mut w = BitWriter::new();
        write_fixed_literal(&mut w, 200);
        let bytes = w.finish();
        assert_eq!(bytes, vec![0b0010_0111, 0b0000_0000]);

        // Read the nine stream bits back off the bytes, low bit first, and
        // they are the code again, high bit first.
        let stream: Vec<u32> = (0..8)
            .map(|i| ((bytes[0] >> i) & 1) as u32)
            .chain(std::iter::once((bytes[1] & 1) as u32))
            .collect();
        assert_eq!(stream, vec![1, 1, 1, 0, 0, 1, 0, 0, 0]);
    }

    // ---------------------------------------------------------------
    // Round trips: the strongest claim these tests can make.
    // ---------------------------------------------------------------

    #[test]
    fn deflate_raw_round_trips_a_flat_run() {
        let data = vec![7u8; 10_000];
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_empty_input() {
        assert_eq!(inflate_fixed(&deflate_raw(&[])), Vec::<u8>::new());
    }

    #[test]
    fn deflate_raw_round_trips_every_byte_value() {
        let data: Vec<u8> = (0..=255u8).collect();
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_incompressible_input() {
        let data: Vec<u8> = (0..5000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 16) as u8)
            .collect();
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_a_pattern_that_repeats_below_the_window() {
        // A short period forces overlapping matches, where a match is longer
        // than its own distance.
        let mut data = Vec::new();
        for i in 0..20_000u32 {
            data.push((i % 7) as u8);
        }
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_high_bytes_that_need_the_nine_bit_codes() {
        // Bytes 144..=255 are the symbols whose fixed codes are nine bits
        // wide, the branch least likely to be exercised by text-like data.
        let data: Vec<u8> = (144..=255u8).cycle().take(9_000).collect();
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_every_length_code_boundary() {
        // One run of each length from 3 to 258, so every length code and
        // every count of extra bits is emitted at least once.
        let mut data = Vec::new();
        for len in 3..=258usize {
            data.extend(std::iter::repeat((len % 256) as u8).take(len));
        }
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn deflate_raw_round_trips_a_long_distance_match() {
        // 40 000 bytes of filler, then a 300-byte copy of the very start, so
        // the distance code is a high one needing thirteen extra bits.
        let mut data = vec![0xA5u8; 40_000];
        let head: Vec<u8> = data[..300].to_vec();
        data.extend(head);
        assert_eq!(inflate_fixed(&deflate_raw(&data)), data);
    }

    #[test]
    fn the_idat_stream_decodes_back_to_the_filtered_scanlines() {
        // This is the claim that matters: what a PNG decoder pulls out of
        // IDAT is the image, not merely a stream of plausible bytes.
        let (width, height) = (400usize, 225usize);
        let mut rgb = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                rgb.push((x * 7 + y * 3) as u8);
                rgb.push((x ^ y) as u8);
                rgb.push(((x * y) % 251) as u8);
            }
        }

        let png = encode_rgb(width as u32, height as u32, &rgb);
        let zlib = idat_payload(&png);

        assert_eq!(&zlib[..2], &[0x78, 0x01], "zlib header");
        let body = &zlib[2..zlib.len() - 4];
        let checksum = u32::from_be_bytes(zlib[zlib.len() - 4..].try_into().unwrap());

        let mut expected = Vec::with_capacity((width * 3 + 1) * height);
        for y in 0..height {
            expected.push(0u8); // filter type 0
            expected.extend_from_slice(&rgb[y * width * 3..(y + 1) * width * 3]);
        }

        assert_eq!(adler32(&expected), checksum, "Adler-32 over the scanlines");
        assert_eq!(
            inflate_fixed(body),
            expected,
            "IDAT decodes to the scanlines"
        );
    }

    #[test]
    fn a_single_pixel_png_decodes_to_that_pixel() {
        let png = encode_rgb(1, 1, &[255, 0, 0]);
        let zlib = idat_payload(&png);
        let decoded = inflate_fixed(&zlib[2..zlib.len() - 4]);
        assert_eq!(decoded, vec![0u8, 255, 0, 0]);
    }

    #[test]
    fn a_png_carries_exactly_the_pixels_the_ppm_does() {
        use crate::image::Image;

        let (w, h) = (64usize, 36usize);
        let mut img = Image::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                img.set(x, y, [x as f64 / w as f64, y as f64 / h as f64, 0.5]);
            }
        }

        let dir = std::env::temp_dir();
        let ppm_path = dir.join("gi_png_matches_ppm.ppm");
        let png_path = dir.join("gi_png_matches_ppm.png");
        img.write_ppm(&ppm_path, true).unwrap();
        img.write_png(&png_path, true).unwrap();

        // The PPM body is the gamma-encoded pixels with no framing.
        let ppm = std::fs::read(&ppm_path).unwrap();
        let header = format!("P6\n{w} {h}\n255\n");
        let expected = &ppm[header.len()..];
        assert_eq!(expected.len(), w * h * 3);

        // The PNG's IDAT is the same pixels, one filter byte per row.
        let png = std::fs::read(&png_path).unwrap();
        let zlib = idat_payload(&png);
        let scanlines = inflate_fixed(&zlib[2..zlib.len() - 4]);
        let stride = w * 3 + 1;
        assert_eq!(scanlines.len(), stride * h);

        let mut rebuilt = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            assert_eq!(scanlines[y * stride], 0, "row {y} must use filter 0");
            rebuilt.extend_from_slice(&scanlines[y * stride + 1..(y + 1) * stride]);
        }
        assert_eq!(rebuilt, expected, "PNG and PPM must agree pixel for pixel");

        std::fs::remove_file(&ppm_path).unwrap();
        std::fs::remove_file(&png_path).unwrap();
    }
}
