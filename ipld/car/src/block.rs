// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};

use super::Error;

/// IPLD Block
#[derive(Clone, Debug)]
pub struct Block {
    pub cid: Cid,
    pub data: Vec<u8>,
}

impl From<Block> for (Cid, Vec<u8>) {
    fn from(block: Block) -> Self {
        (block.cid, block.data)
    }
}

impl From<(Cid, Vec<u8>)> for Block {
    fn from((cid, data): (Cid, Vec<u8>)) -> Self {
        Block { cid, data }
    }
}

impl Block {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        match self.cid.hash().code() {
            0x0 => {
                if self.cid.hash().digest() != self.data {
                    return Err(Error::InvalidFile(
                        "CAR has an identity CID that doesn't match the corresponding data".into(),
                    ));
                }
            }
            code => {
                let code = Code::try_from(code)?;
                let actual = Cid::new_v1(self.cid.codec(), code.digest(&self.data));
                if actual != self.cid {
                    return Err(Error::InvalidFile(format!(
                        "CAR has an incorrect CID: expected {}, found {}",
                        self.cid, actual,
                    )));
                }
            }
        }
        Ok(())
    }
}
