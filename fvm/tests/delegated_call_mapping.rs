// Placeholder for delegated CALL mapping tests at the VM layer.
// These will exercise the DefaultCallManager intercept to ensure:
//  - delegated success returns OK and forwards returndata
//  - delegated revert maps to EVM_CONTRACT_REVERTED and propagates revert bytes

#[test]
#[ignore]
fn delegated_call_success_and_revert_mapping() {
    // Implementation will set EthAccount.delegate_to and invoke InvokeEVM → EthAccount,
    // then assert return mapping and persisted storage root.
}

