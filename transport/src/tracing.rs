use anyhow::Result;
use everscale_types::cell::HashBytes;
use everscale_types::models::{MsgType, OwnedMessage, Transaction};
use everscale_types::prelude::Load;
use futures_util::{Future, Stream};
use nekoton_core::transport::Transport;
use pin_project::pin_project;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Mutex;

type NextTransactionFut = Option<Pin<Box<dyn Future<Output = Result<Option<Transaction>>> + Send>>>;

#[pin_project]
pub struct TraceTransaction {
    inner: Arc<Mutex<TraceTransactionState>>,
    #[pin]
    future: NextTransactionFut,
}

impl TraceTransaction {
    #[allow(unused)]
    pub fn new(root_hash: &HashBytes, transport: Arc<dyn Transport>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TraceTransactionState {
                transport,
                yield_root: false,
                root_hash: Some(*root_hash),
                //root: None,
                queue: Default::default(),
            })),
            future: None,
        }
    }
}

struct TraceTransactionState {
    transport: Arc<dyn Transport>,
    yield_root: bool,
    root_hash: Option<HashBytes>,
    //root: Option<Transaction>,
    queue: VecDeque<HashBytes>,
}

impl TraceTransactionState {
    fn extract_messages(tx: &Transaction, queue: &mut VecDeque<HashBytes>) -> Result<()> {
        let mut hashes = Vec::new();

        for m in tx.out_msgs.iter() {
            let (_, cell) = m?;
            let hash = cell.repr_hash();
            let mut cs = cell.as_slice()?;
            let message = OwnedMessage::load_from(&mut cs)?;
            if matches!(message.ty(), MsgType::Int) {
                hashes.push(*hash);
            }
        }

        queue.extend(hashes);
        Ok(())
    }

    async fn next(&mut self) -> Result<Option<Transaction>> {
        const MIN_INTERVAL_MS: u64 = 500;
        const MAX_INTERVAL_MS: u64 = 3000;
        const FACTOR: u64 = 2;

        let transport = self.transport.as_ref();

        if let Some(root_hash) = &self.root_hash {
            let Some(tx) = transport.get_transaction(root_hash).await? else {
                anyhow::bail!("Root transaction not found");
            };

            Self::extract_messages(&tx, &mut self.queue)?;

            self.root_hash = None;
            if std::mem::take(&mut self.yield_root) {
                return Ok(Some(tx));
            }
        }

        let Some(message_hash) = self.queue.front() else {
            return Ok(None);
        };

        let mut interval_ms = MIN_INTERVAL_MS;
        let tx = loop {
            if let Ok(Some(tx)) = transport.get_dst_transaction(message_hash).await {
                break tx;
            }

            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            interval_ms = std::cmp::min(interval_ms * FACTOR, MAX_INTERVAL_MS);
        };

        Self::extract_messages(&tx, &mut self.queue)?;
        self.queue.pop_front();

        Ok(Some(tx))
    }
}

impl Stream for TraceTransaction {
    type Item = Transaction;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                return match fut.poll(cx) {
                    Poll::Ready(result) => {
                        this.future.set(None);

                        match result {
                            Ok(Some(tx)) => Poll::Ready(Some(tx)),
                            Ok(None) => Poll::Ready(None), // Stream is done
                            Err(e) => {
                                println!("Error in TraceTransaction stream: {}", e);
                                Poll::Ready(None) //TODO: handle error?
                            }
                        }
                    }
                    Poll::Pending => Poll::Pending,
                };
            } else {
                let inner_clone = this.inner.clone();

                let future = Box::pin(async move {
                    let mut state = inner_clone.lock().await;
                    state.next().await
                });

                this.future.set(Some(future));
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::rpc::RpcTransport;
    use crate::tracing::TraceTransaction;
    use everscale_types::cell::HashBytes;
    use futures_util::StreamExt;
    use reqwest::Url;
    use std::str::FromStr;
    use std::sync::Arc;

    #[tokio::test]
    async fn traced_tx() {
        let hash =
            HashBytes::from_str("86c0523831d3be661339cd18be4714ec5d4501779aa6d05ac2b8bca785ddbf43")
                .unwrap();
        let urls = vec![Url::from_str("https://rpc.hamster.network/").unwrap()];
        let rpc_transport = RpcTransport::new(urls, Default::default(), false)
            .await
            .unwrap();

        let mut traced_tx = TraceTransaction::new(&hash, Arc::new(rpc_transport));
        let mut counter = 0;
        while traced_tx.next().await.is_some() {
            counter += 1;
        }
        assert_eq!(counter, 12);
    }
}
