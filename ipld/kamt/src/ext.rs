// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::min;

use crate::hash_bits::{mkmask, HashBits};
use crate::{Error, HashedKey};

/// An optimization for occasions where we don't use key hashing in the KAMT,
/// which can allow keys having long common prefixes and result in parts of
/// the tree being very deep, with most but the deepest being empty. The
/// extension allows a `Pointer::Link` to skip empty levels and point straight
/// to the next non-empty `Node`.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub(crate) struct Extension {
    /// The length (in bits) of the extension between the `Node` containing the `Link`
    /// and the node the `Link` is pointing to. It might be less than the length of the
    /// slice of the path in the extension, for example if the bit width is 3 and we
    /// consumed only 6 bits out of 8, which is the length of a byte.
    length: u32,
    /// A non-empty part of the `HashedKey` that is covered by the extension.
    /// It could be represented as a vector of indices in the levels of `Node`s
    /// which were skipped, but that could take up more space. And because the
    /// path we skip can be as long as 32 bytes, it can't be represented as a number
    /// as returned by `HashBits::next`, which can only consume 8 bits at the max.
    ///
    /// It is required so we can inspect keys and decide whether they are compatible
    /// with the extension, or we need to split it.
    path: Vec<u8>,
}

impl Extension {
    pub fn new(length: u32, path: Vec<u8>) -> Self {
        Self { length, path }
    }

    pub fn len(&self) -> u32 {
        self.length
    }

    pub fn path_bits(&self) -> HashBits {
        HashBits::new_from_slice(&self.path, self.length)
    }

    pub fn path_bytes(&self) -> &[u8] {
        &self.path
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// See how many bits we can match of the path, consuming `bit_width` bits at a time.
    /// Return the total number of consumed bits, and actually consume them from the key.
    pub fn longest_match(&self, hashed_key: &mut HashBits, bit_width: u32) -> Result<u32, Error> {
        let mut path = self.path_bits();
        let mut matched = 0;
        while matched < self.length {
            let consumed = hashed_key.consumed;
            let n1 = hashed_key.next(bit_width)?;
            let n2 = path.next(bit_width)?;
            if n1 != n2 {
                hashed_key.consumed = consumed;
                break;
            }
            matched += bit_width;
        }
        Ok(matched)
    }

    /// Find the longest prefix between this key and a list of other keys, consuming `bit_width` bits at a time,
    /// starting from the point where the key has been consumed so far.
    ///
    /// Return the number of consumed bits and the bytes representing the consumed partial key as an `Extension`.
    pub fn longest_common_prefix<const N: usize>(
        hashed_key: &mut HashBits,
        bit_width: u32,
        hashes: &[HashedKey<N>],
    ) -> Result<Self, Error> {
        let mut hashes = hashes
            .iter()
            .map(|k| HashBits::new_at_index(k, hashed_key.consumed))
            .collect::<Vec<_>>();

        let mut builder = ExtensionBuilder::new();
        let total_bits = hashed_key.len();

        'consume: while hashed_key.consumed < total_bits {
            let consumed = hashed_key.consumed;
            let n = hashed_key.next(bit_width)?;

            for h in hashes.iter_mut() {
                let nh = h.next(bit_width)?;
                if n != nh {
                    hashed_key.consumed = consumed;
                    break 'consume;
                }
            }

            builder.add(bit_width, n as u8)
        }

        Ok(builder.build())
    }

    /// Split the extension after `consumed` bits into a head, a tail, and the bits between.
    ///
    /// Returns error if the consumed bits would be longer than the path.
    pub fn split(&self, consumed: u32, bit_width: u32) -> Result<(Self, Self, Self), Error> {
        let mut path = self.path_bits();
        let head = Self::from_bits(&mut path, consumed)?;
        let idx = Self::from_bits(&mut path, bit_width)?;
        let tail = Self::from_bits(&mut path, self.length - head.length - idx.length)?;
        Ok((head, idx, tail))
    }

    /// Merge two extensions, to undo a prior split.
    pub fn unsplit(ext1: &Self, idx: &Self, ext2: &Self) -> Result<Self, Error> {
        let bit_width = idx.length as u32;
        Self::merge([ext1, idx, ext2], bit_width)
    }

    /// Merge multiple extensions into one.
    fn merge<'a, I>(exts: I, bit_width: u32) -> Result<Self, Error>
    where
        I: IntoIterator<Item = &'a Self>,
    {
        let mut builder = ExtensionBuilder::new();
        for ext in exts {
            let mut path = ext.path_bits();
            let mut bits_left = ext.length as u32;
            while bits_left > 0 {
                let i = min(bit_width, bits_left);
                let n = path.next(i)?;
                builder.add(i, n as u8);
                bits_left -= i;
            }
        }
        Ok(builder.build())
    }

    /// Build an extension from a prefix of some hashed bits, starting from however
    /// far it has been consumed so far, taking the next `length` bits.
    pub fn from_bits(bits: &mut HashBits, mut length: u32) -> Result<Extension, Error> {
        let mut builder = ExtensionBuilder::new();
        while length > 0 {
            let i = min(length, 8);
            let n = bits.next(i as u32)? as u8;
            length -= i;
            builder.add(i as u32, n);
        }
        Ok(builder.build())
    }

    /// Create an extension from an index.
    pub fn from_idx(idx: u8, bit_width: u32) -> Extension {
        let mut builder = ExtensionBuilder::new();
        builder.add(bit_width, idx);
        builder.build()
    }
}

/// Helper to pack bits nibble by nibble.
struct ExtensionBuilder {
    written: u32,
    out: u8,
    path: Vec<u8>,
}

impl ExtensionBuilder {
    pub fn new() -> Self {
        Self {
            written: 0,
            out: 0,
            path: Vec::new(),
        }
    }

    /// Pack the next nibble into the path.
    pub fn add(&mut self, bit_width: u32, n: u8) {
        // See how far we have filled the current byte.
        let j = self.written % 8;
        let i = bit_width;
        if j + i > 8 {
            // The next bits don't fit in our current byte. Take the leftmost bits,
            // append the full byte to the path, then start a new one and write the
            // rightmost bits into that.
            let carry = j + i - 8;
            self.out += n >> carry;
            self.path.push(self.out);
            self.out = n & mkmask(carry as u32) as u8;
            self.out <<= 8 - carry;
        } else {
            // Haven't filled the previous byte yet, so just shift the number to
            // be aligned with where we are and fill the next leftmost bits.
            self.out += n << (8 - j - i);
        }
        self.written += i;

        if self.written % 8 == 0 {
            self.path.push(self.out);
            self.out = 0;
        }
    }

    /// Build the (possibly empty) extension after the last nibble has been added.
    pub fn build(mut self) -> Extension {
        if self.written % 8 != 0 {
            self.path.push(self.out);
        }
        Extension::new(self.written, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest() {
        let mut key: HashedKey<32> = Default::default();
        key[0] = 0b10001000;
        key[1] = 0b10010010;
        key[2] = 0b10101010;
        key[3] = 0b11011011;
        key[4] = 0b11101110;

        let mut key2 = key;
        key2[3] = 0b11010111;
        let mut key3 = key;
        key3[4] = 0b11111110;

        let mut hb = HashBits::new(&key);
        let bit_width = 3;
        // Consume some of the key
        assert_eq!(hb.next(bit_width * 2).unwrap(), 0b00100010);
        // The common prefix should be from here to somewhere inside `key[3]`
        let ext = Extension::longest_common_prefix(&mut hb, bit_width, &[key2, key3]).unwrap();
        // The first 4 bits of `key[3]` match, but we take `bit_width` at a time, and that stops at the 3rd bit.
        assert_eq!(ext.length, 2 + 8 + 8 + 3);
        assert_eq!(ext.path.len(), 3);
        assert_eq!(ext.path[0], 0b00100100);
        assert_eq!(ext.path[1], 0b10101010);
        assert_eq!(ext.path[2], 0b10110000);
        let total_consumed = 2 * bit_width + ext.length as u32;
        assert_eq!(hb.consumed, total_consumed);

        let mut hb = HashBits::new_at_index(&key, 2 * bit_width);
        assert_eq!(ext.longest_match(&mut hb, bit_width).unwrap(), ext.length);
        assert_eq!(hb.consumed, total_consumed);
        // Shouldn't work a second time.
        assert_eq!(ext.longest_match(&mut hb, bit_width).unwrap(), 0);
        assert_eq!(hb.consumed, total_consumed);
    }

    #[test]
    fn test_split() {
        let mut key: HashedKey<32> = Default::default();
        key[0] = 0b10001000;
        key[1] = 0b10010010;
        key[2] = 0b10101010;
        key[3] = 0b11011011;
        key[4] = 0b11101110;

        let bit_width = 3;
        let mut hb = HashBits::new(&key);
        hb.next(bit_width).unwrap();

        let ext = Extension::from_bits(&mut hb, 253).unwrap();
        assert_eq!(ext.length, 253);
        assert_eq!(ext.path[0], 0b01000100);

        let (head, midx, tail) = ext.split(20, bit_width).unwrap();

        assert_eq!(head.length, 20);
        assert_eq!(head.path[0], 0b01000100);
        assert_eq!(head.path[1], 0b10010101);
        assert_eq!(head.path[2], 0b01010000);

        assert_eq!(midx.length, 3);
        assert_eq!(midx.path[0], 0b01100000);

        assert_eq!(tail.length, 230);
        assert_eq!(tail.path[0], 0b01101111);
        assert_eq!(tail.path[1], 0b10111000);

        let ext2 = Extension::unsplit(&head, &midx, &tail).unwrap();
        assert_eq!(ext, ext2);
    }
}
