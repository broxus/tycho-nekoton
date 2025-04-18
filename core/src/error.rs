use everscale_types::abi::AbiType;
use everscale_types::models::{ComputePhaseSkipReason, IntAddr};

#[derive(thiserror::Error, Debug)]
pub enum ExecutionError {
    #[error("Invalid contract structure")]
    InvalidContractStructure,
    #[error("Account {0} is not active")]
    AccountNotActive(IntAddr),
    #[error("Account {0} has no code")]
    AccountHasNoCode(IntAddr),
    #[error("Account does not exist")]
    AccountDoesNotExist,

    #[error("Unexpected Abi Value. Expected: {expected}, actual: {actual}")]
    UnexpectedAbiType { expected: AbiType, actual: String },
    #[error("Invalid address type")]
    InvalidAddressType,

    #[error("Compute hase skipped. Reason: {0:?}")]
    ComputePhaseSkipped(ComputePhaseSkipReason),
    #[error("Transaction error {0:}")]
    TransactionError(#[from] tycho_executor::TxError),

    #[error("Cell error: {0}")]
    CellError(#[from] everscale_types::error::Error),
    #[error("Error: {0}")]
    Other(#[from] anyhow::Error),
}
