use crate::jrpc::JrpcClient;
use everscale_types::cell::DynCell;
use everscale_types::models::StdAddr;
use nekoton_core::transport::{ContractState, Transport};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod jrpc;
mod models;
mod utils;

#[derive(Clone)]
pub enum Endpoint {
    Jrpc(JrpcClient),
}

#[async_trait::async_trait]
impl Transport for Endpoint {
    async fn broadcast_message(&self, message: &DynCell) -> anyhow::Result<()> {
        match &self {
            Endpoint::Jrpc(client) => client.broadcast_message(message).await,
        }
    }

    async fn get_contract_state(&self, address: &StdAddr) -> anyhow::Result<ContractState> {
        match &self {
            Endpoint::Jrpc(client) => client.get_contract_state(address).await,
        }
    }
}

pub struct TransportImpl {
    endpoints: Vec<Endpoint>,
    live_endpoints: RwLock<Vec<Endpoint>>,
    options: ClientOptions,
}

impl TransportImpl {
    async fn get_client(&self) -> Option<Endpoint> {
        todo!()
    }
}

#[async_trait::async_trait]
impl Transport for TransportImpl {
    async fn broadcast_message(&self, message: &DynCell) -> anyhow::Result<()> {
        let client = self.get_client().await.unwrap();
        client.broadcast_message(message).await
    }

    async fn get_contract_state(&self, address: &StdAddr) -> anyhow::Result<ContractState> {
        let client = self.get_client().await.unwrap();
        client.get_contract_state(address).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientOptions {
    /// How often the probe should update health statuses.
    ///
    /// Default: `1 sec`
    pub probe_interval: Duration,

    /// How long to wait for a response from a node.
    ///
    /// Default: `1 sec`
    pub request_timeout: Duration,
    /// How long to wait between health checks in case if all nodes are down.
    ///
    /// Default: `1 sec`
    pub aggressive_poll_interval: Duration,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            probe_interval: Duration::from_secs(1),
            request_timeout: Duration::from_secs(3),
            aggressive_poll_interval: Duration::from_secs(1),
        }
    }
}
