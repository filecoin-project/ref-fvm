// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

#[cfg(feature = "upgrade-actor")]
#[derive(
    Clone,
    Debug,
    Copy,
    PartialEq,
    Eq,
    fvm_ipld_encoding::tuple::Serialize_tuple,
    fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
pub struct UpgradeInfo {
    // the old code cid we are upgrading from
    pub old_code_cid: cid::Cid,
}
