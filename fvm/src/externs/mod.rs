//! This module contains the logic to invoke the node by traversing Boundary A.

mod cgo;

use blockstore::Blockstore;
use fvm_shared::{
    clock::ChainEpoch, consensus::ConsensusFault, crypto::randomness::DomainSeparationTag,
};

pub trait Externs: Rand + Consensus + Blockstore {}

/// Consensus related methods.
pub trait Consensus {
    /// Verify a consensus fault.
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<ConsensusFault>;
}

/// Randomness provider trait
pub trait Rand {
    /// Gets 32 bytes of randomness for ChainRand paramaterized by the DomainSeparationTag,
    /// ChainEpoch, Entropy from the ticket chain.
    fn get_chain_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;

    fn get_chain_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;

    /// Gets 32 bytes of randomness for ChainRand paramaterized by the DomainSeparationTag,
    /// ChainEpoch, Entropy from the latest beacon entry.
    fn get_beacon_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;

    fn get_beacon_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;
}
