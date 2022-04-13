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
use lazy_static::lazy_static;
use num_traits::Zero;

use super::GasCharge;

lazy_static! {
    static ref OH_SNAP_PRICES: PriceList = PriceList {
        compute_gas_multiplier: 1,
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: 38863,
        on_chain_message_storage_base: 36,
        on_chain_message_storage_per_byte: 1,

        on_chain_return_value_per_byte: 1,

        send_base: 29233,
        send_transfer_funds: 27500,
        send_transfer_only_premium: 159672,
        send_invoke_method: -5377,

        create_actor_compute: 1108454,
        create_actor_storage: 36 + 40,
        delete_actor: -(36 + 40),

        bls_sig_cost: 16598605,
        secp256k1_sig_cost: 1637292,

        hashing_base: 31355,
        compute_unsealed_sector_cid_base: 98647,
        verify_seal_base: 2000, // TODO revisit potential removal of this

        verify_aggregate_seal_base: 0,
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                449900
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                359272
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: 103994170},
                        Step{start: 7, cost: 112356810},
                        Step{start: 13, cost: 122912610},
                        Step{start: 26, cost: 137559930},
                        Step{start: 52, cost: 162039100},
                        Step{start: 103, cost: 210960780},
                        Step{start: 205, cost: 318351180},
                        Step{start: 410, cost: 528274980},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: 102581240},
                        Step{start: 7, cost: 110803030},
                        Step{start: 13, cost: 120803700},
                        Step{start: 26, cost: 134642130},
                        Step{start: 52, cost: 157357890},
                        Step{start: 103, cost: 203017690},
                        Step{start: 205, cost: 304253590},
                        Step{start: 410, cost: 509880640},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        verify_consensus_fault: 495422,
        verify_replica_update: 36316136,
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        gas_per_exec_unit: 0,
        get_randomness_base: 0,
        get_randomness_per_byte: 0,

        block_memcpy_per_byte_cost: 0,
        block_io_per_byte_cost: 0,
        block_link_per_byte_cost: 1,

        block_open_base: 114617,
        block_read_base: 0,
        block_create_base: 0,
        block_link_base: 353640,
        block_stat: 0,
    };

    static ref SKYR_PRICES: PriceList = PriceList {
        compute_gas_multiplier: 1,
        storage_gas_multiplier: 1300,

        on_chain_message_compute_base: 38863,
        on_chain_message_storage_base: 36,
        on_chain_message_storage_per_byte: 1,

        on_chain_return_value_per_byte: 1,

        send_base: 29233,
        send_transfer_funds: 27500,
        send_transfer_only_premium: 159672,
        send_invoke_method: -5377,

        create_actor_compute: 1108454,
        create_actor_storage: 36 + 40,
        delete_actor: -(36 + 40),

        bls_sig_cost: 16598605,
        secp256k1_sig_cost: 1637292,

        hashing_base: 31355,
        compute_unsealed_sector_cid_base: 98647,
        verify_seal_base: 2000, // TODO revisit potential removal of this

        verify_aggregate_seal_base: 0,
        verify_aggregate_seal_per: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                449900
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                359272
            )
        ].iter().copied().collect(),
        verify_aggregate_seal_steps: [
            (
                RegisteredSealProof::StackedDRG32GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: 103994170},
                        Step{start: 7, cost: 112356810},
                        Step{start: 13, cost: 122912610},
                        Step{start: 26, cost: 137559930},
                        Step{start: 52, cost: 162039100},
                        Step{start: 103, cost: 210960780},
                        Step{start: 205, cost: 318351180},
                        Step{start: 410, cost: 528274980},
                    ]
                )
            ),
            (
                RegisteredSealProof::StackedDRG64GiBV1P1,
                StepCost (
                    vec![
                        Step{start: 4, cost: 102581240},
                        Step{start: 7, cost: 110803030},
                        Step{start: 13, cost: 120803700},
                        Step{start: 26, cost: 134642130},
                        Step{start: 52, cost: 157357890},
                        Step{start: 103, cost: 203017690},
                        Step{start: 205, cost: 304253590},
                        Step{start: 410, cost: 509880640},
                    ]
                )
            )
        ].iter()
        .cloned()
        .collect(),

        // TODO: PARAM_FINISH: this may need to be increased to account for the cost of an extern
        verify_consensus_fault: 495422,
        verify_replica_update: 36316136,
        verify_post_lookup: [
            (
                RegisteredPoStProof::StackedDRGWindow512MiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow32GiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
            (
                RegisteredPoStProof::StackedDRGWindow64GiBV1,
                ScalingCost {
                    flat: 117680921,
                    scale: 43780,
                },
            ),
        ]
        .iter()
        .copied()
        .collect(),

        block_memcpy_per_byte_cost: 4,
        block_io_per_byte_cost: 2,
        block_link_per_byte_cost: 1,
        // TODO: PARAM_FINISH

        // TODO: PARAM_FINISH
        gas_per_exec_unit: 2,
        // TODO: PARAM_FINISH
        get_randomness_base: 1,
        // TODO: PARAM_FINISH
        get_randomness_per_byte: 1,

        // TODO: PARAM_FINIuiSH
        block_open_base: 1,
        // TODO: PARAM_FINISH
        block_read_base: 1,
        // TODO: PARAM_FINISH
        block_create_base: 1,
        // TODO: PARAM_FINISH
        block_link_base: 1,
        // TODO: PARAM_FINISH
        block_stat: 1,
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

/// Provides prices for operations in the VM
#[derive(Clone, Debug)]
pub struct PriceList {
    /// Compute gas charge multiplier
    // * This multiplier is not currently applied to anything, but is matching lotus.
    // * If the possible values are non 1 or if Lotus adds, we should change also.
    #[allow(unused)]
    pub(crate) compute_gas_multiplier: i64,
    /// Storage gas charge multiplier
    pub(crate) storage_gas_multiplier: i64,

    /// Gas cost charged to the originator of an on-chain message (regardless of
    /// whether it succeeds or fails in application) is given by:
    ///   OnChainMessageBase + len(serialized message)*OnChainMessagePerByte
    /// Together, these account for the cost of message propagation and validation,
    /// up to but excluding any actual processing by the VM.
    /// This is the cost a block producer burns when including an invalid message.
    pub(crate) on_chain_message_compute_base: i64,
    pub(crate) on_chain_message_storage_base: i64,
    pub(crate) on_chain_message_storage_per_byte: i64,

    /// Gas cost charged to the originator of a non-nil return value produced
    /// by an on-chain message is given by:
    ///   len(return value)*OnChainReturnValuePerByte
    pub(crate) on_chain_return_value_per_byte: i64,

    /// Gas cost for any message send execution(including the top-level one
    /// initiated by an on-chain message).
    /// This accounts for the cost of loading sender and receiver actors and
    /// (for top-level messages) incrementing the sender's sequence number.
    /// Load and store of actor sub-state is charged separately.
    pub(crate) send_base: i64,

    /// Gas cost charged, in addition to SendBase, if a message send
    /// is accompanied by any nonzero currency amount.
    /// Accounts for writing receiver's new balance (the sender's state is
    /// already accounted for).
    pub(crate) send_transfer_funds: i64,

    /// Gas cost charged, in addition to SendBase, if message only transfers funds.
    pub(crate) send_transfer_only_premium: i64,

    /// Gas cost charged, in addition to SendBase, if a message invokes
    /// a method on the receiver.
    /// Accounts for the cost of loading receiver code and method dispatch.
    pub(crate) send_invoke_method: i64,

    /// Gas cost for creating a new actor (via InitActor's Exec method).
    /// Note: this costs assume that the extra will be partially or totally refunded while
    /// the base is covering for the put.
    pub(crate) create_actor_compute: i64,
    pub(crate) create_actor_storage: i64,

    /// Gas cost for deleting an actor.
    /// Note: this partially refunds the create cost to incentivise the deletion of the actors.
    pub(crate) delete_actor: i64,

    /// Gas cost for verifying bls signature
    pub(crate) bls_sig_cost: i64,
    /// Gas cost for verifying secp256k1 signature
    pub(crate) secp256k1_sig_cost: i64,

    pub(crate) hashing_base: i64,

    pub(crate) compute_unsealed_sector_cid_base: i64,
    pub(crate) verify_seal_base: i64,
    #[allow(unused)]
    pub(crate) verify_aggregate_seal_base: i64,
    pub(crate) verify_aggregate_seal_per: AHashMap<RegisteredSealProof, i64>,
    pub(crate) verify_aggregate_seal_steps: AHashMap<RegisteredSealProof, StepCost>,

    pub(crate) verify_post_lookup: AHashMap<RegisteredPoStProof, ScalingCost>,
    pub(crate) verify_consensus_fault: i64,
    pub(crate) verify_replica_update: i64,
    // 1 Exec Unit = gas_per_exec_unit * 1 Gas
    pub(crate) gas_per_exec_unit: i64,

    pub(crate) get_randomness_base: i64,
    pub(crate) get_randomness_per_byte: i64,

    pub(crate) block_memcpy_per_byte_cost: i64,
    pub(crate) block_io_per_byte_cost: i64,
    pub(crate) block_link_per_byte_cost: i64,

    pub(crate) block_open_base: i64,
    pub(crate) block_read_base: i64,
    pub(crate) block_create_base: i64,
    pub(crate) block_link_base: i64,
    pub(crate) block_stat: i64,
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
        GasCharge::new("OnVerifyConsensusFault", self.verify_consensus_fault, 0)
    }

    /// Returns the gas required for the specified exec_units.
    #[inline]
    pub fn on_consume_exec_units(&self, exec_units: u64) -> GasCharge<'static> {
        GasCharge::new(
            "OnConsumeExecUnits",
            self.gas_per_exec_unit
                .saturating_mul(i64::try_from(exec_units).unwrap_or(i64::MAX)),
            0,
        )
    }

    /// Converts the specified gas into equivalent exec_units
    /// Note: In rare cases the provided `gas` may be negative
    #[inline]
    pub fn gas_to_exec_units(&self, gas: i64, round_up: bool) -> i64 {
        match self.gas_per_exec_unit {
            0 => 0,
            v => {
                let mut div_result = gas / v;
                if round_up && gas % v != 0 {
                    div_result = div_result.saturating_add(1);
                }
                div_result
            }
        }
    }

    /// Returns the base cost of the gas required for getting randomness from the client.
    #[inline]
    pub fn on_get_randomness_base(&self) -> GasCharge<'static> {
        GasCharge::new("OnGetRandomnessBase", self.get_randomness_base, 0)
    }

    /// Returns the gas required for getting randomness from the client based on the number of bytes of randomness.
    #[inline]
    pub fn on_get_randomness_per_byte(&self, randomness_size: usize) -> GasCharge<'static> {
        GasCharge::new(
            "OnGetRandomnessPerByte",
            self.get_randomness_per_byte
                .saturating_mul(randomness_size as i64),
            0,
        )
    }

    /// Returns the base gas required for loading an object, independent of the object's size.
    #[inline]
    pub fn on_block_open_base(&self) -> GasCharge<'static> {
        GasCharge::new("OnBlockOpenBase", self.block_open_base, 0)
    }

    /// Returns the gas required for loading an object based on the size of the object.
    #[inline]
    pub fn on_block_open_per_byte(&self, data_size: usize) -> GasCharge<'static> {
        // TODO: Should we also throw on a memcpy cost here (see https://github.com/filecoin-project/FIPs/blob/master/FIPS/fip-0032.md#ipld-state-management-fees)
        GasCharge::new(
            "OnBlockOpenPerByte",
            self.block_io_per_byte_cost.saturating_mul(data_size as i64),
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
        GasCharge::new(
            "OnBlockCreate",
            self.block_create_base.saturating_add(
                self.block_memcpy_per_byte_cost
                    .saturating_mul(data_size as i64),
            ),
            0,
        )
    }

    /// Returns the gas required for committing an object to the state blockstore.
    #[inline]
    pub fn on_block_link(&self, data_size: usize) -> GasCharge<'static> {
        // TODO: The FIP makes it sound like this would need 2 memcpys, is that what's desired?
        GasCharge::new(
            "OnBlockLink",
            self.block_link_base,
            // data_size as i64 * self.block_link_per_byte_cost * self.storage_gas_multiplier,
            self.block_link_per_byte_cost
                .saturating_mul(self.storage_gas_multiplier)
                .saturating_mul(data_size as i64),
        )
    }

    /// Returns the gas required for storing an object.
    #[inline]
    pub fn on_block_stat(&self) -> GasCharge<'static> {
        GasCharge::new("OnBlockStat", self.block_stat, 0)
    }
}

/// Returns gas price list by NetworkVersion for gas consumption.
pub fn price_list_by_network_version(network_version: NetworkVersion) -> &'static PriceList {
    match network_version {
        NetworkVersion::V14 | NetworkVersion::V15 => &OH_SNAP_PRICES,
        _ => &SKYR_PRICES,
    }
}
