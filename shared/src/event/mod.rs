use crate::ActorID;

/// Represents an event emitted throughout message execution.
struct Event {
    /// Carries the ID of the actor that emitted this event.
    emitter: ActorID,
    /// Key values making up this event.
    entries: Vec<Entry>,
}

/// Flags associated with an Event entry.
#[repr(transparent)]
struct Flags(u8);

/// Signals that an entry must be indexed.
pub const FLAG_INDEXED: u8 = 0x01;

/// A key value entry inside an Event.
struct Entry {
    /// A bitmap conveying metadata or hints about this entry.
    flags: Flags,
    /// The key of this event.
    key: String,
    /// Any DAG-CBOR encodeable type.
    value: Vec<u8>,
}
