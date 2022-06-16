use fvm_sdk::sys::crypto::compute_unsealed_sector_cid;

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    let piece: Vec<u8> = vec![];
    let mut cid: Vec<u8> = vec![];

    // Should fail for unknown proof type
    unsafe {
        compute_unsealed_sector_cid(100000, piece.as_ptr(), 100000, cid.as_mut_ptr(), 100000)
            .expect("");
    }

    0
}
