use super::*;

mod ipld {

    use std::mem::ManuallyDrop;

    use fvm::kernel::IpldBlockOps;
    use fvm::machine::Machine;
    use fvm::Kernel;
    use fvm_ipld_blockstore::Blockstore;
    use fvm_ipld_encoding::DAG_CBOR;

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let (mut kern, refcell) = build_inspecting_test()?;

        // roundtrip
        let id = kern.block_create(DAG_CBOR, "foo".as_bytes())?;
        let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;
        let stat = kern.block_stat(id)?;
        let (opened_id, opened_stat) = kern.block_open(&cid)?;

        // ok to upgrade into strong pointer since kern can't be mutated anymore
        let arc = &mut refcell.upgrade().unwrap();
        let external_call_manager = arc.as_ref();

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
            external_call_manager.charge_gas_calls,
            // open 2 (load charge and per-byte charge)
            // link 1
            // stat 1
            // create 1
            5
        );

        // drop strong ref *before* weak ref
        drop(kern);
        // ok to drop weak ref
        drop(ManuallyDrop::into_inner(refcell));
        Ok(())
    }

    #[test]
    fn create_ids() -> anyhow::Result<()> {
        let (mut kern, refcell) = build_inspecting_test()?;

        let mut kern1 = TestingKernel::new(
            DummyCallManager::new_stub(),
            BlockRegistry::default(),
            0,
            0,
            0,
            0.into(),
        );

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

        // prevent new mutations
        let _ = &kern;
        // ok to upgrade into strong pointer since kern can't be mutated anymore
        let arc = &mut refcell.upgrade().unwrap();
        let external_call_manager = arc.as_ref();

        {
            assert_eq!(
                external_call_manager.charge_gas_calls, 1,
                "charge_gas should called exactly once per block_create"
            );

            let expected_create_price = external_call_manager
                .machine
                .context()
                .price_list
                .on_block_create(block.len() as usize)
                .total();
            assert_eq!(
                external_call_manager.gas_tracker.gas_used(),
                expected_create_price
            );
        }

        Ok(())
    }
    #[test]
    fn link() -> anyhow::Result<()> {
        let (mut kern, refcell) = build_inspecting_test()?;
        let (mut kern1, refcell1) = build_inspecting_test()?;

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
        let arc = &mut refcell.upgrade().unwrap();
        let external_call_manager = arc.as_ref();
        // prevent new mutations
        let kern1 = kern1;
        // ok to upgrade into strong pointer since kern can't be mutated anymore
        let arc1 = &mut refcell1.upgrade().unwrap();
        let _external_call_manager1 = arc1.as_ref();

        // drop strong ref inside kern *before* weak ref
        drop(kern);
        drop(kern1);

        // assert

        assert!(
            external_call_manager.machine.blockstore().has(&cid)?,
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
                external_call_manager.charge_gas_calls - 1,
                1,
                "charge_gas should only be called exactly once per block_link"
            );

            let expected_block = Block::new(cid.codec(), block);
            let expected_create_price = external_call_manager
                .machine
                .context()
                .price_list
                .on_block_create(block.len() as usize)
                .total();
            let expected_link_price = external_call_manager
                .machine
                .context()
                .price_list
                .on_block_link(expected_block.size() as usize)
                .total();

            assert_eq!(
                external_call_manager.gas_tracker.gas_used(),
                expected_create_price + expected_link_price,
                "cost of creating & linking does not match price list"
            )
        }

        // ok to drop weak ref
        drop(ManuallyDrop::into_inner(refcell));
        drop(ManuallyDrop::into_inner(refcell1));

        Ok(())
    }
}
