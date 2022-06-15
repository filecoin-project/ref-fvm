use super::*;

mod ipld {

    use cid::Cid;
    use fvm::kernel::IpldBlockOps;
    use fvm::machine::Machine;
    use fvm_ipld_blockstore::Blockstore;
    use fvm_ipld_encoding::DAG_CBOR;
    use multihash::MultihashDigest;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        let mut buf = [0u8; 3];
        // roundtrip
        let id = kern.block_create(DAG_CBOR, block)?;
        let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;
        let stat = kern.block_stat(id)?;
        let (opened_id, opened_stat) = kern.block_open(&cid)?;
        let remaining = kern.block_read(id, 0, &mut buf)?;

        let (call_manager, _) = kern.into_inner();
        let test_data = call_manager.test_data.borrow();

        // Create
        assert_eq!(id, 1, "block creation should be ID 1");

        // Link
        let expected_cid = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(block));
        assert_eq!(cid, expected_cid, "CID that came from block_link does not match expected CID: Blake2b256 hash, 32 bytes long, DAG CBOR codec");

        // Stat
        assert_eq!(stat.codec, opened_stat.codec);
        assert_eq!(stat.size, opened_stat.size);

        // Open
        assert_eq!(opened_id, 2, "block open should be ID 2");

        // Read
        assert_eq!(
            remaining, 0,
            "logical buffer should've been exactly the same size as the block"
        );
        assert_eq!(
            &buf, block,
            "data read after roundrip does not match inital data"
        );

        // assert gas charge calls
        assert_eq!(
            test_data.charge_gas_calls,
            // open 2 (load charge and per-byte charge)
            // link 1
            // stat 1
            // create 1
            // read 1
            6,
            "total number of operations that charge gas should be 6"
        );
        Ok(())
    }

    #[test]
    fn create() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        let block_1 = "bar".as_bytes();
        let block_2 = "baz".as_bytes();

        // create blocks
        let id = kern.block_create(DAG_CBOR, block)?;
        let id1 = kern1.block_create(DAG_CBOR, block_1)?;

        assert_eq!(id, 1, "first block id should be 1");
        assert_eq!(
            id, id1,
            "two blocks of the different content but same order should have the same block id"
        );

        let id = kern1.block_create(DAG_CBOR, block_2)?;
        assert_eq!(id, 2, "second created block id should be 2");

        let (call_manager, _) = kern.into_inner();

        // assert gas
        {
            assert_eq!(
                call_manager.test_data.borrow().charge_gas_calls,
                1,
                "charge_gas should be called exactly once per block_create"
            );

            let expected_create_price = call_manager
                .machine
                .context()
                .price_list
                .on_block_create(block.len() as usize)
                .total();
            assert_eq!(
                call_manager.gas_tracker.gas_used(),
                expected_create_price,
                "gas use creating a block does not match price list"
            );
        }

        Ok(())
    }

    #[test]
    fn create_unexpected() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        expect_syscall_err!(IllegalCodec, kern.block_create(0xFF, block));

        // valid for M1, shouldn't be for M2
        let _ = kern.block_create(DAG_CBOR, &[])?;

        // spec audit things arent (yet) tested
        Ok(())
    }

    #[test]
    fn link() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        let other_block = "baz".as_bytes();

        // link a block
        let id = kern.block_create(DAG_CBOR, block)?;
        let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;

        // link a block of the same data inside a different kernel
        let id1 = kern1.block_create(DAG_CBOR, block)?;
        let cid1 = kern1.block_link(id1, Code::Blake2b256.into(), 32)?;

        // link a block of different data into kern1
        let other_id = kern1.block_create(DAG_CBOR, other_block)?;
        let other_cid = kern1.block_link(other_id, Code::Blake2b256.into(), 32)?;

        let (call_manager, _) = kern.into_inner();

        // CIDs match CIDs generated manually from CID crate
        let expected_cid = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(block));
        let expected_other_cid = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(other_block));

        assert_eq!(cid, expected_cid, "CID that came from block_link and {} does not match expected CID: Blake2b256 hash, 32 bytes long, DAG CBOR codec", String::from_utf8_lossy(block));
        assert_eq!(other_cid, expected_other_cid, "CID that came from block_link and {} does not match expected CID: Blake2b256 hash, 32 bytes long, DAG CBOR codec", String::from_utf8_lossy(other_block));

        // Internal CIDs
        assert!(
            call_manager.machine.blockstore().has(&cid)?,
            "block_link was called but CID was not found in the blockstore"
        );
        assert_eq!(cid, cid1, "calling block_link in 2 different kernels of the same data and state should have the same CID");
        assert_ne!(
            cid, other_cid,
            "calling block_link with different data should make different CIDs"
        );
        // assert gas
        {
            assert_eq!(
                call_manager.test_data.borrow().charge_gas_calls - 1,
                1,
                "charge_gas should only be called exactly once per block_link"
            );

            let expected_block = Block::new(cid.codec(), block);
            let expected_create_price = call_manager
                .machine
                .context()
                .price_list
                .on_block_create(block.len() as usize)
                .total();
            let expected_link_price = call_manager
                .machine
                .context()
                .price_list
                .on_block_link(expected_block.size() as usize)
                .total();

            assert_eq!(
                call_manager.gas_tracker.gas_used(),
                expected_create_price + expected_link_price,
                "gas use creating and linking a block does not match price list"
            )
        }

        Ok(())
    }

    #[test]
    fn link_unexpected() -> anyhow::Result<()> {
        let (mut kern, test_data) = build_inspecting_test()?;

        let block = "foo".as_bytes();

        let id = kern.block_create(DAG_CBOR, block)?;
        test_data.borrow_mut().charge_gas_calls = 0;

        // Invalid hash lengths
        expect_syscall_err!(IllegalCid, kern.block_link(id, Code::Blake2b256.into(), 0));
        expect_syscall_err!(
            IllegalCid,
            kern.block_link(id, Code::Blake2b256.into(), 128)
        );

        // Invalid hash function
        expect_syscall_err!(IllegalCid, kern.block_link(id, 0xFF, 32));
        expect_syscall_err!(IllegalCid, kern.block_link(id, 0xFF, 0));

        // Invalid BlockId
        expect_syscall_err!(
            InvalidHandle,
            kern.block_link(123456, Code::Blake2b256.into(), 32)
        );
        expect_syscall_err!(
            InvalidHandle,
            kern.block_link(0, Code::Blake2b256.into(), 32)
        );

        Ok(())
    }

    #[test]
    fn read() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        let other_block = "baz".as_bytes();
        let long_block = "hello world!".as_bytes();

        // add block
        let id = kern.block_create(DAG_CBOR, block)?;

        // add a block of the same data inside a different kernel
        let id1 = kern1.block_create(DAG_CBOR, block)?;

        // add a block of different data inside a different kernel
        let other_id = kern1.block_create(DAG_CBOR, other_block)?;
        let long_id = kern1.block_create(DAG_CBOR, long_block)?;

        // setup buffers
        let mut block_buf = [0u8; 32];
        let mut block1_buf = [0u8; 32];
        let mut other_block_buf = [0u8; 32];
        let mut long_block_buf = [0u8; 6];
        let mut remaining_buf = [0u8; 6];

        // read data
        let buf_i = kern.block_read(id, 0, &mut block_buf)?;
        let buf1_i = kern1.block_read(id1, 0, &mut block1_buf)?;

        assert_eq!(buf_i, 3 - block_buf.len() as i32, "input buffer is larger than bytes to be read, expected value here must be exactly block.len()-buf.len()");
        assert_eq!(
            buf_i, buf1_i,
            "remaining offsets from 2 different kernels of same data are mismatched"
        );
        assert_eq!(
            block_buf, block1_buf,
            "bytes read from 2 different kernels of same data are not equal"
        );

        // read 'other' data
        kern1.block_read(other_id, 0, &mut other_block_buf)?;

        assert_eq!(
            &block_buf[..block.len()],
            block,
            "bytes read from block does not match bytes put in"
        );
        assert_ne!(
            block_buf, other_block_buf,
            "bytes read from 2 different kernels of different ids are equal when they shouldn't be"
        );
        assert_ne!(
            block1_buf, other_block_buf,
            "bytes read from the same kernel of different ids are equal when they shouldn't be"
        );

        // partial read
        let partial_offset = kern1.block_read(long_id, 0, &mut long_block_buf)?;

        // remaining
        assert_eq!(
            partial_offset, 6,
            "6 bytes should be following after reading 6 bytes from a block 12 bytes long"
        );

        // read remaining
        let remaining_offset = kern1.block_read(
            long_id,
            long_block.len() as u32 - partial_offset as u32,
            &mut remaining_buf,
        )?;
        assert_eq!(
            &remaining_buf,
            "world!".as_bytes(),
            "bytes read from block with offset is unexpected"
        );
        assert_eq!(remaining_offset, 0, "number of bytes read here should be exactly the same as the number of bytes in the block");

        // read with non-empty buffer
        {
            let mut garbage_buf = [0xFFu8; 32];
            kern1.block_read(long_id, 0, &mut garbage_buf)?;
            assert_eq!(
                &garbage_buf[..long_block.len()],
                long_block,
                "non-empty input buffer affects bytes read"
            )
        }

        let (call_manager, _) = kern.into_inner();

        // assert gas
        {
            let price_list = call_manager.machine.context().price_list;
            let expected_create_price = price_list.on_block_create(block.len() as usize).total();
            let expected_read_price = price_list.on_block_read(block.len() as usize).total();

            assert_eq!(
                call_manager.test_data.borrow().charge_gas_calls - 1,
                1,
                "charge_gas should be called exactly once in block_read"
            );
            assert_eq!(
                call_manager.gas_tracker.gas_used(),
                expected_create_price + expected_read_price,
                "gas use of creating and reading a block does not match price list"
            )
        }

        Ok(())
    }

    #[test]
    fn read_unexpected() -> anyhow::Result<()> {
        let (mut kern, test_data) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        let buf = &mut [0u8; 3];

        // read before creation
        expect_syscall_err!(InvalidHandle, kern.block_read(1, 0, buf));

        // create block
        let id = kern.block_create(DAG_CBOR, block)?;
        test_data.borrow_mut().charge_gas_calls = 0;

        // ID
        expect_syscall_err!(InvalidHandle, kern.block_read(0, 0, buf));
        expect_syscall_err!(InvalidHandle, kern.block_read(0xFF, 0, buf));

        // Offset
        let buf = &mut [0u8; 258];

        // offset is larger than total bytes in block
        let diff = kern.block_read(id, 255, &mut buf[255..])?;
        assert_eq!(diff, -255);
        let end = (buf.len() as i32 + diff) as usize;
        let _ = &buf[..end];

        Ok(())
    }

    #[test]
    fn stat() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();

        let id = kern.block_create(DAG_CBOR, block)?;

        let stat = kern.block_stat(id)?;

        assert_eq!(stat.codec, DAG_CBOR);
        assert_eq!(stat.size, 3);

        let (call_manager, _) = kern.into_inner();

        // assert gas
        {
            let price_list = call_manager.machine.context().price_list;
            let expected_create_price = price_list.on_block_create(block.len() as usize).total();
            let expected_stat_price = price_list.on_block_stat().total();

            assert_eq!(
                call_manager.test_data.borrow().charge_gas_calls - 1,
                1,
                "charge_gas should be called exactly once in block_stat"
            );
            assert_eq!(
                call_manager.gas_tracker.gas_used(),
                expected_create_price + expected_stat_price,
                "gas use of creating and 'stat'ing a block does not match price list"
            )
        }
        Ok(())
    }

    #[test]
    fn stat_unexpected() -> anyhow::Result<()> {
        let (mut kern, test_data) = build_inspecting_test()?;

        let block = "foo".as_bytes();

        expect_syscall_err!(InvalidHandle, kern.block_stat(1));

        kern.block_create(DAG_CBOR, block)?;
        // reset gas calls
        test_data.borrow_mut().charge_gas_calls = 0;

        expect_syscall_err!(InvalidHandle, kern.block_stat(0));
        expect_syscall_err!(InvalidHandle, kern.block_stat(0xFF));

        Ok(())
    }
}

mod gas {
    use fvm::gas::*;
    use fvm::kernel::GasOps;
    use fvm_shared::version::NetworkVersion;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    #[test]
    fn test() -> anyhow::Result<()> {
        let avaliable = Gas::new(10);
        let gas_tracker = GasTracker::new(avaliable, Gas::new(0));

        let (mut kern, _) = build_inspecting_gas_test(gas_tracker)?;

        assert_eq!(kern.gas_available(), avaliable);
        assert_eq!(kern.gas_used(), Gas::new(0));

        kern.charge_gas("charge 6 gas", Gas::new(6))?;

        assert_eq!(kern.gas_available(), Gas::new(4));
        assert_eq!(kern.gas_used(), Gas::new(6));

        kern.charge_gas("refund 6 gas", Gas::new(-6))?;

        assert_eq!(kern.gas_available(), avaliable);
        assert_eq!(kern.gas_used(), Gas::new(0));
        Ok(())
    }

    #[test]
    fn used() -> anyhow::Result<()> {
        let used = Gas::new(123456);
        let gas_tracker = GasTracker::new(Gas::new(i64::MAX), used);

        let (kern, _) = build_inspecting_gas_test(gas_tracker)?;

        assert_eq!(kern.gas_used(), used);

        Ok(())
    }

    #[test]
    fn available() -> anyhow::Result<()> {
        let avaliable = Gas::new(123456);
        let gas_tracker = GasTracker::new(avaliable, Gas::new(0));

        let (kern, _) = build_inspecting_gas_test(gas_tracker)?;

        assert_eq!(kern.gas_available(), avaliable);

        Ok(())
    }

    #[test]
    fn charge() -> anyhow::Result<()> {
        let test_gas = Gas::new(123456);
        let neg_test_gas = Gas::new(-123456);
        let gas_tracker = GasTracker::new(test_gas, Gas::new(0));

        let (mut kern, _) = build_inspecting_gas_test(gas_tracker)?;

        // charge exactly as much as avaliable
        kern.charge_gas("test test 123", test_gas)?;
        assert_eq!(kern.gas_used(), test_gas);

        // charge over by 1
        expect_out_of_gas!(kern.charge_gas("spend more!", Gas::new(1)));

        assert_eq!(
            kern.gas_used(),
            test_gas,
            "charging gas over what is avaliable and failing should not affect gas used"
        );

        // charge negative (refund) gas
        kern.charge_gas("refund~", neg_test_gas)?;
        assert_eq!(kern.gas_used(), Gas::new(0));
        kern.charge_gas("free gas!", neg_test_gas)?;

        assert_eq!(
            kern.gas_used(),
            neg_test_gas,
            "gas avaliable should be negative"
        );
        assert_eq!(
            kern.gas_available() + kern.gas_used(),
            test_gas,
            "gas avaliable + gas used should be equal to the gas limit"
        );

        // kernel with 0 avaliable gas
        let gas_tracker = GasTracker::new(Gas::new(0), Gas::new(0));
        let (mut kern, _) = build_inspecting_gas_test(gas_tracker)?;
        expect_out_of_gas!(kern.charge_gas("spend more!", test_gas));

        Ok(())
    }

    #[test]
    fn price_list() -> anyhow::Result<()> {
        let (kern, _) = build_inspecting_test()?;

        let expected_list = price_list_by_network_version(STUB_NETWORK_VER);
        assert_eq!(
            kern.price_list(),
            expected_list,
            "price list should be the same as the one used in the kernel {}",
            STUB_NETWORK_VER
        );

        let unexpected_list = price_list_by_network_version(NetworkVersion::V16);
        assert_ne!(kern.price_list(), unexpected_list);

        Ok(())
    }
}
