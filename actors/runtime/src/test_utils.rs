// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use anyhow::anyhow;
use cid::multihash::Code;
use cid::Cid;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::blockstore::{CborStore, MemoryBlockstore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::de::DeserializeOwned;
use fvm_shared::encoding::{blake2b_256, Cbor, RawBytes};
use fvm_shared::error::ExitCode;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum};

use crate::runtime::{ActorCode, MessageInfo, Runtime, Syscalls};
use crate::{actor_error, ActorError};

pub struct MockRuntime {
    pub epoch: ChainEpoch,
    pub miner: Address,
    pub base_fee: TokenAmount,
    pub id_addresses: HashMap<Address, Address>,
    pub actor_code_cids: HashMap<Address, Cid>,
    pub new_actor_addr: Option<Address>,
    pub receiver: Address,
    pub caller: Address,
    pub caller_type: Cid,
    pub value_received: TokenAmount,
    pub hash_func: Box<dyn Fn(&[u8]) -> [u8; 32]>,
    pub network_version: NetworkVersion,

    // Actor State
    pub state: Option<Cid>,
    pub balance: RefCell<TokenAmount>,
    pub received: TokenAmount,

    // VM Impl
    pub in_call: bool,
    pub store: MemoryBlockstore,
    pub in_transaction: bool,

    // Expectations
    pub expectations: RefCell<Expectations>,
}

#[derive(Default)]
pub struct Expectations {
    pub expect_validate_caller_any: bool,
    pub expect_validate_caller_addr: Option<Vec<Address>>,
    pub expect_validate_caller_type: Option<Vec<Cid>>,
    pub expect_sends: VecDeque<ExpectedMessage>,
    pub expect_create_actor: Option<ExpectCreateActor>,
    pub expect_delete_actor: Option<Address>,
    pub expect_verify_sigs: VecDeque<ExpectedVerifySig>,
    pub expect_verify_seal: Option<ExpectVerifySeal>,
    pub expect_verify_post: Option<ExpectVerifyPoSt>,
    pub expect_compute_unsealed_sector_cid: Option<ExpectComputeUnsealedSectorCid>,
    pub expect_verify_consensus_fault: Option<ExpectVerifyConsensusFault>,
}

impl Expectations {
    fn reset(&mut self) {
        self.expect_validate_caller_any = false;
        self.expect_validate_caller_addr = None;
        self.expect_validate_caller_type = None;
        self.expect_create_actor = None;
        self.expect_sends.clear();
        self.expect_verify_sigs.clear();
        self.expect_verify_seal = None;
        self.expect_verify_post = None;
        self.expect_compute_unsealed_sector_cid = None;
        self.expect_verify_consensus_fault = None;
    }
    fn verify(&mut self) {
        assert!(
            !self.expect_validate_caller_any,
            "expected ValidateCallerAny, not received"
        );
        assert!(
            self.expect_validate_caller_addr.is_none(),
            "expected ValidateCallerAddr {:?}, not received",
            self.expect_validate_caller_addr
        );
        assert!(
            self.expect_validate_caller_type.is_none(),
            "expected ValidateCallerType {:?}, not received",
            self.expect_validate_caller_type
        );
        assert!(
            self.expect_sends.is_empty(),
            "expected all message to be send, unsent messages {:?}",
            self.expect_sends
        );
        assert!(
            self.expect_create_actor.is_none(),
            "expected actor to be created, uncreated actor: {:?}",
            self.expect_create_actor
        );
        assert!(
            self.expect_verify_seal.is_none(),
            "expect_verify_seal {:?}, not received",
            self.expect_verify_seal.as_ref().unwrap()
        );
        assert!(
            self.expect_compute_unsealed_sector_cid.is_none(),
            "expect_compute_unsealed_sector_cid not received",
        );
        assert!(
            self.expect_verify_consensus_fault.is_none(),
            "expect_verify_consensus_fault not received",
        );
    }
}

impl Default for MockRuntime {
    fn default() -> Self {
        Self {
            epoch: Default::default(),
            miner: Address::new_id(0),
            base_fee: Default::default(),
            id_addresses: Default::default(),
            actor_code_cids: Default::default(),
            new_actor_addr: Default::default(),
            receiver: Address::new_id(0),
            caller: Address::new_id(0),
            caller_type: Default::default(),
            value_received: Default::default(),
            hash_func: Box::new(|_| [0u8; 32]),
            network_version: NetworkVersion::V0,
            state: Default::default(),
            balance: Default::default(),
            received: Default::default(),
            in_call: Default::default(),
            store: Default::default(),
            in_transaction: Default::default(),
            expectations: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpectCreateActor {
    pub code_id: Cid,
    pub actor_id: ActorID,
}
#[derive(Clone, Debug)]
pub struct ExpectedMessage {
    pub to: Address,
    pub method: MethodNum,
    pub params: RawBytes,
    pub value: TokenAmount,

    // returns from applying expectedMessage
    pub send_return: RawBytes,
    pub exit_code: ExitCode,
}

#[derive(Debug)]
pub struct ExpectedVerifySig {
    pub sig: Signature,
    pub signer: Address,
    pub plaintext: Vec<u8>,
    pub result: Result<(), anyhow::Error>,
}

#[derive(Clone, Debug)]
pub struct ExpectVerifySeal {
    seal: SealVerifyInfo,
    exit_code: ExitCode,
}

#[derive(Clone, Debug)]
pub struct ExpectVerifyPoSt {
    post: WindowPoStVerifyInfo,
    exit_code: ExitCode,
}

#[derive(Clone)]
pub struct ExpectVerifyConsensusFault {
    require_correct_input: bool,
    block_header_1: Vec<u8>,
    block_header_2: Vec<u8>,
    block_header_extra: Vec<u8>,
    fault: Option<ConsensusFault>,
    exit_code: ExitCode,
}

#[derive(Clone)]
pub struct ExpectComputeUnsealedSectorCid {
    reg: RegisteredSealProof,
    pieces: Vec<PieceInfo>,
    cid: Cid,
    exit_code: ExitCode,
}

impl MockRuntime {
    fn require_in_call(&self) {
        assert!(
            self.in_call,
            "invalid runtime invocation outside of method call",
        )
    }
    fn put<C: Cbor>(&self, o: &C) -> Result<Cid, ActorError> {
        Ok(self.store.put_cbor(&o, Code::Blake2b256).unwrap())
    }
    fn _get<T: DeserializeOwned>(&self, cid: Cid) -> Result<T, ActorError> {
        Ok(self.store.get_cbor(&cid).unwrap().unwrap())
    }

    #[allow(dead_code)]
    pub fn get_state<T: Cbor>(&self) -> Result<T, ActorError> {
        // TODO this doesn't handle errors exactly as go implementation
        self.state()
    }

    #[allow(dead_code)]
    pub fn expect_validate_caller_addr(&mut self, addr: Vec<Address>) {
        assert!(!addr.is_empty(), "addrs must be non-empty");
        self.expectations.get_mut().expect_validate_caller_addr = Some(addr);
    }

    #[allow(dead_code)]
    pub fn expect_verify_signature(&self, exp: ExpectedVerifySig) {
        self.expectations
            .borrow_mut()
            .expect_verify_sigs
            .push_back(exp);
    }

    #[allow(dead_code)]
    pub fn set_balance(&mut self, amount: TokenAmount) {
        *self.balance.get_mut() = amount;
    }

    #[allow(dead_code)]
    pub fn add_balance(&mut self, amount: TokenAmount) {
        *self.balance.get_mut() += amount;
    }

    #[allow(dead_code)]
    pub fn expect_verify_consensus_fault(
        &self,
        h1: Vec<u8>,
        h2: Vec<u8>,
        extra: Vec<u8>,
        fault: Option<ConsensusFault>,
        exit_code: ExitCode,
    ) {
        self.expectations.borrow_mut().expect_verify_consensus_fault =
            Some(ExpectVerifyConsensusFault {
                require_correct_input: true,
                block_header_1: h1,
                block_header_2: h2,
                block_header_extra: extra,
                fault,
                exit_code,
            });
    }

    #[allow(dead_code)]
    pub fn expect_compute_unsealed_sector_cid(&self, exp: ExpectComputeUnsealedSectorCid) {
        self.expectations
            .borrow_mut()
            .expect_compute_unsealed_sector_cid = Some(exp);
    }

    #[allow(dead_code)]
    pub fn expect_validate_caller_type(&mut self, types: Vec<Cid>) {
        assert!(!types.is_empty(), "addrs must be non-empty");
        self.expectations.borrow_mut().expect_validate_caller_type = Some(types);
    }

    #[allow(dead_code)]
    pub fn expect_validate_caller_any(&self) {
        self.expectations.borrow_mut().expect_validate_caller_any = true;
    }

    #[allow(dead_code)]
    pub fn expect_delete_actor(&mut self, beneficiary: Address) {
        self.expectations.borrow_mut().expect_delete_actor = Some(beneficiary);
    }

    pub fn call<A: ActorCode>(
        &mut self,
        method_num: MethodNum,
        params: &RawBytes,
    ) -> Result<RawBytes, ActorError> {
        self.in_call = true;
        let prev_state = self.state;
        let res = A::invoke_method(self, method_num, params);

        if res.is_err() {
            self.state = prev_state;
        }
        self.in_call = false;
        res
    }

    pub fn verify(&mut self) {
        self.expectations.borrow_mut().verify()
    }
    pub fn reset(&mut self) {
        self.expectations.borrow_mut().reset();
    }

    #[allow(dead_code)]
    pub fn expect_send(
        &mut self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
        send_return: RawBytes,
        exit_code: ExitCode,
    ) {
        self.expectations
            .borrow_mut()
            .expect_sends
            .push_back(ExpectedMessage {
                to,
                method,
                params,
                value,
                send_return,
                exit_code,
            })
    }

    #[allow(dead_code)]
    pub fn expect_create_actor(&mut self, code_id: Cid, actor_id: ActorID) {
        let a = ExpectCreateActor { code_id, actor_id };
        self.expectations.borrow_mut().expect_create_actor = Some(a);
    }

    #[allow(dead_code)]
    pub fn expect_verify_seal(&mut self, seal: SealVerifyInfo, exit_code: ExitCode) {
        let a = ExpectVerifySeal { seal, exit_code };
        self.expectations.borrow_mut().expect_verify_seal = Some(a);
    }

    #[allow(dead_code)]
    pub fn expect_verify_post(&mut self, post: WindowPoStVerifyInfo, exit_code: ExitCode) {
        let a = ExpectVerifyPoSt { post, exit_code };
        self.expectations.borrow_mut().expect_verify_post = Some(a);
    }

    #[allow(dead_code)]
    pub fn set_caller(&mut self, code_id: Cid, address: Address) {
        self.caller = address;
        self.caller_type = code_id;
        self.actor_code_cids.insert(address, code_id);
    }

    #[allow(dead_code)]
    pub fn set_value(&mut self, value: TokenAmount) {
        self.value_received = value;
    }

    #[allow(dead_code)]
    pub fn replace_state<C: Cbor>(&mut self, obj: &C) {
        self.state = Some(self.store.put_cbor(obj, Code::Blake2b256).unwrap());
    }
}

impl MessageInfo for MockRuntime {
    fn caller(&self) -> Address {
        self.caller
    }
    fn receiver(&self) -> Address {
        self.receiver
    }
    fn value_received(&self) -> TokenAmount {
        self.value_received.clone()
    }
}

impl Runtime<MemoryBlockstore> for MockRuntime {
    fn network_version(&self) -> NetworkVersion {
        self.network_version
    }

    fn message(&self) -> &dyn MessageInfo {
        self.require_in_call();
        self
    }

    fn curr_epoch(&self) -> ChainEpoch {
        self.require_in_call();
        self.epoch
    }

    fn validate_immediate_caller_accept_any(&mut self) -> Result<(), ActorError> {
        self.require_in_call();
        assert!(
            self.expectations.borrow_mut().expect_validate_caller_any,
            "unexpected validate-caller-any"
        );
        self.expectations.borrow_mut().expect_validate_caller_any = false;
        Ok(())
    }

    fn validate_immediate_caller_is<'a, I>(&mut self, addresses: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Address>,
    {
        self.require_in_call();

        let addrs: Vec<Address> = addresses.into_iter().cloned().collect();

        assert!(
            self.expectations
                .borrow_mut()
                .expect_validate_caller_addr
                .is_some(),
            "unexpected validate caller addrs"
        );
        assert_eq!(
            &addrs,
            self.expectations
                .borrow_mut()
                .expect_validate_caller_addr
                .as_ref()
                .unwrap(),
            "unexpected validate caller addrs {:?}, expected {:?}",
            addrs,
            self.expectations.borrow_mut().expect_validate_caller_addr
        );

        for expected in &addrs {
            if self.message().caller() == *expected {
                self.expectations.borrow_mut().expect_validate_caller_addr = None;
                return Ok(());
            }
        }
        self.expectations.borrow_mut().expect_validate_caller_addr = None;
        return Err(actor_error!(ErrForbidden;
                "caller address {:?} forbidden, allowed: {:?}",
                self.message().caller(), &addrs
        ));
    }
    fn validate_immediate_caller_type<'a, I>(&mut self, types: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Cid>,
    {
        self.require_in_call();
        let types: Vec<Cid> = types.into_iter().cloned().collect();
        assert!(
            self.expectations
                .borrow_mut()
                .expect_validate_caller_type
                .is_some(),
            "unexpected validate caller code"
        );
        assert_eq!(
            &types,
            self.expectations
                .borrow_mut()
                .expect_validate_caller_type
                .as_ref()
                .unwrap(),
            "unexpected validate caller code {:?}, expected {:?}",
            types,
            self.expectations.borrow_mut().expect_validate_caller_type
        );

        for expected in &types {
            if &self.caller_type == expected {
                self.expectations.borrow_mut().expect_validate_caller_type = None;
                return Ok(());
            }
        }

        self.expectations.borrow_mut().expect_validate_caller_type = None;

        Err(
            actor_error!(ErrForbidden; "caller type {:?} forbidden, allowed: {:?}",
                self.caller_type, types),
        )
    }

    fn current_balance(&self) -> TokenAmount {
        self.require_in_call();
        self.balance.borrow().clone()
    }

    fn resolve_address(&self, address: &Address) -> Option<Address> {
        self.require_in_call();
        if address.protocol() == Protocol::ID {
            return Some(*address);
        }

        self.id_addresses.get(address).cloned()
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Option<Cid> {
        self.require_in_call();

        self.actor_code_cids.get(addr).cloned()
    }

    fn get_randomness_from_tickets(
        &self,
        _personalization: DomainSeparationTag,
        _rand_epoch: ChainEpoch,
        _entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        unimplemented!()
    }

    fn get_randomness_from_beacon(
        &self,
        _personalization: DomainSeparationTag,
        _rand_epoch: ChainEpoch,
        _entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        unimplemented!()
    }

    fn create<C: Cbor>(&mut self, obj: &C) -> Result<(), ActorError> {
        if self.state.is_some() {
            return Err(actor_error!(SysErrIllegalActor; "state already constructed"));
        }
        self.state = Some(self.store.put_cbor(obj, Code::Blake2b256).unwrap());
        Ok(())
    }

    fn state<C: Cbor>(&self) -> Result<C, ActorError> {
        Ok(self
            .store
            .get_cbor(self.state.as_ref().unwrap())
            .unwrap()
            .unwrap())
    }

    fn transaction<C, RT, F>(&mut self, f: F) -> Result<RT, ActorError>
    where
        C: Cbor,
        F: FnOnce(&mut C, &mut Self) -> Result<RT, ActorError>,
    {
        if self.in_transaction {
            return Err(actor_error!(SysErrIllegalActor; "nested transaction"));
        }
        let mut read_only = self.state()?;
        self.in_transaction = true;
        let ret = f(&mut read_only, self)?;
        self.state = Some(self.put(&read_only).unwrap());
        self.in_transaction = false;
        Ok(ret)
    }

    fn store(&self) -> &MemoryBlockstore {
        &self.store
    }

    fn send(
        &self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    ) -> Result<RawBytes, ActorError> {
        self.require_in_call();
        if self.in_transaction {
            return Err(actor_error!(SysErrIllegalActor; "side-effect within transaction"));
        }

        assert!(
            !self.expectations.borrow_mut().expect_sends.is_empty(),
            "unexpected expectedMessage to: {:?} method: {:?}, value: {:?}, params: {:?}",
            to,
            method,
            value,
            params
        );

        let expected_msg = self
            .expectations
            .borrow_mut()
            .expect_sends
            .pop_front()
            .unwrap();

        assert!(expected_msg.to == to && expected_msg.method == method && expected_msg.params == params && expected_msg.value == value,
            "expectedMessage being sent does not match expectation.\nMessage -\t to: {:?} method: {:?} value: {:?} params: {:?}\nExpected -\t {:?}",
            to, method, value, params, expected_msg);

        {
            let mut balance = self.balance.borrow_mut();
            if value > *balance {
                return Err(actor_error!(SysErrSenderStateInvalid;
                        "cannot send value: {:?} exceeds balance: {:?}",
                        value, *balance
                ));
            }
            *balance -= value;
        }

        match expected_msg.exit_code {
            ExitCode::Ok => Ok(expected_msg.send_return),
            x => Err(ActorError::new(x, "Expected message Fail".to_string())),
        }
    }

    fn new_actor_address(&mut self) -> Result<Address, ActorError> {
        self.require_in_call();
        let ret = *self
            .new_actor_addr
            .as_ref()
            .expect("unexpected call to new actor address");
        self.new_actor_addr = None;
        Ok(ret)
    }

    fn create_actor(&mut self, code_id: Cid, actor_id: ActorID) -> Result<(), ActorError> {
        self.require_in_call();
        if self.in_transaction {
            return Err(actor_error!(SysErrIllegalActor; "side-effect within transaction"));
        }
        let expect_create_actor = self
            .expectations
            .borrow_mut()
            .expect_create_actor
            .take()
            .expect("unexpected call to create actor");

        assert!(expect_create_actor.code_id == code_id && expect_create_actor.actor_id == actor_id, "unexpected actor being created, expected code: {:?} address: {:?}, actual code: {:?} address: {:?}", expect_create_actor.code_id, expect_create_actor.actor_id, code_id, actor_id);
        Ok(())
    }

    fn delete_actor(&mut self, addr: &Address) -> Result<(), ActorError> {
        self.require_in_call();
        if self.in_transaction {
            return Err(actor_error!(SysErrIllegalActor; "side-effect within transaction"));
        }
        let exp_act = self.expectations.borrow_mut().expect_delete_actor.take();
        if exp_act.is_none() {
            panic!("unexpected call to delete actor: {}", addr);
        }
        if exp_act.as_ref().unwrap() != addr {
            panic!(
                "attempt to delete wrong actor. Expected: {}, got: {}",
                exp_act.unwrap(),
                addr
            );
        }
        Ok(())
    }

    fn total_fil_circ_supply(&self) -> TokenAmount {
        unimplemented!();
    }

    fn charge_gas(&mut self, _: &'static str, _: i64) {
        // TODO implement functionality if needed for testing
    }

    fn base_fee(&self) -> TokenAmount {
        self.base_fee.clone()
    }
}

impl Syscalls for MockRuntime {
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> anyhow::Result<()> {
        if self.expectations.borrow_mut().expect_verify_sigs.is_empty() {
            panic!(
                "Unexpected signature verification sig: {:?}, signer: {}, plaintext: {}",
                signature,
                signer,
                hex::encode(plaintext)
            );
        }
        let exp = self
            .expectations
            .borrow_mut()
            .expect_verify_sigs
            .pop_front();
        if let Some(exp) = exp {
            if exp.sig != *signature || exp.signer != *signer || &exp.plaintext[..] != plaintext {
                panic!(
                    "unexpected signature verification\n\
                    sig: {:?}, signer: {}, plaintext: {}\n\
                    expected sig: {:?}, signer: {}, plaintext: {}",
                    signature,
                    signer,
                    hex::encode(plaintext),
                    exp.sig,
                    exp.signer,
                    hex::encode(exp.plaintext)
                )
            }
            exp.result?
        } else {
            panic!(
                "unexpected syscall to verify signature: {:?}, signer: {}, plaintext: {}",
                signature,
                signer,
                hex::encode(plaintext)
            )
        }
        Ok(())
    }

    fn hash_blake2b(&self, data: &[u8]) -> [u8; 32] {
        blake2b_256(data)
    }
    fn compute_unsealed_sector_cid(
        &self,
        reg: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> anyhow::Result<Cid> {
        let exp = self
            .expectations
            .borrow_mut()
            .expect_compute_unsealed_sector_cid
            .take()
            .ok_or_else(
                || actor_error!(ErrIllegalState; "Unexpected syscall to ComputeUnsealedSectorCID"),
            )?;

        if exp.reg != reg {
            return Err(anyhow!(actor_error!(ErrIllegalState;
                "Unexpected compute_unsealed_sector_cid : reg mismatch"
            )));
        }

        if exp.pieces[..].eq(pieces) {
            return Err(anyhow!(actor_error!(ErrIllegalState;
                "Unexpected compute_unsealed_sector_cid : pieces mismatch"
            )));
        }

        if exp.exit_code != ExitCode::Ok {
            return Err(anyhow!(ActorError::new(
                exp.exit_code,
                "Expected Failure".to_string(),
            )));
        }
        Ok(exp.cid)
    }
    fn verify_seal(&self, seal: &SealVerifyInfo) -> anyhow::Result<()> {
        let exp = self
            .expectations
            .borrow_mut()
            .expect_verify_seal
            .take()
            .ok_or_else(|| actor_error!(ErrIllegalState; "Unexpected syscall to verify seal"))?;

        if exp.seal != *seal {
            return Err(anyhow!(
                actor_error!(ErrIllegalState; "Unexpected seal verification"),
            ));
        }
        if exp.exit_code != ExitCode::Ok {
            return Err(anyhow!(ActorError::new(
                exp.exit_code,
                "Expected Failure".to_string(),
            )));
        }
        Ok(())
    }
    fn verify_post(&self, post: &WindowPoStVerifyInfo) -> anyhow::Result<()> {
        let exp = self
            .expectations
            .borrow_mut()
            .expect_verify_post
            .take()
            .ok_or_else(|| actor_error!(ErrIllegalState; "Unexpected syscall to verify PoSt"))?;

        if exp.post != *post {
            return Err(anyhow!(
                actor_error!(ErrIllegalState; "Unexpected PoSt verification"),
            ));
        }
        if exp.exit_code != ExitCode::Ok {
            return Err(anyhow!(ActorError::new(
                exp.exit_code,
                "Expected Failure".to_string(),
            )));
        }
        Ok(())
    }
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<Option<ConsensusFault>> {
        let exp = self
            .expectations
            .borrow_mut()
            .expect_verify_consensus_fault
            .take()
            .ok_or_else(
                || actor_error!(ErrIllegalState; "Unexpected syscall to verify_consensus_fault"),
            )?;
        if exp.require_correct_input {
            if exp.block_header_1 != h1 {
                return Err(anyhow!(actor_error!(ErrIllegalState; "Header 1 mismatch")));
            }
            if exp.block_header_2 != h2 {
                return Err(anyhow!(actor_error!(ErrIllegalState; "Header 2 mismatch")));
            }
            if exp.block_header_extra != extra {
                return Err(anyhow!(
                    actor_error!(ErrIllegalState; "Header extra mismatch"),
                ));
            }
        }
        if exp.exit_code != ExitCode::Ok {
            return Err(anyhow!(ActorError::new(
                exp.exit_code,
                "Expected Failure".to_string(),
            )));
        }
        Ok(exp.fault)
    }
    fn verify_aggregate_seals(
        &self,
        _aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> anyhow::Result<()> {
        // TODO: Implement this if we need it. Currently don't have a need.
        todo!()
    }
}
