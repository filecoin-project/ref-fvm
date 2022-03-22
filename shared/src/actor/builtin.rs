use bimap::BiBTreeMap;
use cid::Cid;
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

    /// Returns the canonical (versioned) actor name, as exists in the manifest
    pub fn name(&self) -> String {
        match self {
            Type::System => String::from("fil/v7/system"),
            Type::Init => String::from("fil/v7/init"),
            Type::Cron => String::from("fil/v7/cron"),
            Type::Account => String::from("fil/v7/account"),
            Type::Power => String::from("fil/v7/storagepower"),
            Type::Miner => String::from("fil/v7/storageminer"),
            Type::Market => String::from("fil/v7/storagemarket"),
            Type::PaymentChannel => String::from("fil/v7/paymentchannel"),
            Type::Multisig => String::from("fil/v7/multisig"),
            Type::Reward => String::from("fil/v7/reward"),
            Type::VerifiedRegistry => String::from("fil/v7/verifiedregistry"),
        }
    }

    /// Returns the actor type from the canonical (versioned) name
    pub fn from_name(s: &String) -> Result<Type, String> {
        match s.as_str() {
            "fil/v7/system" => Ok(Type::System),
            "fil/v7/init" => Ok(Type::Init),
            "fil/v7/cron" => Ok(Type::Cron),
            "fil/v7/account" => Ok(Type::Account),
            "fil/v7/storagepower" => Ok(Type::Power),
            "fil/v7/storageminer" => Ok(Type::Miner),
            "fil/v7/storagemarket" => Ok(Type::Market),
            "fil/v7/paymentchannel" => Ok(Type::PaymentChannel),
            "fil/v7/multisig" => Ok(Type::Multisig),
            "fil/v7/reward" => Ok(Type::Reward),
            "fil/v7/verifiedregistry" => Ok(Type::VerifiedRegistry),
            _ => Err("unknown actor name".to_string()),
        }
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

/// A mapping of builtin actor names to CIDs.
pub type Manifest = BiBTreeMap<String, Cid>;
