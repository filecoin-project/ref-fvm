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
