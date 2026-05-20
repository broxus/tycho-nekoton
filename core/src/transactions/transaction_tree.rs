use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use nekoton_utils::time::Clock;
use tycho_executor::{Executor, ExecutorParams, ParsedConfig};
use tycho_types::cell::{HashBytes, Lazy};
use tycho_types::models::{
    BlockchainConfig, MsgInfo, MsgType, OptionalAccount, OwnedMessage, ShardAccount, StdAddr,
    Transaction,
};
use tycho_types::num::Tokens;

use crate::models::{ContractState, GenTimings};
use crate::transport::Transport;

const A_LOT: u128 = 1_000_000_000_000_000;

#[derive(Debug, Clone)]
pub struct TransactionTreeMessage {
    pub message: OwnedMessage,
    pub parent: Option<HashBytes>,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct TransactionTreeStep {
    pub transaction: Transaction,
    pub message: OwnedMessage,
    pub parent: Option<HashBytes>,
    pub depth: usize,
    pub out_messages: Vec<OwnedMessage>,
}

pub struct TransactionTreeStream {
    states: HashMap<StdAddr, StoredAccount>,
    messages: VecDeque<TransactionTreeMessage>,
    config: BlockchainConfig,
    params: ExecutorParams,
    disable_signature_check: bool,
    unlimited_message_balance: bool,
    unlimited_account_balance: bool,
    transport: Arc<dyn Transport>,
    clock: Arc<dyn Clock>,
}

impl TransactionTreeStream {
    pub fn new(
        message: OwnedMessage,
        config: BlockchainConfig,
        transport: Arc<dyn Transport>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self::with_params(message, config, ExecutorParams::default(), transport, clock)
    }

    pub fn with_params(
        message: OwnedMessage,
        config: BlockchainConfig,
        params: ExecutorParams,
        transport: Arc<dyn Transport>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            states: Default::default(),
            messages: VecDeque::from([TransactionTreeMessage {
                message,
                parent: None,
                depth: 0,
            }]),
            config,
            params,
            disable_signature_check: false,
            unlimited_message_balance: false,
            unlimited_account_balance: false,
            transport,
            clock,
        }
    }

    pub fn disable_signature_check(&mut self) -> &mut Self {
        self.disable_signature_check = true;
        self
    }

    pub fn unlimited_message_balance(&mut self) -> &mut Self {
        self.unlimited_message_balance = true;
        self
    }

    pub fn unlimited_account_balance(&mut self) -> &mut Self {
        self.unlimited_account_balance = true;
        self
    }

    pub fn set_executor_params(&mut self, params: ExecutorParams) -> &mut Self {
        self.params = params;
        self
    }

    pub fn executor_params(&self) -> &ExecutorParams {
        &self.params
    }

    pub fn message_queue(&self) -> &VecDeque<TransactionTreeMessage> {
        &self.messages
    }

    pub fn retain_message_queue<F>(&mut self, f: F)
    where
        F: FnMut(&TransactionTreeMessage) -> bool,
    {
        self.messages.retain(f);
    }

    pub fn peek(&self) -> Option<&TransactionTreeMessage> {
        self.messages.front()
    }

    pub fn cached_state(&self, address: &StdAddr) -> Option<&ShardAccount> {
        self.states.get(address).map(|state| &state.shard_account)
    }

    pub fn cached_states(&self) -> impl Iterator<Item = (&StdAddr, &ShardAccount)> {
        self.states
            .iter()
            .map(|(address, state)| (address, &state.shard_account))
    }

    pub async fn next(&mut self) -> TransactionTreeResult<Option<Transaction>> {
        self.next_step()
            .await
            .map(|step| step.map(|step| step.transaction))
    }

    pub async fn next_step(&mut self) -> TransactionTreeResult<Option<TransactionTreeStep>> {
        match self.messages.pop_front() {
            Some(message) => self.step(message).await.map(Some),
            None => Ok(None),
        }
    }

    async fn step(
        &mut self,
        TransactionTreeMessage {
            mut message,
            parent,
            depth,
        }: TransactionTreeMessage,
    ) -> TransactionTreeResult<TransactionTreeStep> {
        if self.unlimited_message_balance {
            if let MsgInfo::Int(info) = &mut message.info {
                info.value.tokens = Tokens::new(A_LOT);
            }
        }

        let address = message_destination(&message)?.clone();
        let mut state = self.get_state(&address).await?;

        if self.unlimited_account_balance {
            set_unlimited_account_balance(&mut state.shard_account)?;
        }

        let mut params = self.params.clone();
        params.block_unixtime = self.block_unixtime(&state.shard_account)?;
        params.block_lt = state.next_lt;
        if self.disable_signature_check {
            params.vm_modifiers.chksig_always_succeed = true;
        }

        let config = ParsedConfig::parse(self.config.clone(), params.block_unixtime)
            .map_err(into_execution_error)?;
        let executor = Executor::new(&params, &config).with_min_lt(state.next_lt);
        let is_external = !matches!(message.ty(), MsgType::Int);
        let output = executor
            .begin_ordinary(&address, is_external, &message, &state.shard_account)
            .map_err(into_execution_error)?
            .commit()
            .map_err(into_execution_error)?;

        let transaction_hash = *output.transaction.repr_hash();
        let transaction = output.transaction.load().map_err(into_execution_error)?;

        let mut out_messages = Vec::new();
        for message in output.transaction_meta.out_msgs {
            let message = message.load().map_err(into_execution_error)?;
            if matches!(message.ty(), MsgType::Int) {
                self.messages.push_back(TransactionTreeMessage {
                    message: message.clone(),
                    parent: Some(transaction_hash),
                    depth: depth + 1,
                });
            }
            out_messages.push(message);
        }

        self.states.insert(
            address,
            StoredAccount {
                shard_account: output.new_state,
                next_lt: output.transaction_meta.next_lt,
            },
        );

        Ok(TransactionTreeStep {
            transaction,
            message,
            parent,
            depth,
            out_messages,
        })
    }

    async fn get_state(&self, address: &StdAddr) -> TransactionTreeResult<StoredAccount> {
        match self.states.get(address) {
            Some(state) => Ok(state.clone()),
            None => {
                let state = self
                    .transport
                    .get_contract_state(address, None)
                    .await
                    .map_err(TransactionTreeError::TransportError)?;

                state_from_contract_state(state)
            }
        }
    }

    fn block_unixtime(&self, shard_account: &ShardAccount) -> TransactionTreeResult<u32> {
        let now = self.clock.now_sec_u64();
        let now = match shard_account.load_account().map_err(into_state_error)? {
            Some(account) => now.max(u64::from(account.storage_stat.last_paid)),
            None => now,
        };

        Ok(u32::try_from(now).unwrap_or(u32::MAX))
    }
}

pub type TransactionsTreeStream = TransactionTreeStream;

#[derive(Clone)]
struct StoredAccount {
    shard_account: ShardAccount,
    next_lt: u64,
}

pub type TransactionTreeResult<T> = Result<T, TransactionTreeError>;

#[derive(Debug, thiserror::Error)]
pub enum TransactionTreeError {
    #[error("external out messages cannot be executed")]
    ExternalOutMessage,
    #[error("message destination is not a standard address")]
    UnsupportedDestinationAddress,
    #[error("transport returned unchanged contract state for a fresh lookup")]
    UnchangedContractState,
    #[error("transport error: {0}")]
    TransportError(anyhow::Error),
    #[error("state error: {0}")]
    StateError(anyhow::Error),
    #[error("execution error: {0}")]
    ExecutionError(anyhow::Error),
}

fn message_destination(message: &OwnedMessage) -> TransactionTreeResult<&StdAddr> {
    let address = match &message.info {
        MsgInfo::Int(info) => &info.dst,
        MsgInfo::ExtIn(info) => &info.dst,
        MsgInfo::ExtOut(_) => return Err(TransactionTreeError::ExternalOutMessage),
    };

    address
        .as_std()
        .ok_or(TransactionTreeError::UnsupportedDestinationAddress)
}

fn state_from_contract_state(state: ContractState) -> TransactionTreeResult<StoredAccount> {
    Ok(match state {
        ContractState::NotExists { timings } => StoredAccount {
            shard_account: empty_shard_account()?,
            next_lt: timings.gen_lt,
        },
        ContractState::Exists {
            account,
            timings,
            last_transaction_id,
        } => StoredAccount {
            shard_account: ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(*account))).map_err(into_state_error)?,
                last_trans_hash: last_transaction_id.hash,
                last_trans_lt: last_transaction_id.lt,
            },
            next_lt: next_lt(&timings, last_transaction_id.lt),
        },
        ContractState::Unchanged { .. } => {
            return Err(TransactionTreeError::UnchangedContractState)
        }
    })
}

fn empty_shard_account() -> TransactionTreeResult<ShardAccount> {
    Ok(ShardAccount {
        account: Lazy::new(&OptionalAccount::EMPTY).map_err(into_state_error)?,
        last_trans_hash: HashBytes::ZERO,
        last_trans_lt: 0,
    })
}

fn set_unlimited_account_balance(shard_account: &mut ShardAccount) -> TransactionTreeResult<()> {
    let Some(mut account) = shard_account.load_account().map_err(into_state_error)? else {
        return Ok(());
    };

    account.balance.tokens = Tokens::new(A_LOT);
    shard_account.account = Lazy::new(&OptionalAccount(Some(account))).map_err(into_state_error)?;

    Ok(())
}

fn next_lt(timings: &GenTimings, last_transaction_lt: u64) -> u64 {
    timings.gen_lt.max(last_transaction_lt.saturating_add(1))
}

fn into_state_error(error: impl Into<anyhow::Error>) -> TransactionTreeError {
    TransactionTreeError::StateError(error.into())
}

fn into_execution_error(error: impl Into<anyhow::Error>) -> TransactionTreeError {
    TransactionTreeError::ExecutionError(error.into())
}

#[cfg(test)]
mod tests {
    use tycho_types::cell::Cell;
    use tycho_types::models::{ExtInMsgInfo, ExtOutMsgInfo, IntAddr};
    use tycho_types::prelude::CellFamily;

    use super::*;

    fn message(info: MsgInfo) -> OwnedMessage {
        OwnedMessage {
            info,
            init: None,
            body: Cell::empty_cell().into(),
            layout: None,
        }
    }

    #[test]
    fn extracts_standard_destination() {
        let address = StdAddr::new(0, HashBytes::ZERO);
        let message = message(MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(address.clone()),
            ..Default::default()
        }));

        assert_eq!(message_destination(&message).unwrap(), &address);
    }

    #[test]
    fn rejects_external_out_message() {
        let message = message(MsgInfo::ExtOut(ExtOutMsgInfo::default()));

        assert!(matches!(
            message_destination(&message),
            Err(TransactionTreeError::ExternalOutMessage)
        ));
    }

    #[test]
    fn not_exists_state_uses_timings_as_next_lt() {
        let state = state_from_contract_state(ContractState::NotExists {
            timings: GenTimings {
                gen_lt: 123,
                gen_utime: 10,
            },
        })
        .unwrap();

        assert_eq!(state.next_lt, 123);
        assert!(state.shard_account.load_account().unwrap().is_none());
    }

    #[test]
    fn next_lt_is_above_last_transaction_lt() {
        let timings = GenTimings {
            gen_lt: 100,
            gen_utime: 10,
        };

        assert_eq!(next_lt(&timings, 120), 121);
    }
}
