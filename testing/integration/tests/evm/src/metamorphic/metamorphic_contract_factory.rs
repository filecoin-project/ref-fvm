pub use metamorphic_contract_factory::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod metamorphic_contract_factory {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    #[doc = "MetamorphicContractFactory was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs"]
    use std::sync::Arc;

    use ethers::contract::builders::{ContractCall, Event};
    use ethers::contract::{Contract, Lazy};
    use ethers::core::abi::{Abi, Detokenize, InvalidOutputType, Token, Tokenizable};
    use ethers::core::types::*;
    use ethers::providers::Middleware;
    # [rustfmt :: skip] const __ABI : & str = "[{\"inputs\":[{\"internalType\":\"bytes\",\"name\":\"transientContractInitializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"nonpayable\",\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"internalType\":\"address\",\"name\":\"metamorphicContract\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"newImplementation\",\"type\":\"address\"}],\"name\":\"Metamorphosed\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"internalType\":\"address\",\"name\":\"metamorphicContract\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"transientContract\",\"type\":\"address\"}],\"name\":\"MetamorphosedWithConstructor\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"},{\"internalType\":\"bytes\",\"name\":\"implementationContractInitializationCode\",\"type\":\"bytes\"},{\"internalType\":\"bytes\",\"name\":\"metamorphicContractInitializationCalldata\",\"type\":\"bytes\"}],\"name\":\"deployMetamorphicContract\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"payable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"},{\"internalType\":\"address\",\"name\":\"implementationContract\",\"type\":\"address\"},{\"internalType\":\"bytes\",\"name\":\"metamorphicContractInitializationCalldata\",\"type\":\"bytes\"}],\"name\":\"deployMetamorphicContractFromExistingImplementation\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"payable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"},{\"internalType\":\"bytes\",\"name\":\"initializationCode\",\"type\":\"bytes\"}],\"name\":\"deployMetamorphicContractWithConstructor\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"payable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"}],\"name\":\"findMetamorphicContractAddress\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"}],\"name\":\"findMetamorphicContractAddressWithConstructor\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes32\",\"name\":\"salt\",\"type\":\"bytes32\"}],\"name\":\"findTransientContractAddress\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"transientContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"bytes\",\"name\":\"bytecode\",\"type\":\"bytes\"},{\"internalType\":\"bytes32\",\"name\":\"_salt\",\"type\":\"bytes32\"}],\"name\":\"getAddress\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getImplementation\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"implementation\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"metamorphicContractAddress\",\"type\":\"address\"}],\"name\":\"getImplementationContractAddress\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"implementationContractAddress\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getInitializationCode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"initializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getMetamorphicContractInitializationCode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"metamorphicContractInitializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getMetamorphicContractInitializationCodeHash\",\"outputs\":[{\"internalType\":\"bytes32\",\"name\":\"metamorphicContractInitializationCodeHash\",\"type\":\"bytes32\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"transientContractAddress\",\"type\":\"address\"}],\"name\":\"getMetamorphicContractInstanceInitializationCode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"initializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getTransientBytecode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"\",\"type\":\"bytes\"}],\"stateMutability\":\"pure\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getTransientContractInitializationCode\",\"outputs\":[{\"internalType\":\"bytes\",\"name\":\"transientContractInitializationCode\",\"type\":\"bytes\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"getTransientContractInitializationCodeHash\",\"outputs\":[{\"internalType\":\"bytes32\",\"name\":\"transientContractInitializationCodeHash\",\"type\":\"bytes32\"}],\"stateMutability\":\"view\",\"type\":\"function\"}]\n" ;
    #[doc = r" The parsed JSON-ABI of the contract."]
    pub static METAMORPHICCONTRACTFACTORY_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers::core::utils::__serde_json::from_str(__ABI).expect("invalid abi")
        });
    pub struct MetamorphicContractFactory<M>(ethers::contract::Contract<M>);
    impl<M> Clone for MetamorphicContractFactory<M> {
        fn clone(&self) -> Self {
            MetamorphicContractFactory(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for MetamorphicContractFactory<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> std::fmt::Debug for MetamorphicContractFactory<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple(stringify!(MetamorphicContractFactory))
                .field(&self.address())
                .finish()
        }
    }
    impl<M: ethers::providers::Middleware> MetamorphicContractFactory<M> {
        #[doc = r" Creates a new contract instance with the specified `ethers`"]
        #[doc = r" client at the given `Address`. The contract derefs to a `ethers::Contract`"]
        #[doc = r" object"]
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(
                address.into(),
                METAMORPHICCONTRACTFACTORY_ABI.clone(),
                client,
            )
            .into()
        }
        #[doc = "Calls the contract's `deployMetamorphicContract` (0x6f8bda37) function"]
        pub fn deploy_metamorphic_contract(
            &self,
            salt: [u8; 32],
            implementation_contract_initialization_code: ethers::core::types::Bytes,
            metamorphic_contract_initialization_calldata: ethers::core::types::Bytes,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash(
                    [111, 139, 218, 55],
                    (
                        salt,
                        implementation_contract_initialization_code,
                        metamorphic_contract_initialization_calldata,
                    ),
                )
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `deployMetamorphicContractFromExistingImplementation` (0x82cd5833) function"]
        pub fn deploy_metamorphic_contract_from_existing_implementation(
            &self,
            salt: [u8; 32],
            implementation_contract: ethers::core::types::Address,
            metamorphic_contract_initialization_calldata: ethers::core::types::Bytes,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash(
                    [130, 205, 88, 51],
                    (
                        salt,
                        implementation_contract,
                        metamorphic_contract_initialization_calldata,
                    ),
                )
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `deployMetamorphicContractWithConstructor` (0x2c51145c) function"]
        pub fn deploy_metamorphic_contract_with_constructor(
            &self,
            salt: [u8; 32],
            initialization_code: ethers::core::types::Bytes,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([44, 81, 20, 92], (salt, initialization_code))
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `findMetamorphicContractAddress` (0xb5714de6) function"]
        pub fn find_metamorphic_contract_address(
            &self,
            salt: [u8; 32],
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([181, 113, 77, 230], salt)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `findMetamorphicContractAddressWithConstructor` (0x687c42fd) function"]
        pub fn find_metamorphic_contract_address_with_constructor(
            &self,
            salt: [u8; 32],
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([104, 124, 66, 253], salt)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `findTransientContractAddress` (0xa32cfb69) function"]
        pub fn find_transient_contract_address(
            &self,
            salt: [u8; 32],
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([163, 44, 251, 105], salt)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getAddress` (0x48aac392) function"]
        pub fn get_address(
            &self,
            bytecode: ethers::core::types::Bytes,
            salt: [u8; 32],
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([72, 170, 195, 146], (bytecode, salt))
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getImplementation` (0xaaf10f42) function"]
        pub fn get_implementation(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([170, 241, 15, 66], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getImplementationContractAddress` (0xb7d2b0b4) function"]
        pub fn get_implementation_contract_address(
            &self,
            metamorphic_contract_address: ethers::core::types::Address,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Address> {
            self.0
                .method_hash([183, 210, 176, 180], metamorphic_contract_address)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getInitializationCode` (0x57b9f523) function"]
        pub fn get_initialization_code(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([87, 185, 245, 35], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getMetamorphicContractInitializationCode` (0xc762ef58) function"]
        pub fn get_metamorphic_contract_initialization_code(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([199, 98, 239, 88], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getMetamorphicContractInitializationCodeHash` (0x641b2afa) function"]
        pub fn get_metamorphic_contract_initialization_code_hash(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, [u8; 32]> {
            self.0
                .method_hash([100, 27, 42, 250], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getMetamorphicContractInstanceInitializationCode` (0x59449e55) function"]
        pub fn get_metamorphic_contract_instance_initialization_code(
            &self,
            transient_contract_address: ethers::core::types::Address,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([89, 68, 158, 85], transient_contract_address)
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getTransientBytecode` (0x52d56323) function"]
        pub fn get_transient_bytecode(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([82, 213, 99, 35], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getTransientContractInitializationCode` (0x0563ef93) function"]
        pub fn get_transient_contract_initialization_code(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, ethers::core::types::Bytes> {
            self.0
                .method_hash([5, 99, 239, 147], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Calls the contract's `getTransientContractInitializationCodeHash` (0x010fcf85) function"]
        pub fn get_transient_contract_initialization_code_hash(
            &self,
        ) -> ethers::contract::builders::ContractCall<M, [u8; 32]> {
            self.0
                .method_hash([1, 15, 207, 133], ())
                .expect("method not found (this should never happen)")
        }
        #[doc = "Gets the contract's `Metamorphosed` event"]
        pub fn metamorphosed_filter(
            &self,
        ) -> ethers::contract::builders::Event<M, MetamorphosedFilter> {
            self.0.event()
        }
        #[doc = "Gets the contract's `MetamorphosedWithConstructor` event"]
        pub fn metamorphosed_with_constructor_filter(
            &self,
        ) -> ethers::contract::builders::Event<M, MetamorphosedWithConstructorFilter> {
            self.0.event()
        }
        #[doc = r" Returns an [`Event`](#ethers_contract::builders::Event) builder for all events of this contract"]
        pub fn events(
            &self,
        ) -> ethers::contract::builders::Event<M, MetamorphicContractFactoryEvents> {
            self.0.event_with_filter(Default::default())
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>>
        for MetamorphicContractFactory<M>
    {
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
    #[ethevent(name = "Metamorphosed", abi = "Metamorphosed(address,address)")]
    pub struct MetamorphosedFilter {
        pub metamorphic_contract: ethers::core::types::Address,
        pub new_implementation: ethers::core::types::Address,
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
    #[ethevent(
        name = "MetamorphosedWithConstructor",
        abi = "MetamorphosedWithConstructor(address,address)"
    )]
    pub struct MetamorphosedWithConstructorFilter {
        pub metamorphic_contract: ethers::core::types::Address,
        pub transient_contract: ethers::core::types::Address,
    }
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum MetamorphicContractFactoryEvents {
        MetamorphosedFilter(MetamorphosedFilter),
        MetamorphosedWithConstructorFilter(MetamorphosedWithConstructorFilter),
    }
    impl ethers::contract::EthLogDecode for MetamorphicContractFactoryEvents {
        fn decode_log(
            log: &ethers::core::abi::RawLog,
        ) -> ::std::result::Result<Self, ethers::core::abi::Error>
        where
            Self: Sized,
        {
            if let Ok(decoded) = MetamorphosedFilter::decode_log(log) {
                return Ok(MetamorphicContractFactoryEvents::MetamorphosedFilter(
                    decoded,
                ));
            }
            if let Ok(decoded) = MetamorphosedWithConstructorFilter::decode_log(log) {
                return Ok(
                    MetamorphicContractFactoryEvents::MetamorphosedWithConstructorFilter(decoded),
                );
            }
            Err(ethers::core::abi::Error::InvalidData)
        }
    }
    impl ::std::fmt::Display for MetamorphicContractFactoryEvents {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                MetamorphicContractFactoryEvents::MetamorphosedFilter(element) => element.fmt(f),
                MetamorphicContractFactoryEvents::MetamorphosedWithConstructorFilter(element) => {
                    element.fmt(f)
                }
            }
        }
    }
    #[doc = "Container type for all input parameters for the `deployMetamorphicContract` function with signature `deployMetamorphicContract(bytes32,bytes,bytes)` and selector `[111, 139, 218, 55]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "deployMetamorphicContract",
        abi = "deployMetamorphicContract(bytes32,bytes,bytes)"
    )]
    pub struct DeployMetamorphicContractCall {
        pub salt: [u8; 32],
        pub implementation_contract_initialization_code: ethers::core::types::Bytes,
        pub metamorphic_contract_initialization_calldata: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all input parameters for the `deployMetamorphicContractFromExistingImplementation` function with signature `deployMetamorphicContractFromExistingImplementation(bytes32,address,bytes)` and selector `[130, 205, 88, 51]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "deployMetamorphicContractFromExistingImplementation",
        abi = "deployMetamorphicContractFromExistingImplementation(bytes32,address,bytes)"
    )]
    pub struct DeployMetamorphicContractFromExistingImplementationCall {
        pub salt: [u8; 32],
        pub implementation_contract: ethers::core::types::Address,
        pub metamorphic_contract_initialization_calldata: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all input parameters for the `deployMetamorphicContractWithConstructor` function with signature `deployMetamorphicContractWithConstructor(bytes32,bytes)` and selector `[44, 81, 20, 92]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "deployMetamorphicContractWithConstructor",
        abi = "deployMetamorphicContractWithConstructor(bytes32,bytes)"
    )]
    pub struct DeployMetamorphicContractWithConstructorCall {
        pub salt: [u8; 32],
        pub initialization_code: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all input parameters for the `findMetamorphicContractAddress` function with signature `findMetamorphicContractAddress(bytes32)` and selector `[181, 113, 77, 230]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "findMetamorphicContractAddress",
        abi = "findMetamorphicContractAddress(bytes32)"
    )]
    pub struct FindMetamorphicContractAddressCall {
        pub salt: [u8; 32],
    }
    #[doc = "Container type for all input parameters for the `findMetamorphicContractAddressWithConstructor` function with signature `findMetamorphicContractAddressWithConstructor(bytes32)` and selector `[104, 124, 66, 253]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "findMetamorphicContractAddressWithConstructor",
        abi = "findMetamorphicContractAddressWithConstructor(bytes32)"
    )]
    pub struct FindMetamorphicContractAddressWithConstructorCall {
        pub salt: [u8; 32],
    }
    #[doc = "Container type for all input parameters for the `findTransientContractAddress` function with signature `findTransientContractAddress(bytes32)` and selector `[163, 44, 251, 105]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "findTransientContractAddress",
        abi = "findTransientContractAddress(bytes32)"
    )]
    pub struct FindTransientContractAddressCall {
        pub salt: [u8; 32],
    }
    #[doc = "Container type for all input parameters for the `getAddress` function with signature `getAddress(bytes,bytes32)` and selector `[72, 170, 195, 146]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getAddress", abi = "getAddress(bytes,bytes32)")]
    pub struct GetAddressCall {
        pub bytecode: ethers::core::types::Bytes,
        pub salt: [u8; 32],
    }
    #[doc = "Container type for all input parameters for the `getImplementation` function with signature `getImplementation()` and selector `[170, 241, 15, 66]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getImplementation", abi = "getImplementation()")]
    pub struct GetImplementationCall;
    #[doc = "Container type for all input parameters for the `getImplementationContractAddress` function with signature `getImplementationContractAddress(address)` and selector `[183, 210, 176, 180]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getImplementationContractAddress",
        abi = "getImplementationContractAddress(address)"
    )]
    pub struct GetImplementationContractAddressCall {
        pub metamorphic_contract_address: ethers::core::types::Address,
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
    #[doc = "Container type for all input parameters for the `getMetamorphicContractInitializationCode` function with signature `getMetamorphicContractInitializationCode()` and selector `[199, 98, 239, 88]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getMetamorphicContractInitializationCode",
        abi = "getMetamorphicContractInitializationCode()"
    )]
    pub struct GetMetamorphicContractInitializationCodeCall;
    #[doc = "Container type for all input parameters for the `getMetamorphicContractInitializationCodeHash` function with signature `getMetamorphicContractInitializationCodeHash()` and selector `[100, 27, 42, 250]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getMetamorphicContractInitializationCodeHash",
        abi = "getMetamorphicContractInitializationCodeHash()"
    )]
    pub struct GetMetamorphicContractInitializationCodeHashCall;
    #[doc = "Container type for all input parameters for the `getMetamorphicContractInstanceInitializationCode` function with signature `getMetamorphicContractInstanceInitializationCode(address)` and selector `[89, 68, 158, 85]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getMetamorphicContractInstanceInitializationCode",
        abi = "getMetamorphicContractInstanceInitializationCode(address)"
    )]
    pub struct GetMetamorphicContractInstanceInitializationCodeCall {
        pub transient_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all input parameters for the `getTransientBytecode` function with signature `getTransientBytecode()` and selector `[82, 213, 99, 35]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(name = "getTransientBytecode", abi = "getTransientBytecode()")]
    pub struct GetTransientBytecodeCall;
    #[doc = "Container type for all input parameters for the `getTransientContractInitializationCode` function with signature `getTransientContractInitializationCode()` and selector `[5, 99, 239, 147]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getTransientContractInitializationCode",
        abi = "getTransientContractInitializationCode()"
    )]
    pub struct GetTransientContractInitializationCodeCall;
    #[doc = "Container type for all input parameters for the `getTransientContractInitializationCodeHash` function with signature `getTransientContractInitializationCodeHash()` and selector `[1, 15, 207, 133]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthCall,
        ethers :: contract :: EthDisplay,
        Default,
    )]
    #[ethcall(
        name = "getTransientContractInitializationCodeHash",
        abi = "getTransientContractInitializationCodeHash()"
    )]
    pub struct GetTransientContractInitializationCodeHashCall;
    #[derive(Debug, Clone, PartialEq, Eq, ethers :: contract :: EthAbiType)]
    pub enum MetamorphicContractFactoryCalls {
        DeployMetamorphicContract(DeployMetamorphicContractCall),
        DeployMetamorphicContractFromExistingImplementation(
            DeployMetamorphicContractFromExistingImplementationCall,
        ),
        DeployMetamorphicContractWithConstructor(DeployMetamorphicContractWithConstructorCall),
        FindMetamorphicContractAddress(FindMetamorphicContractAddressCall),
        FindMetamorphicContractAddressWithConstructor(
            FindMetamorphicContractAddressWithConstructorCall,
        ),
        FindTransientContractAddress(FindTransientContractAddressCall),
        GetAddress(GetAddressCall),
        GetImplementation(GetImplementationCall),
        GetImplementationContractAddress(GetImplementationContractAddressCall),
        GetInitializationCode(GetInitializationCodeCall),
        GetMetamorphicContractInitializationCode(GetMetamorphicContractInitializationCodeCall),
        GetMetamorphicContractInitializationCodeHash(
            GetMetamorphicContractInitializationCodeHashCall,
        ),
        GetMetamorphicContractInstanceInitializationCode(
            GetMetamorphicContractInstanceInitializationCodeCall,
        ),
        GetTransientBytecode(GetTransientBytecodeCall),
        GetTransientContractInitializationCode(GetTransientContractInitializationCodeCall),
        GetTransientContractInitializationCodeHash(GetTransientContractInitializationCodeHashCall),
    }
    impl ethers::core::abi::AbiDecode for MetamorphicContractFactoryCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::std::result::Result<Self, ethers::core::abi::AbiError> {
            if let Ok(decoded) =
                <DeployMetamorphicContractCall as ethers::core::abi::AbiDecode>::decode(
                    data.as_ref(),
                )
            {
                return Ok(MetamorphicContractFactoryCalls::DeployMetamorphicContract(
                    decoded,
                ));
            }
            if let Ok (decoded) = < DeployMetamorphicContractFromExistingImplementationCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: DeployMetamorphicContractFromExistingImplementation (decoded)) }
            if let Ok (decoded) = < DeployMetamorphicContractWithConstructorCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: DeployMetamorphicContractWithConstructor (decoded)) }
            if let Ok(decoded) =
                <FindMetamorphicContractAddressCall as ethers::core::abi::AbiDecode>::decode(
                    data.as_ref(),
                )
            {
                return Ok(MetamorphicContractFactoryCalls::FindMetamorphicContractAddress(decoded));
            }
            if let Ok (decoded) = < FindMetamorphicContractAddressWithConstructorCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: FindMetamorphicContractAddressWithConstructor (decoded)) }
            if let Ok(decoded) =
                <FindTransientContractAddressCall as ethers::core::abi::AbiDecode>::decode(
                    data.as_ref(),
                )
            {
                return Ok(MetamorphicContractFactoryCalls::FindTransientContractAddress(decoded));
            }
            if let Ok(decoded) =
                <GetAddressCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MetamorphicContractFactoryCalls::GetAddress(decoded));
            }
            if let Ok(decoded) =
                <GetImplementationCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MetamorphicContractFactoryCalls::GetImplementation(decoded));
            }
            if let Ok(decoded) =
                <GetImplementationContractAddressCall as ethers::core::abi::AbiDecode>::decode(
                    data.as_ref(),
                )
            {
                return Ok(
                    MetamorphicContractFactoryCalls::GetImplementationContractAddress(decoded),
                );
            }
            if let Ok(decoded) =
                <GetInitializationCodeCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MetamorphicContractFactoryCalls::GetInitializationCode(
                    decoded,
                ));
            }
            if let Ok (decoded) = < GetMetamorphicContractInitializationCodeCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCode (decoded)) }
            if let Ok (decoded) = < GetMetamorphicContractInitializationCodeHashCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCodeHash (decoded)) }
            if let Ok (decoded) = < GetMetamorphicContractInstanceInitializationCodeCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: GetMetamorphicContractInstanceInitializationCode (decoded)) }
            if let Ok(decoded) =
                <GetTransientBytecodeCall as ethers::core::abi::AbiDecode>::decode(data.as_ref())
            {
                return Ok(MetamorphicContractFactoryCalls::GetTransientBytecode(
                    decoded,
                ));
            }
            if let Ok(decoded) =
                <GetTransientContractInitializationCodeCall as ethers::core::abi::AbiDecode>::decode(
                    data.as_ref(),
                )
            {
                return Ok(
                    MetamorphicContractFactoryCalls::GetTransientContractInitializationCode(
                        decoded,
                    ),
                );
            }
            if let Ok (decoded) = < GetTransientContractInitializationCodeHashCall as ethers :: core :: abi :: AbiDecode > :: decode (data . as_ref ()) { return Ok (MetamorphicContractFactoryCalls :: GetTransientContractInitializationCodeHash (decoded)) }
            Err(ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ethers::core::abi::AbiEncode for MetamorphicContractFactoryCalls {
        fn encode(self) -> Vec<u8> {
            match self { MetamorphicContractFactoryCalls :: DeployMetamorphicContract (element) => element . encode () , MetamorphicContractFactoryCalls :: DeployMetamorphicContractFromExistingImplementation (element) => element . encode () , MetamorphicContractFactoryCalls :: DeployMetamorphicContractWithConstructor (element) => element . encode () , MetamorphicContractFactoryCalls :: FindMetamorphicContractAddress (element) => element . encode () , MetamorphicContractFactoryCalls :: FindMetamorphicContractAddressWithConstructor (element) => element . encode () , MetamorphicContractFactoryCalls :: FindTransientContractAddress (element) => element . encode () , MetamorphicContractFactoryCalls :: GetAddress (element) => element . encode () , MetamorphicContractFactoryCalls :: GetImplementation (element) => element . encode () , MetamorphicContractFactoryCalls :: GetImplementationContractAddress (element) => element . encode () , MetamorphicContractFactoryCalls :: GetInitializationCode (element) => element . encode () , MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCode (element) => element . encode () , MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCodeHash (element) => element . encode () , MetamorphicContractFactoryCalls :: GetMetamorphicContractInstanceInitializationCode (element) => element . encode () , MetamorphicContractFactoryCalls :: GetTransientBytecode (element) => element . encode () , MetamorphicContractFactoryCalls :: GetTransientContractInitializationCode (element) => element . encode () , MetamorphicContractFactoryCalls :: GetTransientContractInitializationCodeHash (element) => element . encode () }
        }
    }
    impl ::std::fmt::Display for MetamorphicContractFactoryCalls {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self { MetamorphicContractFactoryCalls :: DeployMetamorphicContract (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: DeployMetamorphicContractFromExistingImplementation (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: DeployMetamorphicContractWithConstructor (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: FindMetamorphicContractAddress (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: FindMetamorphicContractAddressWithConstructor (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: FindTransientContractAddress (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetAddress (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetImplementation (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetImplementationContractAddress (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetInitializationCode (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCode (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetMetamorphicContractInitializationCodeHash (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetMetamorphicContractInstanceInitializationCode (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetTransientBytecode (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetTransientContractInitializationCode (element) => element . fmt (f) , MetamorphicContractFactoryCalls :: GetTransientContractInitializationCodeHash (element) => element . fmt (f) }
        }
    }
    impl ::std::convert::From<DeployMetamorphicContractCall> for MetamorphicContractFactoryCalls {
        fn from(var: DeployMetamorphicContractCall) -> Self {
            MetamorphicContractFactoryCalls::DeployMetamorphicContract(var)
        }
    }
    impl ::std::convert::From<DeployMetamorphicContractFromExistingImplementationCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: DeployMetamorphicContractFromExistingImplementationCall) -> Self {
            MetamorphicContractFactoryCalls::DeployMetamorphicContractFromExistingImplementation(
                var,
            )
        }
    }
    impl ::std::convert::From<DeployMetamorphicContractWithConstructorCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: DeployMetamorphicContractWithConstructorCall) -> Self {
            MetamorphicContractFactoryCalls::DeployMetamorphicContractWithConstructor(var)
        }
    }
    impl ::std::convert::From<FindMetamorphicContractAddressCall> for MetamorphicContractFactoryCalls {
        fn from(var: FindMetamorphicContractAddressCall) -> Self {
            MetamorphicContractFactoryCalls::FindMetamorphicContractAddress(var)
        }
    }
    impl ::std::convert::From<FindMetamorphicContractAddressWithConstructorCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: FindMetamorphicContractAddressWithConstructorCall) -> Self {
            MetamorphicContractFactoryCalls::FindMetamorphicContractAddressWithConstructor(var)
        }
    }
    impl ::std::convert::From<FindTransientContractAddressCall> for MetamorphicContractFactoryCalls {
        fn from(var: FindTransientContractAddressCall) -> Self {
            MetamorphicContractFactoryCalls::FindTransientContractAddress(var)
        }
    }
    impl ::std::convert::From<GetAddressCall> for MetamorphicContractFactoryCalls {
        fn from(var: GetAddressCall) -> Self {
            MetamorphicContractFactoryCalls::GetAddress(var)
        }
    }
    impl ::std::convert::From<GetImplementationCall> for MetamorphicContractFactoryCalls {
        fn from(var: GetImplementationCall) -> Self {
            MetamorphicContractFactoryCalls::GetImplementation(var)
        }
    }
    impl ::std::convert::From<GetImplementationContractAddressCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetImplementationContractAddressCall) -> Self {
            MetamorphicContractFactoryCalls::GetImplementationContractAddress(var)
        }
    }
    impl ::std::convert::From<GetInitializationCodeCall> for MetamorphicContractFactoryCalls {
        fn from(var: GetInitializationCodeCall) -> Self {
            MetamorphicContractFactoryCalls::GetInitializationCode(var)
        }
    }
    impl ::std::convert::From<GetMetamorphicContractInitializationCodeCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetMetamorphicContractInitializationCodeCall) -> Self {
            MetamorphicContractFactoryCalls::GetMetamorphicContractInitializationCode(var)
        }
    }
    impl ::std::convert::From<GetMetamorphicContractInitializationCodeHashCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetMetamorphicContractInitializationCodeHashCall) -> Self {
            MetamorphicContractFactoryCalls::GetMetamorphicContractInitializationCodeHash(var)
        }
    }
    impl ::std::convert::From<GetMetamorphicContractInstanceInitializationCodeCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetMetamorphicContractInstanceInitializationCodeCall) -> Self {
            MetamorphicContractFactoryCalls::GetMetamorphicContractInstanceInitializationCode(var)
        }
    }
    impl ::std::convert::From<GetTransientBytecodeCall> for MetamorphicContractFactoryCalls {
        fn from(var: GetTransientBytecodeCall) -> Self {
            MetamorphicContractFactoryCalls::GetTransientBytecode(var)
        }
    }
    impl ::std::convert::From<GetTransientContractInitializationCodeCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetTransientContractInitializationCodeCall) -> Self {
            MetamorphicContractFactoryCalls::GetTransientContractInitializationCode(var)
        }
    }
    impl ::std::convert::From<GetTransientContractInitializationCodeHashCall>
        for MetamorphicContractFactoryCalls
    {
        fn from(var: GetTransientContractInitializationCodeHashCall) -> Self {
            MetamorphicContractFactoryCalls::GetTransientContractInitializationCodeHash(var)
        }
    }
    #[doc = "Container type for all return fields from the `deployMetamorphicContract` function with signature `deployMetamorphicContract(bytes32,bytes,bytes)` and selector `[111, 139, 218, 55]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct DeployMetamorphicContractReturn {
        pub metamorphic_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `deployMetamorphicContractFromExistingImplementation` function with signature `deployMetamorphicContractFromExistingImplementation(bytes32,address,bytes)` and selector `[130, 205, 88, 51]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct DeployMetamorphicContractFromExistingImplementationReturn {
        pub metamorphic_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `deployMetamorphicContractWithConstructor` function with signature `deployMetamorphicContractWithConstructor(bytes32,bytes)` and selector `[44, 81, 20, 92]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct DeployMetamorphicContractWithConstructorReturn {
        pub metamorphic_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `findMetamorphicContractAddress` function with signature `findMetamorphicContractAddress(bytes32)` and selector `[181, 113, 77, 230]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct FindMetamorphicContractAddressReturn {
        pub metamorphic_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `findMetamorphicContractAddressWithConstructor` function with signature `findMetamorphicContractAddressWithConstructor(bytes32)` and selector `[104, 124, 66, 253]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct FindMetamorphicContractAddressWithConstructorReturn {
        pub metamorphic_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `findTransientContractAddress` function with signature `findTransientContractAddress(bytes32)` and selector `[163, 44, 251, 105]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct FindTransientContractAddressReturn {
        pub transient_contract_address: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `getAddress` function with signature `getAddress(bytes,bytes32)` and selector `[72, 170, 195, 146]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetAddressReturn(pub ethers::core::types::Address);
    #[doc = "Container type for all return fields from the `getImplementation` function with signature `getImplementation()` and selector `[170, 241, 15, 66]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetImplementationReturn {
        pub implementation: ethers::core::types::Address,
    }
    #[doc = "Container type for all return fields from the `getImplementationContractAddress` function with signature `getImplementationContractAddress(address)` and selector `[183, 210, 176, 180]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetImplementationContractAddressReturn {
        pub implementation_contract_address: ethers::core::types::Address,
    }
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
    #[doc = "Container type for all return fields from the `getMetamorphicContractInitializationCode` function with signature `getMetamorphicContractInitializationCode()` and selector `[199, 98, 239, 88]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetMetamorphicContractInitializationCodeReturn {
        pub metamorphic_contract_initialization_code: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all return fields from the `getMetamorphicContractInitializationCodeHash` function with signature `getMetamorphicContractInitializationCodeHash()` and selector `[100, 27, 42, 250]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetMetamorphicContractInitializationCodeHashReturn {
        pub metamorphic_contract_initialization_code_hash: [u8; 32],
    }
    #[doc = "Container type for all return fields from the `getMetamorphicContractInstanceInitializationCode` function with signature `getMetamorphicContractInstanceInitializationCode(address)` and selector `[89, 68, 158, 85]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetMetamorphicContractInstanceInitializationCodeReturn {
        pub initialization_code: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all return fields from the `getTransientBytecode` function with signature `getTransientBytecode()` and selector `[82, 213, 99, 35]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetTransientBytecodeReturn(pub ethers::core::types::Bytes);
    #[doc = "Container type for all return fields from the `getTransientContractInitializationCode` function with signature `getTransientContractInitializationCode()` and selector `[5, 99, 239, 147]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetTransientContractInitializationCodeReturn {
        pub transient_contract_initialization_code: ethers::core::types::Bytes,
    }
    #[doc = "Container type for all return fields from the `getTransientContractInitializationCodeHash` function with signature `getTransientContractInitializationCodeHash()` and selector `[1, 15, 207, 133]`"]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        ethers :: contract :: EthAbiType,
        ethers :: contract :: EthAbiCodec,
        Default,
    )]
    pub struct GetTransientContractInitializationCodeHashReturn {
        pub transient_contract_initialization_code_hash: [u8; 32],
    }
}
