// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Error;

// Unlike the multiformats "uvarint", we allow 10 bytes here so we can encode a full uint64.
const VARINT_MAX_BYTES: usize = 10;

/// A `BitReader` allows for efficiently reading bits from a byte buffer, up to a byte at a time.
///
/// It works by always storing at least the next 8 bits in `bits`, which lets us conveniently
/// and efficiently read bits that cross a byte boundary.
pub struct BitReader<'a> {
    /// The bytes that have not been read from yet.
    bytes: &'a [u8],
    /// The next bits to be read.
    bits: u64,

    /// The number of bits in `bits` from bytes that came before `next_byte` (at least 8, at most 15).
    num_bits: u32,
}

impl<'a> BitReader<'a> {
    /// Creates a new `BitReader`.
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        // There are infinite implicit "0"s, so we don't expect any trailing zeros in the actual
        // data.
        if bytes.last() == Some(&0) {
            return Err(Error::NotMinimal);
        }
        let mut bits = 0u64;
        for i in 0..2 {
            let byte = bytes.get(i).unwrap_or(&0);
            bits |= (*byte as u64) << (8 * i);
        }

        let bytes = bytes.get(2..).unwrap_or(&[]);

        Ok(Self {
            bytes,
            bits,
            num_bits: 16,
        })
    }

    /// Peeks a given number of bits from the buffer.Will keep returning 0 once
    /// the buffer has been exhausted.
    #[inline(always)]
    pub fn peek(&self, num_bits: u32) -> u8 {
        debug_assert!(num_bits <= 8);

        // creates a mask with a `num_bits` number of 1s in order
        // to get only the bits we need from `self.bits`
        let mask = (1 << num_bits) - 1;
        (self.bits & mask) as u8
    }

    /// Drops a number of bits from the buffer
    #[inline(always)]
    pub fn drop(&mut self, num_bits: u32) {
        debug_assert!(num_bits <= 8);

        // removes the bits
        self.bits >>= num_bits;
        self.num_bits -= num_bits;

        // not sure why this being outside of the if improves the performance
        // bit it does, probably related to keeping caches warm
        let byte = self.bytes.first().unwrap_or(&0);
        self.bits |= (*byte as u64) << self.num_bits;

        // if fewer than 8 bits remain, we skip to loading the next byte
        if self.num_bits < 8 {
            self.num_bits += 8;
            self.bytes = self.bytes.get(1..).unwrap_or(&[]);
        }
    }

    /// Reads a given number of bits from the buffer. Will keep returning 0 once
    /// the buffer has been exhausted.
    pub fn read(&mut self, num_bits: u32) -> u8 {
        debug_assert!(num_bits <= 8);

        let res = self.peek(num_bits);
        self.drop(num_bits);

        res
    }

    /// Reads a varint from the buffer. Returns an error if the
    /// current position on the buffer contains no valid varint.
    fn read_varint(&mut self) -> Result<u64, Error> {
        let mut len = 0u64;

        for i in 0..VARINT_MAX_BYTES {
            let byte = self.read(8);

            // strip off the most significant bit and add
            // it to the output
            len |= (byte as u64 & 0x7f) << (i * 7);

            // if the most significant bit is a 0, we've
            // reached the end of the varint
            if byte & 0x80 == 0 {
                // 1. We only allow the 9th byte to be 1 (overflows u64).
                // 2. The last byte cannot be 0 (not minimally encoded).
                if (i == 9 && byte > 1) || (byte == 0 && i != 0) {
                    break;
                }
                return Ok(len);
            }
        }

        Err(Error::InvalidVarint)
    }

    /// Reads a length from the buffer according to RLE+ encoding.
    pub fn read_len(&mut self) -> Result<Option<u64>, Error> {
        // We're done.
        if !self.has_more() {
            return Ok(None);
        }

        let peek6 = self.peek(6);

        let len = if peek6 & 0b01 != 0 {
            // Block Single (prefix 1)
            self.drop(1);
            1
        } else if peek6 & 0b10 != 0 {
            // Block Short (prefix 01)
            let val = ((peek6 >> 2) & 0x0f) as u64;
            self.drop(6);
            if val < 2 {
                return Err(Error::NotMinimal);
            }
            val
        } else {
            // Block Long (prefix 00)
            self.drop(2);
            let val = self.read_varint()?;
            if val < 16 {
                return Err(Error::NotMinimal);
            }
            val
        };

        Ok(Some(len))
    }

    /// Returns true if there are more non-zero bits to be read.
    pub fn has_more(&self) -> bool {
        self.bits != 0 || !self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::BitReader;

    #[test]
    fn read() {
        let bytes = &[0b1011_1110, 0b0111_0010, 0b0010_1010];
        let mut reader = BitReader::new(bytes).unwrap();

        assert_eq!(reader.read(0), 0);
        assert_eq!(reader.read(1), 0);
        assert_eq!(reader.read(3), 0b111);
        assert_eq!(reader.read(6), 0b101011);
        assert_eq!(reader.read(1), 0);
        assert_eq!(reader.read(4), 0b1110);
        assert_eq!(reader.read(3), 0b100);
        assert_eq!(reader.read(2), 0b10);
        assert_eq!(reader.read(3), 0b010);
        assert_eq!(reader.read(4), 0);
        assert_eq!(reader.read(8), 0);
        assert_eq!(reader.read(0), 0);
    }

    #[test]
    fn read_len() {
        let bytes = &[0b0001_0101, 0b1101_0111, 0b0110_0111, 0b00110010];
        let mut reader = BitReader::new(bytes).unwrap();

        assert_eq!(reader.read_len().unwrap(), Some(1)); // prefix: 1
        assert_eq!(reader.read_len().unwrap(), Some(2)); // prefix: 01, value: 0100 (LSB to MSB)
        assert_eq!(reader.read_len().unwrap(), Some(11)); // prefix: 01, value: 1101
        assert_eq!(reader.read_len().unwrap(), Some(15)); // prefix: 01, value: 1111
        assert_eq!(reader.read_len().unwrap(), Some(147)); // prefix: 00, value: 11001001 10000000
        assert_eq!(reader.read_len().unwrap(), None);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "assertion failed")]
    fn too_many_bits_at_once() {
        let mut reader = BitReader::new(&[]).unwrap();
        reader.read(16);
    }

    #[test]
    fn roundtrip() {
        use rand::{Rng, SeedableRng};
        use rand_xorshift::XorShiftRng;

        use super::super::BitWriter;

        let mut rng = XorShiftRng::seed_from_u64(5);

        for _ in 0..100 {
            let lengths: Vec<u64> = std::iter::repeat_with(|| rng.gen_range(1..200))
                .take(100)
                .collect();

            let mut writer = BitWriter::new();

            for &len in &lengths {
                writer.write_len(len);
            }

            let bytes = writer.finish();
            let mut reader = BitReader::new(&bytes).unwrap();

            for &len in &lengths {
                assert_eq!(reader.read_len().unwrap(), Some(len));
            }

            assert_eq!(reader.read_len().unwrap(), None);
        }
    }
}
