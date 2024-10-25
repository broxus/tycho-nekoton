use everscale_types::models::*;
use everscale_types::prelude::*;
use nekoton_core::transport::{ContractState, Transport};

#[derive(Clone)]
pub struct JrpcClient {
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl Transport for JrpcClient {
    async fn broadcast_message(&self, message: &DynCell) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_contract_state(&self, address: &StdAddr) -> anyhow::Result<ContractState> {
        todo!()
    }
}
