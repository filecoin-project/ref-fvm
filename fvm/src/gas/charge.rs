// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Cow;
use std::time::Duration;

use super::Gas;

/// Single gas charge in the VM. Contains information about what gas was for, as well
/// as the amount of gas needed for computation and storage respectively.
#[derive(Clone, Debug)]
pub struct GasCharge {
    pub name: Cow<'static, str>,
    /// Compute costs
    pub compute_gas: Gas,
    /// Storage costs
    pub storage_gas: Gas,
    /// Execution time related to this charge, if measured.
    pub elapsed: Option<Duration>,
}

impl GasCharge {
    pub fn new(name: impl Into<Cow<'static, str>>, compute_gas: Gas, storage_gas: Gas) -> Self {
        let name = name.into();
        Self {
            name,
            compute_gas,
            storage_gas,
            elapsed: None,
        }
    }

    /// Calculates total gas charge (in milligas) by summing compute and
    /// storage gas associated with this charge.
    pub fn total(&self) -> Gas {
        self.compute_gas + self.storage_gas
    }
}
