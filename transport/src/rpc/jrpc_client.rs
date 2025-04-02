use std::marker::PhantomData;
use std::sync::Arc;

use everscale_types::models::*;
use everscale_types::prelude::*;
use nekoton_core::transport::{ContractState, LatestBlockchainConfig};
use nekoton_utils::serde_helpers::*;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::models::Timings;

#[derive(Clone)]
pub struct JrpcClient {
    client: reqwest::Client,
    endpoint: Arc<String>,
}

impl JrpcClient {
    pub async fn post<Q, R>(&self, data: &Q) -> anyhow::Result<R>
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
    pub async fn broadcast_message(&self, message: &OwnedMessage) -> anyhow::Result<()> {
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

    pub async fn get_timings(&self) -> anyhow::Result<Timings> {
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

    pub async fn get_config(&self) -> anyhow::Result<LatestBlockchainConfig> {
        self.post(&JrpcRequest {
            method: "getBlockchainConfig",
            params: &(),
        })
        .await
    }
}

struct JrpcRequest<'a, T> {
    method: &'a str,
    params: &'a T,
}

impl<T: Serialize> Serialize for JrpcRequest<'_, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
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
    fn deserialize<D>(de: D) -> std::result::Result<Self, D::Error>
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
