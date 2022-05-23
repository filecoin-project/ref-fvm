mod dummy;
mod ops;
mod util;

use std::mem::ManuallyDrop;
use std::sync::Weak;

use dummy::*;
// test target
use fvm::kernel::default::DefaultKernel;
use fvm::kernel::{Block, BlockRegistry};
use fvm::machine::Machine;
use fvm::Kernel;
use multihash::Code;
use util::*;

type TestingKernel = DefaultKernel<dummy::DummyCallManager>;
type ExternalCallManager = ManuallyDrop<Weak<dummy::InnerDummyCallManager>>;

// TODO gas functions assert calls are being charged properly
// TODO maybe make util functions