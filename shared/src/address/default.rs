// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::atomic::{AtomicU8, Ordering};

use super::Network;

static ATOMIC_NETWORK: AtomicU8 = AtomicU8::new(0);

/// Gets the default network
pub fn default_network() -> Network {
    ATOMIC_NETWORK.load(Ordering::Relaxed).into()
}

/// Sets the default network
pub fn set_default_network(network: Network) -> anyhow::Result<()> {
    ATOMIC_NETWORK
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, move |_| {
            Some(network.into())
        })
        .map_err(|err| anyhow::Error::msg(format!("Failed to set default network: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::Address;

    #[test]
    fn set_network() -> anyhow::Result<()> {
        assert_eq!(default_network(), Network::default());
        // TODO: Consider using `enum_iterator::all::<Network>()`
        // which requires rust toolchain upgrade.
        for network in [Network::Mainnet, Network::Testnet] {
            set_default_network(network)?;
            assert_eq!(default_network(), network);
            let addr = Address::new_id(0);
            assert_eq!(addr.network(), network);
        }
        Ok(())
    }
}
