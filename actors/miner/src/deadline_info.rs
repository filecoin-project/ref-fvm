// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::deadlines::{QuantSpec};

/// Constant defining the [QuantSpec] which performs no quantization.
pub const NO_QUANTIZATION: QuantSpec = QuantSpec { unit: 1, offset: 0 };
