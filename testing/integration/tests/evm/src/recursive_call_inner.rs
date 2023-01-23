pub use recursive_call_inner::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod recursive_call_inner {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "RecursiveCallInner was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"name\":\"depth\",\"outputs\":[{\"internalType\":\"uint32\",\"name\":\"\",\"type\":\"uint32\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address[]\",\"name\":\"addresses\",\"type\":\"address[]\"},{\"internalType\":\"uint32\",\"name\":\"max_depth\",\"type\":\"uint32\"},{\"internalType\":\"uint32\",\"name\":\"curr_depth\",\"type\":\"uint32\"}],\"name\":\"recurse\",\"outputs\":[],\"stateMutability\":\"payable\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"sender\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"value\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static RECURSIVECALLINNER_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct RecursiveCallInner<M>(ethers::contract::Contract<M>);
    impl<M> Clone for RecursiveCallInner<M> {
        fn clone(&self) -> Self {
            RecursiveCallInner(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for RecursiveCallInner<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for RecursiveCallInner<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(RecursiveCallInner))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> RecursiveCallInner<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), RECURSIVECALLINNER_ABI.clone(), client)
                .into()
        }
        #[doc = "Calls the contract's `depth` (0x631c56ef) function"]
        pub fn depth(&self) -> ethers::contract::builders::ContractCall<M, u32> {
            self.0
                .method_hash([99, 28, 86, 239], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `recurse` (0xbb878333) function"]
        pub fn recurse(
            &self,
            addresses: ::std::vec::Vec<ethers::core::types::Address>,
            max_depth: u32,
            curr_depth: u32,
        ) -> ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([187, 135, 131, 51], (addresses, max_depth, curr_depth))
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `sender` (0x67e404ce) function"]
        pub fn sender(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([103, 228, 4, 206], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `value` (0x3fa4f245) function"]
        pub fn value(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::U256> {
            self.0
                .method_hash([63, 164, 242, 69], ())
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>>
        for RecursiveCallInner<M>
    {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `depth` function with signature `depth()` and selector `[99, 28, 86, 239]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "depth", abi = "depth()")]
    pub struct DepthCall;
    #[doc = "Container type for all input parameters for the `recurse` function with signature `recurse(address[],uint32,uint32)` and selector `[187, 135, 131, 51]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "recurse", abi = "recurse(address[],uint32,uint32)")]
    pub struct RecurseCall {
        pub addresses: ::std::vec::Vec<ethers::core::types::Address>,
        pub max_depth: u32,
        pub curr_depth: u32,
    }
    #[doc = "Container type for all input parameters for the `sender` function with signature `sender()` and selector `[103, 228, 4, 206]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "sender", abi = "sender()")]
    pub struct SenderCall;
    #[doc = "Container type for all input parameters for the `value` function with signature `value()` and selector `[63, 164, 242, 69]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "value", abi = "value()")]
    pub struct ValueCall;
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum RecursiveCallInnerCalls {
        Depth(DepthCall),
        Recurse(RecurseCall),
        Sender(SenderCall),
        Value(ValueCall),
    }
    impl ethers::core::abi::AbiDecode for RecursiveCallInnerCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::std::result::Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) = <DepthCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(RecursiveCallInnerCalls::Depth(decoded));
            }
            if let Ok(decoded) =
                <RecurseCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(RecursiveCallInnerCalls::Recurse(decoded));
            }
            if let Ok(decoded) = <SenderCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(RecursiveCallInnerCalls::Sender(decoded));
            }
            if let Ok(decoded) = <ValueCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(RecursiveCallInnerCalls::Value(decoded));
            }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for RecursiveCallInnerCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                RecursiveCallInnerCalls::Depth(element) => element.encode(),
                RecursiveCallInnerCalls::Recurse(element) => element.encode(),
                RecursiveCallInnerCalls::Sender(element) => element.encode(),
                RecursiveCallInnerCalls::Value(element) => element.encode(),
            }
        }
    }
    impl ::std::fmt::Display for RecursiveCallInnerCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                RecursiveCallInnerCalls::Depth(element) => element.fmt(f),
                RecursiveCallInnerCalls::Recurse(element) => element.fmt(f),
                RecursiveCallInnerCalls::Sender(element) => element.fmt(f),
                RecursiveCallInnerCalls::Value(element) => element.fmt(f),
            }
        }
    }
    impl ::std::convert::From<DepthCall> for RecursiveCallInnerCalls {
        fn from(var: DepthCall) -> Self {
            RecursiveCallInnerCalls::Depth(var)
        }
    }
    impl ::std::convert::From<RecurseCall> for RecursiveCallInnerCalls {
        fn from(var: RecurseCall) -> Self {
            RecursiveCallInnerCalls::Recurse(var)
        }
    }
    impl ::std::convert::From<SenderCall> for RecursiveCallInnerCalls {
        fn from(var: SenderCall) -> Self {
            RecursiveCallInnerCalls::Sender(var)
        }
    }
    impl ::std::convert::From<ValueCall> for RecursiveCallInnerCalls {
        fn from(var: ValueCall) -> Self {
            RecursiveCallInnerCalls::Value(var)
        }
    }
    #[doc = "Container type for all return fields from the `depth` function with signature `depth()` and selector `[99, 28, 86, 239]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct DepthReturn(pub u32);
    #[doc = "Container type for all return fields from the `sender` function with signature `sender()` and selector `[103, 228, 4, 206]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct SenderReturn(pub ethers::core::types::Address);
    #[doc = "Container type for all return fields from the `value` function with signature `value()` and selector `[63, 164, 242, 69]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct ValueReturn(pub ethers::core::types::U256);
}
