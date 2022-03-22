use bimap::BiBTreeMap;
use cid::Cid;
use num_derive::FromPrimitive;
use serde_repr::{Deserialize_repr, Serialize_repr};
use anyhow::anyhow;

use crate::blockstore::{Blockstore, CborStore};

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
        }.to_string()
    }
}

/// A mapping of builtin actor CIDs to their respective types.
pub type Manifest = BiBTreeMap<Cid, Type>;

pub fn load_manifest<B: Blockstore>(bs: &B, root_cid: &Cid) -> anyhow::Result<Manifest> {
    let vec: Vec<(String, Cid)> = match bs.get_cbor(root_cid)? {
        Some(vec) => vec,
        None => {
            return Err(anyhow!("cannot find manifest root cid"));
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
                return Err(anyhow!(what));
            }
        }
    }
    Ok(manifest)
}
