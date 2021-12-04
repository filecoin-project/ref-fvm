use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::encoding::de::DeserializeOwned;
use fvm_shared::encoding::ser::Serialize;
use fvm_shared::encoding::{from_slice, to_vec, Cbor};

/// CborStore overlays a Blockstore and provides getters and setters that
/// perform high-level object conversions using CBOR as a serialization format.
pub struct CborStore<'a, B> {
    /// The underlying blockstore.
    blockstore: &'a B,
}

impl<'a, B: Blockstore> CborStore<'a, &B> {
    /// Gets the block specified by CID and deserializes it as CBOR before
    /// returning it as the high-level type T.
    pub fn get_cbor<T>(&self, cid: &Cid) -> anyhow::Result<Option<T>>
    where
        T: Cbor + DeserializeOwned,
    {
        match self.blockstore.get(cid)? {
            Some(bz) => Ok(Some(from_slice(&bz)?)),
            None => Ok(None),
        }
    }

    /// Puts the specified object into the blockstore, serializing it as
    /// CBOR first.
    pub fn put_cbor<T>(&mut self, obj: &T) -> anyhow::Result<Cid>
    where
        T: Cbor + Serialize,
    {
        let bytes = to_vec(obj)?;
        // TODO @stebalien to calculate the CID with the right multihash and codec.
        let cid = Cid::default();
        self.blockstore.put(&cid, bytes.as_ref())
    }
}

/// Enables conversion from a Blockstore into a CborStore.
impl<'a, B> From<&B> for CborStore<'a, &B>
where
    B: 'a + Blockstore,
{
    fn from(blockstore: &B) -> Self {
        CborStore { blockstore }
    }
}
