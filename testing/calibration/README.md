# Gas Calibration

The binaries in this crate call `./contract/fil-gas-calibration-actor` with various parameters to exercise certain syscalls,
while collecting gas metrics, on which they runs regressions to estimate coefficients we could use to set gas prices.

The way this is different than the metrics we collect under `conformance` tests in that we also capture the inputs,
so that we can estimate prices based on different input size for example, if that is our hypotheses. The `conformance` tests are
more about backtesting the gas model using the available test vectors, whereas here we are driving the data collection.

The traces and the regression results are exported to `./measurements/out`, but this can be overridden using the `OUTPUT_DIR` env var.

For example:

```shell
cargo run --release --bin on_hashing
```

The calibration uses the machinery from the integration tests, but it's kept separate from them because to get good results we might want to run them for a long time, and on standardized environment. The reason different model targets are in separate binaries is so we can select which one we want to run.

Note that the `--release` flag has a huge impact on runtimes and therefore the model paramters, in the order of 100x.

Alternatively all the scenarios and exports can be executed the following way:

```shell
make run
```

After this the regression results can be found in `./measurements/out/regressions`. The suggested prices can be printed with the `make proposals` command, but always check the charts to see which one to adopt.

## Visualization

The exported observations can be visualized as scatter plots:

```shell
make visualize
```

The results are going to be in `./measurements/out/charts`.

Extraction and visualization can be run together:

```shell
make all
```

### Sanity checks

The charts in `./measurements/out/charts/charges` dispaly the relationship between input size and time on a scatter plot,
which forms the basis for the linear regressions. It's worth checking whether there are any outliers that could skew the models.

The charts also contain a line (as opposed to points) which show the actual gas charge at the various input sizes.
The values have been divided by the expected gas-per-nanosecond cost, so the slope of the line can be directly compared
against the slope of the scatter plot of time, giving us an idea of how good the current price model is.

It's also worth taking a look at the [overall.png](./measurements/out/charts/overall.png) chart, which contains the actual gas charge
of the different syscalls at the same measured times we spent doing them. If the prices are good, we should see each syscall have very
similar slopes - the more time we spend on them, the more gas they should cost, with nothing being relatively cheaper or more expensive
than the others.

One exemption of this rule are the syscalls which take deferred disk IO into account. Here the solution could be to estimate how much longer
it will take to write something to disk at the end, and just multiply the time by that number before exporting the results, creating a
virtual time for the sake of modelling prices.

## Notes

### Negative intercepts

I noticed in the case of `hashing` that while the slopes seem to be stable, the `intercept` field is often negative. This can happen just if the overall runtime differs by a few milliseconds, because the intercepts are so small, a few microseconds. We might want to run the experiments longer, or just treat such values as zero. In any case it's worth running the collection multiple times to see how stable the values are.
