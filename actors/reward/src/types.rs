// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::{address::Address, bigint::bigint_ser, econ::TokenAmount, encoding::tuple::*};

#[derive(Clone, Debug, PartialEq, Serialize_tuple, Deserialize_tuple)]
pub struct AwardBlockRewardParams {
    pub miner: Address,
    #[serde(with = "bigint_ser")]
    pub penalty: TokenAmount,
    #[serde(with = "bigint_ser")]
    pub gas_reward: TokenAmount,
    pub win_count: i64,
}

pub use fvm_shared::reward::ThisEpochRewardReturn;
