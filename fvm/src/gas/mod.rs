// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_network_version, PriceList};
use crate::kernel::{ExecutionError, Result};

mod charge;
mod outputs;
mod price_list;

pub const MILLIGAS_PRECISION: i64 = 1000;

pub struct GasTracker {
    milligas_limit: i64,
    milligas_used: i64,

    own_limit: bool,
}

impl GasTracker {
    pub fn new(gas_limit: i64, gas_used: i64) -> Self {
        Self {
            milligas_limit: gas_to_milligas(gas_limit),
            milligas_used: gas_to_milligas(gas_used),
            own_limit: true,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    fn charge_milligas(&mut self, name: &str, to_use: i64) -> Result<()> {
        if !self.own_limit {
            panic!("charge_gas called when gas_limit owned by execution")
        }

        match self.milligas_used.checked_add(to_use) {
            None => {
                log::trace!("gas overflow: {}", name);
                self.milligas_used = self.milligas_limit;
                Err(ExecutionError::OutOfGas)
            }
            Some(used) => {
                log::trace!("charged {} gas: {}", to_use, name);
                if used > self.milligas_limit {
                    log::trace!("out of gas: {}", name);
                    self.milligas_used = self.milligas_limit;
                    Err(ExecutionError::OutOfGas)
                } else {
                    self.milligas_used = used;
                    Ok(())
                }
            }
        }
    }

    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        self.charge_milligas(
            charge.name,
            charge.total().saturating_mul(MILLIGAS_PRECISION),
        )
    }

    /// returns available milligas; makes the gas tracker block gas charges until
    /// set_available_gas is called
    pub fn borrow_milligas(&mut self) -> Result<i64> {
        if !self.own_limit {
            return Err(ExecutionError::Fatal(anyhow::Error::msg(
                "get_gas called when gas_limit owned by execution",
            )));
        }
        self.own_limit = false;

        Ok(self.milligas_limit - self.milligas_used)
    }

    /// sets new available gas, creating a new gas charge if needed
    pub fn return_milligas(&mut self, name: &str, new_avail_mgas: i64) -> Result<()> {
        if self.own_limit {
            panic!("gastracker already owns gas_limit, charge: {}", name)
        }
        self.own_limit = true;

        let old_avail_milligas = self.milligas_limit - self.milligas_used;
        let used = old_avail_milligas - new_avail_mgas;

        if used < 0 {
            return Err(ExecutionError::Fatal(anyhow::Error::msg(
                "negative gas charge in set_available_gas",
            )));
        }

        self.charge_milligas(name, used)
    }

    /// Getter for gas available.
    pub fn gas_limit(&self) -> i64 {
        milligas_to_gas(self.milligas_limit, false)
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> i64 {
        milligas_to_gas(self.milligas_used, true)
    }
}

/// Converts the specified gas into equivalent fractional gas units
#[inline]
fn gas_to_milligas(gas: i64) -> i64 {
    gas.saturating_mul(MILLIGAS_PRECISION)
}

/// Converts the specified fractional gas units into gas units
#[inline]
fn milligas_to_gas(milligas: i64, round_up: bool) -> i64 {
    let mut div_result = milligas / MILLIGAS_PRECISION;
    if milligas > 0 && round_up && milligas % MILLIGAS_PRECISION != 0 {
        div_result = div_result.saturating_add(1);
    }
    if milligas < 0 && !round_up && milligas % MILLIGAS_PRECISION != 0 {
        div_result = div_result.saturating_sub(1);
    }
    div_result
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

    #[test]
    fn milligas_to_gas_round() {
        assert_eq!(milligas_to_gas(100, false), 0);
        assert_eq!(milligas_to_gas(100, true), 1);
        assert_eq!(milligas_to_gas(-100, false), -1);
        assert_eq!(milligas_to_gas(-100, true), 0);
    }
}
