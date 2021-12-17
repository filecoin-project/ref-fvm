//! This module contains the minimal logic for the FVM to handle account actor
//! auto-creation (on first transfer). This coupling between the FVM and a
//! concrete actor must eventually go. (TODO)

use cid::{multihash::Code, multihash::MultihashDigest, Cid};
use lazy_static::lazy_static;

use fvm_shared::address::Address;
use fvm_shared::bigint::Zero;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{to_vec, tuple::*, Cbor, DAG_CBOR};

use crate::state_tree::ActorState;

// TODO: This shouldn't be defined here.
const IPLD_RAW: u64 = 0x55;

pub const SYSTEM_ACTOR_ID: u64 = 0;
pub const SYSTEM_ACTOR_ADDR: Address = Address::new_id(SYSTEM_ACTOR_ID);
lazy_static!(
    // TODO this may need to be versioned with SnapDeals; and maybe a few more
    //  times before account actors are moved to user-land.
    pub static ref ACCOUNT_ACTOR_CODE_ID: Cid = {
        Cid::new_v1(IPLD_RAW, Code::Identity.digest(b"fil/5/account"))
    };

    /// Cid of the empty array Cbor bytes (`EMPTY_ARR_BYTES`).
    pub static ref EMPTY_ARR_CID: Cid = {
        let empty = to_vec::<[(); 0]>(&[]).unwrap();
        Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty))
    };

    pub static ref ZERO_STATE: ActorState = ActorState {
        code: Cid::new_v1(IPLD_RAW, Code::Identity.digest(b"fil/5/account")),
        state: *EMPTY_ARR_CID,
        sequence: 0,
        balance: TokenAmount::zero(),
    };
);

/// State specifies the key address for the actor.
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address: Address,
}

impl Cbor for State {}

/// Returns true if the code belongs to an account actor.
pub fn is_account_actor(code: &Cid) -> bool {
    code == &*ACCOUNT_ACTOR_CODE_ID
}
