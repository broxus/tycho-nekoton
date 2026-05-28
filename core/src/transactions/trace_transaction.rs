use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::{Future, Stream};
use pin_project::pin_project;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tycho_types::abi::{Event, NamedAbiValue};
use tycho_types::cell::{Cell, CellBuilder, CellSlice, HashBytes};
use tycho_types::models::{MsgInfo, MsgType, OwnedMessage, StdAddr, Transaction};
use tycho_types::prelude::Load;

use crate::transport::Transport;

pub type TraceTransactionResult<T> = Result<T, TraceTransactionError>;

type NextTransactionFut = Option<
    Pin<Box<dyn Future<Output = TraceTransactionResult<Option<TraceTransactionStep>>> + Send>>,
>;

#[derive(Debug, thiserror::Error)]
pub enum TraceTransactionError {
    #[error("root transaction not found: {hash:?}")]
    RootTransactionNotFound { hash: HashBytes },
    #[error("destination transaction for message {hash:?} was not found within {timeout:?}")]
    DestinationTransactionTimeout { hash: HashBytes, timeout: Duration },
    #[error("transport error: {0}")]
    TransportError(anyhow::Error),
    #[error("failed to decode transaction message: {0}")]
    MessageDecodeError(anyhow::Error),
    #[error("failed to compute transaction hash: {0}")]
    TransactionHashError(anyhow::Error),
    #[error("failed to decode event: {0}")]
    EventDecodeError(anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct TraceTransactionPolling {
    pub min_interval: Duration,
    pub max_interval: Duration,
    pub timeout: Option<Duration>,
}

impl Default for TraceTransactionPolling {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_millis(500),
            max_interval: Duration::from_millis(3000),
            timeout: Some(Duration::from_secs(60)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceMessage {
    pub index: Option<u16>,
    pub hash: HashBytes,
    pub message: OwnedMessage,
}

impl TraceMessage {
    pub fn source(&self) -> Option<&StdAddr> {
        match &self.message.info {
            MsgInfo::Int(info) => info.src.as_std(),
            MsgInfo::ExtOut(info) => info.src.as_std(),
            MsgInfo::ExtIn(_) => None,
        }
    }

    pub fn destination(&self) -> Option<&StdAddr> {
        match &self.message.info {
            MsgInfo::Int(info) => info.dst.as_std(),
            MsgInfo::ExtIn(info) => info.dst.as_std(),
            MsgInfo::ExtOut(_) => None,
        }
    }

    pub fn decode_event(
        &self,
        event: &Event,
    ) -> TraceTransactionResult<Option<Vec<NamedAbiValue>>> {
        let slice = CellSlice::apply(&self.message.body).map_err(decode_message_error)?;
        let id = match slice.get_u32(slice.offset_bits()) {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };

        if id != event.id {
            return Ok(None);
        }

        event
            .decode_internal_input(slice)
            .map(Some)
            .map_err(TraceTransactionError::EventDecodeError)
    }
}

#[derive(Debug, Clone)]
pub struct TraceTransactionEvent {
    pub message: TraceMessage,
    pub tokens: Vec<NamedAbiValue>,
}

#[derive(Debug, Clone)]
pub struct TraceTransactionStep {
    pub transaction: Transaction,
    pub hash: HashBytes,
    pub parent: Option<HashBytes>,
    pub depth: usize,
    pub in_message: Option<TraceMessage>,
}

impl TraceTransactionStep {
    pub fn account_matches(&self, address: &StdAddr) -> bool {
        if let Some(destination) = self.in_message.as_ref().and_then(TraceMessage::destination) {
            return destination == address;
        }

        // Transactions store only the account id, while message destinations keep the workchain.
        // For root transactions without an incoming message this is the best available fallback.
        self.transaction.account == address.address
    }

    pub fn out_messages(&self) -> TraceTransactionResult<Vec<TraceMessage>> {
        transaction_out_messages(&self.transaction)
    }

    pub fn find_event(
        &self,
        event: &Event,
    ) -> TraceTransactionResult<Option<TraceTransactionEvent>> {
        for message in self.out_messages()? {
            let Some(tokens) = message.decode_event(event)? else {
                continue;
            };

            return Ok(Some(TraceTransactionEvent { message, tokens }));
        }

        Ok(None)
    }

    pub fn has_event(&self, event: &Event) -> TraceTransactionResult<bool> {
        self.find_event(event).map(|event| event.is_some())
    }
}

#[pin_project]
pub struct TraceTransaction {
    inner: Arc<Mutex<TraceTransactionState>>,
    #[pin]
    future: NextTransactionFut,
}

impl TraceTransaction {
    pub fn new(root_hash: &HashBytes, transport: Arc<dyn Transport>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TraceTransactionState {
                transport,
                include_root: false,
                polling: TraceTransactionPolling::default(),
                root_hash: Some(*root_hash),
                queue: Default::default(),
            })),
            future: None,
        }
    }

    pub fn include_root(mut self) -> Self {
        self.state_mut().include_root = true;
        self
    }

    pub fn with_polling(mut self, polling: TraceTransactionPolling) -> Self {
        self.state_mut().polling = polling;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.state_mut().polling.timeout = Some(timeout);
        self
    }

    pub fn without_timeout(mut self) -> Self {
        self.state_mut().polling.timeout = None;
        self
    }

    pub async fn next_step(&mut self) -> TraceTransactionResult<Option<TraceTransactionStep>> {
        let mut state = self.inner.lock().await;
        state.next().await
    }

    pub async fn find_transaction<F>(
        &mut self,
        mut filter: F,
    ) -> TraceTransactionResult<Option<TraceTransactionStep>>
    where
        F: FnMut(&TraceTransactionStep) -> TraceTransactionResult<bool>,
    {
        while let Some(step) = self.next_step().await? {
            if filter(&step)? {
                return Ok(Some(step));
            }
        }

        Ok(None)
    }

    pub async fn find_by_account(
        &mut self,
        address: &StdAddr,
    ) -> TraceTransactionResult<Option<TraceTransactionStep>> {
        self.find_transaction(|step| Ok(step.account_matches(address)))
            .await
    }

    pub async fn find_by_account_and_event(
        &mut self,
        address: &StdAddr,
        event: &Event,
    ) -> TraceTransactionResult<Option<(TraceTransactionStep, TraceTransactionEvent)>> {
        while let Some(step) = self.find_by_account(address).await? {
            let Some(decoded) = step.find_event(event)? else {
                continue;
            };

            return Ok(Some((step, decoded)));
        }

        Ok(None)
    }

    fn state_mut(&mut self) -> &mut TraceTransactionState {
        Arc::get_mut(&mut self.inner)
            .expect("TraceTransaction options must be configured before polling")
            .get_mut()
    }
}

#[derive(Debug, Clone)]
struct PendingMessage {
    parent: HashBytes,
    depth: usize,
    message: TraceMessage,
}

struct TraceTransactionState {
    transport: Arc<dyn Transport>,
    include_root: bool,
    polling: TraceTransactionPolling,
    root_hash: Option<HashBytes>,
    queue: VecDeque<PendingMessage>,
}

impl TraceTransactionState {
    fn enqueue_out_messages(
        &mut self,
        transaction: &Transaction,
        parent: HashBytes,
        depth: usize,
    ) -> TraceTransactionResult<()> {
        let messages = transaction_out_messages(transaction)?;
        self.queue.extend(
            messages
                .into_iter()
                .filter(|message| matches!(message.message.ty(), MsgType::Int))
                .map(|message| PendingMessage {
                    parent,
                    depth,
                    message,
                }),
        );

        Ok(())
    }

    async fn next(&mut self) -> TraceTransactionResult<Option<TraceTransactionStep>> {
        let transport = self.transport.clone();

        if let Some(hash) = self.root_hash {
            let Some(transaction) = transport
                .get_transaction(&hash)
                .await
                .map_err(TraceTransactionError::TransportError)?
            else {
                return Err(TraceTransactionError::RootTransactionNotFound { hash });
            };

            let in_message = if self.include_root {
                transaction_in_message(&transaction)?
            } else {
                None
            };
            self.enqueue_out_messages(&transaction, hash, 1)?;
            self.root_hash = None;

            if self.include_root {
                return Ok(Some(TraceTransactionStep {
                    transaction,
                    hash,
                    parent: None,
                    depth: 0,
                    in_message,
                }));
            }
        }

        let Some(pending) = self.queue.front().cloned() else {
            return Ok(None);
        };

        let transaction = self.wait_dst_transaction(&pending.message.hash).await?;
        let hash = transaction_hash(&transaction)?;
        self.enqueue_out_messages(&transaction, hash, pending.depth + 1)?;
        self.queue.pop_front();

        Ok(Some(TraceTransactionStep {
            transaction,
            hash,
            parent: Some(pending.parent),
            depth: pending.depth,
            in_message: Some(pending.message),
        }))
    }

    async fn wait_dst_transaction(&self, hash: &HashBytes) -> TraceTransactionResult<Transaction> {
        const FACTOR: u32 = 2;

        let started = Instant::now();
        let mut interval = self.polling.min_interval;

        loop {
            if let Some(transaction) = self
                .transport
                .get_dst_transaction(hash)
                .await
                .map_err(TraceTransactionError::TransportError)?
            {
                return Ok(transaction);
            }

            let sleep = match self.polling.timeout {
                Some(timeout) => {
                    let elapsed = started.elapsed();
                    if elapsed >= timeout {
                        return Err(TraceTransactionError::DestinationTransactionTimeout {
                            hash: *hash,
                            timeout,
                        });
                    }

                    interval.min(timeout - elapsed)
                }
                None => interval,
            };

            tokio::time::sleep(sleep).await;
            interval = (interval * FACTOR).min(self.polling.max_interval);
        }
    }
}

impl Stream for TraceTransaction {
    type Item = TraceTransactionResult<TraceTransactionStep>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                return match fut.poll(cx) {
                    Poll::Ready(result) => {
                        this.future.set(None);

                        match result {
                            Ok(Some(step)) => Poll::Ready(Some(Ok(step))),
                            Ok(None) => Poll::Ready(None),
                            Err(error) => Poll::Ready(Some(Err(error))),
                        }
                    }
                    Poll::Pending => Poll::Pending,
                };
            } else {
                let inner = this.inner.clone();

                let future = Box::pin(async move {
                    let mut state = inner.lock().await;
                    state.next().await
                });

                this.future.set(Some(future));
            }
        }
    }
}

fn transaction_in_message(
    transaction: &Transaction,
) -> TraceTransactionResult<Option<TraceMessage>> {
    transaction
        .in_msg
        .as_ref()
        .map(|cell| decode_message(None, cell))
        .transpose()
}

fn transaction_out_messages(
    transaction: &Transaction,
) -> TraceTransactionResult<Vec<TraceMessage>> {
    let mut messages = Vec::new();

    for entry in transaction.out_msgs.iter() {
        let (index, cell) = entry.map_err(decode_message_error)?;
        messages.push(decode_message(Some(index.into_inner()), &cell)?);
    }

    Ok(messages)
}

fn decode_message(index: Option<u16>, cell: &Cell) -> TraceTransactionResult<TraceMessage> {
    let hash = *cell.repr_hash();
    let mut slice = cell.as_slice().map_err(decode_message_error)?;
    let message = OwnedMessage::load_from(&mut slice).map_err(decode_message_error)?;

    Ok(TraceMessage {
        index,
        hash,
        message,
    })
}

fn transaction_hash(transaction: &Transaction) -> TraceTransactionResult<HashBytes> {
    CellBuilder::build_from(transaction)
        .map(|cell| *cell.repr_hash())
        .map_err(|error| TraceTransactionError::TransactionHashError(error.into()))
}

fn decode_message_error(error: impl Into<anyhow::Error>) -> TraceTransactionError {
    TraceTransactionError::MessageDecodeError(error.into())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use tycho_types::abi::AbiVersion;
    use tycho_types::cell::{CellFamily, Lazy};
    use tycho_types::dict::Dict;
    use tycho_types::models::{
        AccountStatus, Block, CurrencyCollection, ExtOutMsgInfo, IntAddr, IntMsgInfo, MsgInfo,
    };
    use tycho_types::num::Uint15;

    use super::*;
    use crate::models::{ContractState, LatestBlockchainConfig};

    #[derive(Default)]
    struct TestTransport {
        transactions: HashMap<HashBytes, Transaction>,
        dst_transactions: HashMap<HashBytes, Transaction>,
    }

    #[async_trait]
    impl Transport for TestTransport {
        async fn send_message(&self, _: &OwnedMessage) -> anyhow::Result<()> {
            unreachable!()
        }

        async fn send_message_reliable(&self, _: &OwnedMessage) -> anyhow::Result<Transaction> {
            unreachable!()
        }

        async fn get_contract_state(
            &self,
            _: &StdAddr,
            _: Option<u64>,
        ) -> anyhow::Result<ContractState> {
            unreachable!()
        }

        async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig> {
            unreachable!()
        }

        async fn get_latest_key_block(&self) -> anyhow::Result<Block> {
            unreachable!()
        }

        async fn get_library_cell(&self, _: &HashBytes) -> anyhow::Result<Option<Cell>> {
            unreachable!()
        }

        async fn get_transactions(
            &self,
            _: &StdAddr,
            _: Option<u64>,
            _: u8,
        ) -> anyhow::Result<Vec<Transaction>> {
            unreachable!()
        }

        async fn get_accounts_by_code_hash(
            &self,
            _: &HashBytes,
            _: Option<&StdAddr>,
            _: u8,
        ) -> anyhow::Result<Vec<StdAddr>> {
            unreachable!()
        }

        async fn get_transaction(&self, hash: &HashBytes) -> anyhow::Result<Option<Transaction>> {
            Ok(self.transactions.get(hash).cloned())
        }

        async fn get_dst_transaction(
            &self,
            hash: &HashBytes,
        ) -> anyhow::Result<Option<Transaction>> {
            Ok(self.dst_transactions.get(hash).cloned())
        }
    }

    fn address(value: u8) -> StdAddr {
        StdAddr::new(0, HashBytes::from([value; 32]))
    }

    fn empty_body() -> Cell {
        Cell::empty_cell()
    }

    fn internal_message(src: &StdAddr, dst: &StdAddr, body: Cell) -> OwnedMessage {
        OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                src: IntAddr::Std(src.clone()),
                dst: IntAddr::Std(dst.clone()),
                ..Default::default()
            }),
            init: None,
            body: body.into(),
            layout: None,
        }
    }

    fn event_message(src: &StdAddr, body: Cell) -> OwnedMessage {
        OwnedMessage {
            info: MsgInfo::ExtOut(ExtOutMsgInfo {
                src: IntAddr::Std(src.clone()),
                ..Default::default()
            }),
            init: None,
            body: body.into(),
            layout: None,
        }
    }

    fn message_cell(message: &OwnedMessage) -> Cell {
        CellBuilder::build_from(message).unwrap()
    }

    fn message_hash(message: &OwnedMessage) -> HashBytes {
        *message_cell(message).repr_hash()
    }

    fn transaction(
        account: &StdAddr,
        input: Option<&OwnedMessage>,
        out: &[OwnedMessage],
    ) -> Transaction {
        let mut out_msgs = Dict::<Uint15, Cell>::new();
        for (index, message) in out.iter().enumerate() {
            let index = Uint15::new(index.try_into().unwrap());
            out_msgs.set(index, message_cell(message)).unwrap();
        }

        Transaction {
            account: account.address,
            lt: 0,
            prev_trans_hash: HashBytes::ZERO,
            prev_trans_lt: 0,
            now: 0,
            out_msg_count: Uint15::new(out.len().try_into().unwrap()),
            orig_status: AccountStatus::Active,
            end_status: AccountStatus::Active,
            in_msg: input.map(message_cell),
            out_msgs,
            total_fees: CurrencyCollection::ZERO,
            state_update: Lazy::from_raw(Cell::empty_cell()).unwrap(),
            info: Lazy::from_raw(Cell::empty_cell()).unwrap(),
        }
    }

    fn polling() -> TraceTransactionPolling {
        TraceTransactionPolling {
            min_interval: Duration::from_millis(1),
            max_interval: Duration::from_millis(1),
            timeout: Some(Duration::from_millis(1)),
        }
    }

    #[tokio::test]
    async fn finds_transaction_by_account_and_event() {
        let wallet = address(1);
        let target = address(2);
        let event = Event::builder(AbiVersion::V2_0, "Done").build();
        let body = event.encode_internal_input(&[]).unwrap().build().unwrap();
        let event_out = event_message(&target, body);
        let call = internal_message(&wallet, &target, empty_body());
        let root = transaction(&wallet, None, std::slice::from_ref(&call));
        let child = transaction(&target, Some(&call), std::slice::from_ref(&event_out));
        let root_hash = transaction_hash(&root).unwrap();
        let call_hash = message_hash(&call);

        let transport = TestTransport {
            transactions: HashMap::from([(root_hash, root)]),
            dst_transactions: HashMap::from([(call_hash, child)]),
        };

        let mut trace =
            TraceTransaction::new(&root_hash, Arc::new(transport)).with_polling(polling());
        let (step, event) = trace
            .find_by_account_and_event(&target, &event)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(step.depth, 1);
        assert!(step.account_matches(&target));
        assert_eq!(event.message.source(), Some(&target));
        assert!(event.tokens.is_empty());
    }

    #[tokio::test]
    async fn times_out_waiting_for_missing_child_transaction() {
        let wallet = address(1);
        let target = address(2);
        let call = internal_message(&wallet, &target, empty_body());
        let root = transaction(&wallet, None, std::slice::from_ref(&call));
        let root_hash = transaction_hash(&root).unwrap();
        let transport = TestTransport {
            transactions: HashMap::from([(root_hash, root)]),
            dst_transactions: HashMap::new(),
        };

        let mut trace =
            TraceTransaction::new(&root_hash, Arc::new(transport)).with_polling(polling());
        let error = trace.next_step().await.unwrap_err();

        assert!(matches!(
            error,
            TraceTransactionError::DestinationTransactionTimeout { .. }
        ));
    }

    #[tokio::test]
    async fn can_yield_root_step() {
        let wallet = address(1);
        let root = transaction(&wallet, None, &[]);
        let root_hash = transaction_hash(&root).unwrap();
        let transport = TestTransport {
            transactions: HashMap::from([(root_hash, root)]),
            dst_transactions: HashMap::new(),
        };

        let mut trace = TraceTransaction::new(&root_hash, Arc::new(transport))
            .include_root()
            .with_polling(polling());
        let step = trace.next_step().await.unwrap().unwrap();

        assert_eq!(step.depth, 0);
        assert_eq!(step.parent, None);
        assert!(step.account_matches(&wallet));
    }
}
