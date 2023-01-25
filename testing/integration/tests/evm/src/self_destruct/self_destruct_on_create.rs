pub use self_destruct_on_create::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod self_destruct_on_create {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "SelfDestructOnCreate was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_beneficiary\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"constructor\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static SELFDESTRUCTONCREATE_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct SelfDestructOnCreate<M>(ethers::contract::Contract<M>);
    impl<M> Clone for SelfDestructOnCreate<M> {
        fn clone(&self) -> Self {
            SelfDestructOnCreate(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for SelfDestructOnCreate<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for SelfDestructOnCreate<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(SelfDestructOnCreate))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> SelfDestructOnCreate<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(
                address.into(),
                SELFDESTRUCTONCREATE_ABI.clone(),
                client,
            )
            .into()
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>>
        for SelfDestructOnCreate<M>
    {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
}
