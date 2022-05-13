use fvm::externs::{Consensus, Externs, Rand};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;

use crate::rand::ReplayingRand;
use crate::vector::Randomness;

/// The externs stub for testing. Forwards randomness requests to the randomness
/// replayer, which replays randomness stored in the vector.
pub struct TestExterns {
    rand: ReplayingRand,
}

impl TestExterns {
    /// Creates a new TestExterns from randomness contained in a vector.
    pub fn new(r: &Randomness) -> Self {
        TestExterns {
            rand: ReplayingRand::new(r.as_slice()),
        }
    }
}

impl Externs for TestExterns {}

impl Rand for TestExterns {
    fn get_chain_randomness(
        &self,
        pers: i64,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        self.rand.get_chain_randomness(pers, round, entropy)
    }

    fn get_beacon_randomness(
        &self,
        pers: i64,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        self.rand.get_beacon_randomness(pers, round, entropy)
    }
}

impl Consensus for TestExterns {
    fn verify_consensus_fault(
        &self,
        _h1: &[u8],
        _h2: &[u8],
        _extra: &[u8],
    ) -> anyhow::Result<(Option<ConsensusFault>, i64)> {
        todo!()
    }
}
