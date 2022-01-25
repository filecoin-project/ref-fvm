// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub use self::charge::GasCharge;
pub(crate) use self::outputs::GasOutputs;
pub use self::price_list::{price_list_by_epoch, PriceList};
use crate::kernel::SyscallError;
use crate::syscall_error;

mod charge;
mod outputs;
mod price_list;

pub struct GasTracker {
    gas_available: i64,
    gas_used: i64,
}

impl GasTracker {
    pub fn new(gas_available: i64, gas_used: i64) -> Self {
        Self {
            gas_available,
            gas_used,
        }
    }

    /// Safely consumes gas and returns an out of gas error if there is not sufficient
    /// enough gas remaining for charge.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<(), SyscallError> {
        let to_use = charge.total();
        match self.gas_used.checked_add(to_use) {
            None => {
                log::trace!("gas overflow: {}", charge.name);
                self.gas_used = self.gas_available;
                Err(syscall_error!(SysErrOutOfGas;
                    "adding gas_used={} and to_use={} overflowed",
                    self.gas_used, to_use
                ))
            }
            Some(used) => {
                log::trace!("charged {} gas: {}", used, charge.name);
                if used > self.gas_available {
                    log::trace!("out of gas: {}", charge.name);
                    self.gas_used = self.gas_available;
                    Err(syscall_error!(SysErrOutOfGas;
                            "not enough gas (used={}) (available={})",
                       used, self.gas_available
                    ))
                } else {
                    self.gas_used = used;
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
