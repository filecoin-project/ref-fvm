// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;
use std::ops::Mul;

use anyhow::Context;
use fvm_shared::crypto::signature::SignatureType;
use fvm_shared::event::{ActorEvent, Flags};
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredPoStProof, RegisteredSealProof, ReplicaUpdateInfo,
    SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_wasm_instrument::gas_metering::{InstructionCost, Operator, Rules};
use lazy_static::lazy_static;
use num_traits::Zero;

use super::GasCharge;
use crate::gas::Gas;
use crate::kernel::SupportedHashes;

// Each element reserves a `usize` in the table, so we charge 8 bytes per pointer.
// https://docs.rs/wasmtime/2.0.2/wasmtime/struct.InstanceLimits.html#structfield.table_elements
const TABLE_ELEMENT_SIZE: u32 = 8;

/// Create a mapping from enum items to values in a way that guarantees at compile
/// time that we did not miss any member, in any of the prices, even if the enum
/// gets a new member later.
///
/// # Example
///
/// ```
/// use fvm::total_enum_map;
/// use std::collections::HashMap;
///
/// #[derive(Hash, Eq, PartialEq)]
/// enum Foo {
///     Bar,
///     Baz,
/// }
///
/// let foo_cost: HashMap<Foo, u8> = total_enum_map! {
///     Foo {
///         Bar => 10,
///         Baz => 20
///     }
/// };
/// ```
#[macro_export]
macro_rules! total_enum_map {
    ($en:ident { $($item:ident => $value:expr),+ $(,)? }) => {
        [$($en::$item),+].into_iter().map(|m| {
            // This will not compile if a case is missing.
            let v = match m {
                $($en::$item => $value),+
            };
            (m, v)
        }).collect()
    };
}

lazy_static! {
    static ref HYGGE_PRICES: PriceList = PriceList {
        on_chain_message_compute: ScalingCost::fixed(Gas::new(38863)),
        on_chain_message_storage: ScalingCost {
            flat: Gas::new(36),
            scale: Gas::new(1300),
        },

        on_chain_return_compute: ScalingCost::zero(),
        on_chain_return_storage: ScalingCost {
            flat: Zero::zero(),
            scale: Gas::new(1300),
        },

        send_transfer_funds: Gas::new(6000),
        send_invoke_method: Gas::new(75000),

        create_actor_compute: Gas::new(1108454),
        create_actor_storage: Gas::new((36 + 40) * 1300),
        delete_actor: Gas::new(-(36 + 40)*1300),

        sig_cost: total_enum_map!{
            SignatureType {
                Secp256k1 => ScalingCost {
                    flat: Gas::new(1637292),
                    scale: Gas::new(10),
                },
                BLS =>  ScalingCost{
                    flat: Gas::new(16598605),
                    scale: Gas::new(26),
                },
            }
        },
        secp256k1_recover_cost: Gas::new(1637292),
        hashing_cost: total_enum_map! {
            SupportedHashes {
                Sha2_256 => ScalingCost {
                    flat: Gas::zero(),
                    scale: Gas::new(7)
                },
                Blake2b256 => ScalingCost {
                    flat: Gas::zero(),
                    scale: Gas::new(10)
                },
                Blake2b512 => ScalingCost {
                    flat: Gas::zero(),
                    scale: Gas::new(10)
                },
                Keccak256 => ScalingCost {
                    flat: Gas::zero(),
                    scale: Gas::new(33)
                },
                Ripemd160 => ScalingCost {
                    flat: Gas::zero(),
                    scale: Gas::new(35)
                }
            }
        },
        compute_unsealed_sector_cid_base: Gas::new(98647),
        verify_seal_base: Gas::new(2000), // TODO revisit potential removal of this

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

        // TODO(#1277): Implement this first before benchmarking.
        // TODO(#1264): do look at lookback here, but don't bother with the amount of entropy (see the line above).
        get_randomness: Gas::zero(),

        block_allocate: ScalingCost {
            flat: Gas::zero(),
            scale: Gas::new(2),
        },

        block_memcpy: ScalingCost {
            flat: Gas::zero(),
            scale: Gas::from_milligas(400),
        },

        block_memory_retention: ScalingCost {
            flat: Gas::zero(),
            scale: Gas::new(8),
        },

        block_open: ScalingCost {
            // This was benchmarked (#1264) at 187440 gas/read, but we've deducted the existing
            // extern cost here. Once we get accurate charges for all "externs", we can get rid of
            // the separate extern cost all together.
            flat: Gas::new(166440),
            // It costs takes about 0.562 ns/byte (5.6gas) to "read" from a client. However, that
            // includes one allocation and memory copy, which we charge for separately.
            //
            // We disable this charge now because it's entirely covered by the "memory retention"
            // cost. If we do drop the memory retention cost, we need to re-enable this.
            /* scale: Gas::from_milligas(3200), */
            scale: Gas::zero(),
        },

        block_storage: ScalingCost {
            flat: Gas::new(353640),
            scale: Gas::new(1300),
        },

        syscall_cost: Gas::new(14000),
        extern_cost: Gas::new(21000),

        // TODO(#1264) (Consider subtracting from the block storage flat fee).
        block_flush: ScalingCost::zero(),

        // TODO(#1279)
        state_read_base: Zero::zero(),
        // TODO(#1279)
        state_write_base: Zero::zero(),
        // TODO(#1279)
        builtin_actor_manifest_lookup: Zero::zero(),
        // TODO(#1279)
        network_context: Zero::zero(),
        // TODO(#1279)
        message_context: Zero::zero(),

        install_wasm_per_byte_cost: Zero::zero(),

        wasm_rules: WasmGasPrices{
            // Use the default instruction cost of 4 everywhere.
            instruction_default: Gas::new(4),
            math_default: Gas::new(4),
            jump_unconditional: Gas::new(4),
            jump_conditional: Gas::new(4),
            jump_indirect: Gas::new(4),

            // Don't add any additional costs for calls/memory access for now.
            call: Zero::zero(),
            memory_fill_base_cost: Gas::zero(),
            memory_access_cost: Gas::zero(),

            // Don't yet charge anything for copying.
            memory_copy_per_byte_cost: Gas::from_milligas(400),
            memory_fill_per_byte_cost: Gas::from_milligas(400),
        },

        // NOTE: we currently "flush" events, but we need to stop doing that except when asked.
        // For now, just charge for the basic costs.
        // Also: Let's get all the other costs first, then we can work on these.

        // TODO(#1279)
        event_emit_base_cost: Zero::zero(),
        // TODO(#1279)
        event_per_entry_cost: Zero::zero(),
        // TODO(#1279)
        event_entry_index_cost: Zero::zero(),
        // TODO(#1279)
        event_per_byte_cost: Zero::zero(),
    };
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) struct ScalingCost {
    pub flat: Gas,
    pub scale: Gas,
}

impl ScalingCost {
    /// Computes the scaled cost for the given value, or saturates.
    pub fn apply<V>(&self, value: V) -> Gas
    where
        Gas: Mul<V, Output = Gas>,
    {
        self.flat + self.scale * value
    }

    /// Create a new "fixed" cost. Useful when some network versions scale the cost and others don't.
    pub fn fixed(g: Gas) -> Self {
        Self {
            flat: g,
            scale: Gas::zero(),
        }
    }

    /// Create a "zero" scaling cost.
    pub fn zero() -> Self {
        Self {
            flat: Gas::zero(),
            scale: Gas::zero(),
        }
    }
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
    /// Gas cost charged to the originator of an on-chain message (regardless of
    /// whether it succeeds or fails in application) is given by:
    ///   OnChainMessageBase + len(serialized message)*OnChainMessagePerByte
    /// Together, these account for the cost of message propagation and validation,
    /// up to but excluding any actual processing by the VM.
    /// This is the cost a block producer burns when including an invalid message.
    pub(crate) on_chain_message_compute: ScalingCost,
    pub(crate) on_chain_message_storage: ScalingCost,

    /// Gas cost charged to the originator of a non-nil return value produced
    /// by an on-chain message is given by:
    ///   len(return value)*OnChainReturnValuePerByte
    pub(crate) on_chain_return_compute: ScalingCost,
    pub(crate) on_chain_return_storage: ScalingCost,

    // TODO
    pub(crate) send_transfer_funds: Gas,
    // TODO
    pub(crate) send_invoke_method: Gas,

    /// Gas cost for creating a new actor (via InitActor's Exec method).
    /// Note: this costs assume that the extra will be partially or totally refunded while
    /// the base is covering for the put.
    pub(crate) create_actor_compute: Gas,
    pub(crate) create_actor_storage: Gas,

    /// Gas cost for deleting an actor.
    /// Note: this partially refunds the create cost to incentivise the deletion of the actors.
    pub(crate) delete_actor: Gas,

    /// Gas cost for verifying a cryptographic signature.
    pub(crate) sig_cost: HashMap<SignatureType, ScalingCost>,

    /// Gas cost for recovering secp256k1 signer public key
    pub(crate) secp256k1_recover_cost: Gas,

    pub(crate) hashing_cost: HashMap<SupportedHashes, ScalingCost>,

    pub(crate) compute_unsealed_sector_cid_base: Gas,
    pub(crate) verify_seal_base: Gas,
    pub(crate) verify_aggregate_seal_per: HashMap<RegisteredSealProof, Gas>,
    pub(crate) verify_aggregate_seal_steps: HashMap<RegisteredSealProof, StepCost>,

    pub(crate) verify_post_lookup: HashMap<RegisteredPoStProof, ScalingCost>,
    pub(crate) verify_consensus_fault: Gas,
    pub(crate) verify_replica_update: Gas,

    /// Gas cost for fetching randomness.
    pub(crate) get_randomness: Gas,

    /// Gas cost per byte copied.
    pub(crate) block_memcpy: ScalingCost,

    /// Gas cost per byte allocated (computation cost).
    pub(crate) block_allocate: ScalingCost,

    /// Gas cost for every block retained in memory (read and/or written) to ensure we can't retain
    /// more than 1GiB of memory while executing a block.
    ///
    /// This is applied along with `block_allocate` but kept separate so we can benchmark properly.
    pub(crate) block_memory_retention: ScalingCost,

    /// Gas cost for opening a block.
    pub(crate) block_open: ScalingCost,

    /// Gas cost for storing a block.
    pub(crate) block_storage: ScalingCost,

    /// Gas cost to cover the expected cost of flushing.
    pub(crate) block_flush: ScalingCost,

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
    pub(crate) builtin_actor_manifest_lookup: Gas,

    /// Gas cost of accessing the network context.
    pub(crate) network_context: Gas,
    /// Gas cost of accessing the message context.
    pub(crate) message_context: Gas,

    /// Gas cost of compiling a Wasm module during install.
    pub(crate) install_wasm_per_byte_cost: Gas,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WasmGasPrices {
    /// The default gas cost for instructions.
    pub(crate) instruction_default: Gas,
    /// The default gas cost for math instructions.
    pub(crate) math_default: Gas,
    /// The gas cost for unconditional jumps.
    pub(crate) jump_unconditional: Gas,
    /// The gas cost for conditional jumps.
    pub(crate) jump_conditional: Gas,
    /// The gas cost for indirect jumps.
    pub(crate) jump_indirect: Gas,
    /// The gas cost for calls (not including the jump cost).
    pub(crate) call: Gas,

    /// Gas cost for any memory fill instruction (one time charge).
    pub(crate) memory_fill_base_cost: Gas,
    /// Gas cost for every byte "filled" in Wasm memory.
    pub(crate) memory_fill_per_byte_cost: Gas,
    /// Gas cost for any memory copy instruction (one time charge).
    pub(crate) memory_access_cost: Gas,
    /// Gas cost for every byte copied in Wasm memory.
    pub(crate) memory_copy_per_byte_cost: Gas,
}

impl PriceList {
    /// Returns the gas required for storing a message of a given size in the chain.
    #[inline]
    pub fn on_chain_message(&self, msg_size: usize) -> GasCharge {
        GasCharge::new(
            "OnChainMessage",
            self.on_chain_message_compute.apply(msg_size),
            self.on_chain_message_storage.apply(msg_size),
        )
    }

    /// Returns the gas required for storing the response of a message in the chain.
    #[inline]
    pub fn on_chain_return_value(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnChainReturnValue",
            self.on_chain_return_compute.apply(data_size),
            self.on_chain_return_storage.apply(data_size),
        )
    }

    /// Returns the gas required when invoking a method.
    #[inline]
    pub fn on_value_transfer(&self) -> GasCharge {
        GasCharge::new("OnValueTransfer", self.send_transfer_funds, Zero::zero())
    }

    /// Returns the gas required when invoking a method.
    #[inline]
    pub fn on_method_invocation(&self) -> GasCharge {
        GasCharge::new("OnMethodInvocation", self.send_invoke_method, Zero::zero())
    }

    /// Returns the gas cost to be applied on a syscall.
    pub fn on_syscall(&self) -> GasCharge {
        GasCharge::new("OnSyscall", self.syscall_cost, Zero::zero())
    }

    /// Returns the gas required for creating an actor.
    #[inline]
    pub fn on_create_actor(&self, is_new: bool) -> GasCharge {
        let storage_gas = if is_new {
            self.create_actor_storage
        } else {
            Gas::zero()
        };
        GasCharge::new("OnCreateActor", self.create_actor_compute, storage_gas)
    }

    /// Returns the gas required for deleting an actor.
    #[inline]
    pub fn on_delete_actor(&self) -> GasCharge {
        GasCharge::new("OnDeleteActor", Zero::zero(), self.delete_actor)
    }

    /// Returns gas required for signature verification.
    #[inline]
    pub fn on_verify_signature(&self, sig_type: SignatureType, data_len: usize) -> GasCharge {
        let cost = self.sig_cost[&sig_type];
        let gas = cost.apply(data_len);
        GasCharge::new("OnVerifySignature", gas, Zero::zero())
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
    pub fn on_hashing(&self, hasher: SupportedHashes, data_len: usize) -> GasCharge {
        let cost = self.hashing_cost[&hasher];
        let gas = cost.apply(data_len);
        GasCharge::new("OnHashing", gas, Zero::zero())
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

        let gas_used = cost.apply(info.challenged_sectors.len());

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
            Zero::zero(),
            self.extern_cost + self.verify_consensus_fault,
        )
    }

    /// Returns the cost of the gas required for getting randomness from the client, based on the
    /// numebr of bytes of entropy.
    #[inline]
    pub fn on_get_randomness(&self, entropy_size: usize) -> GasCharge {
        const RAND_INITIAL_HASH: u64 =
            // domain separation tag, u64
            8 +
            // vrf digest
            32 +
            // round
            8;

        GasCharge::new(
            "OnGetRandomness",
            Zero::zero(),
            self.extern_cost
                + self.get_randomness // TODO(m2.2): consider different values for
                + self
                .hashing_cost[&SupportedHashes::Blake2b256].apply((entropy_size as u64).saturating_add(RAND_INITIAL_HASH)),
        )
    }

    /// Returns the base gas required for loading an object, independent of the object's size.
    #[inline]
    pub fn on_block_open_base(&self) -> GasCharge {
        GasCharge::new(
            "OnBlockOpenBase",
            Zero::zero(),
            self.extern_cost + self.block_open.flat,
        )
    }

    /// Returns the gas required for loading an object based on the size of the object.
    #[inline]
    pub fn on_block_open_per_byte(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnBlockOpenPerByte",
            self.block_allocate.apply(data_size) + self.block_memcpy.apply(data_size),
            self.block_memory_retention.apply(data_size) + (self.block_open.scale * data_size),
        )
    }

    /// Returns the gas required for reading a loaded object.
    #[inline]
    pub fn on_block_read(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnBlockRead",
            self.block_memcpy.apply(data_size),
            Zero::zero(),
        )
    }

    /// Returns the gas required for adding an object to the FVM cache.
    #[inline]
    pub fn on_block_create(&self, data_size: usize) -> GasCharge {
        GasCharge::new(
            "OnBlockCreate",
            self.block_memcpy.apply(data_size) + self.block_allocate.apply(data_size),
            self.block_memory_retention.apply(data_size),
        )
    }

    /// Returns the gas required for committing an object to the state blockstore.
    #[inline]
    pub fn on_block_link(&self, hash_code: SupportedHashes, data_size: usize) -> GasCharge {
        // First we compute the expected cost of allocating & copying the block. We'll end up
        // charging it twice.
        let memcpy = self.block_memcpy.apply(data_size) + self.block_allocate.apply(data_size);

        // Then the cost of actually hashing the new block.
        let hashing = self.hashing_cost[&hash_code].apply(data_size);

        // Then the expected cost of flushing (client side).
        let flushing = self.block_flush.apply(data_size);

        // Then the cost of actually storing the block.
        let storage = self.block_storage.apply(data_size);

        GasCharge::new("OnBlockLink", memcpy + hashing, storage + memcpy + flushing)
    }

    /// Returns the gas required for storing an object.
    #[inline]
    pub fn on_block_stat(&self) -> GasCharge {
        GasCharge::new("OnBlockStat", Zero::zero(), Zero::zero())
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

    /// Returns the gas required for looking up an actor's delegated address.
    #[inline]
    pub fn on_lookup_delegated_address(&self) -> GasCharge {
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
            self.builtin_actor_manifest_lookup,
            Zero::zero(),
        )
    }

    /// Returns the gas required for looking up the CID of a builtin actor by type.
    #[inline]
    pub fn on_get_code_cid_for_type(&self) -> GasCharge {
        GasCharge::new(
            "OnGetCodeCidForType",
            self.builtin_actor_manifest_lookup,
            Zero::zero(),
        )
    }

    /// Returns the gas required for accessing the network context.
    #[inline]
    pub fn on_network_context(&self) -> GasCharge {
        GasCharge::new("OnNetworkContext", self.network_context, Zero::zero())
    }

    /// Returns the gas required for accessing the message context.
    #[inline]
    pub fn on_message_context(&self) -> GasCharge {
        GasCharge::new("OnMessageContext", self.message_context, Zero::zero())
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
        self.wasm_rules.memory_fill_base_cost
            + self.wasm_rules.memory_fill_per_byte_cost * min_memory_bytes
    }

    /// Returns the gas required for growing memory.
    pub fn grow_memory_gas(&self, grow_memory_bytes: usize) -> Gas {
        self.wasm_rules.memory_fill_base_cost
            + self.wasm_rules.memory_fill_per_byte_cost * grow_memory_bytes
    }

    /// Returns the gas required for initializing tables.
    pub fn init_table_gas(&self, min_table_elements: u32) -> Gas {
        self.wasm_rules.memory_fill_base_cost
            + self.wasm_rules.memory_fill_per_byte_cost * min_table_elements * TABLE_ELEMENT_SIZE
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
        NetworkVersion::V18 => &HYGGE_PRICES,
        _ => panic!("network version {nv} not supported", nv = network_version),
    }
}

impl Rules for WasmGasPrices {
    fn instruction_cost(&self, instruction: &Operator) -> anyhow::Result<InstructionCost> {
        use InstructionCost::*;

        fn linear_cost(
            base: Gas,
            linear: Gas,
            unit_multiplier: u32,
        ) -> anyhow::Result<InstructionCost> {
            let base = base
                .as_milligas()
                .try_into()
                .context("base gas exceeds u32")?;
            let gas_per_unit = linear * unit_multiplier;
            let expansion_cost: u32 = gas_per_unit
                .as_milligas()
                .try_into()
                .context("linear gas exceeds u32")?;
            match expansion_cost
                .try_into().ok() // zero or not zero.
            {
                Some(expansion_cost) => Ok(Linear(base, expansion_cost)),
                None => Ok(Fixed(base)),
            }
        }

        macro_rules! charge_inst {
            (unsupported($message:expr)) => {
                Err(anyhow::anyhow!($message))
            };
            (free()) => {
                Ok(Fixed(0))
            };
            (fixed($e:expr)) => {
                Ok(Fixed(($e).as_milligas() as u64))
            };
            (linear($base:expr,$linear:expr, $multiplier:expr)) => {
                linear_cost($base, $linear, $multiplier)
            };
        }

        macro_rules! charge_table {
            ($($($op:ident),+$(,)? => $kind:ident ($($args:expr),*$(,)?),)*) => {
                match instruction {
                    $(
                        $(| Operator::$op { .. })+ => {
                            charge_inst!($kind($($args),*))
                        },
                    )*
                }
            }
        }

        // Rules valid for nv16. We will need to be generic over Rules (massive
        // generics tax), use &dyn Rules (which breaks other things), or pass
        // in the network version, or rules version, to vary these prices going
        // forward.
        charge_table! {
            /******************/
            /*  Control Flow  */
            /******************/

            // FIP-0032: nop, block, loop, unreachable, return, else, end are priced 0.
            Nop, Block, Loop, Unreachable, Return, Else, End => free(),

            Br       => fixed(self.jump_unconditional),
            BrIf, If => fixed(self.jump_conditional),
            BrTable  => fixed(self.jump_indirect + self.memory_access_cost),

            // TODO M2.2: Charge to jump back, and charge for arguments.
            Call          => fixed(self.jump_unconditional + self.call),
            CallIndirect  => fixed(self.jump_indirect + self.memory_access_cost + self.call),

            /**********************/
            /*  Stack & Registers */
            /**********************/

            // Stack ops. Free due to FIP-0032.
            Drop => free(),

            // Constants, casts, etc.
            I64ExtendI32U,                          // widens
            I32WrapI64,                             // truncates
            I32ReinterpretF32, I64ReinterpretF64,   // casts
            F32ReinterpretI32, F64ReinterpretI64,   // casts other way
            I32Const, I64Const, F32Const, F64Const, // inline constants
            => fixed(self.instruction_default),

            // Locals (TODO M2.2). Consider making these free.
            LocalGet, LocalSet, LocalTee => fixed(self.instruction_default),

            // Globals (TODO M2.2). Consider making these free.
            GlobalGet, GlobalSet         => fixed(self.instruction_default),

            // Select.
            Select, TypedSelect          => fixed(self.instruction_default),

            /*********/
            /*  Math */
            /*********/

            // Sign extension
            I32Extend8S, I32Extend16S,
            I64Extend8S, I64Extend16S, I64Extend32S, I64ExtendI32S,
            => fixed(self.math_default),

            // Bitwise
            I32And, I32Or, I32Xor, I32Shl, I32ShrS, I32ShrU, I32Rotl, I32Rotr,
            I64And, I64Or, I64Xor, I64Shl, I64ShrS, I64ShrU, I64Rotl, I64Rotr,
            => fixed(self.math_default),

            // Comparison
            I32Eqz, I32Eq, I32Ne, I32LtS, I32LtU, I32GtS, I32GtU, I32LeS, I32LeU, I32GeS, I32GeU,
            I64Eqz, I64Eq, I64Ne, I64LtS, I64LtU, I64GtS, I64GtU, I64LeS, I64LeU, I64GeS, I64GeU,
            => fixed(self.math_default),

            // Math
            I32Clz, I32Ctz, I32Popcnt, I32Add, I32Sub, I32Mul, I32DivS, I32DivU, I32RemS, I32RemU,
            I64Clz, I64Ctz, I64Popcnt, I64Add, I64Sub, I64Mul, I64DivS, I64DivU, I64RemS, I64RemU,
            => fixed(self.math_default),

            // Floating point.
            I32TruncF32S, I32TruncF32U, I32TruncF64S, I32TruncF64U,
            I64TruncF32S, I64TruncF32U, I64TruncF64S, I64TruncF64U,
            I32TruncSatF32S, I32TruncSatF32U, I32TruncSatF64S, I32TruncSatF64U,
            I64TruncSatF32S, I64TruncSatF32U, I64TruncSatF64S, I64TruncSatF64U,
            F32Eq, F32Ne, F32Lt, F32Gt, F32Le, F32Ge,
            F64Eq, F64Ne, F64Lt, F64Gt, F64Le, F64Ge,
            F32Abs, F32Neg, F32Ceil, F32Floor, F32Trunc, F32Nearest, F32Add, F32Sub, F32Mul, F32Div, F32Min, F32Max,
            F64Abs, F64Neg, F64Ceil, F64Floor, F64Trunc, F64Nearest, F64Add, F64Sub, F64Mul, F64Div, F64Min, F64Max,
            F64Copysign, F32Copysign, F32DemoteF64, F64PromoteF32,
            F32ConvertI32S, F32ConvertI32U, F32ConvertI64S, F32ConvertI64U,
            F64ConvertI32S, F64ConvertI32U, F64ConvertI64S, F64ConvertI64U,
            => fixed(self.math_default),

            // Sqrt. TODO(M2.2): consider charging more (it's currently not used by the EVM, so it's
            // not a security concern).
            F32Sqrt, F64Sqrt => fixed(self.math_default),

            /************/
            /*  Memory  */
            /************/

            // Loads. These may eventually drop the "instruction" cost.
            F32Load, I32Load, I32Load8U, I32Load16U,
            F64Load, I64Load, I64Load8U, I64Load16U, I64Load32U,
            TableGet,
            => fixed(self.instruction_default + self.memory_access_cost),

            // Sign-extending loads.
            I32Load16S,
            I32Load8S,
            I64Load8S,
            I64Load16S,
            I64Load32S,
            => fixed(self.instruction_default + self.memory_access_cost),

            // Stores cost an instruction and a base fill fee.
            F32Store, I32Store, I32Store8, I32Store16,
            F64Store, I64Store, I64Store8, I64Store16, I64Store32,
            TableSet,
            => fixed(self.instruction_default + self.memory_fill_base_cost),

            // Bulk memory copies & fills
            TableInit, TableCopy => linear(
                self.instruction_default + self.memory_access_cost,
                self.memory_copy_per_byte_cost,
                TABLE_ELEMENT_SIZE,
            ),
            TableFill, TableGrow => linear(
                self.instruction_default + self.memory_fill_base_cost,
                self.memory_fill_per_byte_cost,
                TABLE_ELEMENT_SIZE,
            ),
            MemoryGrow => linear(
                self.instruction_default + self.memory_fill_base_cost,
                self.memory_fill_per_byte_cost,
                // This is the odd-one out because it operates on entire pages.
                wasmtime_environ::WASM_PAGE_SIZE,
            ),
            MemoryFill => linear(
                self.instruction_default + self.memory_fill_base_cost,
                self.memory_fill_per_byte_cost,
                1,
            ),
            MemoryInit, MemoryCopy => linear(
                self.instruction_default + self.memory_access_cost,
                self.memory_copy_per_byte_cost,
                1,
            ),

            // Dropping is an optimization hint and probably shouldn't cost anything. But we don't
            // use this right now anyways.
            // TODO(M2.2) consider making this free.
            DataDrop, ElemDrop => fixed(self.instruction_default),

            // Charge one instruction for getting a table/memory size.
            MemorySize, TableSize => fixed(self.instruction_default),

            /******************/
            /*  Unsupported   */
            /******************/

            // Exception handling.

            Try, Catch, Throw, Rethrow, CatchAll, Delegate,

            // Tail calls.
            ReturnCall, ReturnCallIndirect,

            // Reference ops

            RefNull, RefIsNull, RefFunc,

            // All atomic operations

            MemoryAtomicNotify, MemoryAtomicWait32, MemoryAtomicWait64, AtomicFence,
            I32AtomicLoad, I32AtomicLoad8U, I32AtomicLoad16U,
            I64AtomicLoad, I64AtomicLoad8U, I64AtomicLoad16U, I64AtomicLoad32U,
            I32AtomicStore, I32AtomicStore8, I32AtomicStore16,
            I64AtomicStore, I64AtomicStore8, I64AtomicStore16, I64AtomicStore32,
            I32AtomicRmwAdd, I32AtomicRmw8AddU, I32AtomicRmw16AddU,
            I64AtomicRmwAdd, I64AtomicRmw8AddU, I64AtomicRmw16AddU, I64AtomicRmw32AddU,
            I32AtomicRmwSub, I32AtomicRmw8SubU, I32AtomicRmw16SubU,
            I64AtomicRmwSub, I64AtomicRmw8SubU, I64AtomicRmw16SubU, I64AtomicRmw32SubU,
            I32AtomicRmwAnd, I32AtomicRmw8AndU, I32AtomicRmw16AndU,
            I64AtomicRmwAnd, I64AtomicRmw8AndU, I64AtomicRmw16AndU, I64AtomicRmw32AndU,
            I32AtomicRmwOr, I32AtomicRmw8OrU, I32AtomicRmw16OrU,
            I64AtomicRmwOr, I64AtomicRmw8OrU, I64AtomicRmw16OrU, I64AtomicRmw32OrU,
            I32AtomicRmwXor, I32AtomicRmw8XorU, I32AtomicRmw16XorU,
            I64AtomicRmwXor, I64AtomicRmw8XorU, I64AtomicRmw16XorU, I64AtomicRmw32XorU,
            I32AtomicRmwXchg, I32AtomicRmw8XchgU, I32AtomicRmw16XchgU,
            I64AtomicRmwXchg, I64AtomicRmw8XchgU, I64AtomicRmw16XchgU, I64AtomicRmw32XchgU,
            I32AtomicRmwCmpxchg, I32AtomicRmw8CmpxchgU, I32AtomicRmw16CmpxchgU,
            I64AtomicRmwCmpxchg, I64AtomicRmw8CmpxchgU, I64AtomicRmw16CmpxchgU, I64AtomicRmw32CmpxchgU,

            // All SIMD operations.

            V128Load, V128Store, V128Const,
            V128Load8x8S, V128Load16x4S, V128Load32x2S,
            V128Load8x8U, V128Load16x4U, V128Load32x2U,
            V128Load8Splat, V128Load16Splat, V128Load32Splat, V128Load64Splat,
            V128Load32Zero, V128Load64Zero,
            V128Load8Lane, V128Load16Lane, V128Load32Lane, V128Load64Lane,
            V128Store8Lane, V128Store16Lane, V128Store32Lane, V128Store64Lane,
            I8x16Shuffle,
            I8x16ReplaceLane, I8x16ExtractLaneS, I16x8ExtractLaneS,
            I16x8ReplaceLane, I8x16ExtractLaneU, I16x8ExtractLaneU,
            I32x4ExtractLane, I64x2ExtractLane, F32x4ExtractLane, F64x2ExtractLane,
            I32x4ReplaceLane, I64x2ReplaceLane, F32x4ReplaceLane, F64x2ReplaceLane,
            I8x16Swizzle, I8x16RelaxedSwizzle,
            I8x16Splat, I16x8Splat, I32x4Splat, I64x2Splat, F32x4Splat, F64x2Splat,
            I8x16Eq, I8x16Ne, I8x16LtS, I8x16LtU, I8x16GtS, I8x16GtU, I8x16LeS, I8x16LeU, I8x16GeS, I8x16GeU,
            I16x8Eq, I16x8Ne, I16x8LtS, I16x8LtU, I16x8GtS, I16x8GtU, I16x8LeS, I16x8LeU, I16x8GeS, I16x8GeU,
            I32x4Eq, I32x4Ne, I32x4LtS, I32x4LtU, I32x4GtS, I32x4GtU, I32x4LeS, I32x4LeU, I32x4GeS, I32x4GeU,
            I64x2Eq, I64x2Ne, I64x2LtS, I64x2GtS, I64x2LeS, I64x2GeS,
            F32x4Eq, F32x4Ne, F32x4Lt, F32x4Gt,
            F32x4Le, F32x4Ge, F64x2Eq, F64x2Ne, F64x2Lt, F64x2Gt, F64x2Le, F64x2Ge,
            V128Not, V128And, V128AndNot, V128Or, V128Xor, V128Bitselect, V128AnyTrue,
            I8x16Abs, I8x16Neg, I8x16Popcnt, I8x16AllTrue, I8x16Bitmask, I8x16NarrowI16x8S,
            I8x16NarrowI16x8U, I8x16Shl, I8x16ShrS, I8x16ShrU, I8x16Add, I8x16AddSatS, I8x16AddSatU,
            I8x16Sub, I8x16SubSatS, I8x16SubSatU, I8x16MinS, I8x16MinU, I8x16MaxS, I8x16MaxU, I8x16AvgrU,
            I16x8ExtAddPairwiseI8x16S, I16x8ExtAddPairwiseI8x16U, I16x8Abs, I16x8Neg, I16x8Q15MulrSatS,
            I16x8AllTrue, I16x8Bitmask, I16x8NarrowI32x4S, I16x8NarrowI32x4U, I16x8ExtendLowI8x16S,
            I16x8ExtendHighI8x16S, I16x8ExtendLowI8x16U, I16x8ExtendHighI8x16U, I16x8Shl, I16x8ShrS,
            I16x8ShrU, I16x8Add, I16x8AddSatS, I16x8AddSatU, I16x8Sub, I16x8SubSatS, I16x8SubSatU,
            I16x8Mul, I16x8MinS, I16x8MinU, I16x8MaxS, I16x8MaxU, I16x8AvgrU, I16x8ExtMulLowI8x16S,
            I16x8ExtMulHighI8x16S, I16x8ExtMulLowI8x16U, I16x8ExtMulHighI8x16U,
            I32x4ExtAddPairwiseI16x8S, I32x4ExtAddPairwiseI16x8U, I32x4Abs, I32x4Neg, I32x4AllTrue,
            I32x4Bitmask, I32x4ExtendLowI16x8S, I32x4ExtendHighI16x8S, I32x4ExtendLowI16x8U,
            I32x4ExtendHighI16x8U, I32x4Shl, I32x4ShrS, I32x4ShrU, I32x4Add, I32x4Sub, I32x4Mul,
            I32x4MinS, I32x4MinU, I32x4MaxS, I32x4MaxU, I32x4DotI16x8S, I32x4ExtMulLowI16x8S,
            I32x4ExtMulHighI16x8S, I32x4ExtMulLowI16x8U, I32x4ExtMulHighI16x8U,
            I64x2Abs, I64x2Neg, I64x2AllTrue, I64x2Bitmask, I64x2ExtendLowI32x4S,
            I64x2ExtendHighI32x4S, I64x2ExtendLowI32x4U, I64x2ExtendHighI32x4U, I64x2Shl,
            I64x2ShrS, I64x2ShrU, I64x2Add, I64x2Sub, I64x2Mul, I64x2ExtMulLowI32x4S,
            I64x2ExtMulHighI32x4S, I64x2ExtMulLowI32x4U, I64x2ExtMulHighI32x4U,
            F32x4Ceil, F32x4Floor, F32x4Trunc, F32x4Nearest, F32x4Abs, F32x4Neg, F32x4Sqrt,
            F32x4Add, F32x4Sub, F32x4Mul, F32x4Div, F32x4Min, F32x4Max, F32x4PMin, F32x4PMax,
            F64x2Ceil, F64x2Floor, F64x2Trunc, F64x2Nearest, F64x2Abs, F64x2Neg, F64x2Sqrt,
            F64x2Add, F64x2Sub, F64x2Mul, F64x2Div, F64x2Min, F64x2Max, F64x2PMin, F64x2PMax,
            I32x4TruncSatF32x4S, I32x4TruncSatF32x4U,
            F32x4ConvertI32x4S, F32x4ConvertI32x4U,
            I32x4TruncSatF64x2SZero, I32x4TruncSatF64x2UZero,
            F64x2ConvertLowI32x4S, F64x2ConvertLowI32x4U,
            F32x4DemoteF64x2Zero, F64x2PromoteLowF32x4,
            I32x4RelaxedTruncSatF32x4S, I32x4RelaxedTruncSatF64x2SZero,
            I32x4RelaxedTruncSatF32x4U, I32x4RelaxedTruncSatF64x2UZero,
            F32x4RelaxedFma, F64x2RelaxedFma,
            F32x4RelaxedFnma, F64x2RelaxedFnma,
            I8x16RelaxedLaneselect, I16x8RelaxedLaneselect, I32x4RelaxedLaneselect, I64x2RelaxedLaneselect,
            F32x4RelaxedMin, F32x4RelaxedMax, F64x2RelaxedMin, F64x2RelaxedMax,
            I16x8RelaxedQ15mulrS,
            I16x8DotI8x16I7x16S, I32x4DotI8x16I7x16AddS,
            F32x4RelaxedDotBf16x8AddF32x4,
            => unsupported("unsupported operation"),
        }
    }

    fn gas_charge_cost(&self) -> u64 {
        0
    }

    fn linear_calc_cost(&self) -> u64 {
        0
    }
}
