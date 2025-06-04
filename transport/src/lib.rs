pub mod options;
pub mod rpc;

#[cfg(test)]
pub mod tests {
    use crate::rpc::RpcTransport;
    use everscale_types::cell::HashBytes;
    use futures_util::StreamExt;
    use nekoton_core::transactions::TraceTransaction;
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
