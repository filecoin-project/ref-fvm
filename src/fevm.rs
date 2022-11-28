use cid::Cid;
use anyhow::anyhow;

pub fn run(bundle: Cid, contract: &[u8], entrypoint: &[u8], params: &[u8]) -> anyhow::Result<()> {
    Err(anyhow!("implement me!!!"))
}
