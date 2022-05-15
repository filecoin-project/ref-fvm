// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::fmt::{Debug, Display};
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_network_version, PriceList, WasmGasPrices};
use crate::kernel::{ExecutionError, Result};

mod charge;
mod outputs;
mod price_list;

pub const MILLIGAS_PRECISION: i64 = 1000;

/// A typesafe representation of gas (internally stored as milligas).
///
/// - All math operations are _saturating_ and never overflow.
/// - Enforces correct units by making it impossible to, e.g., get gas squared (by multiplying gas
///   by gas).
/// - Makes it harder to confuse gas and milligas.
#[derive(Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Default)]
pub struct Gas(i64 /* milligas */);

impl Debug for Gas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            f.debug_tuple("Gas").field(&0 as &dyn Debug).finish()
        } else {
            let integral = self.0 / MILLIGAS_PRECISION;
            let fractional = self.0 % MILLIGAS_PRECISION;
            f.debug_tuple("Gas")
                .field(&format_args!("{integral}.{fractional:03}"))
                .finish()
        }
    }
}

impl Display for Gas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            f.write_str("0")
        } else {
            let integral = self.0 / MILLIGAS_PRECISION;
            let fractional = self.0 % MILLIGAS_PRECISION;
            write!(f, "{integral}.{fractional:03}")
        }
    }
}

impl Gas {
    /// Construct a `Gas` from milligas.
    #[inline]
    pub fn from_milligas(milligas: i64) -> Gas {
        Gas(milligas)
    }

    /// Construct a `Gas` from gas, scaling up. If this exceeds the width of an i64, it saturates at
    /// `i64::MAX` milligas.
    #[inline]
    pub fn new(gas: i64) -> Gas {
        Gas(gas.saturating_mul(MILLIGAS_PRECISION))
    }

    #[inline]
    pub fn is_saturated(&self) -> bool {
        self.0 == i64::MAX
    }

    /// Returns the gas value as an integer, rounding the fractional part up.
    #[inline]
    pub fn round_up(&self) -> i64 {
        milligas_to_gas(self.0, true)
    }

    /// Returns the gas value as an integer, truncating the fractional part.
    #[inline]
    pub fn round_down(&self) -> i64 {
        milligas_to_gas(self.0, false)
    }

    /// Returns the gas value as milligas, without loss of precision.
    #[inline]
    pub fn as_milligas(&self) -> i64 {
        self.0
    }
}

impl num_traits::Zero for Gas {
    fn zero() -> Self {
        Gas(0)
    }

    fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Add for Gas {
    type Output = Gas;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for Gas {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0)
    }
}

impl SubAssign for Gas {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0)
    }
}

impl Sub for Gas {
    type Output = Gas;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl Mul<i64> for Gas {
    type Output = Gas;

    #[inline]
    fn mul(self, rhs: i64) -> Self::Output {
        Self(self.0.saturating_mul(rhs))
    }
}

impl Mul<i32> for Gas {
    type Output = Gas;

    #[inline]
    fn mul(self, rhs: i32) -> Self::Output {
        Self(self.0.saturating_mul(rhs.into()))
    }
}

pub struct GasTracker {
    gas_limit: Gas,
    gas_used: Gas,
}

impl GasTracker {
    /// Gas limit and gas used are provided in protocol units (i.e. full units).
    /// They are converted to milligas for internal canonical accounting.
    pub fn new(gas_limit: Gas, gas_used: Gas) -> Self {
        Self {
            gas_limit,
            gas_used,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_gas(&mut self, name: &str, to_use: Gas) -> Result<()> {
        log::trace!("charging gas: {} {}", name, to_use);
        // The gas type uses saturating math.
        self.gas_used += to_use;
        if self.gas_used > self.gas_limit {
            log::trace!("gas limit reached");
            self.gas_used = self.gas_limit;
            Err(ExecutionError::OutOfGas)
        } else {
            Ok(())
        }
    }

    /// Applies the specified gas charge, where quantities are supplied in milligas.
    pub fn apply_charge(&mut self, charge: GasCharge) -> Result<()> {
        self.charge_gas(charge.name, charge.total())
    }

    /// Getter for the maximum gas usable by this message.
    pub fn gas_limit(&self) -> Gas {
        self.gas_limit
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> Gas {
        self.gas_used
    }

    /// Getter for gas available.
    pub fn gas_available(&self) -> Gas {
        self.gas_limit - self.gas_used
    }
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
    use num_traits::Zero;

    use super::*;

    #[test]
    #[allow(clippy::identity_op)]
    fn basic_gas_tracker() -> Result<()> {
        let mut t = GasTracker::new(Gas::new(20), Gas::new(10));
        t.apply_charge(GasCharge::new("", Gas::new(5), Gas::zero()))?;
        assert_eq!(t.gas_used(), Gas::new(15));
        t.apply_charge(GasCharge::new("", Gas::new(5), Gas::zero()))?;
        assert_eq!(t.gas_used(), Gas::new(20));
        assert!(t
            .apply_charge(GasCharge::new("", Gas::new(1), Gas::zero()))
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
