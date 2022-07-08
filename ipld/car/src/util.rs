// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use integer_encoding::{VarIntAsyncReader, VarIntAsyncWriter};

use super::error::Error;

pub(crate) async fn ld_read<R>(mut reader: &mut R) -> Result<Option<Vec<u8>>, Error>
where
    R: AsyncRead + Send + Unpin,
{
    const MAX_ALLOC: usize = 1 << 20;
    let l: usize = match VarIntAsyncReader::read_varint_async(&mut reader).await {
        Ok(len) => len,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(Error::Other(e.to_string()));
        }
    };
    let mut buf = Vec::with_capacity(std::cmp::min(l as usize, MAX_ALLOC));
    let bytes_read = reader
        .take(l as u64)
        .read_to_end(&mut buf)
        .await
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

pub(crate) async fn ld_write<'a, W>(writer: &mut W, bytes: &[u8]) -> Result<(), Error>
where
    W: AsyncWrite + Send + Unpin,
{
    writer.write_varint_async(bytes.len()).await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn read_node<R>(buf_reader: &mut R) -> Result<Option<(Cid, Vec<u8>)>, Error>
where
    R: AsyncRead + Send + Unpin,
{
    match ld_read(buf_reader).await? {
        Some(buf) => {
            let mut cursor = std::io::Cursor::new(&buf);
            let cid = Cid::read_bytes(&mut cursor)?;
            Ok(Some((cid, buf[cursor.position() as usize..].to_vec())))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use async_std::io::Cursor;

    use super::*;

    #[async_std::test]
    async fn ld_read_write() {
        let mut buffer = Vec::<u8>::new();
        ld_write(&mut buffer, b"test bytes").await.unwrap();
        let mut reader = Cursor::new(&buffer);
        let read = ld_read(&mut reader).await.unwrap();
        assert_eq!(read, Some(b"test bytes".to_vec()));
    }
}
