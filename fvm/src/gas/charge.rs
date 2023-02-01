// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Cow;

use super::timer::GasDuration;
use super::Gas;

/// Single gas charge in the VM. Contains information about what gas was for, as well
/// as the amount of gas needed for computation and storage respectively.
#[derive(Clone, Debug)]
pub struct GasCharge {
    pub name: Cow<'static, str>,
    /// Gas charged for immediate computation.
    pub compute_gas: Gas,

    /// Other gas including:
    ///
    /// 1. Storage gas.
    /// 2. Memory retention.
    /// 3. Deferred computation (e.g., flushing blocks.)
    /// 4. Extern costs.
    ///
    /// This is split into a separate field to facilitate benchmarking.
    pub other_gas: Gas,

    /// Execution time related to this charge, if traced and successfully measured.
    pub elapsed: GasDuration,
}

// Implement eq for _testing_ because equality usually isn't something anyone should care about here
// (and we ignore elapsed times).
#[cfg(feature = "testing")]
impl PartialEq for GasCharge {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.compute_gas == other.compute_gas
            && self.other_gas == other.other_gas
    }
}
#[cfg(feature = "testing")]
impl Eq for GasCharge {}

impl GasCharge {
    pub fn new(name: impl Into<Cow<'static, str>>, compute_gas: Gas, other_gas: Gas) -> Self {
        let name = name.into();
        Self {
            name,
            compute_gas,
            other_gas,
            elapsed: GasDuration::default(),
        }
    }

    /// Calculates total gas charge (in milligas) by summing compute and
    /// storage gas associated with this charge.
    pub fn total(&self) -> Gas {
        self.compute_gas + self.other_gas
    }
}
