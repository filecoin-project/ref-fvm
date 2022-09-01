use cid::Cid;

#[derive(thiserror::Error, Debug)]
/// Util errors for the intergration test framework.
pub(crate) enum Error {
    #[error("could not find manifest information for cid: {0}")]
    NoManifestInformation(Cid),
    #[error("could not load builtin manifest")]
    FailedToLoadManifest,
    #[error("could not set state in tree for: {0}")]
    FailedToSetState(String),
    #[error("could not set actor: {0}")]
    FailedToSetActor(String),
    #[error("failed to flush tree")]
    FailedToFlushTree,
}
