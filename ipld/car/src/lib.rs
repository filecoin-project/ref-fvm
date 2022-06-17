// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod error;
mod util;

use std::convert::TryFrom;

use cid::Cid;
pub use error::*;
use futures::{AsyncRead, AsyncWrite, Stream, StreamExt};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{from_slice, to_vec};
use serde::{Deserialize, Serialize};
use util::{ld_read, ld_write, read_node};

/// CAR file header
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CarHeader {
    pub roots: Vec<Cid>,
    pub version: u64,
}

impl CarHeader {
    /// Creates a new CAR file header
    pub fn new(roots: Vec<Cid>, version: u64) -> Self {
        Self { roots, version }
    }

    /// Writes header and stream of data to writer in Car format.
    pub async fn write_stream_async<W, S>(
        &self,
        writer: &mut W,
        stream: &mut S,
    ) -> Result<(), Error>
    where
        W: AsyncWrite + Send + Unpin,
        S: Stream<Item = (Cid, Vec<u8>)> + Unpin,
    {
        // Write header bytes
        let header_bytes = to_vec(self)?;
        ld_write(writer, &header_bytes).await?;

        // Write all key values from the stream
        while let Some((cid, bytes)) = stream.next().await {
            ld_write(writer, &[cid.to_bytes(), bytes].concat()).await?;
        }

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
    R: AsyncRead + Send + Unpin,
{
    /// Creates a new CarReader and parses the Car
    pub async fn new(mut reader: R) -> Result<Self, Error> {
        let buf = ld_read(&mut reader)
            .await?
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
    pub async fn new_unchecked(reader: R) -> Result<Self, Error> {
        let mut reader = Self::new(reader).await?;
        reader.validate = false;
        Ok(reader)
    }

    /// Returns the next IPLD Block in the buffer
    pub async fn next_block(&mut self) -> Result<Option<Block>, Error> {
        use cid::multihash::{self, MultihashDigest};
        // Read node -> cid, bytes
        if let Some((cid, data)) = read_node(&mut self.reader).await? {
            if self.validate {
                match cid.hash().code() {
                    0x0 => {
                        if cid.hash().digest() != data {
                            return Err(Error::InvalidFile(
                                "CAR has an identity CID that doesn't match the corresponding data"
                                    .into(),
                            ));
                        }
                    }
                    code => {
                        let code = multihash::Code::try_from(code)?;
                        let actual = Cid::new_v1(cid.codec(), code.digest(&data));
                        if actual != cid {
                            return Err(Error::InvalidFile(format!(
                                "CAR has an incorrect CID: expected {}, found {}",
                                cid, actual,
                            )));
                        }
                    }
                }
            }
            Ok(Some(Block { cid, data }))
        } else {
            Ok(None)
        }
    }
}

/// IPLD Block
#[derive(Clone, Debug)]
pub struct Block {
    pub cid: Cid,
    pub data: Vec<u8>,
}

/// Loads a CAR buffer into a Blockstore
pub async fn load_car<R, B>(s: &B, reader: R) -> Result<Vec<Cid>, Error>
where
    B: Blockstore,
    R: AsyncRead + Send + Unpin,
{
    load_car_inner(s, reader, true).await
}

/// Loads a CAR buffer into a Blockstore without checking the CIDs.
pub async fn load_car_unchecked<R, B>(s: &B, reader: R) -> Result<Vec<Cid>, Error>
where
    B: Blockstore,
    R: AsyncRead + Send + Unpin,
{
    load_car_inner(s, reader, false).await
}

async fn load_car_inner<R, B>(s: &B, reader: R, verify: bool) -> Result<Vec<Cid>, Error>
where
    B: Blockstore,
    R: AsyncRead + Send + Unpin,
{
    let mut car_reader = if verify {
        CarReader::new(reader).await
    } else {
        CarReader::new_unchecked(reader).await
    }?;

    // Batch write key value pairs from car file
    // TODO: Stream the data once some of the stream APIs stabilize.
    let mut buf = Vec::with_capacity(100);
    while let Some(block) = car_reader.next_block().await? {
        buf.push((block.cid, block.data));
        if buf.len() > 1000 {
            s.put_many_keyed(buf.iter().map(|(k, v)| (*k, &*v)))
                .map_err(|e| Error::Other(e.to_string()))?;
            buf.clear();
        }
    }
    s.put_many_keyed(buf.iter().map(|(k, v)| (*k, &*v)))
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(car_reader.header.roots)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_std::channel::bounded;
    use async_std::io::Cursor;
    use async_std::sync::RwLock;
    use cid::multihash::Code::Blake2b256;
    use cid::multihash::MultihashDigest;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::DAG_CBOR;

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

    #[async_std::test]
    async fn car_write_read() {
        let buffer: Arc<RwLock<Vec<u8>>> = Default::default();
        let cid = Cid::new_v1(DAG_CBOR, Blake2b256.digest(b"test"));
        let header = CarHeader {
            roots: vec![cid],
            version: 1,
        };
        assert_eq!(to_vec(&header).unwrap().len(), 60);

        let (tx, mut rx) = bounded(10);

        let buffer_cloned = buffer.clone();
        let write_task = async_std::task::spawn(async move {
            header
                .write_stream_async(&mut *buffer_cloned.write().await, &mut rx)
                .await
                .unwrap()
        });

        tx.send((cid, b"test".to_vec())).await.unwrap();
        drop(tx);
        write_task.await;

        let buffer: Vec<_> = buffer.read().await.clone();
        let reader = Cursor::new(&buffer);

        let bs = MemoryBlockstore::default();
        load_car(&bs, reader).await.unwrap();

        assert_eq!(bs.get(&cid).unwrap(), Some(b"test".to_vec()));
    }
}
