// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, Cbor};

use crate::error::ExitCode;

/// Result of a state transition from a message
#[derive(Debug, PartialEq, Eq, Clone, Serialize_tuple, Deserialize_tuple)]
pub struct Receipt {
    pub exit_code: ExitCode,
    #[serde(with = "strict_bytes")]
    pub return_data: Vec<u8>,
    pub gas_used: i64,
}

impl Cbor for Receipt {}
