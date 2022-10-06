// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::atomic::{AtomicU8, Ordering};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use super::{MAINNET_PREFIX, TESTNET_PREFIX};

static ATOMIC_NETWORK: AtomicU8 = AtomicU8::new(0);

/// Network defines the preconfigured networks to use with address encoding
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, FromPrimitive, ToPrimitive)]
#[repr(u8)]
#[cfg_attr(feature = "arb", derive(arbitrary::Arbitrary))]
pub enum Network {
    Mainnet = 0,
    Testnet = 1,
}

impl Default for Network {
    fn default() -> Self {
        Network::Mainnet
    }
}

impl Network {
    /// to_prefix is used to convert the network into a string
    /// used when converting address to string
    pub(super) fn to_prefix(self) -> &'static str {
        match self {
            Network::Mainnet => MAINNET_PREFIX,
            Network::Testnet => TESTNET_PREFIX,
        }
    }
}

/// Gets the current network.
pub fn current_network() -> Network {
    Network::from_u8(ATOMIC_NETWORK.load(Ordering::Relaxed)).unwrap_or_default()
}

/// Sets the default network.
///
/// The network is used to differentiate between different filecoin networks _in text_ but isn't
/// actually encoded in the binary representation of addresses. Changing the current network will:
///
/// 1. Change the prefix used when formatting an address as a string.
/// 2. Change the prefix _accepted_ when parsing an address.
pub fn set_current_network(network: Network) {
    ATOMIC_NETWORK.store(network.to_u8().unwrap_or_default(), Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::address::Address;

    #[test]
    fn set_network() {
        assert_eq!(current_network(), Network::default());
        assert_eq!(Network::default(), Network::Mainnet);

        // We're in mainnet mode.
        let addr1 = Address::from_str("f01");
        Address::from_str("t01").expect_err("should have failed to parse testnet address");

        // Switch to testnet mode.
        set_current_network(Network::Testnet);

        // Now we're in testnet mode.
        let addr2 = Address::from_str("t01");
        Address::from_str("f01").expect_err("should have failed to parse testnet address");

        // Networks are relevent for parsing only.
        assert_eq!(addr1, addr2)
    }
}
