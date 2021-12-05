// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;

/// Result of a state transition from a message
#[derive(Debug, PartialEq, Clone, Serialize_tuple, Deserialize_tuple)]
pub struct Receipt {
    pub exit_code: ExitCode,
    pub return_data: RawBytes,
    pub gas_used: i64,
}
