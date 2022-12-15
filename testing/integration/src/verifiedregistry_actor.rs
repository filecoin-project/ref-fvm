use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::Address;
use fil_actors_runtime::MapMap;
use fvm_shared::ActorID;
use fvm_shared::HAMT_BIT_WIDTH;


#[derive(Serialize_tuple, Deserialize_tuple, Debug, Clone)]
pub struct State {
    pub root_key: Address,
    // Maps verifier addresses to data cap minting allowance (in bytes).
    pub verifiers: Cid, // HAMT[Address]DataCap
    pub remove_data_cap_proposal_ids: Cid,
    // Maps client IDs to allocations made by that client.
    pub allocations: Cid, // HAMT[ActorID]HAMT[AllocationID]Allocation
    // Next allocation identifier to use.
    // The value 0 is reserved to mean "no allocation".
    pub next_allocation_id: u64,
    // Maps provider IDs to allocations claimed by that provider.
    pub claims: Cid, // HAMT[ActorID]HAMT[ClaimID]Claim
}

impl State {
    // ideally we would just #[cfg(test)] this, but it is used by non test-gated code in
    // integration/tester.
    #[allow(unused)]
    pub fn new_test<BS: Blockstore>(store: &BS, root_key: Address) -> Self {
        let empty_map = Hamt::<_, ()>::new_with_bit_width(store, 5)
        .flush()
        .unwrap();

    let empty_mapmap =
        MapMap::<_, (), ActorID, u64>::new(store, HAMT_BIT_WIDTH, HAMT_BIT_WIDTH)
            .flush()
            .unwrap();

        Self {
            root_key,
            verifiers: empty_map,
            remove_data_cap_proposal_ids: empty_map,
            allocations: empty_mapmap,
            next_allocation_id: 1,
            claims: empty_mapmap,
        }
    }

}
