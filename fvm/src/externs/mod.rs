// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! This module contains the logic to invoke the node by traversing Boundary A.

use cid::Cid;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;

pub trait Externs: Rand + Consensus + Chain {}

/// Consensus related methods.
pub trait Consensus {
    /// Verify a consensus fault.
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<(Option<ConsensusFault>, i64)>;
}

/// Randomness provider trait
pub trait Rand {
    /// Gets 32 bytes of randomness for ChainRand paramaterized by the DomainSeparationTag,
    /// ChainEpoch, Entropy from the ticket chain.
    fn get_chain_randomness(
        &self,
        pers: i64,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;

    /// Gets 32 bytes of randomness for ChainRand paramaterized by the DomainSeparationTag,
    /// ChainEpoch, Entropy from the latest beacon entry.
    fn get_beacon_randomness(
        &self,
        pers: i64,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]>;
}

/// Chain information provider.
pub trait Chain {
    /// Gets the CID for a given tipset.
    fn get_tipset_cid(&self, epoch: ChainEpoch) -> anyhow::Result<Cid>;
}
