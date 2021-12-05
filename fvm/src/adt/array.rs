// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use ipld_amt::Amt;

pub type Array<'a, BS, V> = Amt<'a, V, BS>;
