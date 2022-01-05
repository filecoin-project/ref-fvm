use crate::kernel::{ClassifyResult, Result};
use crate::syscalls::Memory;
use crate::{syscall_error, Kernel};

pub fn resolve_address(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    addr_off: u32, // Address
    addr_len: u32,
) -> Result<(i32, u64)> {
    let addr = memory.read_address(addr_off, addr_len)?;
    match kernel.resolve_address(&addr)? {
        Some(id) => Ok((0, id)),
        None => Ok((-1, 0)),
    }
}

pub fn get_actor_code_cid(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    addr_off: u32, // Address
    addr_len: u32,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<i32> {
    let addr = memory.read_address(addr_off, addr_len)?;
    match kernel.get_actor_code_cid(&addr)? {
        Some(typ) => {
            let obuf = memory.try_slice_mut(obuf_off, obuf_len)?;
            // TODO: This isn't always an illegal argument error, only when the buffer is too small.
            typ.write_bytes(obuf).or_illegal_argument()?;
            Ok(0)
        }
        None => Ok(-1),
    }
}

/// Generates a new actor address, and writes it into the supplied output buffer.
///
/// The output buffer must be at least 21 bytes long, which is the length of a
/// class 2 address (protocol-generated actor address). This will change in the
/// future when we introduce class 4 addresses to accommodate larger hashes.
///
/// TODO this method will be merged with create_actor in the near future.
pub fn new_actor_address(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    obuf_off: u32, // Address (out)
    obuf_len: u32,
) -> Result<u32> {
    if obuf_len < 21 {
        return Err(
            syscall_error!(SysErrIllegalArgument; "output buffer must have a minimum capacity of 21 bytes").into(),
        );
    }

    let addr = kernel.new_actor_address()?;
    let bytes = addr.to_bytes();

    let len = bytes.len();
    if len > obuf_len as usize {
        return Err(syscall_error!(SysErrIllegalArgument;
            "insufficient output buffer capacity; {} (new address) > {} (buffer capacity)",
            len, obuf_len
        )
        .into());
    }

    let obuf = memory.try_slice_mut(obuf_off, obuf_len)?;
    obuf[..len].copy_from_slice(bytes.as_slice());
    Ok(len as u32)
}

pub fn create_actor(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    actor_id: u64, // Address
    typ_off: u32,  // Cid
) -> Result<()> {
    let typ = memory.read_cid(typ_off)?;
    kernel.create_actor(typ, actor_id)
}
