use super::Hash;

pub type HashedKey = [u8; 32];

/// Algorithm used as the hasher for the Hamt.
pub trait HashAlgorithm {
    fn rt_hash(&self, key: &dyn Hash) -> HashedKey;
}
