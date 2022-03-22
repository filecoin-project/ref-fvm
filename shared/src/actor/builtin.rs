use bimap::BiBTreeMap;
use cid::Cid;
use num_derive::FromPrimitive;
use serde_repr::{Deserialize_repr, Serialize_repr};

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
            Type::System => String::from("system"),
            Type::Init => String::from("init"),
            Type::Cron => String::from("cron"),
            Type::Account => String::from("account"),
            Type::Power => String::from("storagepower"),
            Type::Miner => String::from("storageminer"),
            Type::Market => String::from("storagemarket"),
            Type::PaymentChannel => String::from("paymentchannel"),
            Type::Multisig => String::from("multisig"),
            Type::Reward => String::from("reward"),
            Type::VerifiedRegistry => String::from("verifiedregistry"),
        }
    }
}

/// A mapping of builtin actor CIDs to their respective types.
pub type Manifest = BiBTreeMap<Cid, Type>;

pub fn load_manifest<B: Blockstore>(bs: &B, root_cid: &Cid) -> Result<Manifest, String> {
    let vec: Vec<(String, Cid)> = match bs.get_cbor(root_cid) {
        Ok(Some(vec)) => vec,
        Ok(None) => {
            return Err("cannot find manifest root cid".to_string());
        }
        Err(what) => {
            return Err(what.to_string());
        }
    };
    let mut manifest = Manifest::new();
    for (name, code_cid) in vec {
        let t = Type::try_from(name.as_str())?;
        manifest.insert(code_cid, t);
    }
    Ok(manifest)
}
