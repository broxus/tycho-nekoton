use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::Result;
use nekoton_core::models::{ContractState, LatestBlockchainConfig};
use nekoton_utils::serde_helpers::*;
use nekoton_utils::time::Timings;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tycho_types::cell::Cell;
use tycho_types::models::*;
use tycho_types::prelude::*;

#[derive(Clone)]
pub struct JrpcClient {
    client: reqwest::Client,
    endpoint: Arc<String>,
}

impl JrpcClient {
    pub async fn post<Q, R>(&self, data: &Q) -> Result<R>
    where
        Q: Serialize,
        for<'de> R: Deserialize<'de>,
    {
        let response = self
            .client
            .post(self.endpoint.as_str())
            .json(data)
            .send()
            .await?;

        let res = response.text().await?;
        match serde_json::from_str(&res)? {
            JrpcResponse::Success(res) => Ok(res),
            JrpcResponse::Err(err) => anyhow::bail!(err),
        }
    }
}

impl JrpcClient {
    pub(crate) fn new(endpoint: Url, client: reqwest::Client) -> Self {
        JrpcClient {
            client,
            endpoint: Arc::new(endpoint.to_string()),
        }
    }

    pub(crate) fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
}

impl JrpcClient {
    pub async fn send_message(&self, message: &OwnedMessage) -> Result<()> {
        let message_cell = CellBuilder::build_from(message)?;
        #[derive(Serialize)]
        struct Params<'a> {
            #[serde(with = "Boc")]
            message: &'a DynCell,
        }

        self.post(&JrpcRequest {
            method: "sendMessage",
            params: &Params {
                message: message_cell.as_ref(),
            },
        })
        .await
    }

    pub async fn get_dst_transaction(
        &self,
        message_hash: HashBytes,
    ) -> Result<Option<Transaction>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            message_hash: HashBytes,
        }

        let transaction_boc = self
            .post::<_, Option<String>>(&JrpcRequest {
                method: "getDstTransaction",
                params: &Params { message_hash },
            })
            .await?;

        match transaction_boc {
            None => Ok(None),
            Some(transaction_boc) => {
                let transaction = BocRepr::decode_base64(transaction_boc.as_str())?;
                Ok(Some(transaction))
            }
        }
    }

    pub async fn get_timings(&self) -> Result<Timings> {
        let request = JrpcRequest {
            method: "getTimings",
            params: &(),
        };

        self.post::<_, Timings>(&request).await
    }

    pub async fn get_contract_state(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
    ) -> anyhow::Result<ContractState> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params<'a> {
            address: &'a StdAddr,
            #[serde(default, with = "serde_optional_u64")]
            last_transaction_lt: Option<u64>,
        }

        self.post(&JrpcRequest {
            method: "getContractState",
            params: &Params {
                address,
                last_transaction_lt,
            },
        })
        .await
    }

    pub async fn get_config(&self) -> Result<LatestBlockchainConfig> {
        self.post(&JrpcRequest {
            method: "getBlockchainConfig",
            params: &(),
        })
        .await
    }

    pub async fn get_capabilities(&self) -> Result<Vec<String>> {
        self.post(&JrpcRequest {
            method: "getCapabilities",
            params: &(),
        })
        .await
    }

    pub async fn get_latest_key_block(&self) -> Result<Block> {
        #[derive(Deserialize)]
        struct Response {
            #[serde(with = "BocRepr")]
            block: Block,
        }

        let response = self
            .post::<_, Response>(&JrpcRequest {
                method: "getLatestKeyBlock",
                params: &(),
            })
            .await?;

        Ok(response.block)
    }

    pub async fn get_library_cell(&self, hash: &HashBytes) -> Result<Option<Cell>> {
        #[derive(Serialize)]
        struct Params<'a> {
            hash: &'a HashBytes,
        }

        #[derive(Deserialize)]
        struct Response {
            cell: Option<String>,
        }

        let response = self
            .post::<_, Response>(&JrpcRequest {
                method: "getLibraryCell",
                params: &Params { hash },
            })
            .await?;

        response
            .cell
            .map(|cell| Boc::decode_base64(cell.as_str()).map_err(Into::into))
            .transpose()
    }

    pub async fn get_transactions(
        &self,
        address: &StdAddr,
        last_transaction_lt: Option<u64>,
        limit: u8,
    ) -> Result<Vec<Transaction>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params<'a> {
            account: &'a StdAddr,
            #[serde(default, with = "serde_optional_u64")]
            last_transaction_lt: Option<u64>,
            limit: u8,
        }

        let transactions = self
            .post::<_, Vec<String>>(&JrpcRequest {
                method: "getTransactionsList",
                params: &Params {
                    account: address,
                    last_transaction_lt,
                    limit,
                },
            })
            .await?;

        transactions
            .iter()
            .map(|transaction| BocRepr::decode_base64(transaction.as_str()).map_err(Into::into))
            .collect()
    }

    pub async fn get_accounts_by_code_hash(
        &self,
        code_hash: &HashBytes,
        continuation: Option<&StdAddr>,
        limit: u8,
    ) -> Result<Vec<StdAddr>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params<'a> {
            code_hash: &'a HashBytes,
            continuation: Option<&'a StdAddr>,
            limit: u8,
        }

        self.post(&JrpcRequest {
            method: "getAccountsByCodeHash",
            params: &Params {
                code_hash,
                continuation,
                limit,
            },
        })
        .await
    }

    pub async fn get_transaction(&self, hash: &HashBytes) -> Result<Option<Transaction>> {
        #[derive(Serialize)]
        struct Params<'a> {
            #[serde(default, with = "serde_hex_array")]
            id: &'a HashBytes,
        }
        let params = JrpcRequest {
            method: "getTransaction",
            params: &Params { id: hash },
        };

        let transaction_boc = self.post::<_, Option<String>>(&params).await?;

        match transaction_boc {
            None => Ok(None),
            Some(transaction_boc) => {
                let transaction = BocRepr::decode_base64(transaction_boc.as_str())?;
                Ok(Some(transaction))
            }
        }
    }
}

struct JrpcRequest<'a, T> {
    method: &'a str,
    params: &'a T,
}

impl<T: Serialize> Serialize for JrpcRequest<'_, T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut ser = serializer.serialize_struct("JrpcRequest", 4)?;
        ser.serialize_field("jsonrpc", "2.0")?;
        ser.serialize_field("id", &1)?;
        ser.serialize_field("method", self.method)?;
        ser.serialize_field("params", self.params)?;
        ser.end()
    }
}

enum JrpcResponse<T> {
    Success(T),
    Err(Box<serde_json::value::RawValue>),
}

impl<'de, T> Deserialize<'de> for JrpcResponse<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Field {
            Result,
            Error,
            #[serde(other)]
            Other,
        }

        enum ResponseData<T> {
            Result(T),
            Error(Box<serde_json::value::RawValue>),
        }

        struct ResponseVisitor<T>(PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for ResponseVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = ResponseData<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON-RPC response object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut result = None::<ResponseData<T>>;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Result if result.is_none() => {
                            result = Some(map.next_value().map(ResponseData::Result)?);
                        }
                        Field::Error if result.is_none() => {
                            result = Some(map.next_value().map(ResponseData::Error)?);
                        }
                        Field::Other => {
                            map.next_value::<&serde_json::value::RawValue>()?;
                        }
                        Field::Result => return Err(serde::de::Error::duplicate_field("result")),
                        Field::Error => return Err(serde::de::Error::duplicate_field("error")),
                    }
                }

                result.ok_or_else(|| serde::de::Error::missing_field("result or error"))
            }
        }

        Ok(match de.deserialize_map(ResponseVisitor(PhantomData))? {
            ResponseData::Result(result) => JrpcResponse::Success(result),
            ResponseData::Error(error) => JrpcResponse::Err(error),
        })
    }
}
