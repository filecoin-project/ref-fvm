use bitflags::bitflags;
use fvm_ipld_encoding::Cbor;
use serde::{Deserialize, Serialize};
use serde_tuple::*;

use crate::ActorID;

/// Event with extra information stamped by the FVM. This is the structure that gets committed
/// on-chain via the receipt.
#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
pub struct StampedEvent {
    /// Carries the ID of the actor that emitted this event.
    emitter: ActorID,
    /// The event as emitted by the actor.
    event: ActorEvent,
}

impl Cbor for StampedEvent {}

impl StampedEvent {
    pub fn new(emitter: ActorID, event: ActorEvent) -> Self {
        Self { emitter, event }
    }
}

/// An event as originally emitted by the actor.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ActorEvent {
    pub entries: Vec<Entry>,
}

impl Cbor for ActorEvent {}

impl From<Vec<Entry>> for ActorEvent {
    fn from(entries: Vec<Entry>) -> Self {
        Self { entries }
    }
}

bitflags! {
    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct Flags: u8 {
        const FLAG_INDEXED_KEY      = 0b00000001;
        const FLAG_INDEXED_VALUE    = 0b00000010;
        const FLAG_INDEXED_ALL      = Self::FLAG_INDEXED_KEY.bits | Self::FLAG_INDEXED_VALUE.bits;
    }
}

// /// Flags associated with an Event entry.
// #[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Debug)]
// #[serde(transparent)]
// pub struct Flags(pub u8);

// /// Signals that an entry must be indexed.
// pub const FLAG_INDEXED: u8 = 0x01;

/// A key value entry inside an Event.
#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
pub struct Entry {
    /// A bitmap conveying metadata or hints about this entry.
    pub flags: Flags,
    /// The key of this event.
    pub key: String,
    /// Any DAG-CBOR encodeable type.
    pub value: Vec<u8>,
}

impl Cbor for Entry {}

// TODO write macro
// event!({
//     "foo" | indexed1 | indexed2 => value1,
//     "foo" => value2,
//     "foo" => value3,
// })
