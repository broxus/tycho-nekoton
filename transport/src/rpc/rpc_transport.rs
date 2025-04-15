use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use everscale_types::cell::HashBytes;
use everscale_types::models::{OwnedMessage, StdAddr, Transaction};
use everscale_types::prelude::CellBuilder;
use futures_util::StreamExt;
use nekoton_core::models::{ContractState, LatestBlockchainConfig};
use parking_lot::RwLock;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::options::BlockchainOptions;
use crate::rpc::rpc_connection::RpcConnection;
use crate::{Connection, Transport};

static ROUND_ROBIN_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct RpcTransport {
    inner: Arc<Inner>,
}

struct Inner {
    endpoints: Vec<RpcConnection>,
    live_endpoints: RwLock<Vec<RpcConnection>>,
    options: TransportOptions,

    bc_options: BlockchainOptions,
}

impl RpcTransport {
    pub async fn new<I: IntoIterator<Item = Url> + Send>(
        endpoints: I,
        options: TransportOptions,
        use_proto: bool,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(options.request_timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .http2_adaptive_window(true)
            .http2_keep_alive_interval(Duration::from_secs(60))
            .http2_keep_alive_timeout(Duration::from_secs(1))
            .http2_keep_alive_while_idle(true)
            .gzip(false)
            .build()?;

        let endpoints = endpoints
            .into_iter()
            .map(|endpoint| RpcConnection::new(endpoint, client.clone(), use_proto))
            .collect();

        let transport = Self {
            inner: Arc::new(Inner {
                endpoints,
                options,
                live_endpoints: Default::default(),
                bc_options: Default::default(),
            }),
        };

        let mut live = transport.update_endpoints().await;

        if live == 0 {
            anyhow::bail!("No live endpoints");
        }

        let rc = transport.clone();
        tokio::spawn(async move {
            loop {
                let sleep_time = if live != 0 {
                    rc.inner.options.probe_interval
                } else {
                    rc.inner.options.aggressive_poll_interval
                };

                tokio::time::sleep(sleep_time).await;
                live = rc.update_endpoints().await;
            }
        });

        Ok(transport)
    }

    async fn get_connection(&self) -> Option<RpcConnection> {
        for _ in 0..self.inner.endpoints.len() {
            let client = {
                let live_endpoints = self.inner.live_endpoints.read();
                self.inner.options.choose_strategy.choose(&live_endpoints)
            };

            if client.is_some() {
                return client;
            } else {
                tokio::time::sleep(self.inner.options.aggressive_poll_interval).await;
            }
        }

        None
    }

    async fn with_retries<F, Fut, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: Fn(RpcConnection) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        const NUM_RETRIES: usize = 10;

        for tries in 0..NUM_RETRIES {
            let client = self
                .get_connection()
                .await
                .ok_or(TransportError::NoEndpointsAvailable)?;

            // TODO: lifetimes to avoid of cloning?
            match f(client.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if tries >= NUM_RETRIES - 1 {
                        return Err(e);
                    }

                    let endpoint = client.endpoint();
                    self.remove_endpoint(endpoint);

                    tokio::time::sleep(self.inner.options.aggressive_poll_interval).await;
                }
            }
        }

        unreachable!()
    }

    async fn update_endpoints(&self) -> usize {
        let mut futures = futures_util::stream::FuturesUnordered::new();
        for endpoint in &self.inner.endpoints {
            futures.push(async move { endpoint.is_alive().await.then(|| endpoint.clone()) });
        }

        let mut new_endpoints = Vec::with_capacity(self.inner.endpoints.len());
        while let Some(endpoint) = futures.next().await {
            new_endpoints.extend(endpoint);
        }

        let mut old_endpoints = self.inner.live_endpoints.write();

        *old_endpoints = new_endpoints;
        old_endpoints.len()
    }

    fn remove_endpoint(&self, endpoint: &str) {
        self.inner
            .live_endpoints
            .write()
            .retain(|c| c.endpoint() != endpoint);
    }
}

#[async_trait::async_trait]
impl Transport for RpcTransport {
    async fn send_message(&self, message: &OwnedMessage) -> anyhow::Result<()> {
        self.with_retries(|instance| async move { instance.send_message(message).await })
            .await
    }

    async fn send_message_reliable(&self, message: &OwnedMessage) -> anyhow::Result<Transaction> {
        self.send_message(message).await?;

        let cell = CellBuilder::build_from(message)?;
        let hash = cell.repr_hash();

        for _ in 0..self.inner.bc_options.message_poll_attempts {
            let transaction = self
                .with_retries(|instance| async move { instance.get_dst_transaction(hash).await })
                .await?;

            if let Some(transaction) = transaction {
                return Ok(transaction);
            }

            tokio::time::sleep(self.inner.bc_options.message_poll_interval).await;
        }

        Err(TransportError::MessageTimeout.into())
    }

    async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> anyhow::Result<ContractState> {
        self.with_retries(|instance| async move {
            instance
                .get_contract_state(address, last_transaction_lt)
                .await
        })
        .await
    }

    async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig> {
        self.with_retries(|instance| async move { instance.get_config().await })
            .await
    }

    async fn get_transaction(&self, hash: &HashBytes) -> anyhow::Result<Option<Transaction>> {
        self.with_retries(|instance| async move { instance.get_transaction(hash).await })
            .await
    }

    async fn get_dst_transaction(
        &self,
        message_hash: &HashBytes,
    ) -> anyhow::Result<Option<Transaction>> {
        self.with_retries(
            |instance| async move { instance.get_dst_transaction(message_hash).await },
        )
        .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportOptions {
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

    /// Rotation Strategy.
    ///
    /// Default: `Random`
    pub choose_strategy: ChooseStrategy,
}

impl Default for TransportOptions {
    fn default() -> Self {
        Self {
            probe_interval: Duration::from_secs(5),
            request_timeout: Duration::from_secs(3),
            aggressive_poll_interval: Duration::from_secs(1),
            choose_strategy: ChooseStrategy::Random,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum ChooseStrategy {
    Random,
    RoundRobin,
    /// Choose the rpc with the lowest latency
    TimeBased,
}

impl ChooseStrategy {
    fn choose(&self, endpoints: &[RpcConnection]) -> Option<RpcConnection> {
        use rand::prelude::SliceRandom;

        match self {
            ChooseStrategy::Random => endpoints.choose(&mut rand::thread_rng()).cloned(),
            ChooseStrategy::RoundRobin => {
                let index = ROUND_ROBIN_COUNTER.fetch_add(1, Ordering::Release);
                endpoints.get(index % endpoints.len()).cloned()
            }
            ChooseStrategy::TimeBased => endpoints
                .iter()
                .min_by(|&left, &right| left.cmp(right))
                .cloned(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    #[error("No rpc available")]
    NoEndpointsAvailable,
    #[error("Message processing timed out")]
    MessageTimeout,
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use anyhow::Result;

    use super::*;

    #[tokio::test]
    async fn connection_test() -> Result<()> {
        let endpoints = ["http://57.129.53.62:8080/rpc"]
            .iter()
            .map(|x| x.parse().unwrap())
            .collect::<Vec<_>>();

        let _client = RpcTransport::new(
            endpoints,
            TransportOptions {
                probe_interval: Duration::from_secs(10),
                ..Default::default()
            },
            false,
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn get_config_test() -> Result<()> {
        let endpoints = ["http://57.129.53.62:8080/rpc"]
            .iter()
            .map(|x| x.parse().unwrap())
            .collect::<Vec<_>>();

        let client = RpcTransport::new(
            endpoints,
            TransportOptions {
                probe_interval: Duration::from_secs(10),
                ..Default::default()
            },
            false,
        )
        .await?;

        let config = client.get_config().await?;
        assert_eq!(config.global_id, 2000);

        Ok(())
    }
}
