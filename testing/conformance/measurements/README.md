# Gas Calibration Measurements

This directory contains some support scripts for visualizing the traces we can opt to collect
during test vector execution, which can help calibrating gas prices.

## Trace Collection

To collect traces in the first place, execute conformance tests using an extra `TRACES` env var
to specify where the files should go:

```bash
cd testing/conformance/
mkdir traces
rm -rf traces/*
TRACE_DIR=traces \
  PRICE_NETWORK_VERSION=16 \
  TEST_VECTOR_POSTCONDITION_MISSING_ACTION=ignore \
  VECTOR=test-vectors/corpus \
  cargo test --release conformance -- --nocapture
```

Note the --release flag; without this the contract execution would be much slower and make the timings less relevant.

After this the TRACES_DIR directory will contain:

* A `traces.jsonline` file containing one line per successful test message, with each line pointing at detailed traces just for that particular message, and containing the overall gas charge and execution time.
* A mirror of the structure of the `VECTOR` directory, with each message in the successful test vector having a separate file containing the `GasCharge` records collected during the execution.


## Visualization

You can use the `Makefile` to produce charts in the `./out` directory. It assumes the traces are in `../traces`.

```shell
cd testing/conformance/measurements
make all
```
