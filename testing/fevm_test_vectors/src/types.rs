use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, Cbor};
use serde::{Deserialize, Serialize};

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct CreateParams {
    #[serde(with = "strict_bytes")]
    pub initcode: Vec<u8>,
    pub nonce: u64,
}

impl Cbor for CreateParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractParams(#[serde(with = "strict_bytes")] pub Vec<u8>);

impl Cbor for ContractParams {}
