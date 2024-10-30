use everscale_types::cell::DynCell;
use everscale_types::models::StdAddr;
use nekoton_core::transport::{ContractState, LatestBlockchainConfig, Transport};
use reqwest::Url;

use crate::models::Timings;
use crate::LiveCheckResult;

mod jrpc;

#[derive(Clone)]
pub enum Endpoint {
    Jrpc(jrpc::JrpcClient),
    Proto,
}

#[async_trait::async_trait]
impl Transport for Endpoint {
    async fn broadcast_message(&self, message: &DynCell) -> anyhow::Result<()> {
        match &self {
            Self::Jrpc(client) => client.broadcast_message(message).await,
            Self::Proto => todo!(),
        }
    }

    async fn get_contract_state(&self, address: &StdAddr) -> anyhow::Result<ContractState> {
        match &self {
            Self::Jrpc(client) => client.get_contract_state(address).await,
            Self::Proto => todo!(),
        }
    }

    async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig> {
        match &self {
            Self::Jrpc(client) => client.get_config().await,
            Self::Proto => todo!(),
        }
    }
}

impl Eq for Endpoint {}

impl PartialEq<Self> for Endpoint {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint() == other.endpoint()
    }
}

impl PartialOrd<Self> for Endpoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Endpoint {
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
pub trait Connection: Send + Sync {
    fn new(endpoint: Url, client: reqwest::Client) -> Self;

    async fn is_alive(&self) -> bool {
        let check_result = self.is_alive_inner().await;
        let is_alive = check_result.as_bool();
        self.update_was_dead(!is_alive);

        match check_result {
            LiveCheckResult::Live(stats) => self.set_stats(Some(stats)),
            LiveCheckResult::Dead => {}
        }

        is_alive
    }

    fn endpoint(&self) -> &str;

    fn get_stats(&self) -> Option<Timings>;

    fn set_stats(&self, stats: Option<Timings>);

    fn update_was_dead(&self, is_dead: bool);

    async fn is_alive_inner(&self) -> LiveCheckResult;
}

#[async_trait::async_trait]
impl Connection for Endpoint {
    fn new(endpoint: Url, client: reqwest::Client) -> Self {
        let is_jrpc = endpoint.path().ends_with("/rpc");
        if is_jrpc {
            Self::Jrpc(jrpc::JrpcClient::new(endpoint, client))
        } else {
            Self::Proto
        }
    }

    async fn is_alive(&self) -> bool {
        match &self {
            Self::Jrpc(client) => client.is_alive().await,
            Self::Proto => todo!(),
        }
    }

    fn endpoint(&self) -> &str {
        match &self {
            Self::Jrpc(client) => client.endpoint(),
            Self::Proto => todo!(),
        }
    }

    fn get_stats(&self) -> Option<Timings> {
        match &self {
            Self::Jrpc(client) => client.get_stats(),
            Self::Proto => todo!(),
        }
    }

    fn set_stats(&self, stats: Option<Timings>) {
        match &self {
            Self::Jrpc(client) => client.set_stats(stats),
            Self::Proto => todo!(),
        }
    }

    fn update_was_dead(&self, is_dead: bool) {
        match &self {
            Self::Jrpc(client) => client.update_was_dead(is_dead),
            Self::Proto => todo!(),
        }
    }

    async fn is_alive_inner(&self) -> LiveCheckResult {
        match &self {
            Self::Jrpc(client) => client.is_alive_inner().await,
            Self::Proto => todo!(),
        }
    }
}
