pub use cocoon::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod cocoon {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "Cocoon was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"stateMutability\":\"payable\",\"type\":\"constructor\"},{\"inputs\":[],\"name\":\"description\",\"outputs\":[{\"internalType\":\"string\",\"name\":\"\",\"type\":\"string\"}],\"stateMutability\":\"pure\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"die\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static COCOON_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct Cocoon<M>(ethers::contract::Contract<M>);
    impl<M> Clone for Cocoon<M> {
        fn clone(&self) -> Self {
            Cocoon(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for Cocoon<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for Cocoon<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(Cocoon))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> Cocoon<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), COCOON_ABI.clone(), client).into()
        }
        #[doc = "Calls the contract's `description` (0x7284e416) function"]
        pub fn description(&self) -> ethers::contract::builders::ContractCall<M, String> {
            self.0
                .method_hash([114, 132, 228, 22], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `die` (0x35f46994) function"]
        pub fn die(&self) -> ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([53, 244, 105, 148], ())
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for Cocoon<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `description` function with signature `description()` and selector `[114, 132, 228, 22]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "description", abi = "description()")]
    pub struct DescriptionCall;
    #[doc = "Container type for all input parameters for the `die` function with signature `die()` and selector `[53, 244, 105, 148]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "die", abi = "die()")]
    pub struct DieCall;
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum CocoonCalls {
        Description(DescriptionCall),
        Die(DieCall),
    }
    impl ethers::core::abi::AbiDecode for CocoonCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::std::result::Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) =
                <DescriptionCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(CocoonCalls::Description(decoded));
            }
            if let Ok(decoded) = <DieCall as ethers::core::abi::AbiDecode>::decode(data.as_ref()) {
                return Ok(CocoonCalls::Die(decoded));
            }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for CocoonCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                CocoonCalls::Description(element) => element.encode(),
                CocoonCalls::Die(element) => element.encode(),
            }
        }
    }
    impl ::std::fmt::Display for CocoonCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                CocoonCalls::Description(element) => element.fmt(f),
                CocoonCalls::Die(element) => element.fmt(f),
            }
        }
    }
    impl ::std::convert::From<DescriptionCall> for CocoonCalls {
        fn from(var: DescriptionCall) -> Self {
            CocoonCalls::Description(var)
        }
    }
    impl ::std::convert::From<DieCall> for CocoonCalls {
        fn from(var: DieCall) -> Self {
            CocoonCalls::Die(var)
        }
    }
    #[doc = "Container type for all return fields from the `description` function with signature `description()` and selector `[114, 132, 228, 22]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct DescriptionReturn(pub String);
}
