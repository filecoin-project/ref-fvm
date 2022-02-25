pub mod builtin {
    use num_derive::FromPrimitive;
    use serde_repr::{Deserialize_repr, Serialize_repr};

    /// Identifies the builtin actor types for usage with the
    /// actor::is_builtin_actor syscall.
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
}
