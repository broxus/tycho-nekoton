use std::sync::Arc;

use anyhow::{Context, Result};
use nekoton_core::models::{ContractState, GenTimings, LastTransactionId, LatestBlockchainConfig};
use nekoton_utils::time::Timings;
use prost::Message;
use reqwest::{StatusCode, Url};
use tycho_types::boc::BocRepr;
use tycho_types::cell::HashBytes;
use tycho_types::models::{Account, BlockchainConfig, OwnedMessage, StdAddr, Transaction};
use tycho_types::prelude::{Cell, Load};

use crate::rpc::proto_rpc as rpc;

#[derive(Clone)]
pub struct ProtoClient {
    client: reqwest::Client,
    endpoint: Arc<String>,
}

impl ProtoClient {
    pub(crate) fn new(endpoint: Url, client: reqwest::Client) -> Self {
        Self {
            client,
            endpoint: Arc::new(endpoint.to_string()),
        }
    }

    pub(crate) fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    async fn post<Req, Res>(&self, request: Req) -> Result<Res>
    where
        Req: Message,
        Res: Message + Default,
    {
        let response = self
            .client
            .post(self.endpoint.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .body(request.encode_to_vec())
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                Res::decode(response.bytes().await?).context("failed to decode protobuf response")
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                let error = rpc::Error::decode(response.bytes().await?)
                    .context("failed to decode protobuf error response")?;
                anyhow::bail!(error.message)
            }
            status => anyhow::bail!(status.to_string()),
        }
    }

    pub async fn send_message(&self, message: &OwnedMessage) -> Result<()> {
        let message = BocRepr::encode(message)?;
        let request = rpc::Request {
            call: Some(rpc::request::Call::SendMessage(rpc::request::SendMessage {
                message: message.into(),
            })),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::SendMessage(_)) => Ok(()),
            _ => anyhow::bail!("invalid protobuf response for sendMessage"),
        }
    }

    pub async fn get_dst_transaction(
        &self,
        message_hash: HashBytes,
    ) -> Result<Option<Transaction>> {
        let request = rpc::Request {
            call: Some(rpc::request::Call::GetDstTransaction(
                rpc::request::GetDstTransaction {
                    message_hash: message_hash.as_slice().to_vec().into(),
                },
            )),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::GetRawTransaction(tx)) => {
                tx.transaction.map(decode_transaction).transpose()
            }
            _ => anyhow::bail!("invalid protobuf response for getDstTransaction"),
        }
    }

    pub async fn get_timings(&self) -> Result<Timings> {
        let request = rpc::Request {
            call: Some(rpc::request::Call::GetTimings(rpc::Empty {})),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::GetTimings(timings)) => Ok(parse_timings(timings)),
            _ => anyhow::bail!("invalid protobuf response for getTimings"),
        }
    }

    pub async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> Result<ContractState> {
        let request = rpc::Request {
            call: Some(rpc::request::Call::GetContractState(
                rpc::request::GetContractState {
                    address: addr_to_bytes(address),
                    last_transaction_lt,
                },
            )),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::GetContractState(state)) => parse_contract_state(state),
            _ => anyhow::bail!("invalid protobuf response for getContractState"),
        }
    }

    pub async fn get_config(&self) -> Result<LatestBlockchainConfig> {
        let request = rpc::Request {
            call: Some(rpc::request::Call::GetBlockchainConfig(rpc::Empty {})),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::GetBlockchainConfig(response)) => {
                let config: BlockchainConfig = BocRepr::decode(response.config.as_ref())
                    .context("failed to decode blockchain config boc")?;
                Ok(LatestBlockchainConfig {
                    global_id: response.global_id,
                    seqno: response.seqno,
                    config,
                })
            }
            _ => anyhow::bail!("invalid protobuf response for getBlockchainConfig"),
        }
    }

    pub async fn get_transaction(&self, hash: &HashBytes) -> Result<Option<Transaction>> {
        let request = rpc::Request {
            call: Some(rpc::request::Call::GetTransaction(
                rpc::request::GetTransaction {
                    id: hash.as_slice().to_vec().into(),
                },
            )),
        };

        let response = self.post::<_, rpc::Response>(request).await?;
        match response.result {
            Some(rpc::response::Result::GetRawTransaction(tx)) => {
                tx.transaction.map(decode_transaction).transpose()
            }
            _ => anyhow::bail!("invalid protobuf response for getTransaction"),
        }
    }
}

fn decode_transaction(transaction: prost::bytes::Bytes) -> Result<Transaction> {
    BocRepr::decode(transaction.as_ref()).context("failed to decode transaction boc")
}

fn parse_contract_state(response: rpc::response::GetContractState) -> Result<ContractState> {
    use rpc::response::get_contract_state::State;

    match response.state.context("missing contract state")? {
        State::NotExists(state) => Ok(ContractState::NotExists {
            timings: parse_not_exists_timings(state)?,
        }),
        State::Exists(state) => Ok(ContractState::Exists {
            account: decode_account(state.account)?,
            timings: parse_gen_timings(
                state
                    .gen_timings
                    .context("missing timings for existing contract state")?,
            ),
            last_transaction_id: parse_last_transaction_id(
                state
                    .last_transaction_id
                    .context("missing last transaction id for existing contract state")?,
            )?,
        }),
        State::Unchanged(timings) => Ok(ContractState::Unchanged {
            timings: parse_gen_timings(timings),
        }),
    }
}

fn parse_not_exists_timings(
    state: rpc::response::get_contract_state::NotExists,
) -> Result<GenTimings> {
    use rpc::response::get_contract_state::not_exists::GenTimings;

    match state
        .gen_timings
        .context("missing timings for not existing contract state")?
    {
        GenTimings::Known(timings) => Ok(parse_gen_timings(timings)),
        // The current transport model has no way to preserve "unknown timings".
        // Rejecting this variant is safer than fabricating zero values.
        GenTimings::Unknown(_) => anyhow::bail!("unknown timings are not supported"),
    }
}

fn parse_last_transaction_id(
    last_transaction_id: rpc::response::get_contract_state::exists::LastTransactionId,
) -> Result<LastTransactionId> {
    use rpc::response::get_contract_state::exists::LastTransactionId as ProtoLastTransactionId;

    match last_transaction_id {
        ProtoLastTransactionId::Exact(exact) => Ok(LastTransactionId {
            lt: exact.lt,
            hash: HashBytes::from_slice(exact.hash.as_ref()),
        }),
        // `tycho-nekoton` currently stores an exact `(lt, hash)` pair only.
        // Treat an inexact proto result as a protocol mismatch instead of
        // silently degrading the state model.
        ProtoLastTransactionId::Inexact(_) => {
            anyhow::bail!("inexact last transaction id is not supported")
        }
    }
}

fn parse_gen_timings(timings: rpc::response::get_contract_state::Timings) -> GenTimings {
    GenTimings {
        gen_lt: timings.gen_lt,
        gen_utime: timings.gen_utime,
    }
}

fn parse_timings(timings: rpc::response::GetTimings) -> Timings {
    Timings {
        last_mc_block_seqno: timings.last_mc_block_seqno,
        last_mc_utime: timings.last_mc_utime,
        mc_time_diff: timings.mc_time_diff,
        smallest_known_lt: (timings.smallest_known_lt != 0).then_some(timings.smallest_known_lt),
    }
}

fn decode_account(account: prost::bytes::Bytes) -> Result<Box<Account>> {
    let cell: Cell = BocRepr::decode(account.as_ref()).context("failed to decode account boc")?;
    let s = &mut cell.as_slice().context("failed to create account slice")?;

    Ok(Box::new(Account {
        address: <_>::load_from(s)?,
        storage_stat: <_>::load_from(s)?,
        last_trans_lt: <_>::load_from(s)?,
        balance: <_>::load_from(s)?,
        state: <_>::load_from(s)?,
    }))
}

fn addr_to_bytes(address: &StdAddr) -> prost::bytes::Bytes {
    let mut bytes = Vec::with_capacity(33);
    bytes.push(address.workchain as u8);
    bytes.extend_from_slice(address.address.as_slice());
    bytes.into()
}

#[cfg(test)]
mod tests {
    use prost::bytes::Bytes;

    use super::*;

    #[test]
    fn parses_proto_timings() {
        let timings = parse_timings(rpc::response::GetTimings {
            last_mc_block_seqno: 10,
            last_mc_utime: 20,
            mc_time_diff: -30,
            smallest_known_lt: 40,
        });

        assert_eq!(timings.last_mc_block_seqno, 10);
        assert_eq!(timings.last_mc_utime, 20);
        assert_eq!(timings.mc_time_diff, -30);
        assert_eq!(timings.smallest_known_lt, Some(40));
    }

    #[test]
    fn rejects_unknown_not_exists_timings() {
        let state = rpc::response::get_contract_state::NotExists {
            gen_timings: Some(
                rpc::response::get_contract_state::not_exists::GenTimings::Unknown(rpc::Empty {}),
            ),
        };

        let error = parse_not_exists_timings(state).unwrap_err();
        assert!(error.to_string().contains("unknown timings"));
    }

    #[test]
    fn rejects_inexact_last_transaction_id() {
        let last_transaction_id =
            rpc::response::get_contract_state::exists::LastTransactionId::Inexact(
                rpc::response::get_contract_state::exists::Inexact { latest_lt: 42 },
            );

        let error = parse_last_transaction_id(last_transaction_id)
            .err()
            .expect("expected inexact last transaction id to fail");
        assert!(error.to_string().contains("inexact"));
    }

    #[test]
    fn converts_exact_last_transaction_id() {
        let last_transaction_id =
            rpc::response::get_contract_state::exists::LastTransactionId::Exact(
                rpc::response::get_contract_state::exists::Exact {
                    lt: 42,
                    hash: Bytes::from_static(&[7; 32]),
                },
            );

        let last_transaction_id = parse_last_transaction_id(last_transaction_id).unwrap();
        assert_eq!(last_transaction_id.lt, 42);
        assert_eq!(last_transaction_id.hash.as_slice(), &[7; 32]);
    }

    #[test]
    fn encodes_std_addr_as_proto_bytes() {
        let address = StdAddr::new(-1, HashBytes::from([3; 32]));
        let bytes = addr_to_bytes(&address);

        assert_eq!(bytes.len(), 33);
        assert_eq!(bytes[0], 255);
        assert_eq!(&bytes[1..], &[3; 32]);
    }
}
