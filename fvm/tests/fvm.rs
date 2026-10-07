// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod default_kernel;
mod dummy;

use dummy::*;

use fvm::gas::price_list_by_network_version;
use fvm::machine::{DefaultMachine, Machine};
use fvm_shared::version::NetworkVersion;

#[test]
fn network_version_machine_support() {
    for (version, supported) in [
        (NetworkVersion::V20, false),
        (NetworkVersion::V21, true),
        (NetworkVersion::V29, true),
        (NetworkVersion::V30, cfg!(feature = "nv30-dev")),
        (NetworkVersion::new(31), false),
    ] {
        let mut dummy = DummyMachine::new_stub().unwrap();
        // Set the version directly to test the machine's gate independently of gas pricing.
        dummy.ctx.network_version = version;
        let context = dummy.ctx.clone();
        let result = DefaultMachine::new(&context, dummy.into_store(), DummyExterns);

        if supported {
            assert!(
                result.is_ok(),
                "network version {version}: {:?}",
                result.err()
            );
        } else {
            assert_eq!(
                result
                    .err()
                    .expect("unsupported network version")
                    .to_string(),
                format!("unsupported network version: {version}")
            );
        }
    }
}

#[test]
#[cfg(feature = "nv30-dev")]
fn network_version_30_uses_previous_gas_prices() {
    assert!(std::ptr::eq(
        price_list_by_network_version(NetworkVersion::V29),
        price_list_by_network_version(NetworkVersion::V30),
    ));
}

#[test]
#[cfg(not(feature = "nv30-dev"))]
#[should_panic(expected = "network version 30 not supported")]
fn network_version_30_gas_prices_require_feature() {
    price_list_by_network_version(NetworkVersion::V30);
}

#[test]
#[should_panic(expected = "network version 31 not supported")]
fn network_version_future_gas_prices_are_unsupported() {
    price_list_by_network_version(NetworkVersion::new(31));
}
