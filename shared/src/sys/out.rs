//! This module contains syscall output data carrier structs, shared between
//! the FVM SDK and the FVM itself, wrapping multi-value returns.
//!
//! These are necessary because Rust WASM multi-value return compilation is
//! plagued with issues and catch-22 problems, making it unfeasible to use
//! actual bare multi-value returns in FFI extern definitions.
//!
//! Read more at https://github.com/rust-lang/rust/issues/73755.

pub mod actor {
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct ResolveAddress {
        pub resolved: i32,
        pub value: u64,
    }
}

pub mod ipld {
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct IpldOpen {
        /// TODO could be more efficient to align id, size, codec, depending on padding.
        pub id: u32,
        pub codec: u64,
        pub size: u32,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct IpldStat {
        pub codec: u64,
        pub size: u32,
    }
}

pub mod send {
    use crate::sys::BlockId;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct Send {
        pub exit_code: u32,
        pub return_id: BlockId,
    }
}

pub mod crypto {
    use crate::{ActorID, ChainEpoch};

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct VerifyConsensusFault {
        pub fault: u32,
        pub epoch: ChainEpoch,
        pub target: ActorID,
    }
}
