use fvm_sdk::vm::abort;
use fvm_sdk::NO_DATA_BLOCK_ID;
use fvm_shared::error::ExitCode;

/// Placeholder invoke for testing
#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    match fvm_sdk::message::method_number() {
        // Set initial value
        1 => {
            // Should have no consequence on the test
            fvm_sdk::assert_test!(true);
            // Should exit with an Exit Code 24 and the custom message
            fvm_sdk::assert_test!(false);
        }
        _ => abort(
            ExitCode::USR_UNHANDLED_MESSAGE.value(),
            Some("unrecognized method"),
        ),
    }
    NO_DATA_BLOCK_ID
}
