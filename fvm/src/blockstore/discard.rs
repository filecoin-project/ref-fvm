// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::marker::PhantomData;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;

// A blockstore that accepts but discards all insertions, and returns errors on reads.
// Useful for when the FVM needs to stage ephemeral data structures without persisting them,
// like the events AMT.
#[derive(Copy, Clone)]
pub struct DiscardBlockstore<C = multihash::Code>(PhantomData<fn() -> C>);

impl Default for DiscardBlockstore {
    fn default() -> Self {
        DiscardBlockstore(Default::default())
    }
}

impl<C> Blockstore for DiscardBlockstore<C>
where
    C: multihash::MultihashDigest<64>,
    anyhow::Error: From<C::Error>,
{
    fn get(&self, _: &Cid) -> anyhow::Result<Option<Vec<u8>>> {
        Err(anyhow::anyhow!(
            "Blockstore::get not supported with DiscardBlockstore"
        ))
    }

    fn put_keyed(&self, _: &Cid, _: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }

    fn put(&self, mh_code: u64, block: &dyn fvm_ipld_blockstore::Block) -> anyhow::Result<Cid> {
        let mh_code = C::try_from(mh_code)?;
        let data = block.data();
        let codec = block.codec();
        let digest = mh_code.digest(data);
        Ok(Cid::new_v1(codec, digest))
    }

    fn has(&self, _: &Cid) -> anyhow::Result<bool> {
        Err(anyhow::anyhow!(
            "Blockstore::has not supported with DiscardBlockstore"
        ))
    }

    fn put_many<B, I>(&self, _: I) -> anyhow::Result<()>
    where
        Self: Sized,
        B: fvm_ipld_blockstore::Block,
        I: IntoIterator<Item = (u64, B)>,
    {
        Ok(())
    }

    fn put_many_keyed<D, I>(&self, _: I) -> anyhow::Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Cid, D)>,
    {
        Ok(())
    }
}
