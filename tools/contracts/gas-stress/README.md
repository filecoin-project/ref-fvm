# Contracts for stressing the system

These contracts do not terminate; instead they intend to run "for-ever" -- until they run
out of gus.
The contracts all runnable in fvm-bench with no arguments.
They must exit with an Out Of Gas error; any other exit is inadmissible.

Example:
```
$ ../../../target/release/fvm-bench -b ../../../../builtin-actors/output/builtin-actors.car counter.bin "" ""
ERROR: contract execution failed

Caused by:
    contract invocation failed: 7 -- Some(MessageBacktrace(Backtrace { frames: [Frame { source: 101, method: 3844450837, code: ExitCode { value: 7 }, message: "out of gas" }], cause: None }))
```
