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

/// Gets the default network
pub fn default_network() -> Network {
    Network::from_u8(ATOMIC_NETWORK.load(Ordering::Relaxed)).unwrap_or_default()
}

/// Sets the default network
pub fn set_default_network(network: Network) {
    ATOMIC_NETWORK.store(network.to_u8().unwrap_or_default(), Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::Address;

    #[test]
    fn set_network() {
        assert_eq!(default_network(), Network::default());
        // TODO: Consider using `enum_iterator::all::<Network>()`
        // which requires rust toolchain upgrade.
        for network in [Network::Mainnet, Network::Testnet] {
            set_default_network(network);
            assert_eq!(default_network(), network);
            let addr = Address::new_id(0);
            assert_eq!(addr.network(), network);
        }
    }
}
