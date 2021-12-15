#[link(wasm_import_module = "validation")]
extern "C" {
    /// Signals that this actor accepts calls from any other actor.
    pub fn validate_immediate_caller_accept_any();

    /// Validates that the call being processed originated at one
    /// of the listed addresses.
    ///
    /// The list of addreses is provided as a CBOR encoded list.
    pub fn validate_immediate_caller_addr_one_of(addrs_offset: *const u8, addrs_len: u32);

    /// Validates that the call being processed originated at an
    /// actor of one of the specified types.
    ///
    /// The list of CIDs is provided as a CBOR encoded list.
    pub fn validate_immediate_caller_type_one_of(cids_offset: *const u8, cids_len: u32);
}
