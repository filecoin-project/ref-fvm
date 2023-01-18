use ethers::types::{H160, H256, U256};

pub fn decode_address(raw_address: U256) -> H160 {
    let mut bytes = [0; 32];
    raw_address.to_big_endian(&mut bytes);
    H160::from_slice(&bytes[12..])
}

pub fn U256_to_H256(val: U256) -> H256 {
    let mut bytes = [0; 32];
    val.to_big_endian(&mut bytes);
    H256::from_slice(&bytes)
}

pub fn H256_to_U256(val: H256) -> U256 {
    U256::from_big_endian(val.as_bytes())
}
