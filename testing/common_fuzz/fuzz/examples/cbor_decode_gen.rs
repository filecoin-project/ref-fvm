use std::error::Error;
use std::fs::File;
use std::path::Path;

use arbitrary::Unstructured;
use common_fuzz::cbor::Payload;
use fvm_ipld_encoding as encoding;
use rand::rngs::ThreadRng;
use rand::RngCore;

fn main() -> Result<(), Box<dyn Error>> {
    let mut rng = ThreadRng::default();
    let mut data = vec![0; 4096];
    for i in 0..300 {
        rng.fill_bytes(data.as_mut_slice());
        let p: Payload = Unstructured::new(&data).arbitrary()?;
        dump(i, &p)?;
    }
    Ok(())
}

fn dump(i: u32, p: &Payload) -> Result<(), Box<dyn Error>> {
    let f = File::create(Path::new("corpus/cbor_encode/").join(format!("{:03}", i)))?;
    encoding::to_writer(f, p)?;
    Ok(())
}
