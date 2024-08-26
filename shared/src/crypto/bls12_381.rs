use blstrs;

pub const G1_SIZE: usize = 96;
pub type G1Uncompressed = [u8; G1_SIZE];

#[cfg(feature = "crypto")]
pub mod ops {
    use crate::crypto::bls12_381::G1Uncompressed;

    pub fn g1_add(p1: &G1Uncompressed, p2: &G1Uncompressed) -> Result<G1Uncompressed, String> {
        // Perform a point add using the blstrs library.
        let p1 = blstrs::G1Affine::from_uncompressed(p1).unwrap(); // XXX errors
        let p1 = blstrs::G1Projective::from(&p1);
        let p2 = blstrs::G1Affine::from_uncompressed(p2).unwrap();
        let p2 = blstrs::G1Projective::from(&p2);
        let sum = p1 + p2;
        Ok(sum.to_uncompressed())
    }
}

