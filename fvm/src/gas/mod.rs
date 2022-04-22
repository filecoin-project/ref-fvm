// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_network_version, PriceList};
use crate::kernel::{ExecutionError, Result};

mod charge;
mod outputs;
mod price_list;

pub struct GasTracker {
    gas_limit: i64,
    gas_used: i64,
}

impl GasTracker {
    pub fn new(gas_limit: i64, gas_used: i64) -> Self {
        Self {
            gas_limit,
            gas_used,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        let to_use = charge.total();
        match self.gas_used.checked_add(to_use) {
            None => {
                log::trace!("gas overflow: {}", charge.name);
                self.gas_used = self.gas_limit;
                Err(ExecutionError::OutOfGas)
            }
            Some(used) => {
                log::trace!("charged {} gas: {}", to_use, charge.name);
                if used > self.gas_limit {
                    log::trace!("out of gas: {}", charge.name);
                    self.gas_used = self.gas_limit;
                    Err(ExecutionError::OutOfGas)
                } else {
                    self.gas_used = used;
                    Ok(())
                }
            }
        }
    }

    /// returns unused gas
    pub fn get_gas(&self) -> i64 {
        // todo block charges to make sure we account properly (maybe debug mode only)

        self.gas_limit - self.gas_used
    }

    /// sets new unused gas, creating a new gas charge if needed
    pub fn set_available_gas(&mut self, _name: &str, new_avail_gas: i64) -> Result<()> {
        self.gas_used = self.gas_limit - new_avail_gas;
        // todo use charge_gas
        Ok(())
    }

    /// Getter for gas available.
    pub fn gas_limit(&self) -> i64 {
        self.gas_limit
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> i64 {
        self.gas_used
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
