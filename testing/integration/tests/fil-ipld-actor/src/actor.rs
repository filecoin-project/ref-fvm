// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::{to_vec, BytesSer, DAG_CBOR};
use fvm_sdk as sdk;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

fn gen_test_bytes(size: i32) -> Vec<u8> {
    to_vec(&BytesSer(
        &(0..size).map(|b| (b % 256) as u8).collect::<Vec<u8>>(),
    ))
    .unwrap()
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    test_open_block();
    test_read_block();
    test_create_block();
    test_stat_block();
    test_link_block();

    #[cfg(coverage)]
    sdk::debug::store_artifact("ipld_actor.profraw", minicov::capture_coverage());
    0
}

fn test_open_block() {
    let test_bytes = gen_test_bytes(1 << 10);

    unsafe {
        let cid = sdk::ipld::put(0xb220, 32, DAG_CBOR, &test_bytes).unwrap();

        // The cid should be valid
        let mut buf = [0u8; MAX_CID_LEN];
        cid.write_bytes(&mut buf[..])
            .expect("CID encoding should not fail");
        sdk::sys::ipld::block_open(buf.as_mut_ptr()).expect("shouldwork");

        // Test for invalid Cid
        buf.fill(0);
        let res = sdk::sys::ipld::block_open(buf.as_mut_ptr());
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));

        // Test for invalid Cid pointer
        let some_big_number: i32 = 338473423;
        let res = sdk::sys::ipld::block_open(some_big_number.to_le_bytes()[0] as *const u8);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));

        // TODO (fridrik): Test for very large cid
    }
}

fn test_read_block() {
    let test_bytes = gen_test_bytes(10 << 10);
    let k = sdk::ipld::put(0xb220, 32, DAG_CBOR, &test_bytes).unwrap();
    {
        let block = sdk::ipld::get(&k).unwrap();
        assert_eq!(test_bytes, block);
    }

    unsafe {
        // Open it.
        let k_bytes = k.to_bytes();
        let sdk::sys::ipld::IpldOpen { codec, id, size } =
            sdk::sys::ipld::block_open(k_bytes.as_ptr()).unwrap();

        assert_eq!(test_bytes.len() as u32, size, "block has an incorrect size");
        assert_eq!(codec, DAG_CBOR, "block has an incorrect codec");

        let mut buf = vec![0u8; 2 * test_bytes.len()];

        // Try reading with too much space.
        {
            let remaining =
                sdk::sys::ipld::block_read(id, 0, buf.as_mut_ptr(), buf.len() as u32).unwrap();
            assert_eq!(
                test_bytes.len() as i32,
                -remaining,
                "should have over-allocated by 2x"
            );
            assert_eq!(
                test_bytes,
                buf[..test_bytes.len()],
                "should have read entire block"
            );
        }

        buf.fill(0);

        // Try reading a slice
        {
            let remaining = sdk::sys::ipld::block_read(id, 10, buf.as_mut_ptr(), 10).unwrap();
            assert_eq!(
                remaining,
                (test_bytes.len() - (2 * 10)) as i32,
                "should have all but 20 bytes remaining"
            );

            assert_eq!(
                &test_bytes[10..20],
                &buf[..10],
                "should have read the second 10 bytes"
            );
        }

        // Try reading past the end.
        {
            let remaining =
                sdk::sys::ipld::block_read(id, test_bytes.len() as u32 + 10, buf.as_mut_ptr(), 10)
                    .unwrap();
            assert_eq!(
                remaining, -20,
                "reading past the end of the block should work"
            );
        }

        // Test get_block with no hint
        assert_eq!(
            test_bytes,
            sdk::ipld::get_block(id, None).unwrap(),
            "can read with no hint"
        );

        // Test get_block with a small hint
        assert_eq!(
            test_bytes,
            sdk::ipld::get_block(id, Some(10)).unwrap(),
            "can read with a small hint"
        );

        // Test get_block with an exact hint
        assert_eq!(
            test_bytes,
            sdk::ipld::get_block(id, Some(test_bytes.len() as u32)).unwrap(),
            "can read with the correct size"
        );

        // Test get_block with an oversized hint.
        assert_eq!(
            test_bytes,
            sdk::ipld::get_block(id, Some(test_bytes.len() as u32 + 10)).unwrap(),
            "can read with an over-estimated size"
        );

        // Test an offset that overflows an i32:
        let res = sdk::sys::ipld::block_read(id, (i32::MAX as u32) + 1, buf.as_mut_ptr(), 0);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));

        // Test a length that overflows an i32
        let res = sdk::sys::ipld::block_read(id, 0, buf.as_mut_ptr(), (i32::MAX as u32) + 1);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));

        // Test a combined length + offset that overflow an i32
        let res = sdk::sys::ipld::block_read(id, (i32::MAX as u32) - 10, buf.as_mut_ptr(), 20);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));
    }
}

fn test_create_block() {
    unsafe {
        // Test creating a block with invalid codec
        let test_bytes = gen_test_bytes(100);
        let invalid_codec_id = 191919;
        let res = sdk::sys::ipld::block_create(
            invalid_codec_id,
            test_bytes.as_ptr(),
            test_bytes.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::IllegalCodec));

        // Creating a block just within the 1Mib limit should work
        let test_bytes = gen_test_bytes((1 << 20) - 8);
        sdk::sys::ipld::block_create(DAG_CBOR, test_bytes.as_ptr(), test_bytes.len() as u32)
            .expect("should be within block limit");

        // Test creating a too large block
        let test_bytes = gen_test_bytes((1 << 20) + 8);
        let res =
            sdk::sys::ipld::block_create(DAG_CBOR, test_bytes.as_ptr(), test_bytes.len() as u32);
        assert_eq!(res, Err(ErrorNumber::LimitExceeded));
    }
}

fn test_stat_block() {
    let bytes = gen_test_bytes(10 << 10);

    unsafe {
        let block_id =
            sdk::sys::ipld::block_create(DAG_CBOR, bytes.as_ptr(), bytes.len() as u32).unwrap();

        // Test happy case
        let mut buf = [0u8; MAX_CID_LEN];
        sdk::sys::ipld::block_link(block_id, 0xb220, 32, buf.as_mut_ptr(), buf.len() as u32)
            .expect("should work");
        let fvm_shared::sys::out::ipld::IpldStat { codec, size } =
            sdk::sys::ipld::block_stat(block_id).unwrap();
        assert_eq!(codec, DAG_CBOR);
        assert_eq!(size, bytes.len() as u32);

        // Test that giving invalid block id results in InvalidHandle error
        let invalid_block_id = 1919;
        let res = sdk::sys::ipld::block_stat(invalid_block_id);
        assert_eq!(res, Err(ErrorNumber::InvalidHandle));
    }
}

fn test_link_block() {
    let bytes = gen_test_bytes(10 << 10);

    unsafe {
        let block_id =
            sdk::sys::ipld::block_create(DAG_CBOR, bytes.as_ptr(), bytes.len() as u32).unwrap();

        // Test happy case
        let mut buf = [0u8; MAX_CID_LEN];
        sdk::sys::ipld::block_link(block_id, 0xb220, 32, buf.as_mut_ptr(), buf.len() as u32)
            .expect("should work");

        // Test passing an invalid block id results in InvalidHandle error
        let invalid_block_id = 1919;
        let res = sdk::sys::ipld::block_link(
            invalid_block_id,
            0xb220,
            32,
            buf.as_mut_ptr(),
            buf.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::InvalidHandle));

        // Test that giving too small buffer results in BufferTooSmall error
        let mut short_buf = [0u8; 3];
        let res = sdk::sys::ipld::block_link(
            block_id,
            0xb220,
            32,
            short_buf.as_mut_ptr(),
            short_buf.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::BufferTooSmall));

        // Test that invalid hash function results in IllegalCid error
        let res =
            sdk::sys::ipld::block_link(block_id, 0x1919, 32, buf.as_mut_ptr(), buf.len() as u32);
        assert_eq!(res, Err(ErrorNumber::IllegalCid));

        // Test that invalid hash length results in IllegalCid error
        let res =
            sdk::sys::ipld::block_link(block_id, 0xb220, 31, buf.as_mut_ptr(), buf.len() as u32);
        assert_eq!(res, Err(ErrorNumber::IllegalCid));
    }
}
