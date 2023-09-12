// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fvm::executor::ApplyRet;
use fvm_integration_tests::{tester, testkit};
use fvm_ipld_encoding::BytesDe;
use fvm_shared::address::Address;

// Eth ABI (solidity) panic codes.
const PANIC_ERROR_CODES: [(u64, &str); 10] = [
    (0x00, "Panic()"),
    (0x01, "Assert()"),
    (0x11, "ArithmeticOverflow()"),
    (0x12, "DivideByZero()"),
    (0x21, "InvalidEnumVariant()"),
    (0x22, "InvalidStorageArray()"),
    (0x31, "PopEmptyArray()"),
    (0x32, "ArrayIndexOutOfBounds()"),
    (0x41, "OutOfMemory()"),
    (0x51, "CalledUninitializedFunction()"),
];

// Function Selectors
const ERROR_FUNCTION_SELECTOR: &[u8] = b"\x08\xc3\x79\xa0"; // Error(string)
const PANIC_FUNCTION_SELECTOR: &[u8] = b"\x4e\x48\x7b\x71"; // Panic(uint256)

fn handle_result(tester: &tester::BasicTester, name: &str, res: &ApplyRet) -> anyhow::Result<()> {
    let (trace, events) = tester
        .options
        .as_ref()
        .map(|o| (o.trace, o.events))
        .unwrap_or_default();

    if trace && !res.exec_trace.is_empty() {
        println!();
        println!("**");
        println!("* BEGIN {name} execution trace");
        println!("**");
        println!();
        for tr in &res.exec_trace {
            println!("{:?}", tr)
        }
        println!();
        println!("**");
        println!("* END {name} execution trace");
        println!("**");
        println!();
    }
    if events && !res.events.is_empty() {
        println!();
        println!("**");
        println!("* BEGIN {name} events");
        println!("**");
        println!();
        for evt in &res.events {
            println!("{:?}", evt)
        }
        println!();
        println!("**");
        println!("* END {name} events");
        println!("**");
        println!();
    }

    if let Some(bt) = &res.failure_info {
        println!("{bt}");
    }

    if res.msg_receipt.exit_code.is_success() {
        Ok(())
    } else {
        if res.msg_receipt.exit_code == 33.into() {
            let BytesDe(returnval) = res.msg_receipt.return_data.deserialize().unwrap();
            println!("Revert Reason: {}", parse_eth_revert(&returnval).unwrap());
        }
        Err(anyhow!("{name} failed"))
    }
}

pub fn run(
    tester: &mut tester::BasicTester,
    contract: &[u8],
    entrypoint: &[u8],
    params: &[u8],
    gas: u64,
) -> anyhow::Result<()> {
    let mut account = tester.create_basic_account()?;

    let create_res = testkit::fevm::create_contract(tester, &mut account, contract)?;
    handle_result(tester, "contract creation", &create_res)?;

    let create_return: testkit::fevm::CreateReturn =
        create_res.msg_receipt.return_data.deserialize().unwrap();
    let actor = Address::new_id(create_return.actor_id);

    // invoke contract
    let mut input_data = Vec::from(entrypoint);
    let mut input_params = Vec::from(params);
    input_data.append(&mut input_params);

    let invoke_res = testkit::fevm::invoke_contract(tester, &mut account, actor, &input_data, gas)?;
    let BytesDe(returnval) = invoke_res.msg_receipt.return_data.deserialize().unwrap();
    println!("Exit Code: {}", invoke_res.msg_receipt.exit_code);
    println!("Result: {}", hex::encode(returnval));
    println!("Gas Used: {}", invoke_res.msg_receipt.gas_used);

    handle_result(tester, "contract invocation", &invoke_res)
}

// Parses the error message from a revert reason of type Error(string) or Panic(uint256)
// See https://docs.soliditylang.org/en/latest/control-structures.html#panic-via-assert-and-error-via-require
pub fn parse_eth_revert(returnval: &Vec<u8>) -> anyhow::Result<String> {
    if returnval.is_empty() {
        return Err(anyhow!("invalid return value"));
    }
    if returnval.len() < 4 + 32 {
        return Ok(hex::encode(returnval));
    }
    match &returnval[0..4] {
        PANIC_FUNCTION_SELECTOR => {
            let cbytes = &returnval[4..];
            match bytes_to_u64(&cbytes[..32]) {
                Ok(panic_code) => {
                    let error = panic_error_codes(panic_code);
                    match error {
                        Some(s) => return Ok(format!("Panic Code: {}, msg: {}", s.0, s.1)),
                        None => return Err(anyhow!("Returned with panic code({})", panic_code)),
                    }
                }
                Err(_) => {
                    return Ok(hex::encode(returnval));
                }
            }
        }
        ERROR_FUNCTION_SELECTOR => {
            let cbytes = &returnval[4..];
            let cbytes_len = cbytes.len() as u64;
            if let Ok(offset) = bytes_to_u64(&cbytes[0..32]) {
                if cbytes_len >= offset + 32 {
                    if let Ok(length) = bytes_to_u64(&cbytes[offset as usize..offset as usize + 32])
                    {
                        if cbytes_len >= offset + 32 + length {
                            let msg = String::from_utf8_lossy(
                                &cbytes
                                    [offset as usize + 32..offset as usize + 32 + length as usize],
                            );
                            return Ok(msg.to_string());
                        }
                    }
                }
            }
        }
        _ => return Ok(hex::encode(returnval)),
    };
    Ok(hex::encode(returnval))
}

// Converts a byte slice to a u64
fn bytes_to_u64(bytes: &[u8]) -> Result<u64, anyhow::Error> {
    if bytes.len() != 32 {
        return Err(anyhow::anyhow!("Invalid byte slice length"));
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[24..32]);
    Ok(u64::from_be_bytes(buf))
}

// Returns the panic code and message for a given panic code
fn panic_error_codes(code: u64) -> Option<&'static (u64, &'static str)> {
    PANIC_ERROR_CODES.iter().find(|(c, _)| *c == code)
}

//////////////////////
/////// Tests ///////
/////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eth_revert_empty_returnval() {
        let returnval = vec![];
        let result = parse_eth_revert(&returnval);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "invalid return value");
    }

    #[test]
    fn test_parse_eth_revert_short_returnval() {
        let returnval = vec![0x01, 0x02, 0x03];
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "010203");
    }

    #[test]
    fn test_parse_eth_revert_panic_function_selector() {
        let returnval = vec![
            0x4e, 0x48, 0x7b, 0x71, // function selector for "Panic(uint256)"
            0x00, 0x00, 0x00, 0x00,
        ];
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4e487b7100000000");
    }

    #[test]
    fn test_parse_eth_revert_panic_function_selector_with_message() {
        // assert error from simplecoin contract
        let returnval =
            hex::decode("4e487b710000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Panic Code: 1, msg: Assert()");
    }

    #[test]
    fn test_parse_eth_revert_error_function_selector() {
        // "Less Than ten" error from simplecoin contract
        let returnval = hex::decode("08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d4c657373205468616e2074656e00000000000000000000000000000000000000").unwrap();
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Less Than ten");
    }

    #[test]
    fn test_parse_eth_revert_error_function_selector_invalid_data() {
        // invalid data for error function selector
        let returnval = hex::decode("08c379a000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000").unwrap();
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), hex::encode(&returnval));
    }

    #[test]
    fn test_parse_eth_revert_custom_error() {
        // any other data like custom error, etc. "lessThanFive" custom error of simplecoin contract in this case.
        let returnval = hex::decode("4426661100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000e4c657373207468616e2066697665000000000000000000000000000000000000").unwrap();
        let result = parse_eth_revert(&returnval);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), hex::encode(&returnval));
    }
}
