use anyhow::Result;
use everscale_types::models::*;
use everscale_types::prelude::*;

pub use crate::models::{ContractState, LatestBlockchainConfig};

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn broadcast_message(&self, message: &OwnedMessage) -> Result<()>;

    async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> Result<ContractState>;

    async fn get_config(&self) -> Result<LatestBlockchainConfig>;

    async fn get_transaction(&self, hash: &HashBytes) -> Result<Option<Transaction>>;
}
