// Placeholder for depth limit test: when executing under authority context (depth=1),
// delegation chains are not re-followed.

#[test]
#[ignore]
fn delegated_call_depth_limit_enforced() {
    // Implementation will set A->B and B->C, then CALL->A and assert delegated execution
    // stops at B and does not re-follow to C.
}

