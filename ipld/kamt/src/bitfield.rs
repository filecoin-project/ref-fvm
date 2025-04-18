// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use byteorder::{BigEndian, ByteOrder};
use fvm_ipld_encoding::de::{Deserialize, Deserializer};
use fvm_ipld_encoding::ser::{Serialize, Serializer};
use fvm_ipld_encoding::strict_bytes;
use serde::de::Error;

const MAX_LEN: usize = 4 * 8;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct Bitfield([u64; 4]);

impl Serialize for Bitfield {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut v = [0u8; MAX_LEN];
        // Big endian ordering, to match go
        BigEndian::write_u64(&mut v[..8], self.0[3]);
        BigEndian::write_u64(&mut v[8..16], self.0[2]);
        BigEndian::write_u64(&mut v[16..24], self.0[1]);
        BigEndian::write_u64(&mut v[24..], self.0[0]);

        for i in 0..v.len() {
            if v[i] != 0 {
                return strict_bytes::Serialize::serialize(&v[i..], serializer);
            }
        }

        <[u8] as strict_bytes::Serialize>::serialize(&[], serializer)
    }
}

impl<'de> Deserialize<'de> for Bitfield {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut res = Bitfield::zero();
        let strict_bytes::ByteBuf(bytes) = Deserialize::deserialize(deserializer)?;
        if bytes.len() > MAX_LEN {
            return Err(Error::invalid_length(
                bytes.len(),
                &"bitfield length exceeds maximum",
            ));
        }

        let mut arr = [0u8; MAX_LEN];
        arr[MAX_LEN - bytes.len()..].copy_from_slice(&bytes);
        res.0[3] = BigEndian::read_u64(&arr[..8]);
        res.0[2] = BigEndian::read_u64(&arr[8..16]);
        res.0[1] = BigEndian::read_u64(&arr[16..24]);
        res.0[0] = BigEndian::read_u64(&arr[24..]);

        Ok(res)
    }
}

impl Default for Bitfield {
    fn default() -> Self {
        Bitfield::zero()
    }
}

impl Bitfield {
    pub fn clear_bit(&mut self, idx: u32) {
        let ai = idx / 64;
        let bi = idx % 64;
        self.0[ai as usize] &= u64::MAX - (1 << bi);
    }

    pub fn test_bit(&self, idx: u32) -> bool {
        let ai = idx / 64;
        let bi = idx % 64;

        self.0[ai as usize] & (1 << bi) != 0
    }

    pub fn set_bit(&mut self, idx: u32) {
        let ai = idx / 64;
        let bi = idx % 64;

        self.0[ai as usize] |= 1 << bi;
    }

    pub fn count_ones(&self) -> usize {
        self.0.iter().map(|a| a.count_ones() as usize).sum()
    }

    pub fn last_one_idx(&self) -> Option<usize> {
        let mut idx = self.0.len() as u32 * u64::BITS;
        for v in self.0.iter().rev().map(|&v| v.leading_zeros()) {
            idx -= v;
            if v < u64::BITS {
                return Some((idx - 1) as usize);
            }
        }
        None
    }

    pub fn and(self, other: &Self) -> Self {
        Bitfield([
            self.0[0] & other.0[0],
            self.0[1] & other.0[1],
            self.0[2] & other.0[2],
            self.0[3] & other.0[3],
        ])
    }

    pub fn zero() -> Self {
        Bitfield([0, 0, 0, 0])
    }

    pub fn set_bits_le(self, bit: u32) -> Self {
        if bit == 0 {
            return self;
        }
        self.set_bits_leq(bit - 1)
    }

    pub fn set_bits_leq(mut self, bit: u32) -> Self {
        if bit < 64 {
            self.0[0] = set_bits_leq(self.0[0], bit);
        } else if bit < 128 {
            self.0[0] = u64::MAX;
            self.0[1] = set_bits_leq(self.0[1], bit - 64);
        } else if bit < 192 {
            self.0[0] = u64::MAX;
            self.0[1] = u64::MAX;
            self.0[2] = set_bits_leq(self.0[2], bit - 128);
        } else {
            self.0[0] = u64::MAX;
            self.0[1] = u64::MAX;
            self.0[2] = u64::MAX;
            self.0[3] = set_bits_leq(self.0[3], bit - 192);
        }

        self
    }
}

#[inline]
fn set_bits_leq(v: u64, bit: u32) -> u64 {
    (v as u128 | ((1u128 << (1 + bit)) - 1)) as u64
}

impl std::fmt::Binary for Bitfield {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let val = self.0;

        write!(f, "{:b}_{:b}_{:b}_{:b}", val[0], val[1], val[2], val[3])
    }
}

#[cfg(test)]
mod tests {
    use fvm_ipld_encoding::{from_slice, to_vec};
    use quickcheck_macros::quickcheck;

    use super::*;

    #[test]
    fn test_bitfield() {
        let mut b = Bitfield::zero();
        b.set_bit(8);
        b.set_bit(18);
        b.set_bit(92);
        b.set_bit(255);
        assert!(b.test_bit(8));
        assert!(b.test_bit(18));
        assert!(!b.test_bit(19));
        assert!(b.test_bit(92));
        assert!(!b.test_bit(95));
        assert!(b.test_bit(255));

        b.clear_bit(18);
        assert!(!b.test_bit(18));
    }

    #[test]
    fn test_cbor_serialization() {
        let mut b0 = Bitfield::zero();
        let bz = to_vec(&b0).unwrap();
        assert_eq!(&bz, &[64]);
        assert_eq!(&from_slice::<Bitfield>(&bz).unwrap(), &b0);

        b0.set_bit(0);
        let bz = to_vec(&b0).unwrap();
        assert_eq!(&bz, &[65, 1]);
        assert_eq!(&from_slice::<Bitfield>(&bz).unwrap(), &b0);

        b0.set_bit(64);
        let bz = to_vec(&b0).unwrap();
        assert_eq!(&bz, &[73, 1, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(&from_slice::<Bitfield>(&bz).unwrap(), &b0);
    }

    #[quickcheck]
    fn last_one_idx_of_single_is_self(idx: u8) -> bool {
        let mut b = Bitfield::zero();
        let i1 = b.last_one_idx();
        b.set_bit(idx as u32);
        let i2 = b.last_one_idx();
        i1.is_none() && i2 == Some(idx as usize)
    }

    #[quickcheck]
    fn last_one_idx_of_multi_is_max(idx1: u8, idx2: u8) -> bool {
        let mut b = Bitfield::zero();
        b.set_bit(idx1 as u32);
        b.set_bit(idx2 as u32);
        b.last_one_idx() == Some(std::cmp::max(idx1, idx2) as usize)
    }
}
