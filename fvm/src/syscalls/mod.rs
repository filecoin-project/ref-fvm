use cid::Cid;
use wasmtime::Linker;

use crate::call_manager::backtrace;
use crate::kernel::Result;
use crate::Kernel;

pub(crate) mod error;

mod actor;
mod bind;
mod context;
mod crypto;
mod debug;
mod gas;
mod ipld;
mod message;
mod network;
mod rand;
mod send;
mod sself;
mod vm;

pub(self) use context::Context;

/// Invocation data attached to a wasm "store" and available to the syscall binding.
pub struct InvocationData<K> {
    /// The kernel on which this actor is being executed.
    pub kernel: K,
    /// The last-seen syscall error. This error is considered the abort "cause" if an actor aborts
    /// after receiving this error without calling any other syscalls.
    pub last_error: Option<backtrace::Cause>,

    /// A snapshot is _taken_ every time execution returns _from_ WASM.
    /// This happens at the start of a syscall invocation as well as the end of WASM invocation.
    /// The snapshot is _updated_ every time execution moves _into_ WASM.
    /// This happens at the first WASM invocation, and at the end of a syscall invocation.
    /// Before a snapshot is _updated_, fuel is consumed based on the delta.
    pub gas_available_snapshot: i64,

    /// A snapshot is _taken_ every time execution moves _into_ WASM.
    /// This happens at the first WASM invocation, and at the end of a syscall invocation.
    /// The snapshot is _updated_ every time execution returns _from_ WASM.
    /// This happens at the start of a syscall invocation as well as the end of WASM invocation.
    /// Before a snapshot is _updated_, gas is consumed based on the delta.
    pub fuel_consumed_snapshot: u64,
}

impl<K> InvocationData<K> {
    pub(crate) fn new(kernel: K, gas_available: i64) -> Self {
        Self {
            kernel,
            last_error: None,
            gas_available_snapshot: gas_available,
            fuel_consumed_snapshot: 0,
        }
    }
}

impl<K: Kernel> InvocationData<K> {
    // This method:
    // 1) calculates the available_gas delta from the previous snapshot,
    // 2) updates the available_gas snapshot with the new value
    // 3) returns the value calculated in 1) for its caller to actually consume that fuel
    pub(crate) fn calculate_fuel_for_gas(&mut self) -> u64 {
        let gas_available = self.kernel.gas_available();
        let gas_used = gas_available - self.gas_available_snapshot;
        self.gas_available_snapshot = gas_available;
        self.kernel.price_list().gas_to_fuel(gas_used)
    }

    // This method:
    // 1) charges gas corresponding to the fuel_consumed delta based on the previous snapshot
    // 2) updates the fuel_consumed_snapshot
    pub(crate) fn charge_gas_for_fuel(&mut self, fuel_consumed: u64) -> Result<()> {
        self.kernel
            .charge_fuel(fuel_consumed - self.fuel_consumed_snapshot)?;
        self.fuel_consumed_snapshot = fuel_consumed;
        Ok(())
    }
}

use self::bind::BindSyscall;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

// Binds the syscall handlers so they can handle invocations
// from the actor code.
//
// TODO try to fix the static lifetime here. I want to tell the compiler that
//  the Kernel will live as long as the Machine and the Linker.
pub fn bind_syscalls(
    linker: &mut Linker<InvocationData<impl Kernel + 'static>>,
) -> anyhow::Result<()> {
    linker.bind("vm", "abort", vm::abort)?;

    linker.bind("ipld", "open", ipld::open)?;
    linker.bind("ipld", "create", ipld::create)?;
    linker.bind("ipld", "read", ipld::read)?;
    linker.bind("ipld", "stat", ipld::stat)?;
    linker.bind("ipld", "cid", ipld::cid)?;

    linker.bind("self", "root", sself::root)?;
    linker.bind("self", "set_root", sself::set_root)?;
    linker.bind("self", "current_balance", sself::current_balance)?;
    linker.bind("self", "self_destruct", sself::self_destruct)?;

    linker.bind("message", "caller", message::caller)?;
    linker.bind("message", "receiver", message::receiver)?;
    linker.bind("message", "method_number", message::method_number)?;
    linker.bind("message", "value_received", message::value_received)?;

    linker.bind("network", "base_fee", network::base_fee)?;
    linker.bind(
        "network",
        "total_fil_circ_supply",
        network::total_fil_circ_supply,
    )?;
    linker.bind("network", "version", network::version)?;
    linker.bind("network", "curr_epoch", network::curr_epoch)?;

    linker.bind("actor", "resolve_address", actor::resolve_address)?;
    linker.bind("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
    linker.bind("actor", "new_actor_address", actor::new_actor_address)?;
    linker.bind("actor", "create_actor", actor::create_actor)?;
    linker.bind(
        "actor",
        "resolve_builtin_actor_type",
        actor::resolve_builtin_actor_type,
    )?;
    linker.bind(
        "actor",
        "get_code_cid_for_type",
        actor::get_code_cid_for_type,
    )?;

    linker.bind("crypto", "verify_signature", crypto::verify_signature)?;
    linker.bind("crypto", "hash_blake2b", crypto::hash_blake2b)?;
    linker.bind("crypto", "verify_seal", crypto::verify_seal)?;
    linker.bind("crypto", "verify_post", crypto::verify_post)?;
    linker.bind(
        "crypto",
        "compute_unsealed_sector_cid",
        crypto::compute_unsealed_sector_cid,
    )?;
    linker.bind(
        "crypto",
        "verify_consensus_fault",
        crypto::verify_consensus_fault,
    )?;
    linker.bind(
        "crypto",
        "verify_aggregate_seals",
        crypto::verify_aggregate_seals,
    )?;
    linker.bind(
        "crypto",
        "verify_replica_update",
        crypto::verify_replica_update,
    )?;
    linker.bind("crypto", "batch_verify_seals", crypto::batch_verify_seals)?;

    linker.bind("rand", "get_chain_randomness", rand::get_chain_randomness)?;
    linker.bind("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

    linker.bind("gas", "charge", gas::charge_gas)?;

    // Ok, this singled-out syscall should probably be in another category.
    linker.bind("send", "send", send::send)?;

    linker.bind("debug", "log", debug::log)?;
    linker.bind("debug", "enabled", debug::enabled)?;

    Ok(())
}

// Computes the encoded size of a varint.
// TODO: move this to the varint crate.
pub(self) fn uvarint_size(num: u64) -> u32 {
    let bits = u64::BITS - num.leading_zeros();
    ((bits / 7) + (bits % 7 > 0) as u32).max(1) as u32
}

/// Returns the size cid would be, once encoded.
// TODO: move this to the cid/multihash crates.
pub(self) fn encoded_cid_size(k: &Cid) -> u32 {
    let mh = k.hash();
    let mh_size = uvarint_size(mh.code()) + uvarint_size(mh.size() as u64) + mh.size() as u32;
    match k.version() {
        cid::Version::V0 => mh_size,
        cid::Version::V1 => mh_size + uvarint_size(k.codec()) + 1,
    }
}
