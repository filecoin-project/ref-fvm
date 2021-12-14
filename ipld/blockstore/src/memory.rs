use std::{cell::RefCell, collections::HashMap, fmt};

use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};

use crate::{BlockStore, DAG_CBOR};

#[derive(Default, Clone)]
pub struct MemoryBlockstore {
    map: RefCell<HashMap<Cid, Vec<u8>>>,
}

impl fmt::Debug for MemoryBlockstore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(&*self.map.borrow()).finish()
    }
}

impl MemoryBlockstore {
    pub fn new() -> Self {
        Default::default()
    }
}

impl BlockStore for MemoryBlockstore {
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        Ok(self.map.borrow().get(cid).cloned())
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn std::error::Error>> {
        let cid = Cid::new_v1(DAG_CBOR, code.digest(bytes));
        self.map.borrow_mut().insert(cid, bytes.into());
        Ok(cid)
    }
}

// Ideally we'd have a blanket impl of BlockStore for &T where T is BlockStore. But we already have that for Blockstore -> BlockStore.
//
// We should find a way to deduplicate these traits.
impl BlockStore for &MemoryBlockstore {
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        (*self).get_bytes(cid)
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn std::error::Error>> {
        (*self).put_raw(bytes, code)
    }
}
