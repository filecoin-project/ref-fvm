// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use actors_runtime::runtime::{ActorCode, Runtime};
use actors_runtime::{
    actor_error, make_map_with_root_and_bitwidth, resolve_to_id_addr, wasm_trampoline,
    ActorDowncast, ActorError, Map, STORAGE_MARKET_ACTOR_ADDR, SYSTEM_ACTOR_ADDR,
};
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser::BigIntDe;
use fvm_shared::blockstore::Blockstore;
use fvm_shared::encoding::de::DeserializeOwned;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::{MethodNum, HAMT_BIT_WIDTH, METHOD_CONSTRUCTOR};
use ipld_hamt::BytesKey;
use num_derive::FromPrimitive;
use num_traits::{FromPrimitive, Signed, Zero};

pub use self::state::State;
pub use self::types::*;

/// Export the wasm binary
#[cfg(not(feature = "runtime-wasm"))]
pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_wasm_binaries() {
            assert!(!WASM_BINARY.unwrap().is_empty());
            assert!(!WASM_BINARY_BLOATY.unwrap().is_empty());
        }
    }
}

wasm_trampoline!(Actor);

mod state;
mod types;

// * Updated to specs-actors commit: 845089a6d2580e46055c24415a6c32ee688e5186 (v3.0.0)

/// Account actor methods available
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
    AddVerifier = 2,
    RemoveVerifier = 3,
    AddVerifiedClient = 4,
    UseBytes = 5,
    RestoreBytes = 6,
    RemoveVerifiedClientDataCap = 7,
}

pub struct Actor;
impl Actor {
    /// Constructor for Registry Actor
    pub fn constructor<BS, RT>(rt: &mut RT, root_key: Address) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_is(std::iter::once(&*SYSTEM_ACTOR_ADDR))?;

        // root should be an ID address
        let id_addr = rt
            .resolve_address(&root_key)
            .ok_or_else(|| actor_error!(ErrIllegalArgument, "root should be an ID address"))?;

        let st = State::new(rt.store(), id_addr).map_err(|e| {
            e.downcast_default(ExitCode::ErrIllegalState, "Failed to create verifreg state")
        })?;

        rt.create(&st)?;
        Ok(())
    }

    pub fn add_verifier<BS, RT>(rt: &mut RT, params: AddVerifierParams) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        if &params.allowance < &MINIMUM_VERIFIED_DEAL_SIZE {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Allowance {} below minimum deal size for add verifier {}",
                params.allowance,
                params.address
            ));
        }

        let verifier = resolve_to_id_addr(rt, &params.address).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalState,
                format!("failed to resolve addr {} to ID addr", params.address),
            )
        })?;

        let st: State = rt.state()?;
        rt.validate_immediate_caller_is(std::iter::once(&st.root_key))?;

        if verifier == st.root_key {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Rootkey cannot be added as verifier"
            ));
        }

        rt.transaction(|st: &mut State, rt| {
            let mut verifiers =
                make_map_with_root_and_bitwidth(&st.verifiers, rt.store(), HAMT_BIT_WIDTH)
                    .map_err(|e| {
                        e.downcast_default(
                            ExitCode::ErrIllegalState,
                            "failed to load verified clients",
                        )
                    })?;
            let verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
                &st.verified_clients,
                rt.store(),
                HAMT_BIT_WIDTH,
            )
            .map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
            })?;

            let found = verified_clients
                .contains_key(&verifier.to_bytes())
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to get client state for {}", verifier),
                    )
                })?;
            if found {
                return Err(actor_error!(
                    ErrIllegalArgument,
                    "verified client {} cannot become a verifier",
                    verifier
                ));
            }

            verifiers
                .set(
                    verifier.to_bytes().into(),
                    BigIntDe(params.allowance.clone()),
                )
                .map_err(|e| {
                    e.downcast_default(ExitCode::ErrIllegalState, "failed to add verifier")
                })?;
            st.verifiers = verifiers.flush().map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to flush verifiers")
            })?;

            Ok(())
        })?;

        Ok(())
    }

    pub fn remove_verifier<BS, RT>(rt: &mut RT, verifier_addr: Address) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        let verifier = resolve_to_id_addr(rt, &verifier_addr).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalState,
                format!("failed to resolve addr {} to ID addr", verifier_addr),
            )
        })?;

        let state: State = rt.state()?;
        rt.validate_immediate_caller_is(std::iter::once(&state.root_key))?;

        rt.transaction(|st: &mut State, rt| {
            let mut verifiers = make_map_with_root_and_bitwidth::<_, BigIntDe>(
                &st.verifiers,
                rt.store(),
                HAMT_BIT_WIDTH,
            )
            .map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
            })?;
            verifiers
                .delete(&verifier.to_bytes())
                .map_err(|e| {
                    e.downcast_default(ExitCode::ErrIllegalState, "failed to remove verifier")
                })?
                .ok_or_else(|| {
                    actor_error!(ErrIllegalArgument, "failed to remove verifier: not found")
                })?;

            st.verifiers = verifiers.flush().map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to flush verifiers")
            })?;
            Ok(())
        })?;

        Ok(())
    }

    pub fn add_verified_client<BS, RT>(
        rt: &mut RT,
        params: AddVerifierClientParams,
    ) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        // The caller will be verified by checking table below
        rt.validate_immediate_caller_accept_any()?;

        if params.allowance < *MINIMUM_VERIFIED_DEAL_SIZE {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Allowance {} below MinVerifiedDealSize for add verified client {}",
                params.allowance,
                params.address
            ));
        }

        let client = resolve_to_id_addr(rt, &params.address).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalState,
                format!("failed to resolve addr {} to ID addr", params.address),
            )
        })?;

        let st: State = rt.state()?;
        if client == st.root_key {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Rootkey cannot be added as verifier"
            ));
        }

        rt.transaction(|st: &mut State, rt| {
            let mut verifiers =
                make_map_with_root_and_bitwidth(&st.verifiers, rt.store(), HAMT_BIT_WIDTH)
                    .map_err(|e| {
                        e.downcast_default(
                            ExitCode::ErrIllegalState,
                            "failed to load verified clients",
                        )
                    })?;
            let mut verified_clients =
                make_map_with_root_and_bitwidth(&st.verified_clients, rt.store(), HAMT_BIT_WIDTH)
                    .map_err(|e| {
                    e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
                })?;

            // Validate caller is one of the verifiers.
            let verifier = rt.message().caller();
            let BigIntDe(verifier_cap) = verifiers
                .get(&verifier.to_bytes())
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to get Verifier {}", verifier),
                    )
                })?
                .ok_or_else(|| {
                    actor_error!(ErrNotFound, format!("no such Verifier {}", verifier))
                })?;

            // Validate client to be added isn't a verifier
            let found = verifiers.contains_key(&client.to_bytes()).map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to get verifier")
            })?;
            if found {
                return Err(actor_error!(
                    ErrIllegalArgument,
                    "verifier {} cannot be added as a verified client",
                    client
                ));
            }

            // Compute new verifier cap and update.
            if verifier_cap < &params.allowance {
                return Err(actor_error!(
                    ErrIllegalArgument,
                    "Add more DataCap {} for VerifiedClient than allocated {}",
                    params.allowance,
                    verifier_cap
                ));
            }
            let new_verifier_cap = verifier_cap - &params.allowance;

            verifiers
                .set(verifier.to_bytes().into(), BigIntDe(new_verifier_cap))
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("Failed to update new verifier cap for {}", verifier),
                    )
                })?;

            let client_cap = verified_clients.get(&client.to_bytes()).map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalState,
                    format!("Failed to get verified client {}", client),
                )
            })?;
            // if verified client exists, add allowance to existing cap
            // otherwise, create new client with allownace
            let client_cap = if let Some(BigIntDe(client_cap)) = client_cap {
                client_cap + params.allowance
            } else {
                params.allowance
            };

            verified_clients
                .set(client.to_bytes().into(), BigIntDe(client_cap.clone()))
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!(
                            "Failed to add verified client {} with cap {}",
                            client, client_cap,
                        ),
                    )
                })?;

            st.verifiers = verifiers.flush().map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to flush verifiers")
            })?;
            st.verified_clients = verified_clients.flush().map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalState,
                    "failed to flush verified clients",
                )
            })?;

            Ok(())
        })?;

        Ok(())
    }

    /// Called by StorageMarketActor during PublishStorageDeals.
    /// Do not allow partially verified deals (DealSize must be greater than equal to allowed cap).
    /// Delete VerifiedClient if remaining DataCap is smaller than minimum VerifiedDealSize.
    pub fn use_bytes<BS, RT>(rt: &mut RT, params: UseBytesParams) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_is(std::iter::once(&*STORAGE_MARKET_ACTOR_ADDR))?;

        let client = resolve_to_id_addr(rt, &params.address).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalState,
                format!("failed to resolve addr {} to ID addr", params.address),
            )
        })?;

        if params.deal_size < *MINIMUM_VERIFIED_DEAL_SIZE {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Verified Dealsize {} is below minimum in usedbytes",
                params.deal_size
            ));
        }

        rt.transaction(|st: &mut State, rt| {
            let mut verified_clients =
                make_map_with_root_and_bitwidth(&st.verified_clients, rt.store(), HAMT_BIT_WIDTH)
                    .map_err(|e| {
                    e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
                })?;

            let BigIntDe(vc_cap) = verified_clients
                .get(&client.to_bytes())
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to get verified client {}", &client),
                    )
                })?
                .ok_or_else(|| actor_error!(ErrNotFound, "no such verified client {}", client))?;
            if vc_cap.is_negative() {
                return Err(actor_error!(
                    ErrIllegalState,
                    "negative cap for client {}: {}",
                    client,
                    vc_cap
                ));
            }

            if &params.deal_size > vc_cap {
                return Err(actor_error!(
                    ErrIllegalArgument,
                    "Deal size of {} is greater than verifier_cap {} for verified client {}",
                    params.deal_size,
                    vc_cap,
                    client
                ));
            };

            let new_vc_cap = vc_cap - &params.deal_size;
            if new_vc_cap < *MINIMUM_VERIFIED_DEAL_SIZE {
                // Delete entry if remaining DataCap is less than MinVerifiedDealSize.
                // Will be restored later if the deal did not get activated with a ProvenSector.
                verified_clients
                    .delete(&client.to_bytes())
                    .map_err(|e| {
                        e.downcast_default(
                            ExitCode::ErrIllegalState,
                            format!("Failed to delete verified client {}", client),
                        )
                    })?
                    .ok_or_else(|| {
                        actor_error!(
                            ErrIllegalState,
                            "Failed to delete verified client {}: not found",
                            client
                        )
                    })?;
            } else {
                verified_clients
                    .set(client.to_bytes().into(), BigIntDe(new_vc_cap))
                    .map_err(|e| {
                        e.downcast_default(
                            ExitCode::ErrIllegalState,
                            format!("Failed to update verified client {}", client),
                        )
                    })?;
            }

            st.verified_clients = verified_clients.flush().map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalState,
                    "failed to flush verified clients",
                )
            })?;
            Ok(())
        })?;

        Ok(())
    }

    /// Called by HandleInitTimeoutDeals from StorageMarketActor when a VerifiedDeal fails to init.
    /// Restore allowable cap for the client, creating new entry if the client has been deleted.
    pub fn restore_bytes<BS, RT>(rt: &mut RT, params: RestoreBytesParams) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_is(std::iter::once(&*STORAGE_MARKET_ACTOR_ADDR))?;
        if params.deal_size < *MINIMUM_VERIFIED_DEAL_SIZE {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Below minimum VerifiedDealSize requested in RestoreBytes: {}",
                params.deal_size
            ));
        }

        let client = resolve_to_id_addr(rt, &params.address).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalState,
                format!("failed to resolve addr {} to ID addr", params.address),
            )
        })?;

        let st: State = rt.state()?;
        if client == st.root_key {
            return Err(actor_error!(
                ErrIllegalArgument,
                "Cannot restore allowance for Rootkey"
            ));
        }

        rt.transaction(|st: &mut State, rt| {
            let mut verified_clients =
                make_map_with_root_and_bitwidth(&st.verified_clients, rt.store(), HAMT_BIT_WIDTH)
                    .map_err(|e| {
                    e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
                })?;
            let verifiers = make_map_with_root_and_bitwidth::<_, BigIntDe>(
                &st.verifiers,
                rt.store(),
                HAMT_BIT_WIDTH,
            )
            .map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to load verifiers")
            })?;

            // validate we are NOT attempting to do this for a verifier
            let found = verifiers.contains_key(&client.to_bytes()).map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to get verifier")
            })?;
            if found {
                return Err(actor_error!(
                    ErrIllegalArgument,
                    "cannot restore allowance for a verifier {}",
                    client
                ));
            }

            // Get existing cap
            let BigIntDe(vc_cap) = verified_clients
                .get(&client.to_bytes())
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to get verified client {}", &client),
                    )
                })?
                .cloned()
                .unwrap_or_default();

            // Update to new cap
            let new_vc_cap = vc_cap + &params.deal_size;
            verified_clients
                .set(client.to_bytes().into(), BigIntDe(new_vc_cap))
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("Failed to put verified client {}", client),
                    )
                })?;

            st.verified_clients = verified_clients.flush().map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalState,
                    "failed to flush verified clients",
                )
            })?;
            Ok(())
        })?;

        Ok(())
    }

    /// Removes DataCap allocated to a verified client.
    pub fn remove_verified_client_data_cap<BS, RT>(
        rt: &mut RT,
        params: RemoveDataCapParams,
    ) -> Result<RemoveDataCapReturn, ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        let client = resolve_to_id_addr(rt, &params.verified_client_to_remove).map_err(|e| {
            e.downcast_default(
                ExitCode::ErrIllegalArgument,
                format!(
                    "failed to resolve client addr {} to ID addr",
                    params.verified_client_to_remove
                ),
            )
        })?;

        let verifier_1 =
            resolve_to_id_addr(rt, &params.verifier_request_1.verifier).map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalArgument,
                    format!(
                        "failed to resolve verifier addr {} to ID addr",
                        params.verifier_request_1.verifier
                    ),
                )
            })?;

        let verifier_2 =
            resolve_to_id_addr(rt, &params.verifier_request_2.verifier).map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalArgument,
                    format!(
                        "failed to resolve verifier addr {} to ID addr",
                        params.verifier_request_2.verifier
                    ),
                )
            })?;

        if verifier_1 == verifier_2 {
            return Err(actor_error!(
                ErrIllegalArgument,
                "need two different verifiers to send remove datacap request"
            ));
        }

        let mut removed_data_cap_amount = DataCap::default();
        let st: State = rt.state()?;
        rt.transaction(|st: &mut State, rt| {
            rt.validate_immediate_caller_is(std::iter::once(&st.root_key))?;

            // get current verified clients
            let mut verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
                &st.verified_clients,
                rt.store(),
                HAMT_BIT_WIDTH,
            )
            .map_err(|e| {
                e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
            })?;

            // check that `client` is currently a verified client
            if !is_verifier(rt, st, client)? {
                return Err(actor_error!(
                    ErrNotFound,
                    "{} is not a verified client",
                    client
                ));
            }

            // get existing cap allocated to client
            let BigIntDe(previous_data_cap) = verified_clients
                .get(&client.to_bytes())
                .map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to get verified client {}", &client),
                    )
                })?
                .cloned()
                .unwrap_or_default();

            // check that `verifier_1` is currently a verified client
            if !is_verifier(rt, st, verifier_1)? {
                return Err(actor_error!(
                    ErrNotFound,
                    "{} is not a verified client",
                    verifier_1
                ));
            }

            // check that `verifier_2` is currently a verified client
            if !is_verifier(rt, st, verifier_2)? {
                return Err(actor_error!(
                    ErrNotFound,
                    "{} is not a verified client",
                    verifier_2
                ));
            }

            // validate signatures
            let mut proposal_ids = make_map_with_root_and_bitwidth::<_, RemoveDataCapProposalID>(
                &st.remove_data_cap_proposal_ids,
                rt.store(),
                HAMT_BIT_WIDTH,
            )
            .map_err(|e| {
                e.downcast_default(
                    ExitCode::ErrIllegalState,
                    "failed to load datacap removal proposal ids",
                )
            })?;

            let verifier_1_id = use_proposal_id(rt, &mut proposal_ids, verifier_1, client)?;
            let verifier_2_id = use_proposal_id(rt, &mut proposal_ids, verifier_2, client)?;

            remove_data_cap_request_is_valid_or_abort(
                rt,
                &params.verifier_request_1,
                verifier_1_id,
                &params.data_cap_amount_to_remove,
                client,
            )?;
            remove_data_cap_request_is_valid_or_abort(
                rt,
                &params.verifier_request_2,
                verifier_2_id,
                &params.data_cap_amount_to_remove,
                client,
            )?;

            let new_data_cap = &previous_data_cap - &params.data_cap_amount_to_remove;
            if new_data_cap <= Zero::zero() {
                // no DataCap remaining, delete verified client
                verified_clients.delete(&client.to_bytes()).map_err(|e| {
                    e.downcast_default(
                        ExitCode::ErrIllegalState,
                        format!("failed to delete verified client {}", &client),
                    )
                })?;
                removed_data_cap_amount = previous_data_cap;
            } else {
                // update DataCap amount after removal
                verified_clients
                    .set(BytesKey::from(client.to_bytes()), BigIntDe(new_data_cap))
                    .map_err(|e| {
                        e.downcast_default(
                            ExitCode::ErrIllegalState,
                            format!("failed to update datacap for verified client {}", &client),
                        )
                    })?;
                removed_data_cap_amount = params.data_cap_amount_to_remove.clone();
            }

            st.remove_data_cap_proposal_ids = proposal_ids.flush().map_err(|e| {
                actor_error! {
                    ErrIllegalState,
                    "failed to flush proposal ids: {}",
                    e
                }
            })?;
            st.verified_clients = verified_clients.flush().map_err(|e| {
                actor_error! {
                    ErrIllegalState,
                    "failed to flush verified clients: {}",
                    e
                }
            })?;
            Ok(())
        })?;

        Ok(RemoveDataCapReturn {
            verified_client: params.verified_client_to_remove,
            data_cap_removed: removed_data_cap_amount,
        })
    }
}

fn is_verifier<BS, RT>(rt: &RT, st: &State, address: Address) -> Result<bool, ActorError>
where
    BS: Blockstore,
    RT: Runtime<BS>,
{
    let verified_clients = make_map_with_root_and_bitwidth::<_, BigIntDe>(
        &st.verified_clients,
        rt.store(),
        HAMT_BIT_WIDTH,
    )
    .map_err(|e| {
        e.downcast_default(ExitCode::ErrIllegalState, "failed to load verified clients")
    })?;

    // check that the `address` is currently a verified client
    let found = verified_clients
        .contains_key(&address.to_bytes())
        .map_err(|e| e.downcast_default(ExitCode::ErrIllegalState, "failed to get verifier"))?;

    Ok(found)
}

fn use_proposal_id<BS, RT>(
    rt: &RT,
    proposal_ids: &mut Map<BS, RemoveDataCapProposalID>,
    verifier: Address,
    client: Address,
) -> Result<RemoveDataCapProposalID, ActorError>
where
    BS: Blockstore,
    RT: Runtime<BS>,
    //  V: DeserializeOwned + Serialize,
{
    let key = AddrPairKey::new(verifier, client);

    let maybe_id = proposal_ids.get(&key.to_bytes()).map_err(|e| {
        actor_error!(
            ErrIllegalState,
            "failed to get proposal id for verifier {} and client {}",
            verifier,
            client
        )
    })?;

    let curr_id = if let Some(RemoveDataCapProposalID(id)) = maybe_id {
        RemoveDataCapProposalID(*id)
    } else {
        RemoveDataCapProposalID(0)
    };

    let next_id = RemoveDataCapProposalID(curr_id.0 + 1);
    proposal_ids
        .set(BytesKey::from(key.to_bytes()), next_id.clone())
        .map_err(|e| {
            actor_error!(
                ErrIllegalState,
                "failed to update proposal id for verifier {} and client {}",
                verifier,
                client
            )
        })?;

    Ok(next_id)
}

fn remove_data_cap_request_is_valid_or_abort<BS, RT>(
    rt: &RT,
    request: &RemoveDataCapRequest,
    id: RemoveDataCapProposalID,
    to_remove: &DataCap,
    client: Address,
) -> Result<(), ActorError>
where
    BS: Blockstore,
    RT: Runtime<BS>,
{
    let proposal = RemoveDataCapProposal {
        removal_proposal_id: id,
        data_cap_amount: to_remove.clone(),
        verified_client: client,
    };

    // TODO: in lotus this does rt.Abortf, what's the equivalent here?
    let buf = RawBytes::serialize(proposal).map_err(|e| {
        actor_error!(
            ErrSerialization,
            "failed to marshal remove datacap request: {}",
            e
        )
    })?;

    // verify signature of proposal
    // TOOD: same here w/ the Abortf
    rt.verify_signature(&request.signature, &request.verifier, &buf)
        .map_err(|e| {
            actor_error!(
                ErrIllegalArgument,
                "invalid signature for datacap removal request: {}",
                e
            )
        })?;

    Ok(())
}

impl ActorCode for Actor {
    fn invoke_method<BS, RT>(
        rt: &mut RT,
        method: MethodNum,
        params: &RawBytes,
    ) -> Result<RawBytes, ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        match FromPrimitive::from_u64(method) {
            Some(Method::Constructor) => {
                Self::constructor(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::AddVerifier) => {
                Self::add_verifier(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::RemoveVerifier) => {
                Self::remove_verifier(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::AddVerifiedClient) => {
                Self::add_verified_client(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::UseBytes) => {
                Self::use_bytes(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::RestoreBytes) => {
                Self::restore_bytes(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::RemoveVerifiedClientDataCap) => {
                Self::remove_verified_client_data_cap(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            None => Err(actor_error!(SysErrInvalidMethod; "Invalid method")),
        }
    }
}
