use anyhow::Result;
use everscale_types::models::*;
use everscale_types::prelude::*;

mod models;

pub use self::models::{ContractState, LatestBlockchainConfig};

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn broadcast_message(&self, message: &DynCell) -> Result<()>;

    async fn get_contract_state(&self, address: &StdAddr) -> Result<ContractState>;

    async fn get_config(&self) -> Result<LatestBlockchainConfig>;
}
