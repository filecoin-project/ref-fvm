use fvm_integration_tests;

#[test]
pub fn test_metadata() {
    let setup = fvm_integration_tests::setup().unwrap();
    let mut exec = fvm_integration_tests::create_executor(
        setup.state_root,
        setup.builtin_actors,
        setup.blockstore,
    );
}
