# with perf (doesnt work great because not granular enough- gets a ton of noise from overhead)
# needs some 
## build 
`cargo build --release --bin perf-conformance`

## check that perf-conformance works (DOES IT???)
`CARGO_PROFILE_BENCH_DEBUG=true VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v6/TestMeasurePreCommitGas/ff3438ebc9c42d99d23a8654c4a5d5c8408f575950c05e504be9de58aa521167-t0100-t0101-storageminer-25.json ./target/release/perf-conformance `

## perf it 
`CARGO_PROFILE_BENCH_DEBUG=true VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v6/TestMeasurePreCommitGas/ff3438ebc9c42d99d23a8654c4a5d5c8408f575950c05e504be9de58aa521167-t0100-t0101-storageminer-25.json perf record -k mono ./target/release/perf-conformance`

## add the jit data (this dumps random files everywhereeee)
`perf inject --jit --input perf.data --output perf.jit.data`

## stare into abyss?
`perf report --input perf.jit.data --hierarchy`