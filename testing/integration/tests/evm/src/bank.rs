pub use bank::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod bank {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "Bank was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[],\"stateMutability\":\"payable\",\"type\":\"constructor\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"name\":\"accounts\",\"outputs\":[{\"internalType\":\"contract Account\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"openAccount\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"payable\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"owner\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static BANK_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct Bank<M>(ethers::contract::Contract<M>);
    impl<M> Clone for Bank<M> {
        fn clone(&self) -> Self {
            Bank(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for Bank<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for Bank<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(Bank))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> Bank<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), BANK_ABI.clone(), client).into()
        }
        #[doc = "Calls the contract's `accounts` (0xf2a40db8) function"]
        pub fn accounts(
            &self,
            p0: ethers::core::types::U256,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([242, 164, 13, 184], p0)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `openAccount` (0x292eb75d) function"]
        pub fn open_account(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([41, 46, 183, 93], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `owner` (0x8da5cb5b) function"]
        pub fn owner(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([141, 165, 203, 91], ())
                .expect("method not found (this should never happen)")
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for Bank<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[doc = "Container type for all input parameters for the `accounts` function with signature `accounts(uint256)` and selector `[242, 164, 13, 184]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "accounts", abi = "accounts(uint256)")]
    pub struct AccountsCall(pub ethers::core::types::U256);
    #[doc = "Container type for all input parameters for the `openAccount` function with signature `openAccount()` and selector `[41, 46, 183, 93]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "openAccount", abi = "openAccount()")]
    pub struct OpenAccountCall;
    #[doc = "Container type for all input parameters for the `owner` function with signature `owner()` and selector `[141, 165, 203, 91]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "owner", abi = "owner()")]
    pub struct OwnerCall;
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum BankCalls {
        Accounts(AccountsCall),
        OpenAccount(OpenAccountCall),
        Owner(OwnerCall),
    }
    impl ethers::core::abi::AbiDecode for BankCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::std::result::Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) =
                <AccountsCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(BankCalls::Accounts(decoded));
            }
            if let Ok(decoded) =
                <OpenAccountCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(BankCalls::OpenAccount(decoded));
            }
            if let Ok(decoded) = <OwnerCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(BankCalls::Owner(decoded));
            }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for BankCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                BankCalls::Accounts(element) => element.encode(),
                BankCalls::OpenAccount(element) => element.encode(),
                BankCalls::Owner(element) => element.encode(),
            }
        }
    }
    impl ::std::fmt::Display for BankCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                BankCalls::Accounts(element) => element.fmt(f),
                BankCalls::OpenAccount(element) => element.fmt(f),
                BankCalls::Owner(element) => element.fmt(f),
            }
        }
    }
    impl ::std::convert::From<AccountsCall> for BankCalls {
        fn from(var: AccountsCall) -> Self {
            BankCalls::Accounts(var)
        }
    }
    impl ::std::convert::From<OpenAccountCall> for BankCalls {
        fn from(var: OpenAccountCall) -> Self {
            BankCalls::OpenAccount(var)
        }
    }
    impl ::std::convert::From<OwnerCall> for BankCalls {
        fn from(var: OwnerCall) -> Self {
            BankCalls::Owner(var)
        }
    }
    #[doc = "Container type for all return fields from the `accounts` function with signature `accounts(uint256)` and selector `[242, 164, 13, 184]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct AccountsReturn(pub ethers::core::types::Address);
    #[doc = "Container type for all return fields from the `openAccount` function with signature `openAccount()` and selector `[41, 46, 183, 93]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct OpenAccountReturn(pub ethers::core::types::Address);
    #[doc = "Container type for all return fields from the `owner` function with signature `owner()` and selector `[141, 165, 203, 91]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct OwnerReturn(pub ethers::core::types::Address);
}
