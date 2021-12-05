// TODO: Temporarily copied from the SDK. We need a shared types crate.

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AbiTokenAmount {
    // DO NOT reorder these fields. The layout is equivalent to u128 on a big-endian system, and
    // optimizes well.
    lo: u64,
    hi: u64,
}

impl AbiTokenAmount {
    pub const fn zero() -> Self {
        Self { hi: 0, lo: 0 }
    }
}

impl From<u128> for AbiTokenAmount {
    #[inline]
    fn from(v: u128) -> Self {
        Self {
            lo: v as u64,
            hi: (v >> u64::BITS) as u64,
        }
    }
}

impl From<AbiTokenAmount> for u128 {
    #[inline]
    fn from(v: AbiTokenAmount) -> Self {
        (v.hi as u128) << u64::BITS | (v.lo as u128)
    }
}

#[allow(dead_code)]
pub type Metadata = Metadata1;

// TODO: we should probably move this definition to a shared crate with shared types (addresses, metadata, etc.).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Metadata1 {
    pub epoch: u64,
    // Message
    pub method: u64,
    pub caller: u64,
    pub receiver: u64,
    pub value_received: AbiTokenAmount,
    // TODO: do we really need this now that we have a VM?
    pub network_version: u32,
    // TODO: does this pull its weight? IMO, we may want thi
}
