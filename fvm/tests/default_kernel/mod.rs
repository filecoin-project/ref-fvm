mod dummy;
mod ops;
mod util;

use std::cell::RefCell;

use dummy::*;
// test target
use fvm::kernel::default::DefaultKernel;
use fvm::kernel::{Block, Kernel};
use fvm::machine::Machine;
use multihash::Code;
use util::*;

type TestingKernel = DefaultKernel<dummy::DummyCallManager>;

// TODO gas functions assert calls are being charged properly
// TODO maybe make more util functions
