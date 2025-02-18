pub use fvm::externs::Chain;
pub use fvm::externs::Consensus;
pub use fvm::externs::ConsensusFault;
pub use fvm::externs::Externs;
pub use fvm_shared::address::Address;
pub use fvm_shared::consensus::ConsensusFaultType;

// #[cfg(test)]
pub mod externs_tests {
    use super::*;
    // use fvm::externs::Chain;
    // use fvm::externs::Externs;
    // use fvm::externs::Consensus;
    // use fvm::externs::ConsensusFault;

    use cid::Cid;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::{CborStore, DAG_CBOR};
    use fvm_shared::state::StateTreeVersion;
    use multihash_codetable::Code;

    use fvm::call_manager::DefaultCallManager;
    use fvm::engine::EnginePool;
    use fvm::executor;
    //use fvm::externs::{Chain, Consensus, Rand};
    use fvm::kernel::filecoin::DefaultFilecoinKernel;
    use fvm::machine::{DefaultMachine, Machine, Manifest, NetworkConfig};
    use fvm::state_tree::StateTree;
    use num_traits::Zero;

    // #[test]
    pub fn verify_consensus_fault_test<C, F, G>(mock_consensus: F, mock_block: G)
    where
        C: Consensus,
        F: FnOnce(i64) -> Box<dyn Consensus> + std::marker::Copy,
        G: Fn(u64, i64, u64, u64, Vec<Cid>) -> (Vec<u8>, Cid) + std::marker::Copy,
    {
        let same_blocks_should_fail = || {
            let consensus = mock_consensus(0);

            let h = b"block_header";
            let extra = b"extra_data";

            let result = consensus.verify_consensus_fault(h, h, extra);

            assert!(result.is_err());
        };

        let same_blocks_cid_should_fail = || {
            let consensus = mock_consensus(0);

            let (h1, _) = mock_block(0, 0, 0, 0, vec![]);
            let (h2, _) = mock_block(0, 0, 0, 0, vec![]); // we should have a way to verify block signature
            let extra = b"extra_data";

            let result = consensus.verify_consensus_fault(&h1, &h2, extra);

            assert!(result.is_err());
        };

        let blocks_mined_by_different_miner_should_fail = || {
            let consensus = mock_consensus(0);

            let bh_1_miner_address = 0;
            let bh_2_miner_address = 1;
            let (h1, _) = mock_block(bh_1_miner_address, 0, 0, 0, vec![]);
            let (h2, _) = mock_block(bh_2_miner_address, 0, 0, 0, vec![]);
            let extra = b"extra_data";

            let result = consensus.verify_consensus_fault(&h1, &h2, extra);

            assert!(result.is_err());
        };

        let block_2_mined_at_an_earlier_method_should_fail = || {
            let consensus = mock_consensus(0);

            let bh_1_epoch = 1;
            let bh_2_epoch = 0;
            let (h1, _) = mock_block(0, bh_1_epoch, 0, 0, vec![]);
            let (h2, _) = mock_block(0, bh_2_epoch, 0, 0, vec![]);
            let extra = b"extra_data";

            let result = consensus.verify_consensus_fault(&h1, &h2, extra);

            assert!(result.is_err());
        };

        let double_fork_mining = || {
            let consensus = mock_consensus(0);

            let bh_1_miner_address = 1;
            let bh_2_epoch = 0;
            let bh_1_weigth = 0;
            let bh_2_weigth = 1;

            let (bh_1, _) = mock_block(bh_1_miner_address, bh_2_epoch, bh_1_weigth, 0, vec![]);
            let (bh_2, _) = mock_block(bh_1_miner_address, bh_2_epoch, bh_2_weigth, 0, vec![]);
            let empty_extra = b""; // we need to test with non-empty extra but no parent grinding

            let result = consensus.verify_consensus_fault(&bh_1, &bh_2, empty_extra);

            let (consensus_fault, gas) = result.unwrap();

            assert_eq!(
                consensus_fault.unwrap(),
                ConsensusFault {
                    target: Address::new_id(bh_1_miner_address),
                    epoch: bh_2_epoch,
                    fault_type: ConsensusFaultType::DoubleForkMining,
                }
            );

            assert!(gas.is_zero());
        };

        let parent_griding = || {
            let consensus = mock_consensus(0);

            let bh_1_miner_address = 1;
            let bh_2_epoch = 0;
            let bh_1_weigth = 0;
            let bh_2_weigth = 1;

            let (bh_1, _cid_1) = mock_block(bh_1_miner_address, bh_2_epoch, bh_1_weigth, 0, vec![]);
            let (bh_3, cid_3) = mock_block(bh_1_miner_address, bh_2_epoch, bh_2_weigth, 0, vec![]);
            let (bh_2, _cid_2) =
                mock_block(bh_1_miner_address, bh_2_epoch, bh_2_weigth, 0, vec![cid_3]);

            let result = consensus.verify_consensus_fault(&bh_1, &bh_2, &bh_3);

            let (consensus_fault, gas) = result.unwrap();

            assert_eq!(
                consensus_fault.unwrap(),
                ConsensusFault {
                    target: Address::new_id(bh_1_miner_address),
                    epoch: bh_2_epoch,
                    fault_type: ConsensusFaultType::ParentGrinding,
                }
            );

            assert!(gas.is_zero());
        };

        let fault_type_is_none = || {
            let consensus = mock_consensus(0);

            let bh_1_miner_address = 1;
            let bh_1_epoch = 0;
            let bh_2_epoch = 1;
            let bh_1_weigth = 0;
            let bh_2_weigth = 1;

            let (bh_1, _) = mock_block(bh_1_miner_address, bh_1_epoch, bh_1_weigth, 0, vec![]);
            let (bh_2, _) = mock_block(
                bh_1_miner_address,
                bh_2_epoch,
                bh_2_weigth,
                0,
                vec![Cid::default()],
            );
            let empty_extra = b"";

            let result = consensus.verify_consensus_fault(&bh_1, &bh_2, empty_extra);

            let (consensus_fault, gas) = result.unwrap();

            assert!(consensus_fault.is_none(),);

            assert!(gas.is_zero());
        };

        same_blocks_should_fail();
        same_blocks_cid_should_fail();
        blocks_mined_by_different_miner_should_fail();
        block_2_mined_at_an_earlier_method_should_fail();

        double_fork_mining();
        parent_griding();

        fault_type_is_none();
    }

    // #[test]
    pub fn get_tipset_cid_test<C>()
    where
        C: Chain,
    {
        // TODO(elmattic): add all possible asserts here

        assert!(true);
    }
}
