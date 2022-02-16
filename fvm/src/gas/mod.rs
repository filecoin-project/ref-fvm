// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_network_version, PriceList};
use crate::kernel::{ExecutionError, Result};

mod charge;
mod outputs;
mod price_list;

#[cfg(feature = "tracing")]
pub mod tracer;

pub struct GasTracker {
    gas_available: i64,
    gas_used: i64,
    compute_gas_real: i64,
}

impl GasTracker {
    pub fn new(gas_available: i64, gas_used: i64) -> Self {
        Self {
            gas_available,
            gas_used,
            compute_gas_real: 0,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        let to_use = charge.total();
        match self.gas_used.checked_add(to_use) {
            None => {
                log::trace!("gas overflow: {}", charge.name);
                self.gas_used = self.gas_available;
                Err(ExecutionError::OutOfGas)
            }
            Some(used) => {
                log::trace!("charged {} gas: {}", to_use, charge.name);
                if used > self.gas_available {
                    log::trace!("out of gas: {}", charge.name);
                    self.gas_used = self.gas_available;
                    Err(ExecutionError::OutOfGas)
                } else {
                    self.gas_used = used;
                    // can't overflow if the sum doesn't overflow.
                    self.compute_gas_real += charge.compute_gas;
                    Ok(())
                }
            }
        }
    }

    /// Getter for gas available.
    pub fn gas_available(&self) -> i64 {
        self.gas_available
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> i64 {
        self.gas_used
    }

    /// Getter for the "real" compute gas. That is, the compute gas that was actually _used_, not
    /// including the storage gas and gas charged when we run out of gas.
    pub fn compute_gas_real(&self) -> i64 {
        self.compute_gas_real
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_gas_tracker() {
        let mut t = GasTracker::new(20, 10);
        t.charge_gas(GasCharge::new("", 5, 0)).unwrap();
        assert_eq!(t.gas_used(), 15);
        t.charge_gas(GasCharge::new("", 5, 0)).unwrap();
        assert_eq!(t.gas_used(), 20);
        assert!(t.charge_gas(GasCharge::new("", 1, 0)).is_err())
    }
}
