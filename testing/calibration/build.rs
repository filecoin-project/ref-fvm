// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

fn main() {
    // intentionally empty here since fil_gas_calibration_actor is already built by
    // fvm_test_builder. However, we need this build.rs file to exist so cargo passes
    // OUT_DIR env variable that we can reference when constructing the path to the
    // GAS_CALIBRATION_ACTOR_PATH
}
