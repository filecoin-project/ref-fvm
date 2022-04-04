//! This module contains syscall output data carrier structs, shared between
//! the FVM SDK and the FVM itself, wrapping multi-value returns.
//!
//! These are necessary because Rust WASM multi-value return compilation is
//! plagued with issues and catch-22 problems, making it unfeasible to use
//! actual bare multi-value returns in FFI extern definitions.
//!
//! Read more at https://github.com/rust-lang/rust/issues/73755.

pub mod actor {
    #[derive(Debug, Copy, Clone)]
    #[repr(packed, C)]
    pub struct ResolveAddress {
        pub resolved: i32,
        pub value: u64,
    }
}

pub mod ipld {
    #[derive(Debug, Copy, Clone)]
    #[repr(packed, C)]
    pub struct IpldOpen {
        pub codec: u64,
        pub id: u32,
        pub size: u32,
    }

    #[derive(Debug, Copy, Clone)]
    #[repr(packed, C)]
    pub struct IpldStat {
        pub codec: u64,
        pub size: u32,
    }
}

pub mod send {
    use crate::sys::BlockId;

    #[derive(Debug, Copy, Clone)]
    #[repr(packed, C)]
    pub struct Send {
        pub exit_code: u32,
        pub return_id: BlockId,
    }
}

pub mod crypto {
    use crate::{ActorID, ChainEpoch};

    #[derive(Debug, Copy, Clone)]
    #[repr(packed, C)]
    pub struct VerifyConsensusFault {
        pub epoch: ChainEpoch,
        pub target: ActorID,
        pub fault: u32,
    }
}
