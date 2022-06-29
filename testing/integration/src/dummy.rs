use fvm::externs::{Consensus, Externs, Rand};

pub struct DummyExterns;

impl Externs for DummyExterns {}

impl Rand for DummyExterns {
    fn get_chain_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let msg = "mel was here".as_bytes();
        let mut out = [0u8; 32];
        out[..msg.len()].copy_from_slice(msg);
        Ok(out)
    }

    fn get_beacon_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        todo!()
    }
}

impl Consensus for DummyExterns {
    fn verify_consensus_fault(
        &self,
        _h1: &[u8],
        _h2: &[u8],
        _extra: &[u8],
    ) -> anyhow::Result<(Option<fvm_shared::consensus::ConsensusFault>, i64)> {
        todo!()
    }
}
