// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_network_version, PriceList, WasmGasPrices};
use crate::kernel::{ExecutionError, Result};

mod charge;
mod outputs;
mod price_list;

pub const MILLIGAS_PRECISION: i64 = 1000;

// Type aliases to disambiguate units in interfaces.
pub type Gas = i64;
pub type Milligas = i64;

pub struct GasTracker {
    milligas_limit: i64,
    milligas_used: i64,
}

impl GasTracker {
    /// Gas limit and gas used are provided in protocol units (i.e. full units).
    /// They are converted to milligas for internal canonical accounting.
    pub fn new(gas_limit: Gas, gas_used: Gas) -> Self {
        Self {
            milligas_limit: gas_to_milligas(gas_limit),
            milligas_used: gas_to_milligas(gas_used),
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_milligas(&mut self, name: &str, to_use: Milligas) -> Result<()> {
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

    /// Applies the specified gas charge, where quantities are supplied in milligas.
    pub fn apply_charge(&mut self, charge: GasCharge) -> Result<()> {
        self.charge_milligas(charge.name, charge.total())
    }

    /// Getter for gas available.
    pub fn gas_limit(&self) -> Gas {
        milligas_to_gas(self.milligas_limit, false)
    }

    /// Getter for milligas available.
    pub fn milligas_limit(&self) -> Milligas {
        self.milligas_limit
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> Gas {
        milligas_to_gas(self.milligas_used, true)
    }

    /// Getter for milligas used.
    pub fn milligas_used(&self) -> Milligas {
        self.milligas_used
    }

    pub fn gas_available(&self) -> Gas {
        milligas_to_gas(self.milligas_available(), false)
    }

    pub fn milligas_available(&self) -> Milligas {
        self.milligas_limit.saturating_sub(self.milligas_used)
    }
}

/// Converts the specified gas into equivalent fractional gas units
#[inline]
pub(crate) fn gas_to_milligas(gas: i64) -> i64 {
    gas.saturating_mul(MILLIGAS_PRECISION)
}

/// Converts the specified fractional gas units into gas units
#[inline]
pub(crate) fn milligas_to_gas(milligas: i64, round_up: bool) -> i64 {
    let mut div_result = milligas / MILLIGAS_PRECISION;
    if milligas > 0 && round_up && milligas % MILLIGAS_PRECISION != 0 {
        div_result = div_result.saturating_add(1);
    } else if milligas < 0 && !round_up && milligas % MILLIGAS_PRECISION != 0 {
        div_result = div_result.saturating_sub(1);
    }
    div_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::identity_op)]
    fn basic_gas_tracker() -> Result<()> {
        let mut t = GasTracker::new(20, 10);
        t.apply_charge(GasCharge::new("", 5 * MILLIGAS_PRECISION, 0))?;
        assert_eq!(t.gas_used(), 15);
        t.apply_charge(GasCharge::new("", 5 * MILLIGAS_PRECISION, 0))?;
        assert_eq!(t.gas_used(), 20);
        assert!(t
            .apply_charge(GasCharge::new("", 1 * MILLIGAS_PRECISION, 0))
            .is_err());
        Ok(())
    }

    #[test]
    fn milligas_to_gas_round() {
        assert_eq!(milligas_to_gas(100, false), 0);
        assert_eq!(milligas_to_gas(100, true), 1);
        assert_eq!(milligas_to_gas(-100, false), -1);
        assert_eq!(milligas_to_gas(-100, true), 0);
    }
}
