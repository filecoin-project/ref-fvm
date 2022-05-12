use cid::Cid;
use fvm_shared::actor::builtin::Type;
use fvm_shared::version::NetworkVersion;

#[derive(thiserror::Error, Debug)]
/// Util errors for the intergration test framework.
pub(crate) enum Error {
    #[error("multiple root cid for network: {0}")]
    MultipleRootCid(NetworkVersion),
    #[error("no root cid for network: {0}")]
    NoRootCid(NetworkVersion),
    #[error("could not find manifest information for cid: {0}")]
    NoManifestInformation(Cid),
    #[error("could not load builtin manifest")]
    FailedToLoadManifest,
    #[error("manifest has no cid for builtin actor: {0}")]
    NoCidInManifest(Type),
    #[error("could not set state in tree for: {0}")]
    FailedToSetState(String),
    #[error("could not set actor: {0}")]
    FailedToSetActor(String),
    #[error("failed to flush tree")]
    FailedToFlushTree,
}
