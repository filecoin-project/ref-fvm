// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod block;
mod error;
mod util;

use std::io;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::from_slice;
use serde::{Deserialize, Serialize};
use util::{ld_read, ld_write, read_node};

pub use block::Block;
pub use error::Error;

/// CAR file header
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CarHeader {
    pub roots: Vec<Cid>,
    pub version: u64,
}

impl CarHeader {
    /// Creates a new CAR file header
    pub fn new(roots: Vec<Cid>, version: u64) -> Self {
        Self { roots, version }
    }
}

/// A car writer.
pub struct CarWriter<W> {
    // We keep a temp buffer here to avoid having to allocate a buffer for every block written.
    buffer: Vec<u8>,
    writer: W,
}

impl<W> CarWriter<W>
where
    W: io::Write,
{
    /// Create a new CarWriter, starting by writing the car header.
    pub fn new(header: CarHeader, writer: W) -> Result<CarWriter<W>, Error> {
        let mut w = Self {
            buffer: Vec::new(),
            writer,
        };

        fvm_ipld_encoding::to_writer(&mut w.buffer, &header)?;
        ld_write(&mut w.writer, &w.buffer)?;
        Ok(w)
    }

    /// Writes a block to the car.
    pub fn write(&mut self, block: Block) -> Result<(), Error> {
        // We always clear the buffer before writing, not after, just in case we error out
        // somewhere. It doesn't really matter much, it mostly makes the code easier to reason
        // about and it doesn't make a performance difference (we keep the memory anyways).
        self.buffer.clear();

        block.cid.write_bytes(&mut self.buffer)?;
        self.buffer.extend_from_slice(&block.data);
        ld_write(&mut self.writer, &self.buffer)?;

        Ok(())
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush()?;
        Ok(())
    }
}

impl From<Vec<Cid>> for CarHeader {
    fn from(roots: Vec<Cid>) -> Self {
        Self { roots, version: 1 }
    }
}

/// Reads CAR files that are in a BufReader
pub struct CarReader<R> {
    pub reader: R,
    pub header: CarHeader,
    pub validate: bool,
}

impl<R> CarReader<R>
where
    R: io::Read,
{
    /// Creates a new CarReader and parses the Car
    pub fn new(mut reader: R) -> Result<Self, Error> {
        let buf = ld_read(&mut reader)?
            .ok_or_else(|| Error::ParsingError("failed to parse uvarint for header".to_string()))?;
        let header: CarHeader = from_slice(&buf).map_err(|e| Error::ParsingError(e.to_string()))?;
        if header.roots.is_empty() {
            return Err(Error::ParsingError("empty CAR file".to_owned()));
        }
        if header.version != 1 {
            return Err(Error::InvalidFile("CAR file version must be 1".to_owned()));
        }
        Ok(CarReader {
            reader,
            header,
            validate: true,
        })
    }

    /// Creates a new CarReader that parses the Car, but doesn't validate the inner CIDs.
    pub fn new_unchecked(reader: R) -> Result<Self, Error> {
        let mut reader = Self::new(reader)?;
        reader.validate = false;
        Ok(reader)
    }

    /// Loads the CAR file into the given blockstore, returning the roots.
    pub fn read_into(mut self, s: &impl Blockstore) -> Result<Vec<Cid>, Error> {
        let mut buf = Vec::with_capacity(100);
        for block in &mut self {
            buf.push(block?.into());
            if buf.len() > 1000 {
                s.put_many_keyed(buf.drain(..))
                    .map_err(|e| Error::Other(e.to_string()))?;
            }
        }
        s.put_many_keyed(buf)
            .map_err(|e| Error::Other(e.to_string()))?;
        Ok(self.header.roots)
    }
}

impl<R> Iterator for CarReader<R>
where
    R: io::Read,
{
    type Item = Result<Block, Error>;

    /// Returns the next IPLD Block in the buffer
    fn next(&mut self) -> Option<Result<Block, Error>> {
        // Read node -> cid, bytes
        match read_node(&mut self.reader) {
            Ok(Some(block)) => {
                if self.validate {
                    if let Err(e) = block.validate() {
                        return Some(Err(e));
                    }
                }
                Some(Ok(block))
            }
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// Loads a CAR buffer into a Blockstore
pub fn load_car(s: &impl Blockstore, reader: impl io::Read) -> Result<Vec<Cid>, Error> {
    let car_reader = CarReader::new(reader)?;
    car_reader.read_into(s)
}

/// Loads a CAR buffer into a Blockstore without checking the CIDs.
pub fn load_car_unchecked(s: &impl Blockstore, reader: impl io::Read) -> Result<Vec<Cid>, Error> {
    let car_reader = CarReader::new_unchecked(reader)?;
    car_reader.read_into(s)
}

#[cfg(test)]
mod tests {
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::{DAG_CBOR, to_vec};
    use multihash_codetable::{Code::Blake2b256, MultihashDigest};

    use super::*;

    #[test]
    fn symmetric_header() {
        let cid = Cid::new_v1(DAG_CBOR, Blake2b256.digest(b"test"));

        let header = CarHeader {
            roots: vec![cid],
            version: 1,
        };

        let bytes = to_vec(&header).unwrap();
        assert_eq!(from_slice::<CarHeader>(&bytes).unwrap(), header);
    }

    #[test]
    fn car_write_read() {
        let cid = Cid::new_v1(DAG_CBOR, Blake2b256.digest(b"test"));
        let header = CarHeader {
            roots: vec![cid],
            version: 1,
        };
        assert_eq!(to_vec(&header).unwrap().len(), 60);

        let mut buffer = Vec::new();
        let mut writer = CarWriter::new(header, &mut buffer).unwrap();
        writer
            .write(Block {
                cid,
                data: b"test".to_vec(),
            })
            .unwrap();
        writer.flush().unwrap();

        let mut reader = io::Cursor::new(buffer);
        let bs = MemoryBlockstore::default();
        load_car(&bs, &mut reader).unwrap();

        assert_eq!(bs.get(&cid).unwrap(), Some(b"test".to_vec()));
    }
}
