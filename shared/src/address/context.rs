// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::atomic::{AtomicU8, Ordering};

use once_cell::sync::OnceCell;

use super::Network;

/// Single context that stores the [Network] info
/// to allow properly tagging the [super::Address] when using
/// constructors that does not take [Network] parameter
/// applications are responsible for setting up / make changes to
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

    pub fn set_network(&mut self, network: Network) {
        self.network = network.into();
    }
}

#[derive(Debug)]
struct AtomicNetwork(AtomicU8);

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

impl From<Network> for AtomicNetwork {
    fn from(v: Network) -> Self {
        Self(AtomicU8::new(match v {
            Network::Mainnet => 0,
            Network::Testnet => 1,
        }))
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
}
