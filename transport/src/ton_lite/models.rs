use anyhow::Context;
use everscale_types::cell::{Cell, CellSlice, HashBytes, Load};
use everscale_types::dict::Dict;
use everscale_types::error::Error;
use everscale_types::models::{BlockchainConfig, CurrencyCollection, ShardAccounts, ShardHashes};
use nekoton_core::models::{GenTimings, LastTransactionId};

pub(crate) struct ParsedProofs {
    pub timings: GenTimings,
    pub state_root: Cell,
}

impl ParsedProofs {
    pub(crate) fn get_last_transaction_id(
        &self,
        account: &HashBytes,
    ) -> anyhow::Result<LastTransactionId> {
        type ShardAccountsShort = Dict<HashBytes, TonShardAccountShort>;

        let proof = self
            .state_root
            .parse::<TonShardStateShort>()
            .context("invalid state proof")?;

        let accounts = proof
            .accounts
            .parse::<ShardAccounts>()
            .context("failed to parse shard accounts")?;
        let accounts = ShardAccountsShort::from_raw(accounts.dict().root().clone());

        let Some(state) = accounts.get(account).context("failed to get tx id")? else {
            anyhow::bail!("account state not found");
        };

        Ok(LastTransactionId {
            hash: state.last_trans_hash,
            lt: state.last_trans_lt,
        })
    }
}

#[derive(Load)]
#[tlb(tag = "#cc26")]
pub(crate) struct TonMcStateExtraShort {
    _shard_hashes: ShardHashes,
    pub config: BlockchainConfig,
}

#[derive(Load)]
#[tlb(tag = "#9023afe2")]
pub(crate) struct TonShardStateShort {
    _out_msg_queue_info: Cell,
    accounts: Cell,
}

pub(crate) struct TonShardAccountShort {
    last_trans_hash: HashBytes,
    last_trans_lt: u64,
}

impl<'a> Load<'a> for TonShardAccountShort {
    fn load_from(slice: &mut CellSlice<'a>) -> anyhow::Result<Self, Error> {
        // Skip `split_depth`
        slice.skip_first(5, 0)?;
        // Skip balance.
        _ = CurrencyCollection::load_from(slice)?;
        // Skip account.
        Cell::load_from(slice)?;

        Ok(Self {
            last_trans_hash: slice.load_u256()?,
            last_trans_lt: slice.load_u64()?,
        })
    }
}
