use std::convert::TryFrom;

use anyhow::Result;
use cid::multihash::Code;
use cid::Cid;
use fvm_sdk as fvm;
use fvm_sdk::blockstore::Blockstore;
use fvm_shared::blockstore::Block;

use crate::actor_error;

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

/// Implements a blockstore delegating to IPLD syscalls.
impl fvm_shared::blockstore::Blockstore for ActorBlockstore {
    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        Blockstore
            .get(cid)
            .map_err(|err| actor_error!(ErrIllegalState; err.to_string()).into())
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        Blockstore
            .put_keyed(k, block)
            .map_err(|err| actor_error!(ErrSerialization; err.to_string()).into())
    }

    fn put<D>(&self, code: Code, block: &Block<D>) -> Result<Cid>
    where
        D: AsRef<[u8]>,
    {
        Blockstore
            .put(code, block)
            .map_err(|err| actor_error!(ErrIllegalState; err.to_string()).into())
    }
}
