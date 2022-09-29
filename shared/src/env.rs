use cid::Cid;

#[derive(PartialEq, Clone, Debug, Hash, Eq, Default)]
pub struct ChainContext {
    pub timestamp: u64,
    pub tipsets: Vec<Cid>,
}
