use super::{Address, ChainEpoch};
use crate::encoding::{repr::*, tuple::*, Cbor};

/// Result of checking two headers for a consensus fault.
#[derive(Clone, Serialize_tuple, Deserialize_tuple)]
pub struct ConsensusFault {
    /// Address of the miner at fault (always an ID address).
    pub target: Address,
    /// Epoch of the fault, which is the higher epoch of the two blocks causing it.
    pub epoch: ChainEpoch,
    /// Type of fault.
    pub fault_type: ConsensusFaultType,
}

/// Consensus fault types in VM.
#[derive(Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ConsensusFaultType {
    DoubleForkMining = 1,
    ParentGrinding = 2,
    TimeOffsetMining = 3,
}

// For syscall marshalling.
impl Cbor for ConsensusFault {}
