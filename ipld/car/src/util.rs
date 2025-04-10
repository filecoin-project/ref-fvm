// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::io::{self, Read};

use cid::Cid;
use unsigned_varint::io::ReadError;

use super::Block;
use super::Error;

pub(crate) fn ld_read(reader: &mut impl io::Read) -> Result<Option<Vec<u8>>, Error> {
    const MAX_ALLOC: usize = 1 << 20;
    let l: usize = match unsigned_varint::io::read_usize(&mut *reader) {
        Ok(len) => len,
        Err(ReadError::Io(e)) => {
            return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Ok(None)
            } else {
                Err(Error::Io(e))
            }
        }
        Err(ReadError::Decode(e)) => return Err(Error::ParsingError(e.to_string())),
        Err(e) => return Err(Error::Other(e.to_string())),
    };
    let mut buf = Vec::with_capacity(std::cmp::min(l, MAX_ALLOC));
    let bytes_read = reader
        .take(l as u64)
        .read_to_end(&mut buf)
        .map_err(|e| Error::Other(e.to_string()))?;
    if bytes_read != l {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!(
                "expected to read at least {} bytes, but read {}",
                l, bytes_read
            ),
        )));
    }
    Ok(Some(buf))
}

pub(crate) fn ld_write(writer: &mut impl io::Write, bytes: &[u8]) -> Result<(), Error> {
    let mut buff = unsigned_varint::encode::usize_buffer();
    let len = unsigned_varint::encode::usize(bytes.len(), &mut buff);
    writer.write_all(len)?;
    writer.write_all(bytes)?;
    Ok(())
}

pub(crate) fn read_node(buf_reader: &mut impl io::Read) -> Result<Option<Block>, Error> {
    match ld_read(buf_reader)? {
        Some(buf) => {
            let mut cursor = std::io::Cursor::new(&buf);
            let cid = Cid::read_bytes(&mut cursor)?;
            Ok(Some(Block {
                cid,
                data: buf[cursor.position() as usize..].to_vec(),
            }))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ld_read_write() {
        let mut buffer = Vec::<u8>::new();
        ld_write(&mut buffer, b"test bytes").unwrap();
        let mut reader = std::io::Cursor::new(buffer);
        let read = ld_read(&mut reader).unwrap();
        assert_eq!(read, Some(b"test bytes".to_vec()));
    }
}
