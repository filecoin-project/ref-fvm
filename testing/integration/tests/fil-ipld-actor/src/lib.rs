use fvm_ipld_encoding::{to_vec, BytesSer, DAG_CBOR};
use fvm_sdk as sdk;
use fvm_shared::error::ExitCode;

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    std::panic::set_hook(Box::new(|info| {
        sdk::vm::abort(
            ExitCode::USR_ASSERTION_FAILED.value(),
            Some(&format!("{}", info)),
        )
    }));

    test_read_block();

    #[cfg(coverage)]
    sdk::debug::store_artifact("ipld_actor.profraw", minicov::capture_coverage());
    0
}

fn test_read_block() {
    let test_bytes: Vec<u8> = to_vec(&BytesSer(
        &(0..(10 << 10))
            .map(|b| (b % 256) as u8)
            .collect::<Vec<u8>>(),
    ))
    .unwrap();
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
        sdk::sys::ipld::block_read(id, (i32::MAX as u32) + 1, buf.as_mut_ptr(), 0)
            .expect_err("expected it to fail");

        // Test a length that overflows an i32
        sdk::sys::ipld::block_read(id, 0, buf.as_mut_ptr(), (i32::MAX as u32) + 1)
            .expect_err("expected it to fail");

        // Test a combined length + offset that overflow an i32
        sdk::sys::ipld::block_read(id, (i32::MAX as u32) - 10, buf.as_mut_ptr(), 20)
            .expect_err("expected it to fail");
    }
}
