pub use self_destruct_chain::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod self_destruct_chain {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "SelfDestructChain was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"stateMutability\":\"payable\",\"type\":\"constructor\"},{\"inputs\":[{\"internalType\":\"address[]\",\"name\":\"_addresses\",\"type\":\"address[]\"},{\"internalType\":\"uint32\",\"name\":\"_curr_depth\",\"type\":\"uint32\"}],\"name\":\"destroy\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static SELFDESTRUCTCHAIN_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct SelfDestructChain<M>(ethers::contract::Contract<M>);
    impl<M> Clone for SelfDestructChain<M> {
        fn clone(&self) -> Self {
            SelfDestructChain(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for SelfDestructChain<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for SelfDestructChain<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(SelfDestructChain))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> SelfDestructChain<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), SELFDESTRUCTCHAIN_ABI.clone(), client)
                .into()
        }
        #[doc = "Calls the contract's `destroy` (0x9240b9db) function"]
        pub fn destroy(
            &self,
            addresses: ::std::vec::Vec<ethers::core::types::Address>,
            curr_depth: u32,
        ) -> ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([146, 64, 185, 219], (addresses, curr_depth))
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>>
        for SelfDestructChain<M>
    {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `destroy` function with signature `destroy(address[],uint32)` and selector `[146, 64, 185, 219]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "destroy", abi = "destroy(address[],uint32)")]
    pub struct DestroyCall {
        pub addresses: ::std::vec::Vec<ethers::core::types::Address>,
        pub curr_depth: u32,
    }
}
