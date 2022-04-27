use std::fmt::{Debug, Display, Formatter};

use anyhow::anyhow;
use bimap::BiBTreeMap;
use cid::CidGeneric;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use num_derive::FromPrimitive;
use serde_repr::{Deserialize_repr, Serialize_repr};

/// Identifies the builtin actor types for usage with the
/// actor::resolve_builtin_actor_type syscall.
#[derive(
    PartialEq,
    Eq,
    Clone,
    Copy,
    PartialOrd,
    Ord,
    FromPrimitive,
    Debug,
    Deserialize_repr,
    Hash,
    Serialize_repr,
)]
#[repr(i32)]
pub enum Type {
    System = 1,
    Init = 2,
    Cron = 3,
    Account = 4,
    Power = 5,
    Miner = 6,
    Market = 7,
    PaymentChannel = 8,
    Multisig = 9,
    Reward = 10,
    VerifiedRegistry = 11,
}

impl Type {
    /// Returns true if the actor kind represents a singleton actor. That is, an actor
    /// that cannot be constructed by a user.
    pub fn is_singleton_actor(&self) -> bool {
        self == &Type::System
            || self == &Type::Init
            || self == &Type::Reward
            || self == &Type::Cron
            || self == &Type::Power
            || self == &Type::Market
            || self == &Type::VerifiedRegistry
    }

    /// Returns true if the code belongs to an account actor.
    pub fn is_account_actor(&self) -> bool {
        self == &Type::Account
    }

    /// Tests whether an actor type represents an actor that can be an external
    /// principal: i.e. an account or multisig.
    pub fn is_principal(&self) -> bool {
        self == &Type::Account || self == &Type::Multisig
    }
}

pub const CALLER_TYPES_SIGNABLE: &[Type] = &[Type::Account, Type::Multisig];

impl TryFrom<&str> for Type {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let ret = match value {
            "system" => Type::System,
            "init" => Type::Init,
            "cron" => Type::Cron,
            "account" => Type::Account,
            "storagepower" => Type::Power,
            "storageminer" => Type::Miner,
            "storagemarket" => Type::Market,
            "paymentchannel" => Type::PaymentChannel,
            "multisig" => Type::Multisig,
            "reward" => Type::Reward,
            "verifiedregistry" => Type::VerifiedRegistry,
            _ => return Err(String::from("unrecognized actor type")),
        };
        Ok(ret)
    }
}

impl From<&Type> for String {
    fn from(t: &Type) -> String {
        match t {
            Type::System => "system",
            Type::Init => "init",
            Type::Cron => "cron",
            Type::Account => "account",
            Type::Power => "storagepower",
            Type::Miner => "storageminer",
            Type::Market => "storagemarket",
            Type::PaymentChannel => "paymentchannel",
            Type::Multisig => "multisig",
            Type::Reward => "reward",
            Type::VerifiedRegistry => "verifiedregistry",
        }
        .to_string()
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

/// A mapping of builtin actor CIDs to their respective types.
// Currently a default size of `64` is used for the Multihashes to match the size of the default
// code table of the `multihash` crate. When a custom code table is used, this could be reduced
// to `32`.
pub type Manifest<const S: usize = 64> = BiBTreeMap<CidGeneric<S>, Type>;

pub fn load_manifest<B: Blockstore<S>, const S: usize>(
    bs: &B,
    root_cid: &CidGeneric<S>,
    ver: u32,
) -> anyhow::Result<Manifest<S>> {
    match ver {
        0 => load_manifest_v0(bs, root_cid),
        1 => load_manifest_v1(bs, root_cid),
        _ => Err(anyhow!("unknown manifest version {}", ver)),
    }
}

pub fn load_manifest_v0<B: Blockstore<S>, const S: usize>(
    bs: &B,
    root_cid: &CidGeneric<S>,
) -> anyhow::Result<Manifest<S>> {
    match bs.get_cbor::<Manifest<S>>(root_cid)? {
        Some(mf) => Ok(mf),
        None => Err(anyhow!("cannot find manifest root cid {}", root_cid)),
    }
}

pub fn load_manifest_v1<B: Blockstore<S>, const S: usize>(
    bs: &B,
    root_cid: &CidGeneric<S>,
) -> anyhow::Result<Manifest<S>> {
    let vec: Vec<(String, CidGeneric<S>)> = match bs.get_cbor(root_cid)? {
        Some(vec) => vec,
        None => {
            return Err(anyhow!("cannot find manifest root cid {}", root_cid));
        }
    };
    let mut manifest = Manifest::new();
    for (name, code_cid) in vec {
        let t = Type::try_from(name.as_str());
        match t {
            Ok(t) => {
                manifest.insert(code_cid, t);
            }
            Err(what) => {
                return Err(anyhow!("bad builtin actor name: {}: {} ", name, what));
            }
        }
    }
    Ok(manifest)
}
