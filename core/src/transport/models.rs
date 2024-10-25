use everscale_types::models::*;
use everscale_types::prelude::*;
use serde::{Deserialize, Serialize};

use nekoton_utils::serde_helpers;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ContractState {
    NotExists {
        timings: GenTimings,
    },
    #[serde(rename_all = "camelCase")]
    Exists {
        #[serde(deserialize_with = "deserialize_account")]
        account: Box<Account>,
        timings: GenTimings,
        last_transaction_id: LastTransactionId,
    },
    Unchanged {
        timings: GenTimings,
    },
}

fn deserialize_account<'de, D>(deserializer: D) -> Result<Box<Account>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use everscale_types::cell::Load;
    use serde::de::Error;

    fn read_account(cell: Cell) -> Result<Box<Account>, everscale_types::error::Error> {
        let s = &mut cell.as_slice()?;
        Ok(Box::new(Account {
            address: <_>::load_from(s)?,
            storage_stat: <_>::load_from(s)?,
            last_trans_lt: <_>::load_from(s)?,
            balance: <_>::load_from(s)?,
            state: <_>::load_from(s)?,
            init_code_hash: if s.is_data_empty() {
                None
            } else {
                Some(<_>::load_from(s)?)
            },
        }))
    }

    Boc::deserialize(deserializer).and_then(|cell| read_account(cell).map_err(Error::custom))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTimings {
    #[serde(with = "serde_helpers::string")]
    pub gen_lt: u64,
    pub gen_utime: u32,
}

#[derive(Deserialize)]
pub struct LastTransactionId {
    #[serde(with = "serde_helpers::string")]
    pub lt: u64,
    pub hash: HashBytes,
}
