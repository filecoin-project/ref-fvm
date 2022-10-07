use thiserror::Error;

#[derive(Copy, Clone, Debug, Error)]
#[error("actor does not exist in state-tree")]
pub struct NoStateError;

#[derive(Copy, Clone, Debug, Error)]
pub enum ActorDeleteError {
    #[error("deletion beneficiary is the current actor")]
    BeneficiaryIsSelf,
    #[error("deletion beneficiary does not exist")]
    BeneficiaryDoesNotExist,
}

#[derive(Copy, Clone, Debug, Error)]
pub enum EpochBoundsError {
    #[error("the requested epoch isn't valid")]
    Invalid,
    #[error("the requested epoch exceeds the maximum lookback")]
    ExceedsLookback,
}
