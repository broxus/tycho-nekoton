use crate::models::Timings;
use anyhow::Result;
use everscale_types::models::{OwnedMessage, StdAddr, Transaction};
use everscale_types::prelude::HashBytes;
use nekoton_core::models::{ContractState, LatestBlockchainConfig};

pub mod models;
pub mod options;
pub mod rpc;
pub mod ton_lite;
mod traced_transaction;
mod utils;

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send_message(&self, message: &OwnedMessage) -> Result<()>;
    async fn send_message_reliable(&self, message: &OwnedMessage) -> Result<Transaction>;
    async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> Result<ContractState>;
    async fn get_config(&self) -> Result<LatestBlockchainConfig>;
    async fn get_transaction(&self, hash: &HashBytes) -> Result<Option<Transaction>>;
    async fn get_dst_transaction(&self, message_hash: &HashBytes) -> Result<Option<Transaction>>;
}

#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    async fn is_alive(&self) -> bool;

    fn endpoint(&self) -> &str;

    fn get_stats(&self) -> Option<Timings>;

    fn set_stats(&self, stats: Option<Timings>);

    fn force_update_is_alive(&self, is_alive: bool);

    async fn update_is_alive_internally(&self);
}
