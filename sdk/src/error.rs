use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;

/// SDK functions performing a syscall return a SyscallResult type, where the
/// Error type is an ExitCode. ExitCode::Ok is translated to an Ok result, while
/// error codes are propagated as Err(ExitCode).
///
/// Error messages don't make it across the boundary, but are logged at the FVM
/// level for debugging and informational purposes.
pub type SyscallResult<T> = core::result::Result<T, ExitCode>;

pub fn to_syscall_result(code: u32) -> SyscallResult<()> {
    let exit_code: ExitCode =
        FromPrimitive::from_u32(code).expect("syscall returned unrecognized exit code");
    match exit_code {
        ExitCode::Ok => Ok(()),
        e => Err(e),
    }
}

// TODO The below was a dumb but quick solution, which was discarded.
//
//  Ideally we'd use Use traits and macros to provide a nicer experience:
//   sys::actor::resolve_address.exec(address) // converts the status code to a SyscallError
//      .map(|(found, actor_id)| {
//         ....
//      })
//
//
// pub fn handle_err1<R1>(ret: (u32, R1)) -> SyscallResult<R1> {
//     match ret.0 {
//         0 => Ok(ret.1),
//         e => Err(FromPrimitive::from_u32(e).expect("syscall returned unrecognized exit code")),
//     }
// }
//
// pub fn handle_err2<R1, R2>(ret: (u32, R1, R2)) -> SyscallResult<(R1, R2)> {
//     match ret.0 {
//         0 => Ok((ret.1, ret.2)),
//         e => Err(FromPrimitive::from_u32(e).expect("syscall returned unrecognized exit code")),
//     }
// }
//
// pub fn handle_err3<R1, R2, R3>(ret: (u32, R1, R2, R3)) -> SyscallResult<(R1, R2, R3)> {
//     match ret.0 {
//         0 => Ok((ret.1, ret.2, ret.3)),
//         e => Err(FromPrimitive::from_u32(e).expect("syscall returned unrecognized exit code")),
//     }
// }
//
// pub fn handle_err4<R1, R2, R3, R4>(ret: (u32, R1, R2, R3, R4)) -> SyscallResult<(R1, R2, R3, R4)> {
//     match ret.0 {
//         0 => Ok((ret.1, ret.2, ret.3, ret.4)),
//         e => Err(FromPrimitive::from_u32(e).expect("syscall returned unrecognized exit code")),
//     }
// }
