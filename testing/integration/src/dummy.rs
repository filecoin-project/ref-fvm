use fvm::externs::{Consensus, Externs, Rand};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
pub struct DummyExterns;

impl Externs for DummyExterns {}

impl Rand for DummyExterns {
    fn get_chain_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rng: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        Ok(<[u8; 32]>::try_from(rng.into_bytes()).unwrap())
    }

    fn get_beacon_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rng: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        Ok(<[u8; 32]>::try_from(rng.into_bytes()).unwrap())
    }
}

impl Consensus for DummyExterns {
    fn verify_consensus_fault(
        &self,
        _h1: &[u8],
        _h2: &[u8],
        _extra: &[u8],
    ) -> anyhow::Result<(Option<fvm_shared::consensus::ConsensusFault>, i64)> {
        Ok((None, 0))
    }
}
