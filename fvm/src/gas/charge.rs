// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Gas;

/// Single gas charge in the VM. Contains information about what gas was for, as well
/// as the amount of gas needed for computation and storage respectively.
pub struct GasCharge<'a> {
    pub name: &'a str,
    /// Compute costs
    pub compute_gas: Gas,
    /// Storage costs
    pub storage_gas: Gas,
}

impl<'a> GasCharge<'a> {
    pub fn new(name: &'a str, compute_gas: Gas, storage_gas: Gas) -> Self {
        Self {
            name,
            compute_gas,
            storage_gas,
        }
    }

    /// Calculates total gas charge (in milligas) by summing compute and
    /// storage gas associated with this charge.
    pub fn total(&self) -> Gas {
        self.compute_gas + self.storage_gas
    }
}
