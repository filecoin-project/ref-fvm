//! This module contains the logic to invoke the node by traversing Boundary A.

use crate::state_tree::StateTree;
use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::econ::TokenAmount;
use std::error::Error;

pub trait NodeInvoker<B: Blockstore>:
    Rand + CircSupplyCalc + Blockstore + LookbackStateGetter<B>
{
}

/// Allows generation of the current circulating supply
/// given some context.
pub trait CircSupplyCalc {
    /// Retrieves total circulating supply on the network.
    fn get_supply<B: Blockstore>(
        &self,
        height: ChainEpoch,
        state_tree: &StateTree<B>,
    ) -> anyhow::Result<TokenAmount>;
}

/// Trait to allow VM to retrieve state at an old epoch.
pub trait LookbackStateGetter<'db, B> {
    /// Returns a state tree from the given epoch.
    fn state_lookback(&self, epoch: ChainEpoch) -> anyhow::Result<StateTree<'db, B>>;
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

struct CgoNodeInvoker {
    // TODO implement
}
