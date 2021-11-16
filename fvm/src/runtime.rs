use blockstore::Blockstore;
use cid::Cid;

#[derive(Copy, Clone)]
pub struct Config {
    pub max_pages: usize,
}

pub trait Runtime {
    type Blockstore: Blockstore;
    fn store(&mut self) -> &mut Self::Blockstore;
    fn root(&self) -> &Cid;
    fn set_root(&mut self, root: Cid);
    fn config(&self) -> &Config;
}

pub struct DefaultRuntime<B> {
    bs: B,
    root: Cid,
    config: Config,
}

impl<B> Runtime for DefaultRuntime<B>
where
    B: Blockstore,
{
    type Blockstore = B;

    fn store(&mut self) -> &mut Self::Blockstore {
        &mut self.bs
    }

    fn root(&self) -> &Cid {
        &self.root
    }

    fn set_root(&mut self, new: Cid) {
        self.root = new;
        // TODO: actually do something.
    }

    fn config(&self) -> &Config {
        &self.config
    }
}
