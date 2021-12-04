use std::{cell::RefCell, collections::HashMap};

use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};

use crate::{BlockStore, DAG_CBOR};

#[derive(Default, Clone)]
pub struct MemoryBlockstore {
    map: RefCell<HashMap<Cid, Vec<u8>>>,
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
