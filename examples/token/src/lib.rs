//! This is an extremely rough-and-ugly implementation of an ERC20-like native
//! token actor for the Filecoin network. It uses no SDK sugar (because none
//! exists yet), neither for syscalls nor or error handling.
//!
//! ## Supported features
//!
//! The current code supports token symbols, names, maximum supplies, and
//! simple transfers. It DOES NOT YET support authorizations for delegate
//! spending.
//!
//! ## On developer experience
//!
//! As stated above, this is code is still ugly. The developer experience will
//! improve dramatically as the FVM project advances towards Milestone 2.
//!
//! The author deliberately didn't use this opportunity to improve the SDK.
//! Instead, he chose to implement things low-level to create a baseline of
//! where we are. As we introduce better DX, this actor will evolve and benefit.
//!
//! NOTE: There is _some_ pre-existing sugar in the actor/runtime module
//! that could facilitate things slightly easier here, but this actor does not
//! use it to avoid dragging in unnecessary dependencies (related to built-in
//! actors). The upcoming sugar will eventually be provided by the Rust SDK,
//! and not the built-in actors toolkit.
//!
//! ## Boilerplate
//!
//! The following responsiblities are taken care of here, but they are
//! absolutely boilerplate, and should eventually be superseded by clever sugar
//! in the form of proc macro attributes.
//!
//! - State loading.
//! - State mutations.
//! - Method dispatch.
//! - Deserialization of parameters.
//! - Serialization of return data.
//!
//! ## Addressing
//!
//! For simplicity, this actor uses ActorIDs in the balances and allowances
//! HAMT (map) keys. This poses two problems:
//!
//! 1. A sender cannot send tokens to an inexistent account actor.
//! 2. This actor is not reorg-safe (ActorIDs can change in the chain gets
//!    reorg-ed).
//!
//! We _could_ do sophisticated things here to solve for these problems, by
//! associating balances to class-{1,2,3} addresses only, and rejecting calls
//! with class-0 (ID addresses).
//!
//! However, all of this will change when we introduce [class-4 addresses](https://github.com/filecoin-project/fvm-specs/blob/main/04-evm-mapping.md#proposed-solution-universal-stable-addresses)
//! (reorg stable universal addresses), so it's not worth the complexity now,
//! as this actor only exists for illustration purposes.
//!
//! ## NOT MEANT FOR PRODUCTION USAGE
//!
//! This actor should NEVER be deployed on the Filecoin network. It is purely
//! meant for illustration and test purposes.
//!

use cid::multihash::Code;
use cid::Cid;
use fvm_sdk as sdk;
use fvm_sdk::blockstore::Blockstore;
use fvm_sdk::message::{params_cbor, NO_DATA_BLOCK_ID};
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser::{BigIntDe, BigIntSer};
use fvm_shared::bigint::{bigint_ser, Zero};
use fvm_shared::blockstore::CborStore;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::{Cbor, RawBytes, DAG_CBOR};
use fvm_shared::ActorID;
use ipld_hamt::Hamt;

/// A macro to abort execution by signalling a non-zero exit code.
macro_rules! abort {
    ($code:ident, $msg:literal $(, $ex:expr)*) => {
        fvm_sdk::vm::abort(
            fvm_shared::error::ExitCode::$code as u32,
            Some(format!($msg, $($ex,)*).as_str()),
        )
    };
}

/// A macro to CBOR-encode a return value.
macro_rules! encode {
    ($ex:expr) => {
        match RawBytes::serialize($ex) {
            Ok(ret) => ret,
            Err(err) => {
                abort!(ErrSerialization, "failed to encode return value: {:?}", err)
            }
        }
    };
}

/// The state object of the token actor.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug)]
pub struct State {
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    #[serde(with = "bigint_ser")]
    pub max_supply: TokenAmount,
    // map[ActorID] => TokenAmount
    pub balances: Cid,
    // TODO should never use ActorID as allowees, since it's an unstable address
    //  make sure to check for this
    //  map[ActorID] => map[Address]TokenAmount
    // pub allowances: Cid,
}

/// The actor's WASM entrypoint. It takes the ID of the parameters block,
/// and returns the ID of the return value block.
#[no_mangle]
pub fn invoke(params_id: u32) -> u32 {
    // First, load the current state root.
    let root = match sdk::sself::root() {
        Ok(root) => root,
        Err(err) => abort!(ErrIllegalState, "failed to get root: {}", err),
    };

    // Load the actor state from the state tree.
    let state = match Blockstore.get_cbor::<State>(&root) {
        Ok(Some(state)) => state,
        Ok(None) => abort!(ErrIllegalState, "state does not exist"),
        Err(err) => abort!(ErrIllegalState, "failed to get state: {}", err),
    };

    // Conduct method dispatch. Handle input parameters and return data.
    let ret: Option<RawBytes> = match sdk::message::method_number() {
        1 => Some(name(state)),
        2 => Some(symbol(state)),
        3 => Some(max_supply(state)),
        4 => {
            let params: TransferParams = match params_cbor(params_id) {
                Ok(params) => params,
                Err(err) => abort!(ErrIllegalArgument, "failed to parse params: {:?}", err),
            };
            transfer(state, params);
            None
        }
        5 => {
            let addr: Address = match params_cbor(params_id) {
                Ok(addr) => addr,
                Err(err) => abort!(ErrIllegalArgument, "failed to parse address: {:?}", err),
            };
            Some(balance_of(state, addr))
        }

        // TODO this exit code should move to actor space given that dispatch
        //  is now conducted in-actor.
        _ => abort!(SysErrInvalidMethod, "unrecognized method"),
    };

    // Insert the return data block if necessary, and return the correct
    // block ID.
    match ret {
        None => NO_DATA_BLOCK_ID,
        Some(v) => match sdk::ipld::put_block(DAG_CBOR, v.bytes()) {
            Ok(id) => id,
            Err(err) => abort!(ErrSerialization, "failed to store return value: {}", err),
        },
    }
}

/// Returns the token name. Method number 1.
#[inline]
pub fn name(state: State) -> RawBytes {
    match String::from_utf8(state.name) {
        Ok(s) => encode!(s.into_bytes()),
        Err(err) => abort!(ErrIllegalState, "could not parse name as UTF-8: {:?}", err),
    }
}

/// Returns the symbol. Method number 2.
#[inline]
pub fn symbol(state: State) -> RawBytes {
    match String::from_utf8(state.symbol) {
        Ok(s) => encode!(s.into_bytes()),
        Err(err) => abort!(ErrIllegalState, "could not parse name as UTF-8: {:?}", err),
    }
}

/// Returns the maximum supply. Method number 3.
#[inline]
pub fn max_supply(state: State) -> RawBytes {
    encode!(BigIntSer(&state.max_supply))
}

/// The input parameters for a transfer.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug)]
pub struct TransferParams {
    pub recipient: Address,
    #[serde(with = "bigint_ser")]
    pub amount: TokenAmount,
}

impl Cbor for TransferParams {}

/// Transfer a token amount.
pub fn transfer(mut state: State, params: TransferParams) {
    // Load the balances HAMT.
    // TODO Using BitIntDe because it's both Ser and De; this is a misnomer and
    //  we should fix it.
    let mut balances =
        match Hamt::<Blockstore, BigIntDe, ActorID>::load(&state.balances, Blockstore) {
            Ok(map) => map,
            Err(err) => abort!(ErrIllegalState, "failed to load balances hamt: {:?}", err),
        };

    // Load the sender's balance.
    let sender_id = fvm_sdk::message::caller();
    let mut sender_bal = match balances.get(&sender_id) {
        Ok(Some(bal)) => bal.clone(),
        Ok(None) => BigIntDe(TokenAmount::zero()),
        Err(err) => abort!(ErrIllegalState, "failed to get balance: {:?}", err),
    };

    // Sender has insufficient balance.
    if sender_bal.0 < params.amount {
        abort!(ErrInsufficientFunds, "sender has insufficient balance")
    }

    // Resolve the recipient into an ID address.
    // TODO See addressing section on module docs.
    let recipient_id = match fvm_sdk::actor::resolve_address(&params.recipient) {
        Some(id) => id,
        None => abort!(ErrIllegalArgument, "failed to resolve address"),
    };

    // Load the recipient's balance.
    let mut recipient_bal = match balances.get(&recipient_id) {
        Ok(Some(bal)) => bal.clone(),
        Ok(None) => BigIntDe(TokenAmount::zero()),
        Err(err) => abort!(
            ErrIllegalState,
            "failed to query hamt when getting recipient balance: {:?}",
            err
        ),
    };

    // Update balances.
    sender_bal.0 -= &params.amount;
    recipient_bal.0 += &params.amount;

    // Set the updated sender balance in the balances HAMT.
    if let Err(err) = balances.set(sender_id, sender_bal.clone()) {
        abort!(
            ErrIllegalState,
            "failed to set new sender balance in balances hamt: {:?}",
            err
        )
    }

    // Set the updated recipient balance in the balances HAMT.
    if let Err(err) = balances.set(recipient_id, recipient_bal.clone()) {
        abort!(
            ErrIllegalState,
            "failed to set new recipient balance in balances hamt: {:?}",
            err
        )
    }

    // Flush the HAMT to generate the new root CID to update the actor's state.
    let cid = match balances.flush() {
        Ok(cid) => cid,
        Err(err) => abort!(
            ErrIllegalState,
            "failed to query hamt when getting recipient balance: {:?}",
            err
        ),
    };

    // Update the actor's state.
    state.balances = cid;
    let root = match Blockstore.put_cbor(&state, Code::Blake2b256) {
        Ok(cid) => cid,
        Err(err) => abort!(ErrIllegalState, "failed to write new state: {:?}", err),
    };

    if let Err(err) = fvm_sdk::sself::set_root(&root) {
        abort!(ErrIllegalState, "failed to set new state root: {:?}", err)
    }
}

/// Gets the token balance of the supplied actor.
pub fn balance_of(state: State, addr: Address) -> RawBytes {
    // Load the balances HAMT.
    let balances = match Hamt::<Blockstore, BigIntDe, ActorID>::load(&state.balances, Blockstore) {
        Ok(map) => map,
        Err(err) => abort!(ErrIllegalState, "failed to load balances hamt: {:?}", err),
    };

    // Resolve the queried address.
    let addr_id = match fvm_sdk::actor::resolve_address(&addr) {
        Some(id) => id,
        None => return encode!(BigIntDe(TokenAmount::zero())),
    };

    // Get the balance.
    let balance = match balances.get(&addr_id) {
        Ok(Some(bal)) => bal.clone(),
        Ok(None) => BigIntDe(TokenAmount::zero()),
        Err(err) => abort!(ErrIllegalState, "failed to get balance: {:?}", err),
    };

    encode!(balance)
}
