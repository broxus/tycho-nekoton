use crate::contracts::*;
use crate::models::{ContractState, LastTransactionId, LatestBlockchainConfig};
use nekoton_utils::time::{Clock, SimpleClock, Timings};
use std::collections::HashMap;
use tycho_executor::{ExecutorParams, ParsedConfig};
use tycho_types::cell::HashBytes;
use tycho_types::models::{
    BlockchainConfig, MsgInfo, OwnedMessage, ShardAccount, StdAddr, Transaction,
};

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send_message(&self, message: &OwnedMessage) -> anyhow::Result<()>;
    async fn send_message_reliable(&self, message: &OwnedMessage) -> anyhow::Result<Transaction>;
    async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> anyhow::Result<ContractState>;
    async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig>;
    async fn get_transaction(&self, hash: &HashBytes) -> anyhow::Result<Option<Transaction>>;
    async fn get_dst_transaction(
        &self,
        message_hash: &HashBytes,
    ) -> anyhow::Result<Option<Transaction>>;
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

pub struct SimpleTransport {
    accounts: HashMap<StdAddr, ShardAccount>,
    config: BlockchainConfig,
}

impl SimpleTransport {
    pub fn new<I: IntoIterator<Item = ShardAccount> + Send>(
        accounts: I,
        config: BlockchainConfig,
    ) -> anyhow::Result<SimpleTransport> {
        let mut accs = HashMap::new();
        for acc in accounts {
            let Some(a) = acc.load_account()? else {
                continue;
            };

            let addr = match a.address.as_std() {
                Some(addr) => addr,
                None => anyhow::bail!("unsupported address format"),
            };

            accs.insert(addr.clone(), acc);
        }

        Ok(SimpleTransport {
            config,
            accounts: accs,
        })
    }
}

#[async_trait::async_trait]
impl Transport for SimpleTransport {
    async fn send_message(&self, _: &OwnedMessage) -> anyhow::Result<()> {
        todo!()
    }

    async fn send_message_reliable(&self, message: &OwnedMessage) -> anyhow::Result<Transaction> {
        let dst = match &message.info {
            MsgInfo::Int(info) => &info.dst,
            MsgInfo::ExtIn(info) => &info.dst,
            _ => return Err(anyhow::anyhow!("unsupported message type")),
        };
        let address = dst.as_std().unwrap();
        let account = self
            .accounts
            .get(address)
            .ok_or(anyhow::anyhow!("no address found"))?;

        let config = ParsedConfig::parse(self.config.clone(), SimpleClock.now_sec_u64() as u32)?;

        local_executor::execute_ordinary_transaction(
            account,
            message,
            &ExecutorParams::default(),
            &config,
        )
        .map_err(Into::into)
    }

    async fn get_contract_state(
        &self,
        address: &StdAddr,
        _: Option<u64>,
    ) -> anyhow::Result<ContractState> {
        let shard_account = match self.accounts.get(address) {
            Some(account) => account,
            None => anyhow::bail!("no account found"),
        };
        let timings = utils::get_gen_timings(&SimpleClock, shard_account.last_trans_lt);

        let account = match shard_account.load_account()? {
            Some(account) => account,
            None => return Ok(ContractState::NotExists { timings }),
        };

        Ok(ContractState::Exists {
            account: Box::new(account),
            timings,
            last_transaction_id: LastTransactionId {
                lt: shard_account.last_trans_lt,
                hash: shard_account.last_trans_hash,
            },
        })
    }

    async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig> {
        Ok(LatestBlockchainConfig {
            global_id: 0,
            seqno: 0,
            config: self.config.clone(),
        })
    }

    async fn get_transaction(&self, _: &HashBytes) -> anyhow::Result<Option<Transaction>> {
        todo!()
    }

    async fn get_dst_transaction(&self, _: &HashBytes) -> anyhow::Result<Option<Transaction>> {
        todo!()
    }
}
