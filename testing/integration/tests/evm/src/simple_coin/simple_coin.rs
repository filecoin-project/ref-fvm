pub use simple_coin::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod simple_coin {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "SimpleCoin was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"_from\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"_to\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"Transfer\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"addr\",\"type\":\"address\"}],\"name\":\"getBalance\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"addr\",\"type\":\"address\"}],\"name\":\"getBalanceInEth\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"receiver\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"amount\",\"type\":\"uint256\"}],\"name\":\"sendCoin\",\"outputs\":[{\"internalType\":\"bool\",\"name\":\"sufficient\",\"type\":\"bool\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static SIMPLECOIN_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct SimpleCoin<M>(ethers::contract::Contract<M>);
    impl<M> Clone for SimpleCoin<M> {
        fn clone(&self) -> Self {
            SimpleCoin(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for SimpleCoin<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for SimpleCoin<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(SimpleCoin))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> SimpleCoin<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), SIMPLECOIN_ABI.clone(), client).into()
        }
        #[doc = "Calls the contract's `getBalance` (0xf8b2cb4f) function"]
        pub fn get_balance(
            &self,
            addr: ethers::core::types::Address,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::U256> {
            self.0
                .method_hash([248, 178, 203, 79], addr)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getBalanceInEth` (0x7bd703e8) function"]
        pub fn get_balance_in_eth(
            &self,
            addr: ethers::core::types::Address,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::U256> {
            self.0
                .method_hash([123, 215, 3, 232], addr)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `sendCoin` (0x90b98a11) function"]
        pub fn send_coin(
            &self,
            receiver: ethers::core::types::Address,
            amount: ethers::core::types::U256,
        ) -> ethers::contract::builders::ContractCall<M, bool> {
            self.0
                .method_hash([144, 185, 138, 17], (receiver, amount))
                .expect("method not found (this should never happen)")
        }
        #[doc = "Gets the contract's `Transfer` event"]
        pub fn transfer_filter(&self) -> ethers::contract::builders::Event<M, TransferFilter> {
            self.0.event()
        }
        #[doc = r" Returns an [`Event`](#ethers_contract::builders::Event) builder for all events of this contract"]
        pub fn events(&self) -> ethers::contract::builders::Event<M, TransferFilter> {
            self.0.event_with_filter(Default::default())
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for SimpleCoin<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthEvent,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethevent(name = "Transfer", abi = "Transfer(address,address,uint256)")]
    pub struct TransferFilter {
        #[ethevent(indexed)]
        pub from: ethers::core::types::Address,
        #[ethevent(indexed)]
        pub to: ethers::core::types::Address,
        pub value: ethers::core::types::U256,
    }
    #[doc = "Container type for all input parameters for the `getBalance` function with signature `getBalance(address)` and selector `[248, 178, 203, 79]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getBalance", abi = "getBalance(address)")]
    pub struct GetBalanceCall {
        pub addr: ethers::core::types::Address,
    }
    #[doc = "Container type for all input parameters for the `getBalanceInEth` function with signature `getBalanceInEth(address)` and selector `[123, 215, 3, 232]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getBalanceInEth", abi = "getBalanceInEth(address)")]
    pub struct GetBalanceInEthCall {
        pub addr: ethers::core::types::Address,
    }
    #[doc = "Container type for all input parameters for the `sendCoin` function with signature `sendCoin(address,uint256)` and selector `[144, 185, 138, 17]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "sendCoin", abi = "sendCoin(address,uint256)")]
    pub struct SendCoinCall {
        pub receiver: ethers::core::types::Address,
        pub amount: ethers::core::types::U256,
    }
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum SimpleCoinCalls {
        GetBalance(GetBalanceCall),
        GetBalanceInEth(GetBalanceInEthCall),
        SendCoin(SendCoinCall),
    }
    impl ethers::core::abi::AbiDecode for SimpleCoinCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::std::result::Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) =
                <GetBalanceCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(SimpleCoinCalls::GetBalance(decoded));
            }
            if let Ok(decoded) =
                <GetBalanceInEthCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(SimpleCoinCalls::GetBalanceInEth(decoded));
            }
            if let Ok(decoded) =
                <SendCoinCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(SimpleCoinCalls::SendCoin(decoded));
            }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for SimpleCoinCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                SimpleCoinCalls::GetBalance(element) => element.encode(),
                SimpleCoinCalls::GetBalanceInEth(element) => element.encode(),
                SimpleCoinCalls::SendCoin(element) => element.encode(),
            }
        }
    }
    impl ::std::fmt::Display for SimpleCoinCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                SimpleCoinCalls::GetBalance(element) => element.fmt(f),
                SimpleCoinCalls::GetBalanceInEth(element) => element.fmt(f),
                SimpleCoinCalls::SendCoin(element) => element.fmt(f),
            }
        }
    }
    impl ::std::convert::From<GetBalanceCall> for SimpleCoinCalls {
        fn from(var: GetBalanceCall) -> Self {
            SimpleCoinCalls::GetBalance(var)
        }
    }
    impl ::std::convert::From<GetBalanceInEthCall> for SimpleCoinCalls {
        fn from(var: GetBalanceInEthCall) -> Self {
            SimpleCoinCalls::GetBalanceInEth(var)
        }
    }
    impl ::std::convert::From<SendCoinCall> for SimpleCoinCalls {
        fn from(var: SendCoinCall) -> Self {
            SimpleCoinCalls::SendCoin(var)
        }
    }
    #[doc = "Container type for all return fields from the `getBalance` function with signature `getBalance(address)` and selector `[248, 178, 203, 79]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetBalanceReturn(pub ethers::core::types::U256);
    #[doc = "Container type for all return fields from the `getBalanceInEth` function with signature `getBalanceInEth(address)` and selector `[123, 215, 3, 232]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetBalanceInEthReturn(pub ethers::core::types::U256);
    #[doc = "Container type for all return fields from the `sendCoin` function with signature `sendCoin(address,uint256)` and selector `[144, 185, 138, 17]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct SendCoinReturn {
        pub sufficient: bool,
    }
}
