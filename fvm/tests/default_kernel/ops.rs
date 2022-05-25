use super::*;

mod ipld {

    use fvm::kernel::IpldBlockOps;
    use fvm::machine::Machine;
    use fvm_ipld_blockstore::Blockstore;
    use fvm_ipld_encoding::DAG_CBOR;

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;

        // roundtrip
        let id = kern.block_create(DAG_CBOR, "foo".as_bytes())?;
        let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;
        let stat = kern.block_stat(id)?;
        let (opened_id, opened_stat) = kern.block_open(&cid)?;

        let (call_manager, _) = kern.into_inner();
        let test_data = call_manager.test_data.borrow();
        // create op should be 1
        assert_eq!(id, 1);
        // open op should be 2
        assert_eq!(opened_id - 1, id);

        // Stat
        assert_eq!(stat.codec, opened_stat.codec);
        assert_eq!(stat.codec, DAG_CBOR);
        assert_eq!(stat.size, opened_stat.size);
        assert_eq!(stat.size, 3);

        // assert gas charge calls
        assert_eq!(
            test_data.charge_gas_calls,
            // open 2 (load charge and per-byte charge)
            // link 1
            // stat 1
            // create 1
            5
        );
        Ok(())
    }

    #[test]
    fn create_ids() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        let block = "foo".as_bytes();
        // make a block
        let id = kern.block_create(DAG_CBOR, block)?;
        let id1 = kern1.block_create(DAG_CBOR, "bar".as_bytes())?;

        // TODO are these assumption correct? other ID values could be used although it would be weird
        assert_eq!(id, 1, "first block id should be 1");
        assert_eq!(
            id, id1,
            "two blocks of the different content but same order should have the same block id"
        );

        let id = kern1.block_create(DAG_CBOR, "baz".as_bytes())?;
        assert_eq!(id, 2, "second created block id should be 2");

        let (call_manager, _) = kern.into_inner();
        {
            assert_eq!(
                call_manager.test_data.borrow().charge_gas_calls,
                1,
                "charge_gas should called exactly once per block_create"
            );

            let expected_create_price = call_manager
                .machine
                .context()
                .price_list
                .on_block_create(block.len() as usize)
                .total();
            assert_eq!(call_manager.gas_tracker.gas_used(), expected_create_price);
        }

        Ok(())
    }
    #[test]
    fn link() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        // setup

        let block = "foo".as_bytes();
        let other_block = "baz".as_bytes();
        // link a block
        let id = kern.block_create(DAG_CBOR, block)?;
        let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;

        // link a block of the same data inside a different kernel
        let id1 = kern1.block_create(DAG_CBOR, block)?;
        let cid1 = kern1.block_link(id1, Code::Blake2b256.into(), 32)?;

        let other_id = kern1.block_create(DAG_CBOR, other_block)?;
        let other_cid = kern1.block_link(other_id, Code::Blake2b256.into(), 32)?;

        // ok to upgrade into strong pointer since kern can't be mutated anymore
        let (call_manager, _) = kern.into_inner();

        // drop strong ref inside kern *before* weak ref

        // assert

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
                "cost of creating & linking does not match price list"
            )
        }

        Ok(())
    }

    #[test]
    fn read() -> anyhow::Result<()> {
        let (mut kern, _) = build_inspecting_test()?;
        let (mut kern1, _) = build_inspecting_test()?;

        // setup

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

        let mut block_buf = [0u8; 32];
        let mut block1_buf = [0u8; 32];
        let mut other_block_buf = [0u8; 32];
        let mut long_block_buf = [0u8; 6];

        let buf_i = kern.block_read(id, 0, &mut block_buf)?;
        let buf1_i = kern1.block_read(id1, 0, &mut block1_buf)?;
        let long_buf_i = kern1.block_read(long_id, 0, &mut long_block_buf)? as usize;

        // TODO is bytes-remaining the right wasy of calling this?
        assert_eq!(buf_i, 0, "input buffer should be large enough to hold read data, but returned non-zero number for bytes remaining");
        assert_eq!(
            buf_i, buf1_i,
            "remaining offsets from 2 different kernels of same data are mismatched"
        );
        assert_eq!(
            block_buf, block1_buf,
            "bytes read from 2 different kernels of same data are not equal"
        );

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

        // remaining
        assert_eq!(
            long_buf_i, 6,
            "6 bytes should be following after reading 10 bytes from long block"
        );

        let mut remaining_buf = [0u8; 6];
        let remaining_offset = kern1.block_read(
            long_id,
            (long_block.len() - long_buf_i) as u32,
            &mut remaining_buf,
        )?;
        // TODO better message
        assert_eq!(
            &remaining_buf,
            "world!".as_bytes(),
            "read offset is incorrect"
        );
        assert_eq!(remaining_offset, 0);

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
                "gas price of creating and reading a block does not match price list"
            )
        }

        Ok(())
    }
}
