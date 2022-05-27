mod ops;
mod util;

// test target
use fvm::kernel::default::DefaultKernel;
use fvm::kernel::Block;
use fvm::Kernel;
use multihash::Code;
use util::*;

use super::*;

type TestingKernel = DefaultKernel<DummyCallManager>;

// TODO gas functions assert calls are being charged properly
// TODO maybe make more util functions
