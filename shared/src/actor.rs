pub mod builtin {
    use num_derive::FromPrimitive;
    use serde_repr::Deserialize_repr;

    /// Identifies the builtin actor types for usage with the
    /// actor::is_builtin_actor syscall.
    #[derive(
        PartialEq, Eq, Clone, Copy, PartialOrd, Ord, FromPrimitive, Debug, Deserialize_repr, Hash,
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
}
