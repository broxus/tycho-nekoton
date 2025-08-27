pub mod options;
pub mod rpc;

#[cfg(test)]
pub mod tests {
    use crate::rpc::RpcTransport;
    use core::panic;
    use futures_util::StreamExt;
    use nekoton_core::contracts::blockchain_context::BlockchainContextBuilder;
    use nekoton_core::transactions::TraceTransaction;
    use nekoton_core::transport::{SimpleTransport, Transport};
    use num_bigint::BigUint;
    use num_traits::Zero;
    use reqwest::Url;
    use std::str::FromStr;
    use std::sync::Arc;
    use tycho_types::abi::{AbiHeaderType, AbiType, AbiValue, AbiVersion, Function};
    use tycho_types::boc::Boc;
    use tycho_types::cell::HashBytes;
    use tycho_types::models::BlockchainConfig;

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

    #[tokio::test]
    async fn get_state_with_retries_for_libraries_when_contract_code_is_library() {
        let config_cell = Boc::decode_base64("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").unwrap();
        let config = config_cell.parse::<BlockchainConfig>().unwrap();

        let account_cell = Boc::decode_base64("te6ccgEBBAEA3wACboARPmKFmZb7UI1FgPY5MbJYJKPzmu4RUDN9k8WayPe7kGQRAqUGil+P0AAAT3BmG1o6AvrwgCYDAQGTAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMAIurEomVjZY/EfuvEyNXOvBmybt5bgpnmtwgyymLnInvgCAGOACOx2tye/5lfR4Ih+BBGfLQA67cTHekeUzHx9YuGo73UAAAAAAAAAAAAAAAAAAAABUAhCApZ8lwbZVdfn7LyATToKdb89MzDWfAhUcDRsBzsioiQS").unwrap();

        let inputs = vec![AbiType::Uint(32).named("answerId")];

        let outputs = vec![AbiType::Uint(128).named("value0")];

        let headers = vec![
            AbiHeaderType::PublicKey,
            AbiHeaderType::Time,
            AbiHeaderType::Expire,
        ];
        let function = Function::builder(AbiVersion::V2_3, "balance")
            .with_headers(headers)
            .with_inputs(inputs)
            .with_outputs(outputs)
            .build();

        let transport = SimpleTransport::new(vec![], config.clone()).unwrap();

        let context = BlockchainContextBuilder::new()
            .with_config(config)
            .with_transport(Arc::new(transport))
            .build()
            .unwrap();

        let mut account = context
            .get_account_from_cell(account_cell.as_ref())
            .unwrap();

        let values = vec![AbiValue::Uint(32, BigUint::zero()).named("answerId")];

        let urls = vec![Url::from_str("https://rpc-testnet.tychoprotocol.com/").unwrap()];
        let rpc_transport = RpcTransport::new(urls, Default::default(), false)
            .await
            .unwrap();

        loop {
            match account.run_local(&function, values.as_slice()) {
                Ok(output) if output.exit_code == 1 || output.exit_code == 0 => {
                    println!("output: {output:?}");
                    break;
                }
                Ok(output) => {
                    if let Some(missing_library) = output.missing_library {
                        if let Some(library_cell) = rpc_transport
                            .get_library_cell(&missing_library)
                            .await
                            .unwrap()
                        {
                            account.add_library(missing_library, library_cell).unwrap();
                        } else {
                            panic!("library not found");
                        }
                    } else {
                        panic!("unexpected exit code: {}", output.exit_code);
                    }
                }
                Err(e) => panic!("error: {e:?}"),
            }
        }
    }

    #[tokio::test]
    async fn get_state_with_retries_for_libraries_when_library_call_is_inside_contract() {
        let config_cell = Boc::decode_base64("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").unwrap();
        let config = config_cell.parse::<BlockchainConfig>().unwrap();

        let account_cell = Boc::decode_base64("te6ccgECGAEAAwwAAm6AEIILpzALwEYKeX5W1GHHiStoLPHdkKlh9K2SRRV6uS6kYQo4horspSAAAFC2X/KIKlloLwAmAwEBkb1NrLsMcFKYoHq5uXkbVJzUkOEiypI/caoqHm/AFDeIAAABmOrGPYaAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACMACCEICTunDKDBYzp1WMjhzkahaIKI373nqkQCMY/dQTQhEVEoBEv8A9KQT9LzyCwQCASASBQOe8n+J+Gkh2zzTAAGOFIMI1xgg+CjIzs7J+QBY+EL5EPKo3tM/AfhDIbnytCD4I4ED6KiCCBt3QKC58rT4Y9Mf+CNYufK50x8B9KQg9KHyPBEWBgIBSAwHAgEgCggCb7qtnHw/hG8uBM0//U0dDT/9HbPCGOHCPQ0wH6QDAxyM+HIM6CEOrZx8PPC4HL/8lw+wCRMOLbPICQ8AGPhK0O0eWYEdUFUC2AJvu0pKWt+Eby4EzT/9TR0NP/0ds8IY4cI9DTAfpAMDHIz4cgzoIQ1KSlrc8Lgcv/yXD7AJEw4ts8gLFAEkcPgA+ErQ7R5aiYEyhlUD2PhrEQIBIA4NAj+7yR4cX4Qm7jAPhG8nPT/9H4AMjPhArL/3/PI/hq2zyBYUAnu6WhFNz4RvLgTNP/1NHQ0//R2zwijiIk0NMB+kAwMcjPhyDOcc8LYQLIz5IWhFNyy//L/83JcPsAkVvi2zyBAPACjtRNDT/9M/MfhDWMjL/8s/zsntVAAc+AD4StDtHlmBP/5VAtgAQ4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABADZNJw7UTQgQFA1yHXCgD4ZiLQ0wP6QDD4aak4ANwhxwDjAiHXDR/yvCHjAwH0pCD0ofI8FxcTAyqgTOUUevhG8uBM2zzT/9P/0ds82zwWFRQAKvhL+Er4Q/hCyMv/yz/Pg8zL/8ntVAAyghCy0F4AcvsC+ErQ7R5Z+EmBMoZVA9j4awAu7UTQ0//TP9MA1NP/0fhr+Gr4Zvhj+GIACvhG8uBM").unwrap();

        let inputs = vec![AbiType::Uint(256).named("a"), AbiType::Uint(256).named("b")];

        let outputs = vec![AbiType::Uint(256).named("value0")];

        let headers = vec![AbiHeaderType::Time, AbiHeaderType::Expire];
        let function = Function::builder(AbiVersion::V2_3, "testAddGetter")
            .with_headers(headers)
            .with_inputs(inputs)
            .with_outputs(outputs)
            .build();

        let transport = SimpleTransport::new(vec![], config.clone()).unwrap();

        let context = BlockchainContextBuilder::new()
            .with_config(config)
            .with_transport(Arc::new(transport))
            .build()
            .unwrap();

        let mut account = context
            .get_account_from_cell(account_cell.as_ref())
            .unwrap();

        let values = vec![
            AbiValue::Uint(256, BigUint::from(1u32)).named("a"),
            AbiValue::Uint(256, BigUint::from(1u32)).named("b"),
        ];

        let urls = vec![Url::from_str("https://rpc-testnet.tychoprotocol.com/").unwrap()];
        let rpc_transport = RpcTransport::new(urls, Default::default(), false)
            .await
            .unwrap();

        loop {
            match account.run_local(&function, values.as_slice()) {
                Ok(output) if output.exit_code == 1 || output.exit_code == 0 => {
                    println!("output: {output:?}");
                    break;
                }
                Ok(output) => {
                    if let Some(missing_library) = output.missing_library {
                        if let Some(library_cell) = rpc_transport
                            .get_library_cell(&missing_library)
                            .await
                            .unwrap()
                        {
                            account.add_library(missing_library, library_cell).unwrap();
                        } else {
                            panic!("library not found");
                        }
                    } else {
                        panic!("unexpected exit code: {}", output.exit_code);
                    }
                }
                Err(e) => panic!("error: {e:?}"),
            }
        }
    }
}
