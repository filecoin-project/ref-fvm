#![allow(unused)] // TODO: remove this when we implement these
use cid::Cid;

use anyhow::Result;
use blockstore::cgo::CgoBlockstore;
use fvm_shared::{
    address::Address,
    clock::ChainEpoch,
    consensus::{ConsensusFault, ConsensusFaultType},
    crypto::randomness::DomainSeparationTag,
};

use crate::externs::{Blockstore, Consensus, Externs, Rand};

/// TODO this will be the externs implementation that delegates to a Go node
/// (e.g. Lotus) via Cgo to resolve externs.
pub struct CgoExterns {
    handle: i32,
}

impl CgoExterns {
    /// Construct a new blockstore from a handle.
    pub fn new(handle: i32) -> CgoExterns {
        CgoExterns { handle }
    }
}

impl Rand for CgoExterns {
    fn get_chain_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let res = [0u8; 32];
        Ok(res)
    }

    fn get_chain_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let res = [0u8; 32];
        Ok(res)
    }

    fn get_beacon_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let res = [0u8; 32];
        Ok(res)
    }

    fn get_beacon_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let res = [0u8; 32];
        Ok(res)
    }
}

impl Consensus for CgoExterns {
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<Option<ConsensusFault>> {
        Ok(Some(ConsensusFault {
            target: Address::new_id(0),
            epoch: 0,
            fault_type: ConsensusFaultType::DoubleForkMining,
        }))
    }
}

impl Blockstore for CgoExterns {
    type Error = blockstore::cgo::Error;
    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        let bs = CgoBlockstore::new(self.handle);
        bs.has(k)
    }
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        let bs = CgoBlockstore::new(self.handle);
        bs.get(k)
    }
    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        let bs = CgoBlockstore::new(self.handle);
        bs.put_keyed(k, block)
    }
}

impl Externs for CgoExterns {}
