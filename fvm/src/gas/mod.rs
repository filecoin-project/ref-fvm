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

    own_limit: bool,
}

impl GasTracker {
    pub fn new(gas_limit: i64, gas_used: i64) -> Self {
        Self {
            gas_limit,
            gas_used,
            own_limit: true,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        if !self.own_limit {
            panic!("charge_gas called when gas_limit owned by execution")
        }

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

    /// returns available gas; makes the gas tracker block gas charges until
    /// set_available_gas is called
    pub fn get_gas(&mut self) -> i64 {
        if !self.own_limit {
            panic!("get_gas called when gas_limit owned by execution")
        }
        self.own_limit = false;

        self.gas_limit - self.gas_used
    }

    /// sets new available gas, creating a new gas charge if needed
    pub fn set_available_gas(&mut self, name: &str, new_avail_gas: i64) -> Result<()> {
        if self.own_limit {
            panic!("gastracker already owns gas_limit, charge: {}", name)
        }
        self.own_limit = true;

        let old_avail_gas = self.gas_limit - self.gas_used;
        let used = old_avail_gas - new_avail_gas;

        if used < 0 {
            return Err(ExecutionError::Fatal(anyhow::Error::msg(
                "negative gas charge in set_available_gas",
            )));
        }

        self.charge_gas(GasCharge {
            name,
            compute_gas: used,
            storage_gas: 0,
        })
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
