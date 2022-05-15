// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use ahash::AHashMap;
use fvm_shared::crypto::signature::SignatureType;
use fvm_shared::econ::TokenAmount;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredPoStProof, RegisteredSealProof, ReplicaUpdateInfo,
    SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{MethodNum, METHOD_SEND};
use fvm_wasm_instrument::gas_metering::{MemoryGrowCost, Rules};
use fvm_wasm_instrument::parity_wasm::elements::Instruction;
use lazy_static::lazy_static;
use num_traits::Zero;

use super::GasCharge;
use crate::gas::Milligas;

/// Converts a static value to milligas. This operation does not saturate,
/// and should only be used with constant values in pricelists.
macro_rules! static_milligas {
    ($ex:expr) => {
        $ex * $crate::gas::MILLIGAS_PRECISION
    };
}

lazy_static! {
    static ref OH_SNAP_PRICES: PriceList = PriceList {
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: static_milligas!(38863),
        on_chain_message_storage_base: static_milligas!(36),
        on_chain_message_storage_per_byte: static_milligas!(1),

        on_chain_return_value_per_byte: static_milligas!(1),

        send_base: static_milligas!(29233),
        send_transfer_funds: static_milligas!(27500),
        send_transfer_only_premium: static_milligas!(159672),
        send_invoke_method: static_milligas!(-5377),

        create_actor_compute: static_milligas!(1108454),
        create_actor_storage: static_milligas!(36 + 40),
        delete_actor: static_milligas!(-(36 + 40)),

        bls_sig_cost: static_milligas!(16598605),
        secp256k1_sig_cost: static_milligas!(1637292),

        hashing_base: static_milligas!(31355),
        compute_unsealed_sector_cid_base: static_milligas!(98647),
        verify_seal_base: static_milligas!(2000), // TODO revisit potential removal of this

        verify_aggregate_seal_base: 0,
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                static_milligas!(449900)
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                static_milligas!(359272)
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: static_milligas!(103994170)},
                        Step{start: 7, cost: static_milligas!(112356810)},
                        Step{start: 13, cost: static_milligas!(122912610)},
                        Step{start: 26, cost: static_milligas!(137559930)},
                        Step{start: 52, cost: static_milligas!(162039100)},
                        Step{start: 103, cost: static_milligas!(210960780)},
                        Step{start: 205, cost: static_milligas!(318351180)},
                        Step{start: 410, cost: static_milligas!(528274980)},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: static_milligas!(102581240)},
                        Step{start: 7, cost: static_milligas!(110803030)},
                        Step{start: 13, cost: static_milligas!(120803700)},
                        Step{start: 26, cost: static_milligas!(134642130)},
                        Step{start: 52, cost: static_milligas!(157357890)},
                        Step{start: 103, cost: static_milligas!(203017690)},
                        Step{start: 205, cost: static_milligas!(304253590)},
                        Step{start: 410, cost: static_milligas!(509880640)},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        verify_consensus_fault: static_milligas!(495422),
        verify_replica_update: static_milligas!(36316136),
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        get_randomness_base: 0,
        get_randomness_per_byte: 0,

        block_memcpy_per_byte_cost: 0,

        block_open_base: static_milligas!(114617),
        block_open_memret_per_byte_cost: 0,

        block_link_base: static_milligas!(353640),
        block_link_storage_per_byte_cost: static_milligas!(1),

        block_create_base: 0,
        block_create_memret_per_byte_cost: 0,

        block_read_base: 0,
        block_stat_base: 0,

        syscall_cost: 0,
        extern_cost: 0,

        wasm_rules: WasmGasPrices{
            exec_instruction_cost_milli: 0,
        },
    };

    static ref SKYR_PRICES: PriceList = PriceList {
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: static_milligas!(38863),
        on_chain_message_storage_base: static_milligas!(36),
        on_chain_message_storage_per_byte: static_milligas!(1),

        on_chain_return_value_per_byte: static_milligas!(1),

        send_base: static_milligas!(29233),
        send_transfer_funds: static_milligas!(27500),
        send_transfer_only_premium: static_milligas!(159672),
        send_invoke_method: static_milligas!(-5377),

        create_actor_compute: static_milligas!(1108454),
        create_actor_storage: static_milligas!(36 + 40),
        delete_actor: static_milligas!(-(36 + 40)),

        bls_sig_cost: static_milligas!(16598605),
        secp256k1_sig_cost: static_milligas!(1637292),

        hashing_base: static_milligas!(31355),
        compute_unsealed_sector_cid_base: static_milligas!(98647),
        verify_seal_base: static_milligas!(2000), // TODO revisit potential removal of this

        verify_aggregate_seal_base: 0,
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                static_milligas!(449900)
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                static_milligas!(359272)
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: static_milligas!(103994170)},
                        Step{start: 7, cost: static_milligas!(112356810)},
                        Step{start: 13, cost: static_milligas!(122912610)},
                        Step{start: 26, cost: static_milligas!(137559930)},
                        Step{start: 52, cost: static_milligas!(162039100)},
                        Step{start: 103, cost: static_milligas!(210960780)},
                        Step{start: 205, cost: static_milligas!(318351180)},
                        Step{start: 410, cost: static_milligas!(528274980)},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: static_milligas!(102581240)},
                        Step{start: 7, cost: static_milligas!(110803030)},
                        Step{start: 13, cost: static_milligas!(120803700)},
                        Step{start: 26, cost: static_milligas!(134642130)},
                        Step{start: 52, cost: static_milligas!(157357890)},
                        Step{start: 103, cost: static_milligas!(203017690)},
                        Step{start: 205, cost: static_milligas!(304253590)},
                        Step{start: 410, cost: static_milligas!(509880640)},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        // TODO: PARAM_FINISH: this may need to be increased to account for the cost of an extern
        verify_consensus_fault: static_milligas!(495422),
        verify_replica_update: static_milligas!(36316136),
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: static_milligas!(117680921),
                    scale: static_milligas!(43780),
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        get_randomness_base: 0,
        get_randomness_per_byte: 0,

        block_memcpy_per_byte_cost: 500,

        block_open_base: static_milligas!(114617),
        block_open_memret_per_byte_cost: static_milligas!(10),

        block_link_base: static_milligas!(353640),
        block_link_storage_per_byte_cost: static_milligas!(1),

        block_create_base: 0,
        block_create_memret_per_byte_cost: static_milligas!(10),

        block_read_base: 0,
        block_stat_base: 0,

        syscall_cost: static_milligas!(14000),
        extern_cost: static_milligas!(21000),

        wasm_rules: WasmGasPrices{
            exec_instruction_cost_milli: static_milligas!(4) as u64,
        },
    };
}

#[derive(Clone, Debug, Copy)]
pub(crate) struct ScalingCost {
    flat: i64,
    scale: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct StepCost(Vec<Step>);

#[derive(Clone, Debug, Copy)]
pub(crate) struct Step {
    start: i64,
    cost: i64,
}

impl StepCost {
    pub(crate) fn lookup(&self, x: i64) -> i64 {
        let mut i: i64 = 0;
        while i < self.0.len() as i64 {
            if self.0[i as usize].start > x {
                break;
            }
            i += 1;
        }
        i -= 1;
        if i < 0 {
            return 0;
        }
        self.0[i as usize].cost
    }
}

/// Provides prices for operations in the VM.
/// All costs are in milligas.
#[derive(Clone, Debug)]
pub struct PriceList {
    /// Storage gas charge multiplier
    pub(crate) storage_gas_multiplier: i64,

    /// Gas cost charged to the originator of an on-chain message (regardless of
    /// whether it succeeds or fails in application) is given by:
    ///   OnChainMessageBase + len(serialized message)*OnChainMessagePerByte
    /// Together, these account for the cost of message propagation and validation,
    /// up to but excluding any actual processing by the VM.
    /// This is the cost a block producer burns when including an invalid message.
    pub(crate) on_chain_message_compute_base: Milligas,
    pub(crate) on_chain_message_storage_base: Milligas,
    pub(crate) on_chain_message_storage_per_byte: Milligas,

    /// Gas cost charged to the originator of a non-nil return value produced
    /// by an on-chain message is given by:
    ///   len(return value)*OnChainReturnValuePerByte
    pub(crate) on_chain_return_value_per_byte: Milligas,

    /// Gas cost for any message send execution(including the top-level one
    /// initiated by an on-chain message).
    /// This accounts for the cost of loading sender and receiver actors and
    /// (for top-level messages) incrementing the sender's sequence number.
    /// Load and store of actor sub-state is charged separately.
    pub(crate) send_base: Milligas,

    /// Gas cost charged, in addition to SendBase, if a message send
    /// is accompanied by any nonzero currency amount.
    /// Accounts for writing receiver's new balance (the sender's state is
    /// already accounted for).
    pub(crate) send_transfer_funds: Milligas,

    /// Gas cost charged, in addition to SendBase, if message only transfers funds.
    pub(crate) send_transfer_only_premium: Milligas,

    /// Gas cost charged, in addition to SendBase, if a message invokes
    /// a method on the receiver.
    /// Accounts for the cost of loading receiver code and method dispatch.
    pub(crate) send_invoke_method: Milligas,

    /// Gas cost for creating a new actor (via InitActor's Exec method).
    /// Note: this costs assume that the extra will be partially or totally refunded while
    /// the base is covering for the put.
    pub(crate) create_actor_compute: Milligas,
    pub(crate) create_actor_storage: Milligas,

    /// Gas cost for deleting an actor.
    /// Note: this partially refunds the create cost to incentivise the deletion of the actors.
    pub(crate) delete_actor: Milligas,

    /// Gas cost for verifying bls signature
    pub(crate) bls_sig_cost: Milligas,
    /// Gas cost for verifying secp256k1 signature
    pub(crate) secp256k1_sig_cost: Milligas,

    pub(crate) hashing_base: Milligas,

    pub(crate) compute_unsealed_sector_cid_base: Milligas,
    pub(crate) verify_seal_base: Milligas,
    #[allow(unused)]
    pub(crate) verify_aggregate_seal_base: Milligas,
    pub(crate) verify_aggregate_seal_per: AHashMap<RegisteredSealProof, i64>,
    pub(crate) verify_aggregate_seal_steps: AHashMap<RegisteredSealProof, StepCost>,

    pub(crate) verify_post_lookup: AHashMap<RegisteredPoStProof, ScalingCost>,
    pub(crate) verify_consensus_fault: Milligas,
    pub(crate) verify_replica_update: Milligas,

    /// Gas cost for fetching randomness.
    pub(crate) get_randomness_base: Milligas,
    /// Gas cost per every byte of randomness fetched.
    pub(crate) get_randomness_per_byte: Milligas,

    /// Gas cost per every block byte memcopied across boundaries.
    pub(crate) block_memcpy_per_byte_cost: Milligas,

    /// Gas cost for opening a block.
    pub(crate) block_open_base: Milligas,
    /// Gas cost for every byte retained in FVM space when opening a block.
    pub(crate) block_open_memret_per_byte_cost: Milligas,

    /// Gas cost for linking a block.
    pub(crate) block_link_base: Milligas,
    /// Multiplier for storage gas per byte.
    pub(crate) block_link_storage_per_byte_cost: Milligas,

    /// Gas cost for creating a block.
    pub(crate) block_create_base: Milligas,
    /// Gas cost for every byte retained in FVM space when writing a block.
    pub(crate) block_create_memret_per_byte_cost: Milligas,

    /// Gas cost for reading a block into actor space.
    pub(crate) block_read_base: Milligas,
    /// Gas cost for statting a block.
    pub(crate) block_stat_base: Milligas,

    /// General gas cost for performing a syscall, accounting for the overhead thereof.
    pub(crate) syscall_cost: Milligas,
    /// General gas cost for calling an extern, accounting for the overhead thereof.
    pub(crate) extern_cost: Milligas,

    /// Rules for execution gas.
    pub(crate) wasm_rules: WasmGasPrices,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WasmGasPrices {
    pub(crate) exec_instruction_cost_milli: u64,
}

impl PriceList {
    /// Returns the gas required for storing a message of a given size in the chain.
    #[inline]
    pub fn on_chain_message(&self, msg_size: usize) -> GasCharge<'static> {
        GasCharge::new(
            "OnChainMessage",
            self.on_chain_message_compute_base,
            (self.on_chain_message_storage_base
                + self.on_chain_message_storage_per_byte * msg_size as i64)
                * self.storage_gas_multiplier,
        )
    }

    /// Returns the gas required for storing the response of a message in the chain.
    #[inline]
    pub fn on_chain_return_value(&self, data_size: usize) -> GasCharge<'static> {
        GasCharge::new(
            "OnChainReturnValue",
            0,
            data_size as i64 * self.on_chain_return_value_per_byte * self.storage_gas_multiplier,
        )
    }

    /// Returns the gas required when invoking a method.
    #[inline]
    pub fn on_method_invocation(
        &self,
        value: &TokenAmount,
        method_num: MethodNum,
    ) -> GasCharge<'static> {
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
        GasCharge::new("OnMethodInvocation", ret, 0)
    }

    /// Returns the gas cost to be applied on a syscall.
    pub fn on_syscall(&self) -> GasCharge<'static> {
        GasCharge::new("OnSyscall", self.syscall_cost, 0)
    }

    /// Returns the gas required for creating an actor.
    #[inline]
    pub fn on_create_actor(&self) -> GasCharge<'static> {
        GasCharge::new(
            "OnCreateActor",
            self.create_actor_compute,
            self.create_actor_storage * self.storage_gas_multiplier,
        )
    }

    /// Returns the gas required for deleting an actor.
    #[inline]
    pub fn on_delete_actor(&self) -> GasCharge<'static> {
        GasCharge::new(
            "OnDeleteActor",
            0,
            self.delete_actor * self.storage_gas_multiplier,
        )
    }

    /// Returns gas required for signature verification.
    #[inline]
    pub fn on_verify_signature(&self, sig_type: SignatureType) -> GasCharge<'static> {
        let val = match sig_type {
            SignatureType::BLS => self.bls_sig_cost,
            SignatureType::Secp256k1 => self.secp256k1_sig_cost,
        };
        GasCharge::new("OnVerifySignature", val, 0)
    }

    /// Returns gas required for hashing data.
    #[inline]
    pub fn on_hashing(&self, _: usize) -> GasCharge<'static> {
        GasCharge::new("OnHashing", self.hashing_base, 0)
    }

    /// Returns gas required for computing unsealed sector Cid.
    #[inline]
    pub fn on_compute_unsealed_sector_cid(
        &self,
        _proof: RegisteredSealProof,
        _pieces: &[PieceInfo],
    ) -> GasCharge<'static> {
        GasCharge::new(
            "OnComputeUnsealedSectorCid",
            self.compute_unsealed_sector_cid_base,
            0,
        )
    }

    /// Returns gas required for seal verification.
    #[inline]
    pub fn on_verify_seal(&self, _info: &SealVerifyInfo) -> GasCharge<'static> {
        GasCharge::new("OnVerifySeal", self.verify_seal_base, 0)
    }
    #[inline]
    pub fn on_verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> GasCharge<'static> {
        let proof_type = aggregate.seal_proof;
        let per_proof = self
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
            0,
        )
    }

    /// Returns gas required for replica verification.
    #[inline]
    pub fn on_verify_replica_update(&self, _replica: &ReplicaUpdateInfo) -> GasCharge<'static> {
        GasCharge::new("OnVerifyReplicaUpdate", self.verify_replica_update, 0)
    }

    /// Returns gas required for PoSt verification.
    #[inline]
    pub fn on_verify_post(&self, info: &WindowPoStVerifyInfo) -> GasCharge<'static> {
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

        let gas_used = cost.flat + info.challenged_sectors.len() as i64 * cost.scale;

        GasCharge::new("OnVerifyPost", gas_used, 0)
    }

    /// Returns gas required for verifying consensus fault.
    #[inline]
    pub fn on_verify_consensus_fault(&self) -> GasCharge<'static> {
        GasCharge::new(
            "OnVerifyConsensusFault",
            self.extern_cost.saturating_add(self.verify_consensus_fault),
            0,
        )
    }

    /// Returns the cost of the gas required for getting randomness from the client, based on the
    /// numebr of bytes of entropy.
    #[inline]
    pub fn on_get_randomness(&self, entropy_size: usize) -> GasCharge<'static> {
        GasCharge::new(
            "OnGetRandomness",
            self.extern_cost
                .saturating_add(self.get_randomness_base)
                .saturating_add(
                    self.get_randomness_per_byte
                        .saturating_mul(entropy_size as i64),
                ),
            0,
        )
    }

    /// Returns the base gas required for loading an object, independent of the object's size.
    #[inline]
    pub fn on_block_open_base(&self) -> GasCharge<'static> {
        GasCharge::new(
            "OnBlockOpenBase",
            self.extern_cost.saturating_add(self.block_open_base),
            0,
        )
    }

    /// Returns the gas required for loading an object based on the size of the object.
    #[inline]
    pub fn on_block_open_per_byte(&self, data_size: usize) -> GasCharge<'static> {
        let size = data_size as i64;
        GasCharge::new(
            "OnBlockOpenPerByte",
            self.block_open_memret_per_byte_cost.saturating_mul(size)
                + self.block_memcpy_per_byte_cost.saturating_mul(size),
            0,
        )
    }

    /// Returns the gas required for reading a loaded object.
    #[inline]
    pub fn on_block_read(&self, data_size: usize) -> GasCharge<'static> {
        GasCharge::new(
            "OnBlockRead",
            self.block_read_base.saturating_add(
                self.block_memcpy_per_byte_cost
                    .saturating_mul(data_size as i64),
            ),
            0,
        )
    }

    /// Returns the gas required for adding an object to the FVM cache.
    #[inline]
    pub fn on_block_create(&self, data_size: usize) -> GasCharge<'static> {
        let size = data_size as i64;
        let mem_costs = self
            .block_create_memret_per_byte_cost
            .saturating_mul(size)
            .saturating_add(self.block_memcpy_per_byte_cost.saturating_mul(size));
        GasCharge::new(
            "OnBlockCreate",
            self.block_create_base.saturating_add(mem_costs),
            0,
        )
    }

    /// Returns the gas required for committing an object to the state blockstore.
    #[inline]
    pub fn on_block_link(&self, data_size: usize) -> GasCharge<'static> {
        let size = data_size as i64;
        let memcpy = self.block_memcpy_per_byte_cost.saturating_mul(size);
        GasCharge::new(
            "OnBlockLink",
            // twice the memcpy cost:
            // - one from the block registry to the FVM BufferedBlockstore
            // - one from the FVM BufferedBlockstore to the Node's Blockstore
            //   when the machine finishes.
            self.block_link_base
                .saturating_add((2_i64).saturating_mul(memcpy)),
            self.block_link_storage_per_byte_cost
                .saturating_mul(self.storage_gas_multiplier)
                .saturating_mul(size),
        )
    }

    /// Returns the gas required for storing an object.
    #[inline]
    pub fn on_block_stat(&self) -> GasCharge<'static> {
        GasCharge::new("OnBlockStat", self.block_stat_base, 0)
    }
}

/// Returns gas price list by NetworkVersion for gas consumption.
pub fn price_list_by_network_version(network_version: NetworkVersion) -> &'static PriceList {
    match network_version {
        NetworkVersion::V15 => &OH_SNAP_PRICES,
        _ => &SKYR_PRICES,
    }
}

impl Rules for WasmGasPrices {
    fn instruction_cost(&self, instruction: &Instruction) -> Option<u64> {
        if self.exec_instruction_cost_milli == 0 {
            return Some(0);
        }

        // Rules valid for nv16. We will need to be generic over Rules (massive
        // generics tax), use &dyn Rules (which breaks other things), or pass
        // in the network version, or rules version, to vary these prices going
        // forward.
        match instruction {
            // FIP-0032: nop, drop, block, loop, unreachable, return, else, end are priced 0.
            Instruction::Nop
            | Instruction::Drop
            | Instruction::Block(_)
            | Instruction::Loop(_)
            | Instruction::Unreachable
            | Instruction::Return
            | Instruction::Else
            | Instruction::End => Some(0),
            _ => Some(self.exec_instruction_cost_milli),
        }
    }

    fn memory_grow_cost(&self) -> MemoryGrowCost {
        MemoryGrowCost::Free
    }
}
