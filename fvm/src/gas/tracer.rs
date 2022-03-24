use cid::Cid;
use fvm_shared::MethodNum;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
pub struct GasTracer {
    #[serde(skip_serializing)]
    started: Instant,
    #[serde(skip_serializing)]
    previous: Instant,
    /// Accumulated traces.
    traces: Vec<GasTrace>,
}

impl GasTracer {
    pub fn new() -> GasTracer {
        let now = Instant::now();
        GasTracer {
            started: now,
            previous: now,
            traces: Default::default(),
        }
    }

    pub fn record(&mut self, context: Context, point: Point, consumption: Consumption) {
        let trace = GasTrace {
            context,
            point,
            consumption,
            timing: {
                let now = Instant::now();
                let prev = self.previous;
                self.previous = now;
                Timing {
                    elapsed_cum: now.duration_since(self.started),
                    elapsed_rel: now.duration_since(prev),
                }
            },
        };
        self.traces.push(trace)
    }

    pub fn finish(self) -> Vec<GasTrace> {
        self.traces
    }
}

#[derive(Debug, Serialize)]
pub struct GasTrace {
    /// Context annotates the trace with the source context.   
    pub context: Context,
    /// Point represents the location from where the trace was emitted.
    pub point: Point,
    /// The consumption at the moment of trace.
    pub consumption: Consumption,
    /// Timing information.
    pub timing: Timing,
}

#[derive(Debug, Serialize, Default)]
pub struct Consumption {
    /// Wasmtime fuel consumed reports how much fuel has been consumed at this point.
    /// May be optional if the point had no access to this information, or if non applicable.
    pub fuel_consumed: Option<u64>,
    /// Gas consumed reports how much gas has been consumed at this point.
    /// May be optional if the point had no access to this information, or if non applicable.
    pub gas_consumed: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Timing {
    /// Total time elapsed since the GasTracer was created.
    #[serde(
        serialize_with = "ser::serialize_duration_as_nanos",
        rename = "elapsed_cum_ns"
    )]
    pub elapsed_cum: Duration,
    /// Relative time elapsed since the previous trace was recorded.
    #[serde(
        serialize_with = "ser::serialize_duration_as_nanos",
        rename = "elapsed_rel_ns"
    )]
    pub elapsed_rel: Duration,
}

#[derive(Debug, Serialize, Default)]
pub struct Context {
    #[serde(serialize_with = "ser::serialize_cid")]
    pub code_cid: Cid,
    pub method_num: MethodNum,
}

#[derive(Debug, Serialize)]
pub struct Point {
    pub event: Event,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub enum Event {
    Started,
    EnterCall,
    PreSyscall,
    PostSyscall,
    PreExtern,
    PostExtern,
    ExitCall,
    Finished,
}

#[test]
fn test_tracer() {
    let mut tracer = GasTracer::new();
    tracer.record(
        Context {
            code_cid: Default::default(),
            method_num: 0,
        },
        Point {
            event: Event::Started,
            label: "".to_string(),
        },
        Consumption {
            fuel_consumed: None,
            gas_consumed: None,
        },
    );

    std::thread::sleep(Duration::from_millis(1000));
    tracer.record(
        Context {
            code_cid: Default::default(),
            method_num: 0,
        },
        Point {
            event: Event::Started,
            label: "".to_string(),
        },
        Consumption {
            fuel_consumed: None,
            gas_consumed: None,
        },
    );
    let traces = tracer.finish();
    println!("{:?}", traces);

    let str = serde_json::to_string(&traces).unwrap();
    println!("{}", str);
}

mod ser {
    use cid::Cid;
    use serde::{Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize_cid<S>(c: &Cid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        c.to_string().serialize(serializer)
    }

    pub fn serialize_duration_as_nanos<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        d.as_nanos().serialize(serializer)
    }
}
