// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::atomic::{AtomicU8, Ordering};

use once_cell::sync::OnceCell;

use super::Network;

/// Singleton context that stores the [Network] info
/// to allow properly tagging the [super::Address] when using
/// constructors that does not take [Network] parameter.
///
/// applications are responsible for setting up / making changes to
/// this context at proper time of its lifecycle
#[derive(Debug, Default)]
pub struct AddressContext {
    network: AtomicNetwork,
}

impl AddressContext {
    pub fn instance() -> &'static Self {
        static CELL: OnceCell<AddressContext> = OnceCell::new();
        CELL.get_or_init(AddressContext::default)
    }

    pub fn network(&self) -> Network {
        (&self.network).into()
    }

    pub fn set_network(&self, network: Network) -> Result<(), u8> {
        let (tag,) = network.into();
        self.network.update(tag)
    }
}

#[derive(Debug)]
struct AtomicNetwork(AtomicU8);

impl AtomicNetwork {
    fn update(&self, tag: u8) -> Result<(), u8> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, move |_| Some(tag))?;
        Ok(())
    }
}

impl Default for AtomicNetwork {
    fn default() -> Self {
        Network::default().into()
    }
}

impl From<&AtomicNetwork> for Network {
    fn from(v: &AtomicNetwork) -> Self {
        match v.0.load(Ordering::Relaxed) {
            0 => Network::Mainnet,
            _ => Network::Testnet,
        }
    }
}

impl From<Network> for (u8,) {
    fn from(v: Network) -> Self {
        (match v {
            Network::Mainnet => 0,
            Network::Testnet => 1,
        },)
    }
}

impl From<Network> for AtomicNetwork {
    fn from(v: Network) -> Self {
        let (u,) = v.into();
        Self(AtomicU8::new(u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_atomic_convertion_roundtrip() {
        // TODO: Consider using `enum_iterator::all::<Network>()`
        // which requires rust toolchain upgrade.
        for network in [Network::Mainnet, Network::Testnet] {
            let atomic: AtomicNetwork = network.into();
            let from_atomic: Network = (&atomic).into();
            assert_eq!(network, from_atomic);
        }
    }

    #[test]
    fn set_network() -> Result<(), u8> {
        let cxt = AddressContext::instance();
        assert_eq!(cxt.network(), Network::default());
        // TODO: Consider using `enum_iterator::all::<Network>()`
        // which requires rust toolchain upgrade.
        for network in [Network::Mainnet, Network::Testnet] {
            cxt.set_network(network)?;
            assert_eq!(cxt.network(), network);
        }
        Ok(())
    }
}
