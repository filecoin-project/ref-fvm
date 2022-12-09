// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;

use anyhow::Context;
use fvm_shared::crypto::signature::SignatureType;
use fvm_shared::econ::TokenAmount;
use fvm_shared::event::{ActorEvent, Flags};
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredPoStProof, RegisteredSealProof, ReplicaUpdateInfo,
    SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{MethodNum, METHOD_SEND};
use fvm_wasm_instrument::gas_metering::{InstructionCost, Operator, Rules};
use lazy_static::lazy_static;
use num_traits::Zero;

use super::GasCharge;
use crate::gas::Gas;

lazy_static! {
    static ref OH_SNAP_PRICES: PriceList = PriceList {
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: Gas::new(38863),
        on_chain_message_storage_base: Gas::new(36),
        on_chain_message_storage_per_byte: Gas::new(1),

        on_chain_return_value_per_byte: Gas::new(1),

        send_base: Gas::new(29233),
        send_transfer_funds: Gas::new(27500),
        send_transfer_only_premium: Gas::new(159672),
        send_invoke_method: Gas::new(-5377),

        create_actor_compute: Gas::new(1108454),
        create_actor_storage: Gas::new(36 + 40),
        delete_actor: Gas::new(-(36 + 40)),

        bls_sig_cost: Gas::new(16598605),
        secp256k1_sig_cost: Gas::new(1637292),
        secp256k1_recover_cost: Gas::new(1637292), // TODO measure & revisit this value

        hashing_base: Gas::new(31355),
        compute_unsealed_sector_cid_base: Gas::new(98647),
        verify_seal_base: Gas::new(2000), // TODO revisit potential removal of this

        verify_aggregate_seal_base: Zero::zero(),
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                Gas::new(449900)
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                Gas::new(359272)
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(103994170)},
                        Step{start: 7, cost: Gas::new(112356810)},
                        Step{start: 13, cost: Gas::new(122912610)},
                        Step{start: 26, cost: Gas::new(137559930)},
                        Step{start: 52, cost: Gas::new(162039100)},
                        Step{start: 103, cost: Gas::new(210960780)},
                        Step{start: 205, cost: Gas::new(318351180)},
                        Step{start: 410, cost: Gas::new(528274980)},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(102581240)},
                        Step{start: 7, cost: Gas::new(110803030)},
                        Step{start: 13, cost: Gas::new(120803700)},
                        Step{start: 26, cost: Gas::new(134642130)},
                        Step{start: 52, cost: Gas::new(157357890)},
                        Step{start: 103, cost: Gas::new(203017690)},
                        Step{start: 205, cost: Gas::new(304253590)},
                        Step{start: 410, cost: Gas::new(509880640)},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        verify_consensus_fault: Gas::new(495422),
        verify_replica_update: Gas::new(36316136),
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        get_randomness_base: Zero::zero(),
        get_randomness_per_byte: Zero::zero(),

        block_memcpy_per_byte_cost: Zero::zero(),

        block_open_base: Gas::new(114617),
        block_open_memret_per_byte_cost: Zero::zero(),

        block_link_base: Gas::new(353640),
        block_link_storage_per_byte_cost: Gas::new(1),

        block_create_base: Zero::zero(),
        block_create_memret_per_byte_cost: Zero::zero(),

        block_read_base: Zero::zero(),
        block_stat_base: Zero::zero(),

        syscall_cost: Zero::zero(),
        extern_cost: Zero::zero(),

        wasm_rules: WasmGasPrices{
            exec_instruction_cost: Zero::zero(),
            memory_expansion_per_byte_cost: Zero::zero(),
        },

        event_emit_base_cost: Zero::zero(),
        event_per_entry_cost: Zero::zero(),
        event_entry_index_cost: Zero::zero(),
        event_per_byte_cost: Zero::zero(),

        state_read_base: Zero::zero(),
        state_write_base: Zero::zero(),
        builtin_actor_base: Zero::zero(),
        context_base: Zero::zero(),
        install_wasm_per_byte_cost: Zero::zero(),
    };

    static ref SKYR_PRICES: PriceList = PriceList {
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: Gas::new(38863),
        on_chain_message_storage_base: Gas::new(36),
        on_chain_message_storage_per_byte: Gas::new(1),

        on_chain_return_value_per_byte: Gas::new(1),

        send_base: Gas::new(29233),
        send_transfer_funds: Gas::new(27500),
        send_transfer_only_premium: Gas::new(159672),
        send_invoke_method: Gas::new(-5377),

        create_actor_compute: Gas::new(1108454),
        create_actor_storage: Gas::new(36 + 40),
        delete_actor: Gas::new(-(36 + 40)),

        bls_sig_cost: Gas::new(16598605),
        secp256k1_sig_cost: Gas::new(1637292),
        secp256k1_recover_cost: Gas::new(1637292), // TODO measure & revisit this value

        hashing_base: Gas::new(31355),
        compute_unsealed_sector_cid_base: Gas::new(98647),
        verify_seal_base: Gas::new(2000), // TODO revisit potential removal of this

        verify_aggregate_seal_base: Zero::zero(),
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                Gas::new(449900)
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                Gas::new(359272)
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(103994170)},
                        Step{start: 7, cost: Gas::new(112356810)},
                        Step{start: 13, cost: Gas::new(122912610)},
                        Step{start: 26, cost: Gas::new(137559930)},
                        Step{start: 52, cost: Gas::new(162039100)},
                        Step{start: 103, cost: Gas::new(210960780)},
                        Step{start: 205, cost: Gas::new(318351180)},
                        Step{start: 410, cost: Gas::new(528274980)},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(102581240)},
                        Step{start: 7, cost: Gas::new(110803030)},
                        Step{start: 13, cost: Gas::new(120803700)},
                        Step{start: 26, cost: Gas::new(134642130)},
                        Step{start: 52, cost: Gas::new(157357890)},
                        Step{start: 103, cost: Gas::new(203017690)},
                        Step{start: 205, cost: Gas::new(304253590)},
                        Step{start: 410, cost: Gas::new(509880640)},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        verify_consensus_fault: Gas::new(495422),
        verify_replica_update: Gas::new(36316136),
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        get_randomness_base: Zero::zero(),
        get_randomness_per_byte: Zero::zero(),

        block_memcpy_per_byte_cost: Gas::from_milligas(500),

        block_open_base: Gas::new(114617),
        block_open_memret_per_byte_cost: Gas::new(10),

        block_link_base: Gas::new(353640),
        block_link_storage_per_byte_cost: Gas::new(1),

        block_create_base: Zero::zero(),
        block_create_memret_per_byte_cost: Gas::new(10),

        block_read_base: Zero::zero(),
        block_stat_base: Zero::zero(),

        syscall_cost: Gas::new(14000),
        extern_cost: Gas::new(21000),

        wasm_rules: WasmGasPrices{
            exec_instruction_cost: Gas::new(4),
            memory_expansion_per_byte_cost: Zero::zero(),
        },

        event_emit_base_cost: Zero::zero(),
        event_per_entry_cost: Zero::zero(),
        event_entry_index_cost: Zero::zero(),
        event_per_byte_cost: Zero::zero(),

        state_read_base: Zero::zero(),
        state_write_base: Zero::zero(),
        builtin_actor_base: Zero::zero(),
        context_base: Zero::zero(),
        install_wasm_per_byte_cost: Zero::zero(),
    };

    static ref HYGGE_PRICES: PriceList = PriceList {
        // START (Copied from SKYR_PRICES)

        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: Gas::new(38863),
        on_chain_message_storage_base: Gas::new(36),
        on_chain_message_storage_per_byte: Gas::new(1),

        on_chain_return_value_per_byte: Gas::new(1),

        send_base: Gas::new(29233),
        send_transfer_funds: Gas::new(27500),
        send_transfer_only_premium: Gas::new(159672),
        send_invoke_method: Gas::new(-5377),

        create_actor_compute: Gas::new(1108454),
        create_actor_storage: Gas::new(36 + 40),
        delete_actor: Gas::new(-(36 + 40)),

        bls_sig_cost: Gas::new(16598605),
        secp256k1_sig_cost: Gas::new(1637292),
        secp256k1_recover_cost: Gas::new(1637292), // TODO measure & revisit this value

        hashing_base: Gas::new(31355),
        compute_unsealed_sector_cid_base: Gas::new(98647),
        verify_seal_base: Gas::new(2000), // TODO revisit potential removal of this

        verify_aggregate_seal_base: Zero::zero(),
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                Gas::new(449900)
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                Gas::new(359272)
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(103994170)},
                        Step{start: 7, cost: Gas::new(112356810)},
                        Step{start: 13, cost: Gas::new(122912610)},
                        Step{start: 26, cost: Gas::new(137559930)},
                        Step{start: 52, cost: Gas::new(162039100)},
                        Step{start: 103, cost: Gas::new(210960780)},
                        Step{start: 205, cost: Gas::new(318351180)},
                        Step{start: 410, cost: Gas::new(528274980)},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: Gas::new(102581240)},
                        Step{start: 7, cost: Gas::new(110803030)},
                        Step{start: 13, cost: Gas::new(120803700)},
                        Step{start: 26, cost: Gas::new(134642130)},
                        Step{start: 52, cost: Gas::new(157357890)},
                        Step{start: 103, cost: Gas::new(203017690)},
                        Step{start: 205, cost: Gas::new(304253590)},
                        Step{start: 410, cost: Gas::new(509880640)},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        verify_consensus_fault: Gas::new(495422),
        verify_replica_update: Gas::new(36316136),
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: Gas::new(117680921),
                    scale: Gas::new(43780),
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        get_randomness_base: Zero::zero(),
        get_randomness_per_byte: Zero::zero(),

        block_memcpy_per_byte_cost: Gas::from_milligas(500),

        block_open_base: Gas::new(114617),
        block_open_memret_per_byte_cost: Gas::new(10),

        block_link_base: Gas::new(353640),
        block_link_storage_per_byte_cost: Gas::new(1),

        block_create_base: Zero::zero(),
        block_create_memret_per_byte_cost: Gas::new(10),

        block_read_base: Zero::zero(),
        block_stat_base: Zero::zero(),

        syscall_cost: Gas::new(14000),
        extern_cost: Gas::new(21000),

        wasm_rules: WasmGasPrices{
            exec_instruction_cost: Gas::new(4),
            memory_expansion_per_byte_cost: Zero::zero(),
        },

        // END (Copied from SKYR_PRICES)

        // TODO GAS_PARAM
        event_emit_base_cost: Zero::zero(),
        // TODO GAS_PARAM
        event_per_entry_cost: Zero::zero(),
        // TODO GAS_PARAM
        event_entry_index_cost: Zero::zero(),
        // TODO GAS_PARAM
        event_per_byte_cost: Zero::zero(),

        state_read_base: Zero::zero(),
        state_write_base: Zero::zero(),
        builtin_actor_base: Zero::zero(),
        context_base: Zero::zero(),
        install_wasm_per_byte_cost: Zero::zero(),
    };
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) struct ScalingCost {
    flat: Gas,
    scale: Gas,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StepCost(Vec<Step>);

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) struct Step {
    start: i64,
    cost: Gas,
}

impl StepCost {
    pub(crate) fn lookup(&self, x: i64) -> Gas {
        let mut i: i64 = 0;
        while i < self.0.len() as i64 {
            if self.0[i as usize].start > x {
                break;
            }
            i += 1;
        }
        i -= 1;
        if i < 0 {
            return Gas::zero();
        }
        self.0[i as usize].cost
    }
}

/// Provides prices for operations in the VM.
/// All costs are in milligas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceList {
    /// Storage gas charge multiplier
    pub(crate) storage_gas_multiplier: i64,

    /// Gas cost charged to the originator of an on-chain message (regardless of
    /// whether it succeeds or fails in application) is given by:
    ///   OnChainMessageBase + len(serialized message)*OnChainMessagePerByte
    /// Together, these account for the cost of message propagation and validation,
    /// up to but excluding any actual processing by the VM.
    /// This is the cost a block producer burns when including an invalid message.
    pub(crate) on_chain_message_compute_base: Gas,
    pub(crate) on_chain_message_storage_base: Gas,
    pub(crate) on_chain_message_storage_per_byte: Gas,

    /// Gas cost charged to the originator of a non-nil return value produced
    /// by an on-chain message is given by:
    ///   len(return value)*OnChainReturnValuePerByte
    pub(crate) on_chain_return_value_per_byte: Gas,

    /// Gas cost for any message send execution(including the top-level one
    /// initiated by an on-chain message).
    /// This accounts for the cost of loading sender and receiver actors and
    /// (for top-level messages) incrementing the sender's sequence number.
    /// Load and store of actor sub-state is charged separately.
    pub(crate) send_base: Gas,

    /// Gas cost charged, in addition to SendBase, if a message send
    /// is accompanied by any nonzero currency amount.
    /// Accounts for writing receiver's new balance (the sender's state is
    /// already accounted for).
    pub(crate) send_transfer_funds: Gas,

    /// Gas cost charged, in addition to SendBase, if message only transfers funds.
    pub(crate) send_transfer_only_premium: Gas,

    /// Gas cost charged, in addition to SendBase, if a message invokes
    /// a method on the receiver.
    /// Accounts for the cost of loading receiver code and method dispatch.
    pub(crate) send_invoke_method: Gas,

    /// Gas cost for creating a new actor (via InitActor's Exec method).
    /// Note: this costs assume that the extra will be partially or totally refunded while
    /// the base is covering for the put.
    pub(crate) create_actor_compute: Gas,
    pub(crate) create_actor_storage: Gas,

    /// Gas cost for deleting an actor.
    /// Note: this partially refunds the create cost to incentivise the deletion of the actors.
    pub(crate) delete_actor: Gas,

    /// Gas cost for verifying bls signature
    pub(crate) bls_sig_cost: Gas,
    /// Gas cost for verifying secp256k1 signature
    pub(crate) secp256k1_sig_cost: Gas,
    /// Gas cost for recovering secp256k1 signer public key
    pub(crate) secp256k1_recover_cost: Gas,

    pub(crate) hashing_base: Gas,

    pub(crate) compute_unsealed_sector_cid_base: Gas,
    pub(crate) verify_seal_base: Gas,
    #[allow(unused)]
    pub(crate) verify_aggregate_seal_base: Gas,
    pub(crate) verify_aggregate_seal_per: HashMap<RegisteredSealProof, Gas>,
    pub(crate) verify_aggregate_seal_steps: HashMap<RegisteredSealProof, StepCost>,

    pub(crate) verify_post_lookup: HashMap<RegisteredPoStProof, ScalingCost>,
    pub(crate) verify_consensus_fault: Gas,
    pub(crate) verify_replica_update: Gas,

    /// Gas cost for fetching randomness.
    pub(crate) get_randomness_base: Gas,
    /// Gas cost per every byte of randomness fetched.
    pub(crate) get_randomness_per_byte: Gas,

    /// Gas cost per every block byte memcopied across boundaries.
    pub(crate) block_memcpy_per_byte_cost: Gas,

    /// Gas cost for opening a block.
    pub(crate) block_open_base: Gas,
    /// Gas cost for every byte retained in FVM space when opening a block.
    pub(crate) block_open_memret_per_byte_cost: Gas,

    /// Gas cost for linking a block.
    pub(crate) block_link_base: Gas,
    /// Multiplier for storage gas per byte.
    pub(crate) block_link_storage_per_byte_cost: Gas,

    /// Gas cost for creating a block.
    pub(crate) block_create_base: Gas,
    /// Gas cost for every byte retained in FVM space when writing a block.
    pub(crate) block_create_memret_per_byte_cost: Gas,

    /// Gas cost for reading a block into actor space.
    pub(crate) block_read_base: Gas,
    /// Gas cost for statting a block.
    pub(crate) block_stat_base: Gas,

    /// General gas cost for performing a syscall, accounting for the overhead thereof.
    pub(crate) syscall_cost: Gas,
    /// General gas cost for calling an extern, accounting for the overhead thereof.
    pub(crate) extern_cost: Gas,

    /// Rules for execution gas.
    pub(crate) wasm_rules: WasmGasPrices,

    // Event-related pricing factors.
    pub(crate) event_emit_base_cost: Gas,
    pub(crate) event_per_entry_cost: Gas,
    pub(crate) event_entry_index_cost: Gas,
    pub(crate) event_per_byte_cost: Gas,

    /// Gas cost of looking up an actor in the common state tree.
    ///
    /// The cost varies depending on whether the data is cached, and how big the state tree is,
    /// but that is independent of the contract in question. Might need periodic repricing.
    pub(crate) state_read_base: Gas,

    /// Gas cost of storing an updated actor in the common state tree.
    ///
    /// The cost varies depending on how big the state tree is, and how many other writes will be
    /// buffered together by the end of the calls when changes are flushed. Might need periodic repricing.
    pub(crate) state_write_base: Gas,

    /// Gas cost of doing lookups in the builtin actor mappings.
    pub(crate) builtin_actor_base: Gas,

    /// Gas cost of accessing the machine context.
    pub(crate) context_base: Gas,

    /// Gas cost of compiling a Wasm module during install.
    pub(crate) install_wasm_per_byte_cost: Gas,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WasmGasPrices {
    pub(crate) exec_instruction_cost: Gas,
    /// Gas cost for every byte made writeable in Wasm memory.
    pub(crate) memory_expansion_per_byte_cost: Gas,
}

impl PriceList {
    /// Returns the gas required for storing a message of a given size in the chain.
    #[inline]
    pub fn on_chain_message(&self, msg_size: usize) -> GasCharge {
        GasCharge::new(
            "OnChainMessage",
            self.on_chain_message_compute_base,
            (self.on_chain_message_storage_base
                + self.on_chain_message_storage_per_byte * msg_size)
                * self.storage_gas_multiplier,
        )
    }

    /// Returns the gas required for storing the response of a message in the chain.
    #[inline]
    pub fn on_chain_return_value(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnChainReturnValue",
            Zero::zero(),
            self.on_chain_return_value_per_byte * data_size * self.storage_gas_multiplier,
        )
    }

    /// Returns the gas required when invoking a method.
    #[inline]
    pub fn on_method_invocation(&self, value: &TokenAmount, method_num: MethodNum) -> GasCharge {
        let mut ret = self.send_base;
        if value != &TokenAmount::zero() {
            ret += self.send_transfer_funds;
            if method_num == METHOD_SEND {
                ret += self.send_transfer_only_premium;
            }
        }
        if method_num != METHOD_SEND {
            ret += self.send_invoke_method;
        }
        GasCharge::new("OnMethodInvocation", ret, Zero::zero())
    }

    /// Returns the gas cost to be applied on a syscall.
    pub fn on_syscall(&self) -> GasCharge {
        GasCharge::new("OnSyscall", self.syscall_cost, Zero::zero())
    }

    /// Returns the gas required for creating an actor.
    #[inline]
    pub fn on_create_actor(&self, is_new: bool) -> GasCharge {
        let storage_gas = if is_new {
            self.create_actor_storage * self.storage_gas_multiplier
        } else {
            Gas::zero()
        };
        GasCharge::new("OnCreateActor", self.create_actor_compute, storage_gas)
    }

    /// Returns the gas required for deleting an actor.
    #[inline]
    pub fn on_delete_actor(&self) -> GasCharge {
        GasCharge::new(
            "OnDeleteActor",
            Zero::zero(),
            self.delete_actor * self.storage_gas_multiplier,
        )
    }

    /// Returns gas required for signature verification.
    #[inline]
    pub fn on_verify_signature(&self, sig_type: SignatureType, _data_len: usize) -> GasCharge {
        let val = match sig_type {
            SignatureType::BLS => self.bls_sig_cost,
            SignatureType::Secp256k1 => self.secp256k1_sig_cost,
        };
        GasCharge::new("OnVerifySignature", val, Zero::zero())
    }

    /// Returns gas required for recovering signer pubkey from signature
    #[inline]
    pub fn on_recover_secp_public_key(&self) -> GasCharge {
        GasCharge::new(
            "OnRecoverSecpPublicKey",
            self.secp256k1_recover_cost,
            Zero::zero(),
        )
    }

    /// Returns gas required for hashing data.
    #[inline]
    pub fn on_hashing(&self, _data_len: usize) -> GasCharge {
        GasCharge::new("OnHashing", self.hashing_base, Zero::zero())
    }

    /// Returns gas required for computing unsealed sector Cid.
    #[inline]
    pub fn on_compute_unsealed_sector_cid(
        &self,
        _proof: RegisteredSealProof,
        _pieces: &[PieceInfo],
    ) -> GasCharge {
        GasCharge::new(
            "OnComputeUnsealedSectorCid",
            self.compute_unsealed_sector_cid_base,
            Zero::zero(),
        )
    }

    /// Returns gas required for seal verification.
    #[inline]
    pub fn on_verify_seal(&self, _info: &SealVerifyInfo) -> GasCharge {
        GasCharge::new("OnVerifySeal", self.verify_seal_base, Zero::zero())
    }
    #[inline]
    pub fn on_verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> GasCharge {
        let proof_type = aggregate.seal_proof;
        let per_proof = *self
            .verify_aggregate_seal_per
            .get(&proof_type)
            .unwrap_or_else(|| {
                self.verify_aggregate_seal_per
                    .get(&RegisteredSealProof::StackedDRG32GiBV1P1)
                    .expect(
                        "There is an implementation error where proof type does not exist in table",
                    )
            });

        let step = self
            .verify_aggregate_seal_steps
            .get(&proof_type)
            .unwrap_or_else(|| {
                self.verify_aggregate_seal_steps
                    .get(&RegisteredSealProof::StackedDRG32GiBV1P1)
                    .expect(
                        "There is an implementation error where proof type does not exist in table",
                    )
            });
        // Should be safe because there is a limit to how much seals get aggregated
        let num = aggregate.infos.len() as i64;
        GasCharge::new(
            "OnVerifyAggregateSeals",
            per_proof * num + step.lookup(num),
            Zero::zero(),
        )
    }

    /// Returns gas required for replica verification.
    #[inline]
    pub fn on_verify_replica_update(&self, _replica: &ReplicaUpdateInfo) -> GasCharge {
        GasCharge::new(
            "OnVerifyReplicaUpdate",
            self.verify_replica_update,
            Zero::zero(),
        )
    }

    /// Returns gas required for PoSt verification.
    #[inline]
    pub fn on_verify_post(&self, info: &WindowPoStVerifyInfo) -> GasCharge {
        let p_proof = info
            .proofs
            .first()
            .map(|p| p.post_proof)
            .unwrap_or(RegisteredPoStProof::StackedDRGWindow512MiBV1);
        let cost = self.verify_post_lookup.get(&p_proof).unwrap_or_else(|| {
            self.verify_post_lookup
                .get(&RegisteredPoStProof::StackedDRGWindow512MiBV1)
                .expect("512MiB lookup must exist in price table")
        });

        let gas_used = cost.flat + cost.scale * info.challenged_sectors.len();

        GasCharge::new("OnVerifyPost", gas_used, Zero::zero())
    }

    /// Returns gas required for verifying consensus fault.
    #[inline]
    pub fn on_verify_consensus_fault(
        &self,
        _h1_len: usize,
        _h2_len: usize,
        _extra_len: usize,
    ) -> GasCharge {
        GasCharge::new(
            "OnVerifyConsensusFault",
            self.extern_cost + self.verify_consensus_fault,
            Zero::zero(),
        )
    }

    /// Returns the cost of the gas required for getting randomness from the client, based on the
    /// numebr of bytes of entropy.
    #[inline]
    pub fn on_get_randomness(&self, entropy_size: usize) -> GasCharge {
        GasCharge::new(
            "OnGetRandomness",
            self.extern_cost
                + self.get_randomness_base
                + (self.get_randomness_per_byte * entropy_size),
            Zero::zero(),
        )
    }

    /// Returns the base gas required for loading an object, independent of the object's size.
    #[inline]
    pub fn on_block_open_base(&self) -> GasCharge {
        GasCharge::new(
            "OnBlockOpenBase",
            self.extern_cost + self.block_open_base,
            Zero::zero(),
        )
    }

    /// Returns the gas required for loading an object based on the size of the object.
    #[inline]
    pub fn on_block_open_per_byte(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnBlockOpenPerByte",
            (self.block_open_memret_per_byte_cost * data_size)
                + (self.block_memcpy_per_byte_cost * data_size),
            Zero::zero(),
        )
    }

    /// Returns the gas required for reading a loaded object.
    #[inline]
    pub fn on_block_read(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnBlockRead",
            self.block_read_base + (self.block_memcpy_per_byte_cost * data_size),
            Zero::zero(),
        )
    }

    /// Returns the gas required for adding an object to the FVM cache.
    #[inline]
    pub fn on_block_create(&self, data_size: usize) -> GasCharge {
        let mem_costs = (self.block_create_memret_per_byte_cost * data_size)
            + (self.block_memcpy_per_byte_cost * data_size);
        GasCharge::new(
            "OnBlockCreate",
            self.block_create_base + mem_costs,
            Zero::zero(),
        )
    }

    /// Returns the gas required for committing an object to the state blockstore.
    #[inline]
    pub fn on_block_link(&self, data_size: usize) -> GasCharge {
        let memcpy = self.block_memcpy_per_byte_cost * data_size;
        GasCharge::new(
            "OnBlockLink",
            // twice the memcpy cost:
            // - one from the block registry to the FVM BufferedBlockstore
            // - one from the FVM BufferedBlockstore to the Node's Blockstore
            //   when the machine finishes.
            self.block_link_base + (memcpy * 2),
            self.block_link_storage_per_byte_cost * self.storage_gas_multiplier * data_size,
        )
    }

    /// Returns the gas required for storing an object.
    #[inline]
    pub fn on_block_stat(&self) -> GasCharge {
        GasCharge::new("OnBlockStat", self.block_stat_base, Zero::zero())
    }

    /// Returns the gas required for accessing the actor state root.
    #[inline]
    pub fn on_root(&self) -> GasCharge {
        GasCharge::new("OnRoot", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for modifying the actor state root.
    #[inline]
    pub fn on_set_root(&self) -> GasCharge {
        // The modification needs a lookup first, then a deferred write via the snapshots,
        // which might end up being amortized by having other writes buffered till the end.
        GasCharge::new(
            "OnSetRoot",
            self.state_read_base + self.state_write_base,
            Zero::zero(),
        )
    }

    /// Returns the gas required for accessing the current balance.
    #[inline]
    pub fn on_current_balance(&self) -> GasCharge {
        GasCharge::new("OnCurrentBalance", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for accessing the balance of an actor.
    #[inline]
    pub fn on_balance_of(&self) -> GasCharge {
        GasCharge::new("OnBalanceOf", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for resolving an actor address.
    ///
    /// Might require lookup in the state tree as well as loading the state of the init actor.
    #[inline]
    pub fn on_resolve_address(&self) -> GasCharge {
        GasCharge::new("OnResolveAddress", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for looking up an actor address.
    #[inline]
    pub fn on_lookup_address(&self) -> GasCharge {
        GasCharge::new("OnLookupAddress", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for getting the CID of the code of an actor.
    ///
    /// Might require looking up the actor in the state tree.
    #[inline]
    pub fn on_get_actor_code_cid(&self) -> GasCharge {
        GasCharge::new("OnGetActorCodeCid", self.state_read_base, Zero::zero())
    }

    /// Returns the gas required for looking up the type of a builtin actor by CID.
    #[inline]
    pub fn on_get_builtin_actor_type(&self) -> GasCharge {
        GasCharge::new(
            "OnGetBuiltinActorType",
            self.builtin_actor_base,
            Zero::zero(),
        )
    }

    /// Returns the gas required for looking up the CID of a builtin actor by type.
    #[inline]
    pub fn on_get_code_cid_for_type(&self) -> GasCharge {
        GasCharge::new("OnGetCodeCidForType", self.builtin_actor_base, Zero::zero())
    }

    /// Returns the gas required for accessing the network context.
    #[inline]
    pub fn on_network_context(&self) -> GasCharge {
        GasCharge::new("OnNetworkContext", self.context_base, Zero::zero())
    }

    /// Returns the gas required for accessing the message context.
    #[inline]
    pub fn on_message_context(&self) -> GasCharge {
        GasCharge::new("OnMessageContext", self.context_base, Zero::zero())
    }

    /// Returns the gas required for installing an actor.
    #[cfg(feature = "m2-native")]
    pub fn on_install_actor(&self, wasm_size: usize) -> GasCharge {
        GasCharge::new(
            "OnInstallActor",
            self.install_wasm_per_byte_cost * wasm_size,
            Zero::zero(),
        )
    }

    /// Returns the gas required for initializing memory.
    pub fn init_memory_gas(&self, min_memory_bytes: usize) -> Gas {
        self.wasm_rules.memory_expansion_per_byte_cost * min_memory_bytes
    }

    /// Returns the gas required for growing memory.
    pub fn grow_memory_gas(&self, grow_memory_bytes: usize) -> Gas {
        self.wasm_rules.memory_expansion_per_byte_cost * grow_memory_bytes
    }

    /// Returns the gas required for initializing tables.
    pub fn init_table_gas(&self, min_table_elements: u32) -> Gas {
        // Each element reserves a `usize` in the table, so we charge 8 bytes per pointer.
        // https://docs.rs/wasmtime/2.0.2/wasmtime/struct.InstanceLimits.html#structfield.table_elements
        self.wasm_rules.memory_expansion_per_byte_cost * min_table_elements * 8
    }

    #[inline]
    pub fn on_actor_event(&self, evt: &ActorEvent) -> GasCharge {
        let (mut indexed_entries, mut total_bytes) = (0, 0);
        for evt in evt.entries.iter() {
            indexed_entries += evt
                .flags
                .intersection(Flags::FLAG_INDEXED_KEY | Flags::FLAG_INDEXED_VALUE)
                .bits()
                .count_ones();
            total_bytes += evt.key.len() + evt.value.bytes().len();
        }

        GasCharge::new(
            "OnActorEvent",
            self.event_emit_base_cost + (self.event_per_entry_cost * evt.entries.len()),
            (self.event_entry_index_cost * indexed_entries)
                + (self.event_per_byte_cost * total_bytes),
        )
    }
}

/// Returns gas price list by NetworkVersion for gas consumption.
pub fn price_list_by_network_version(network_version: NetworkVersion) -> &'static PriceList {
    match network_version {
        NetworkVersion::V15 => &OH_SNAP_PRICES,
        NetworkVersion::V16 | NetworkVersion::V17 => &SKYR_PRICES,
        NetworkVersion::V18 => &HYGGE_PRICES,
        _ => panic!("network version {nv} not supported", nv = network_version),
    }
}

impl Rules for WasmGasPrices {
    fn instruction_cost(&self, instruction: &Operator) -> anyhow::Result<InstructionCost> {
        if self.exec_instruction_cost.is_zero() {
            return Ok(InstructionCost::Fixed(0));
        }

        // Rules valid for nv16. We will need to be generic over Rules (massive
        // generics tax), use &dyn Rules (which breaks other things), or pass
        // in the network version, or rules version, to vary these prices going
        // forward.
        match instruction {
            // FIP-0032: nop, drop, block, loop, unreachable, return, else, end are priced 0.
            Operator::Nop
            | Operator::Drop
            | Operator::Block { .. }
            | Operator::Loop { .. }
            | Operator::Unreachable
            | Operator::Return
            | Operator::Else
            | Operator::End => Ok(InstructionCost::Fixed(0)),
            Operator::MemoryGrow { .. } => Ok({
                // Saturating. If there's an overflow, we'll catch it later.
                let gas_per_page =
                    self.memory_expansion_per_byte_cost * wasmtime_environ::WASM_PAGE_SIZE;

                let expansion_cost: u32 = gas_per_page
                    .as_milligas()
                    .try_into()
                    .context("memory expansion cost exceeds u32")?;
                match expansion_cost
                    .try_into().ok() // zero or not zero.
                {
                    Some(cost) => InstructionCost::Linear(self.exec_instruction_cost.as_milligas() as u64, cost),
                    None => InstructionCost::Fixed(self.exec_instruction_cost.as_milligas() as u64),
                }
            }),
            _ => Ok(InstructionCost::Fixed(
                self.exec_instruction_cost.as_milligas() as u64,
            )),
        }
    }

    fn gas_charge_cost(&self) -> u64 {
        0
    }

    fn linear_calc_cost(&self) -> u64 {
        0
    }
}
