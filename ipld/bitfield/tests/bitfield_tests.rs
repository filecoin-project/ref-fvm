// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashSet;

use fvm_ipld_bitfield::{bitfield, BitField, UnvalidatedBitField};
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

fn random_indices(range: u64, seed: u64) -> Vec<u64> {
    let mut rng = XorShiftRng::seed_from_u64(seed);
    (0..range).filter(|_| rng.gen::<bool>()).collect()
}

#[test]
fn bitfield_slice() {
    let vals = random_indices(10000, 2);
    let bf = BitField::try_from_bits(vals.iter().copied()).unwrap();

    let slice = bf.slice(600, 500).unwrap();
    let out_vals: Vec<_> = slice.iter().collect();
    let expected_slice = &vals[600..1100];

    assert_eq!(out_vals[..500], expected_slice[..500]);
}

#[test]
fn bitfield_slice_small() {
    let bf: BitField = bitfield![0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1, 0, 0];
    let slice = bf.slice(1, 3).unwrap();

    assert_eq!(slice.len(), 3);
    assert_eq!(slice.iter().collect::<Vec<_>>(), &[4, 7, 9]);

    // Test all combinations
    let vals = [1, 5, 6, 7, 10, 11, 12, 15];

    let test_permutations = |start: usize, count: usize| {
        let bf = BitField::try_from_bits(vals.iter().copied()).unwrap();
        let sl = bf.slice(start as u64, count as u64).unwrap();
        let exp = &vals[start..start + count];
        let out: Vec<_> = sl.iter().collect();
        assert_eq!(out, exp);
    };

    for i in 0..vals.len() {
        for j in 0..vals.len() - i {
            test_permutations(i, j);
        }
    }
}

fn set_up_test_bitfields() -> (Vec<u64>, Vec<u64>, BitField, BitField) {
    let a = random_indices(100, 1);
    let b = random_indices(100, 2);

    let bf_a = BitField::try_from_bits(a.iter().copied()).unwrap();
    let bf_b = BitField::try_from_bits(b.iter().copied()).unwrap();

    (a, b, bf_a, bf_b)
}

#[test]
fn bitfield_union() {
    let (a, b, bf_a, bf_b) = set_up_test_bitfields();

    let mut expected: HashSet<_> = a.iter().copied().collect();
    expected.extend(b);

    let merged = &bf_a | &bf_b;
    assert_eq!(expected, merged.iter().collect());
}

#[test]
fn bitfield_intersection() {
    let (a, b, bf_a, bf_b) = set_up_test_bitfields();

    let hs_a: HashSet<_> = a.into_iter().collect();
    let hs_b: HashSet<_> = b.into_iter().collect();
    let expected: HashSet<_> = hs_a.intersection(&hs_b).copied().collect();

    let merged = &bf_a & &bf_b;
    assert_eq!(expected, merged.iter().collect());
}

#[test]
fn bitfield_difference() {
    let (a, b, bf_a, bf_b) = set_up_test_bitfields();

    let mut expected: HashSet<_> = a.into_iter().collect();
    for i in b.iter() {
        expected.remove(i);
    }

    let merged = &bf_a - &bf_b;
    assert_eq!(expected, merged.iter().collect());
}

// Ported test from go impl (specs-actors)
#[test]
fn subtract_more() {
    let have = BitField::try_from_bits(vec![5, 6, 8, 10, 11, 13, 14, 17]).unwrap();
    let s1 = &BitField::try_from_bits(vec![5, 6]).unwrap() - &have;
    let s2 = &BitField::try_from_bits(vec![8, 10]).unwrap() - &have;
    let s3 = &BitField::try_from_bits(vec![11, 13]).unwrap() - &have;
    let s4 = &BitField::try_from_bits(vec![14, 17]).unwrap() - &have;

    let u = BitField::union(&[s1, s2, s3, s4]);
    assert_eq!(u.len(), 0);
}

#[test]
fn contains_any() {
    assert!(!BitField::try_from_bits(vec![0, 4])
        .unwrap()
        .contains_any(&BitField::try_from_bits(vec![1, 3, 5]).unwrap()));

    assert!(BitField::try_from_bits(vec![0, 2, 5, 6])
        .unwrap()
        .contains_any(&BitField::try_from_bits(vec![1, 3, 5]).unwrap()));

    assert!(BitField::try_from_bits(vec![1, 2, 3])
        .unwrap()
        .contains_any(&BitField::try_from_bits(vec![1, 2, 3]).unwrap()));
}

#[test]
fn contains_all() {
    assert!(!BitField::try_from_bits(vec![0, 2, 4])
        .unwrap()
        .contains_all(&BitField::try_from_bits(vec![0, 2, 4, 5]).unwrap()));

    assert!(BitField::try_from_bits(vec![0, 2, 4, 5])
        .unwrap()
        .contains_all(&BitField::try_from_bits(vec![0, 2, 4]).unwrap()));

    assert!(BitField::try_from_bits(vec![1, 2, 3])
        .unwrap()
        .contains_all(&BitField::try_from_bits(vec![1, 2, 3]).unwrap()));
}

#[test]
fn bit_ops() {
    let a = &BitField::try_from_bits(vec![1, 2, 3]).unwrap()
        & &BitField::try_from_bits(vec![1, 3, 4]).unwrap();
    assert_eq!(a.iter().collect::<Vec<_>>(), &[1, 3]);

    let mut a = BitField::try_from_bits(vec![1, 2, 3]).unwrap();
    a &= &BitField::try_from_bits(vec![1, 3, 4]).unwrap();
    assert_eq!(a.iter().collect::<Vec<_>>(), &[1, 3]);

    let a = &BitField::try_from_bits(vec![1, 2, 3]).unwrap()
        | &BitField::try_from_bits(vec![1, 3, 4]).unwrap();
    assert_eq!(a.iter().collect::<Vec<_>>(), &[1, 2, 3, 4]);

    let mut a = BitField::try_from_bits(vec![1, 2, 3]).unwrap();
    a |= &BitField::try_from_bits(vec![1, 3, 4]).unwrap();
    assert_eq!(a.iter().collect::<Vec<_>>(), &[1, 2, 3, 4]);
}

#[test]
fn ranges() {
    let mut bit_field = bitfield![0, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0];

    assert_eq!(bit_field.ranges().count(), 4);
    bit_field.set(5);
    assert_eq!(bit_field.ranges().count(), 3);
    bit_field.unset(4);
    assert_eq!(bit_field.ranges().count(), 4);
    bit_field.unset(2);
    assert_eq!(bit_field.ranges().count(), 4);
}

#[test]
fn serialize_node_symmetric() {
    let bit_field = bitfield![0, 1, 0, 1, 1, 1, 1, 1, 1];
    let cbor_bz = fvm_ipld_encoding::to_vec(&bit_field).unwrap();
    let deserialized: BitField = fvm_ipld_encoding::from_slice(&cbor_bz).unwrap();
    assert_eq!(deserialized.len(), 7);
    assert_eq!(deserialized, bit_field);
}

#[test]
// ported test from specs-actors `bitfield_test.go` with added vector
fn bit_vec_unset_vector() {
    let mut bf = BitField::new();
    bf.set(1);
    bf.set(2);
    bf.set(3);
    bf.set(4);
    bf.set(5);

    bf.unset(3);

    assert!(!bf.get(3));
    assert_eq!(bf.len(), 4);

    // Test cbor marshal and unmarshal
    let cbor_bz = fvm_ipld_encoding::to_vec(&bf).unwrap();
    assert_eq!(&cbor_bz, &[0x42, 0xa8, 0x54]);

    let deserialized: BitField = fvm_ipld_encoding::from_slice(&cbor_bz).unwrap();
    assert_eq!(deserialized.len(), 4);
    assert!(!deserialized.get(3));
}

#[test]
fn padding() {
    // bits: 0 1 0 1
    // rle+: 0 0 0 1 1 1 1
    // when deserialized it will have an extra 0 at the end for padding,
    // which is not part of a block prefix

    let mut bf = BitField::new();
    bf.set(1);
    bf.set(3);

    let cbor = fvm_ipld_encoding::to_vec(&bf).unwrap();
    let deserialized: BitField = fvm_ipld_encoding::from_slice(&cbor).unwrap();
    assert_eq!(deserialized, bf);
}

#[test]
fn exceeds_bitfield_range() {
    let mut bf = BitField::new();
    bf.try_set(u64::MAX)
        .expect_err("expected setting u64::MAX to fail");
    bf.try_set(u64::MAX - 1)
        .expect("expected setting u64::MAX-1 to succeed");
    BitField::try_from_bits([0, 1, 4, 99, u64::MAX])
        .expect_err("expected setting u64::MAX to fail");
    BitField::try_from_bits([0, 1, 4, 99, u64::MAX - 1])
        .expect("expected setting u64::MAX-1 to succeed");
}

#[test]
fn bitfield_custom() {
    let mut bf = BitField::new();

    // Set alternating bits for worst-case size performance
    let mut i = 0;
    while i < 1_000_000 {
        bf.set(i);
        i += 2;
    }
    println!("# Set bits: {}", bf.len());

    // Standard serialization catches MAX_ENCODING_SIZE issues
    println!("Attempting to serialize...");
    match fvm_ipld_encoding::to_vec(&bf) {
        Ok(_) => panic!("This should have failed!"),
        Err(_) => println!("Standard serialization failed, as expected"),
    }

    // Bypass to_vec enc size check so we can test deserialization
    println!("Manually serializing...");
    // CBOR prefix for the bytes
    let mut cbor = vec![0x5A, 0x00, 0x01, 0xE8, 0x49];
    cbor.extend_from_slice(&bf.to_bytes());
    println!("Success!");

    println!("# bytes of cbor: {}", cbor.len());
    println!("Header: {:#010b}", cbor[0]);
    println!("-- maj type {}", (cbor[0] & 0xe0) >> 5);

    // Get size of payload size
    let info = cbor[0] & 0x1f;
    println!("-- adtl info {}", info);

    // Get payload size
    let size = match info {
        0..=23 => info as usize,
        24 => cbor[1] as usize,
        25 => u16::from_be_bytes([cbor[1], cbor[2]]) as usize,
        26 => u32::from_be_bytes([cbor[1], cbor[2], cbor[3], cbor[4]]) as usize,
        27 => u64::from_be_bytes([
            cbor[1], cbor[2], cbor[3], cbor[4], cbor[5], cbor[6], cbor[7], cbor[8],
        ]) as usize,
        _ => {
            println!("OUT OF RANGE");
            0
        }
    };

    println!("{} byte payload", size);

    // Deserialize and validate malicious payload
    println!("Attempting to deserialize and validate...");
    match fvm_ipld_encoding::from_slice::<UnvalidatedBitField>(&cbor) {
        Ok(mut bitfield) => {
            bitfield.validate_mut().unwrap();
            panic!("Error - deserialized/validated payload over 32768 bytes.");
        }
        Err(_) => {
            println!("Success - payload over 32768 bytes cannot be deserialized");
        }
    }
}
