use num_derive::FromPrimitive;

use super::{Address, ChainEpoch};

/// Result of checking two headers for a consensus fault.
#[derive(Clone, Debug)]
pub struct ConsensusFault {
    /// Address of the miner at fault (always an ID address).
    pub target: Address,
    /// Epoch of the fault, which is the higher epoch of the two blocks causing it.
    pub epoch: ChainEpoch,
    /// Type of fault.
    pub fault_type: ConsensusFaultType,
}

/// Result of checking two headers for a consensus fault, with the gas used
/// Only for v14 and earlier
#[derive(Clone, Debug)]
pub struct ConsensusFaultWithGas {
    /// The fault.
    pub fault: Option<ConsensusFault>,
    /// Gas used in checking the fault
    pub gas_used: i64,
}

/// Consensus fault types in VM.
#[derive(FromPrimitive, Clone, Copy, Debug)]
#[repr(u8)]
pub enum ConsensusFaultType {
    DoubleForkMining = 1,
    ParentGrinding = 2,
    TimeOffsetMining = 3,
}
