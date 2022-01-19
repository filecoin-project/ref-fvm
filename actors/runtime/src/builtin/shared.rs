// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::address::Address;
use fvm_shared::blockstore::Blockstore;
use fvm_shared::METHOD_SEND;

use crate::runtime::Runtime;

pub const HAMT_BIT_WIDTH: u32 = 5;

/// ResolveToIDAddr resolves the given address to it's ID address form.
/// If an ID address for the given address dosen't exist yet, it tries to create one by sending
/// a zero balance to the given address.
pub fn resolve_to_id_addr<BS, RT>(rt: &mut RT, address: &Address) -> anyhow::Result<Address>
where
    BS: Blockstore,
    RT: Runtime<BS>,
{
    // if we are able to resolve it to an ID address, return the resolved address
    if let Some(addr) = rt.resolve_address(address) {
        return Ok(addr);
    }

    // send 0 balance to the account so an ID address for it is created and then try to resolve
    rt.send(
        *address,
        METHOD_SEND,
        Default::default(),
        Default::default(),
    )
    .map_err(|e| {
        e.wrap(&format!(
            "failed to send zero balance to address {}",
            address
        ))
    })?;

    rt.resolve_address(address).ok_or_else(|| {
        anyhow::anyhow!(
            "failed to resolve address {} to ID address even after sending zero balance",
            address,
        )
    })
}
