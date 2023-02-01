pub use factory_interface::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod factory_interface {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "FactoryInterface was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"name\":\"getInitializationCode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"initializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"view\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static FACTORYINTERFACE_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct FactoryInterface<M>(ethers::contract::Contract<M>);
    impl<M> Clone for FactoryInterface<M> {
        fn clone(&self) -> Self {
            FactoryInterface(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for FactoryInterface<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for FactoryInterface<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(FactoryInterface))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> FactoryInterface<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), FACTORYINTERFACE_ABI.clone(), client)
                .into()
        }
        #[doc = "Calls the contract's `getInitializationCode` (0x57b9f523) function"]
        pub fn get_initialization_code(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([87, 185, 245, 35], ())
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for FactoryInterface<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `getInitializationCode` function with signature `getInitializationCode()` and selector `[87, 185, 245, 35]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getInitializationCode", abi = "getInitializationCode()")]
    pub struct GetInitializationCodeCall;
    #[doc = "Container type for all return fields from the `getInitializationCode` function with signature `getInitializationCode()` and selector `[87, 185, 245, 35]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetInitializationCodeReturn {
        pub initialization_code: ethers::core::types::Bytes,
    }
}
