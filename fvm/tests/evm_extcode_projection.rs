mod common;

use common::{new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::ActorID;
use multihash_codetable::MultihashDigest;

fn extcodecopy_program(target20: [u8; 20], offset: u8, size: u8) -> Vec<u8> {
    // Stack order for EXTCODECOPY is: [size, offset, dest, address] (top to bottom), so we push
    // address first, then dest, then offset, then size.
    let mut code = Vec::with_capacity(1 + 20 + 2 + 2 + 2 + 1 + 2 + 2 + 1);
    code.push(0x73); // PUSH20 <target>
    code.extend_from_slice(&target20);
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 dest=0
    code.extend_from_slice(&[0x60, offset]); // PUSH1 code offset
    code.extend_from_slice(&[0x60, size]); // PUSH1 size
    code.push(0x3C); // EXTCODECOPY
    code.extend_from_slice(&[0x60, size]); // PUSH1 size
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    code.push(0xF3); // RETURN
    code
}

fn wrap_init_with_runtime(runtime: &[u8]) -> Vec<u8> {
    let len = runtime.len();
    assert!(len <= 0xFF);
    let offset: u8 = 12;
    let mut init = Vec::with_capacity(12 + len);
    init.extend_from_slice(&[0x60, len as u8]);
    init.extend_from_slice(&[0x60, offset]);
    init.extend_from_slice(&[0x60, 0x00]);
    init.push(0x39); // CODECOPY
    init.extend_from_slice(&[0x60, len as u8]);
    init.extend_from_slice(&[0x60, 0x00]);
    init.push(0xF3); // RETURN
    init.extend_from_slice(runtime);
    init
}


#[test]
fn evm_extcode_projection_size_hash_copy() {
    // Build harness with events enabled to mirror runtime conditions.
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");

    // Create an account to deploy contracts.
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Choose a constant 20-byte delegate address; EXTCODE* pointer projection only depends on
    // the mapping, not on the delegate actor's existence.
    let delegate_eth: [u8; 20] = [
        0xDE, 0xAD, 0xBE, 0xEF, 0x00,
        0x11, 0x22, 0x33, 0x44, 0x55,
        0x66, 0x77, 0x88, 0x99, 0xAA,
        0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
    ];

    // Create an authority EthAccount with delegate_to set to the delegate contract.
    // Pick a stable f4 address for the authority (use EAM namespace id=10 + 20 bytes address).
    let authority_f4 = Address::new_delegated(10, &[
        0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0x11, 0x22, 0x33, 0x44, 0x55,
        0x66, 0x77, 0x88, 0x99, 0x00,
    ]).expect("f4 address");
    let _authority_id: ActorID = set_ethaccount_with_delegate(&mut h, authority_f4, delegate_eth)
        .expect("install ethaccount");

    // Instantiate the machine to freeze state tree and create an executor.
    h.tester.instantiate_machine(fvm_integration_tests::dummy::DummyExterns).unwrap();

    // Deploy a caller program that EXTCODECOPYs from the authority address and returns 23 bytes.
    let caller_prog = extcodecopy_program(
        // The EVM uses the 20-byte EthAddress for targets; this must match the f4 payload.
        [
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99, 0x00,
        ],
        0,
        23,
    );
    let caller_init = wrap_init_with_runtime(&caller_prog);
    let caller_res = fevm::create_contract(&mut h.tester, &mut owner, &caller_init).unwrap();
    assert!(caller_res.msg_receipt.exit_code.is_success(), "caller deploy failed: {:?}", caller_res);
    let caller_ret = caller_res.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap();
    let caller_addr = caller_ret.robust_address.expect("robust");

    // Invoke the caller (no calldata); it should return the 23-byte pointer image.
    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_addr, &[], fevm::DEFAULT_GAS).unwrap();
    assert!(inv.msg_receipt.exit_code.is_success(), "invoke failed: {:?}", inv);
    let out = inv.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(out.len(), 23, "expected 23-byte pointer code");

    // Expected pointer code: 0xEF 0x01 0x00 || delegate(20)
    let mut expected = Vec::with_capacity(23);
    expected.extend_from_slice(&[0xEF, 0x01, 0x00]);
    expected.extend_from_slice(&delegate_eth);
    assert_eq!(out, expected, "pointer code mismatch");

    // Confirm EXTCODEHASH equals keccak(pointer_code)
    // Compute keccak256 using multihash and compare to EVM's EXTCODEHASH via a tiny program.
    let mh = multihash_codetable::Code::Keccak256.digest(&expected);
    let expected_hash = mh.digest().to_vec();

    // Program: EXTCODEHASH(target) then return 32 bytes from memory.
    let mut prog = Vec::new();
    prog.push(0x73); // PUSH20 <target>
    prog.extend_from_slice(&[
        0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0x11, 0x22, 0x33, 0x44, 0x55,
        0x66, 0x77, 0x88, 0x99, 0x00,
    ]);
    prog.push(0x3F); // EXTCODEHASH
    prog.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    prog.push(0x52); // MSTORE (store hash at offset 0)
    prog.extend_from_slice(&[0x60, 0x20, 0x60, 0x00, 0xF3]); // return(0, 32)

    let prog = wrap_init_with_runtime(&prog);
    let hprog_res = fevm::create_contract(&mut h.tester, &mut owner, &prog).unwrap();
    assert!(hprog_res.msg_receipt.exit_code.is_success());
    let hprog_ret = hprog_res.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap();
    let hprog_ret = hprog_res.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap();
    let hprog_addr = hprog_ret.robust_address.expect("robust");
    let inv2 = fevm::invoke_contract(&mut h.tester, &mut owner, hprog_addr, &[], fevm::DEFAULT_GAS).unwrap();
    assert!(inv2.msg_receipt.exit_code.is_success());
    let hash_out = inv2.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(hash_out.len(), 32);
    assert_eq!(hash_out, expected_hash, "extcodehash mismatch");
    // Windowing cases
    // 1) offset=1, size=22 → expected[1..]
    let caller_prog_w1 = wrap_init_with_runtime(&extcodecopy_program([
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99, 0x00,
        ], 1, 22));
    let res_w1 = fevm::create_contract(&mut h.tester, &mut owner, &caller_prog_w1).unwrap();
    assert!(res_w1.msg_receipt.exit_code.is_success());
    let addr_w1 = res_w1.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap().robust_address.unwrap();
    let inv_w1 = fevm::invoke_contract(&mut h.tester, &mut owner, addr_w1, &[], fevm::DEFAULT_GAS).unwrap();
    let out_w1 = inv_w1.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(out_w1, expected[1..].to_vec());

    // 2) offset=23, size=1 → zero
    let caller_prog_w2 = wrap_init_with_runtime(&extcodecopy_program([
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99, 0x00,
        ], 23, 1));
    let res_w2 = fevm::create_contract(&mut h.tester, &mut owner, &caller_prog_w2).unwrap();
    assert!(res_w2.msg_receipt.exit_code.is_success());
    let addr_w2 = res_w2.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap().robust_address.unwrap();
    let inv_w2 = fevm::invoke_contract(&mut h.tester, &mut owner, addr_w2, &[], fevm::DEFAULT_GAS).unwrap();
    let out_w2 = inv_w2.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(out_w2, vec![0x00]);

    // 3) offset=100, size=10 → zeros
    let caller_prog_w3 = wrap_init_with_runtime(&extcodecopy_program([
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99, 0x00,
        ], 100, 10));
    let res_w3 = fevm::create_contract(&mut h.tester, &mut owner, &caller_prog_w3).unwrap();
    assert!(res_w3.msg_receipt.exit_code.is_success());
    let addr_w3 = res_w3.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap().robust_address.unwrap();
    let inv_w3 = fevm::invoke_contract(&mut h.tester, &mut owner, addr_w3, &[], fevm::DEFAULT_GAS).unwrap();
    let out_w3 = inv_w3.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(out_w3, vec![0u8; 10]);

}
