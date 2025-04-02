use anyhow::{Context, Result};
use everscale_types::boc::BocRepr;
use everscale_types::cell::HashBytes;
use everscale_types::merkle::MerkleProof;
use everscale_types::models::{BlockId, OptionalAccount, OwnedMessage, StdAddr, Transaction};
use everscale_types::prelude::{Boc, Cell, CellFamily, CellSlice, Load};
use proof_api_util::block::{BlockchainBlock, BlockchainModels, TonModels};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use ton_lite_client::{LiteClient, LiteClientConfig, NodeInfo};

use crate::ton_lite::models::{ParsedProofs, TonMcStateExtraShort};
use nekoton_core::models::GenTimings;
use nekoton_core::transport::{ContractState, LatestBlockchainConfig, Transport};

pub struct TonLiteTransport {
    inner: Arc<Inner>,
}

struct Inner {
    client: LiteClient,

    last_mc_block: Arc<Mutex<Option<BlockId>>>,
    ping_interval: Duration,
    mc_block_task: Option<AbortHandle>,
}

impl Clone for Inner {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            last_mc_block: self.last_mc_block.clone(),
            ping_interval: self.ping_interval.clone(),
            mc_block_task: None,
        }
    }
}

impl Inner {
    async fn update_last_mc_block_task(self) {
        let mut interval = tokio::time::interval(self.ping_interval);
        loop {
            interval.tick().await;
            if let Ok(block) = self.client.get_last_mc_block_id().await {
                let mut guard = self.last_mc_block.lock().await;
                *guard = Some(block);
            }
        }
    }

    async fn get_last_mc_block_id(&self) -> Result<BlockId> {
        let guard = self.last_mc_block.lock().await;
        match guard.as_ref() {
            Some(block) => Ok(block.clone()),
            None => self.client.get_last_mc_block_id().await,
        }
    }
}

impl TonLiteTransport {
    pub fn new<I>(config: LiteClientConfig, nodes: I) -> Self
    where
        I: IntoIterator<Item = NodeInfo>,
    {
        let mut inner = Inner {
            client: LiteClient::new(config, nodes),
            last_mc_block: Arc::default(),
            ping_interval: Duration::from_secs(5),

            mc_block_task: None,
        };

        inner.mc_block_task =
            Some(tokio::spawn(inner.clone().update_last_mc_block_task()).abort_handle());

        Self {
            inner: Arc::new(inner),
        }
    }
}

#[async_trait::async_trait]
impl Transport for TonLiteTransport {
    async fn broadcast_message(&self, message: &OwnedMessage) -> Result<()> {
        let message_bytes = BocRepr::encode(message)?;
        let _result = self.inner.client.send_message(message_bytes).await?;
        Ok(())
    }

    async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> Result<ContractState> {
        let latest_block = self.inner.get_last_mc_block_id().await?;
        let account_state = self
            .inner
            .client
            .get_account(&latest_block, address)
            .await?;

        let proofs = parse_proofs(account_state.proof)?;
        if account_state.state.is_empty() {
            return Ok(ContractState::NotExists {
                timings: proofs.timings,
            });
        }

        let last_transaction_id = proofs
            .get_last_transaction_id(&address.address)
            .context("failed to get last transaction id")?;

        if let Some(lt) = last_transaction_lt {
            if last_transaction_id.lt == lt {
                return Ok(ContractState::Unchanged {
                    timings: proofs.timings,
                });
            }
        }

        let cell = Boc::decode(&account_state.state)?;
        let OptionalAccount(Some(account)) = cell.parse()? else {
            return Ok(ContractState::NotExists {
                timings: proofs.timings,
            });
        };

        Ok(ContractState::Exists {
            account: Box::new(account),
            timings: proofs.timings,
            last_transaction_id,
        })
    }

    async fn get_config(&self) -> Result<LatestBlockchainConfig> {
        let latest_block = self.inner.get_last_mc_block_id().await?;

        let config = self.inner.client.get_config(&latest_block).await?;
        let state_proof = Boc::decode(&config.config_proof)?
            .parse_exotic::<MerkleProof>()?
            .cell;

        let mut cs: CellSlice<'_> = state_proof.as_slice()?;
        cs.only_last(1, 1)?;
        let extra = <Option<Cell>>::load_from(&mut cs)
            .context("failed to read McStateExtra")?
            .context("expected McStateExtra")?
            .parse::<TonMcStateExtraShort>()?;

        let global_id = extra.config.get_global_id()?;
        let config = LatestBlockchainConfig {
            global_id,
            seqno: latest_block.seqno,
            config: extra.config,
        };

        Ok(config)
    }

    async fn get_transaction(&self, hash: &HashBytes) -> Result<Option<Transaction>> {
        todo!()
    }
}

fn parse_proofs(proofs: Vec<u8>) -> Result<ParsedProofs> {
    use everscale_types::boc::de::{BocHeader, Options};

    let header = BocHeader::decode(
        &proofs,
        &Options {
            max_roots: Some(2),
            min_roots: Some(2),
        },
    )?;

    let block_proof_id = *header.roots().first().context("block proof not found")?;
    let state_proof_id = *header.roots().get(1).context("state proof not found")?;
    let cells = header.finalize(Cell::empty_context())?;

    let block = cells
        .get(block_proof_id)
        .context("block proof not found")?
        .parse_exotic::<MerkleProof>()?
        .cell
        .parse::<<TonModels as BlockchainModels>::Block>()?;

    let info = block.load_info()?;
    let timings = GenTimings {
        gen_lt: info.end_lt,
        gen_utime: info.gen_utime,
    };

    let state_root = cells
        .get(state_proof_id)
        .context("state proof not found")?
        .parse_exotic::<MerkleProof>()?
        .cell;

    Ok(ParsedProofs {
        timings,
        state_root,
    })
}
