use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use everscale_types::cell::HashBytes;
use everscale_types::models::{OwnedMessage, StdAddr, Transaction};
use nekoton_core::models::{ContractState, LatestBlockchainConfig};
use parking_lot::Mutex;
use reqwest::Url;

use crate::models::Timings;
use crate::rpc::jrpc_client;
use crate::Connection;

#[derive(Clone)]
pub struct RpcConnection {
    is_available: Arc<AtomicBool>,
    rpc_type: RpcType,
    stats: Arc<Mutex<Option<Timings>>>,
}

#[derive(Clone)]
pub enum RpcType {
    Jrpc(jrpc_client::JrpcClient),
    Proto, //TODO: implement proto
}

impl RpcConnection {
    pub(crate) fn new(endpoint: Url, client: reqwest::Client) -> Self {
        let is_jrpc = endpoint.path().ends_with("/rpc");
        if is_jrpc {
            Self {
                is_available: Arc::new(Default::default()),
                rpc_type: RpcType::Jrpc(jrpc_client::JrpcClient::new(endpoint, client)),
                stats: Arc::new(Default::default()),
            }
        } else {
            Self {
                is_available: Arc::new(Default::default()),
                rpc_type: RpcType::Proto,
                stats: Arc::new(Default::default()),
            }
        }
    }
    pub(crate) async fn send_message(&self, message: &OwnedMessage) -> Result<()> {
        match &self.rpc_type {
            RpcType::Jrpc(client) => client.send_message(message).await,
            RpcType::Proto => todo!(),
        }
    }

    pub(crate) async fn get_dst_transaction(
        &self,
        hash_bytes: HashBytes,
    ) -> Result<Option<Transaction>> {
        match &self.rpc_type {
            RpcType::Jrpc(client) => client.get_dst_transaction(hash_bytes).await,
            RpcType::Proto => todo!(),
        }
    }

    pub(crate) async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> anyhow::Result<ContractState> {
        match &self.rpc_type {
            RpcType::Jrpc(client) => {
                client
                    .get_contract_state(address, last_transaction_lt)
                    .await
            }
            RpcType::Proto => todo!(),
        }
    }

    pub(crate) async fn get_config(&self) -> Result<LatestBlockchainConfig> {
        match &self.rpc_type {
            RpcType::Jrpc(client) => client.get_config().await,
            RpcType::Proto => todo!(),
        }
    }

    pub(crate) async fn get_transaction(
        &self,
        hash_bytes: &HashBytes,
    ) -> Result<Option<Transaction>> {
        match &self.rpc_type {
            RpcType::Jrpc(jrpc_client) => jrpc_client.get_transaction(hash_bytes).await,
            RpcType::Proto => todo!(),
        }
    }

    fn get_stats(&self) -> Option<Timings> {
        self.stats.lock().clone()
    }

    fn set_stats(&self, stats: Option<Timings>) {
        *self.stats.lock() = stats;
    }

    fn update_is_available(&self, is_available: bool) {
        self.is_available.store(is_available, Ordering::Release);
    }
}

pub enum LiveCheckResult {
    /// GetTimings request was successful
    Live(Timings),
    Dead,
}

impl LiveCheckResult {
    fn as_bool(&self) -> bool {
        match self {
            LiveCheckResult::Live(metrics) => metrics.is_reliable(),
            LiveCheckResult::Dead => false,
        }
    }
}

impl Eq for RpcConnection {}

impl PartialEq<Self> for RpcConnection {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint() == other.endpoint()
    }
}

impl PartialOrd<Self> for RpcConnection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RpcConnection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.eq(other) {
            std::cmp::Ordering::Equal
        } else {
            let left_stats = self.get_stats();
            let right_stats = other.get_stats();

            match (left_stats, right_stats) {
                (Some(left_stats), Some(right_stats)) => left_stats.cmp(&right_stats),
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }
    }
}

#[async_trait::async_trait]
impl Connection for RpcConnection {
    async fn is_alive(&self) -> bool {
        self.is_available.load(Ordering::Acquire)
    }

    fn endpoint(&self) -> &str {
        match &self.rpc_type {
            RpcType::Jrpc(client) => client.endpoint(),
            RpcType::Proto => todo!(),
        }
    }

    fn get_stats(&self) -> Option<Timings> {
        self.stats.lock().clone()
    }

    fn set_stats(&self, new_stats: Option<Timings>) {
        let mut stats = self.stats.lock();
        *stats = new_stats;
    }

    fn force_update_is_alive(&self, is_alive: bool) {
        self.is_available.store(is_alive, Ordering::Release);
    }

    async fn update_is_alive_internally(&self) {
        match &self.rpc_type {
            RpcType::Jrpc(client) => match client.get_timings().await {
                Ok(timings) => {
                    self.force_update_is_alive(true);
                    self.set_stats(Some(timings));
                }
                Err(_) => {
                    self.force_update_is_alive(false);
                }
            },
            RpcType::Proto => todo!(),
        }
    }
}
