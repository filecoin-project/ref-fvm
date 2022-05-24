#![no_main]

use common_fuzz::cbor::Payload;
use fvm_ipld_encoding as encoding;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let p = encoding::from_slice::<Payload>(data);
    if p.is_err() {
        return;
    }
    let p = p.unwrap();
    if p.serde_bytes_bytes.len() > 128 << 20 {
        panic!("too large array {}", p.serde_bytes_bytes.len())
    }

    let out = encoding::to_vec(&p).expect("decoded payload must be possible to encode");

    let p2 = encoding::from_slice::<Payload>(&out).expect("everything that encodes must decode");
    let out2 = encoding::to_vec(&p2).expect("decoded payload must be possible to encode2");
    if !out.eq(&out2) {
        panic!("repeated encodings must be stable");
    }
});
