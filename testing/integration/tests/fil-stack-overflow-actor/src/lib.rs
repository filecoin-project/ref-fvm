use fvm_sdk as sdk;
use fvm_shared::address::Address;

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    let m = sdk::message::method_number();
    if m > 1024 {
        return 0;
    }

    // 5 stack elems per level (wasm-instrument charges for highest use in the
    // function) + some overhead mean that with the 2048 element wasm limit we
    // can do 396 recursive calls while still being able do do a send at that
    // depth
    recurse(m, 396)
}

// we need two recurse functions; just one gets optimized into wasm loop

#[inline(never)]
pub fn recurse(m: u64, n: u64) -> u32 {
    if n > 0 {
        call_extern();

        return recurse2(m, n - 1);
    }
    do_send(m)
}

#[inline(never)]
pub fn recurse2(m: u64, n: u64) -> u32 {
    if n > 0 {
        call_extern();

        return recurse(m, n - 1);
    }
    do_send(m)
}

// external call to prevent the compiler from doing smart things
#[inline(never)]
pub fn call_extern() {
    let _ = sdk::message::method_number();
}

#[inline(never)]
pub fn do_send(m: u64) -> u32 {
    let r = sdk::send::send(
        &Address::new_id(10000),
        m + 1,
        Vec::new().into(),
        0.into(),
    );
    r.unwrap().exit_code.value()
}
