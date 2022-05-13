// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm::externs::Rand;
use fvm_shared::clock::ChainEpoch;

use crate::vector::{RandomnessKind, RandomnessMatch, RandomnessRule};

/// Takes recorded randomness and replays it when input parameters match.
/// When there's no match, it falls back to TestFallbackRand, which returns a
/// fixed output.
pub struct ReplayingRand {
    pub recorded: Vec<RandomnessMatch>,
    pub fallback: TestFallbackRand,
}

/// Implements the Rand extern and returns static values as randomness outputs
/// when there's a vector miss.
pub struct TestFallbackRand;

impl Rand for TestFallbackRand {
    fn get_chain_randomness(&self, _: i64, _: ChainEpoch, _: &[u8]) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }

    fn get_beacon_randomness(&self, _: i64, _: ChainEpoch, _: &[u8]) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }
}

impl ReplayingRand {
    pub fn new(recorded: &[RandomnessMatch]) -> Self {
        Self {
            recorded: Vec::from(recorded), // TODO this copies, maybe optimize
            fallback: TestFallbackRand,
        }
    }

    pub fn matches(&self, requested: RandomnessRule) -> Option<[u8; 32]> {
        for other in &self.recorded {
            if other.on == requested {
                let mut randomness = [0u8; 32];
                randomness.copy_from_slice(&other.ret);
                return Some(randomness);
            }
        }
        None
    }
}

impl Rand for ReplayingRand {
    fn get_chain_randomness(
        &self,
        dst: i64,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Chain,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_chain_randomness(dst, epoch, entropy)
        }
    }
    fn get_beacon_randomness(
        &self,
        dst: i64,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Beacon,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_beacon_randomness(dst, epoch, entropy)
        }
    }
}
