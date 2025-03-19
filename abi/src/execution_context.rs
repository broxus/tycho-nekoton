use crate::function_ext::{ExecutionOutput, FunctionExt};
use everscale_types::abi::{Function, NamedAbiValue};
use everscale_types::cell::HashBytes;
use everscale_types::models::{
    Account, BlockchainConfig, ExtInMsgInfo, IntAddr, IntMsgInfo, LibDescr, MsgInfo, OwnedMessage,
};
use everscale_types::prelude::{Cell, CellBuilder, CellFamily, Dict};
use nekoton_utils::time::{Clock, SimpleClock};
use tycho_vm::OwnedCellSlice;

#[derive(Clone)]
pub struct ExecutionContext<'a> {
    clock: &'a dyn Clock,
    rand_seed: HashBytes,
    libraries: Dict<HashBytes, LibDescr>,
    account: Account,
}

impl<'a> ExecutionContext<'a> {
    pub fn run_local(
        &mut self,
        function: &Function,
        values: &[NamedAbiValue],
        config: BlockchainConfig,
    ) -> anyhow::Result<ExecutionOutput> {
        function.run_local(
            &mut self.account,
            values,
            self.clock,
            false,
            self.rand_seed,
            &self.libraries,
            config,
        )
    }

    pub fn run_local_responsible(
        &mut self,
        function: &Function,
        values: &[NamedAbiValue],
        config: BlockchainConfig,
    ) -> anyhow::Result<ExecutionOutput> {
        function.run_local(
            &mut self.account,
            values,
            self.clock,
            true,
            self.rand_seed,
            &self.libraries,
            config,
        )
    }
}

pub struct ExecutionContextBuilder<'a> {
    pub clock: Option<&'a dyn Clock>,
    pub rand_seed: Option<HashBytes>,
    pub libraries: Dict<HashBytes, LibDescr>,
    account: Account,
}

impl<'a> ExecutionContextBuilder<'a> {
    pub fn new(account: &'a Account) -> ExecutionContextBuilder {
        Self {
            clock: None,
            rand_seed: None,
            libraries: Dict::default(),
            account: account.clone(),
        }
    }

    pub fn with_clock(mut self, clock: &'a dyn Clock) -> Self {
        self.clock = Some(clock);
        self
    }
    pub fn with_rand_seed(mut self, rand_seed: HashBytes) -> Self {
        self.rand_seed = Some(rand_seed);
        self
    }

    pub fn with_libraries(mut self, libraries: Dict<HashBytes, LibDescr>) -> Self {
        self.libraries = libraries;
        self
    }

    pub fn build(self) -> ExecutionContext<'a> {
        ExecutionContext {
            clock: self.clock.unwrap_or(&SimpleClock),
            rand_seed: self.rand_seed.unwrap_or_default(),
            libraries: self.libraries,
            account: self.account.clone(),
        }
    }
}

pub struct MessageBuilder {
    pub info: MsgInfo,
    pub body: OwnedCellSlice,
}

impl MessageBuilder {
    pub fn new_internal_in(src: IntAddr, dst: IntAddr) -> Self {
        let info = MsgInfo::Int(IntMsgInfo {
            src,
            dst,
            ..Default::default()
        });
        Self {
            info,
            body: OwnedCellSlice::new_allow_exotic(Cell::empty_cell()), //Cell::empty_cell_ref().as_slice_allow_exotic()
        }
    }

    pub fn new_external_in(dst: IntAddr) -> Self {
        let info = MsgInfo::ExtIn(ExtInMsgInfo {
            dst,
            ..Default::default()
        });
        Self {
            info,
            body: OwnedCellSlice::new_allow_exotic(Cell::empty_cell()),
        }
    }

    pub fn with_body<T: IntoMessageBody>(mut self, body: T) -> anyhow::Result<Self> {
        self.body = body.into_message_body()?;
        Ok(self)
    }

    pub fn build(&self) -> OwnedMessage {
        OwnedMessage {
            info: self.info.clone(),
            init: None,
            body: self.body.clone().into(),
            layout: None,
        }
    }

    pub fn build_cell(&self) -> anyhow::Result<Cell> {
        let cell = CellBuilder::build_from(self.build())?;
        Ok(cell)
    }
}

trait IntoMessageBody {
    fn into_message_body(self) -> anyhow::Result<OwnedCellSlice>;
}

impl IntoMessageBody for CellBuilder {
    fn into_message_body(self) -> anyhow::Result<OwnedCellSlice> {
        let cell = self.build()?;
        Ok(OwnedCellSlice::new_allow_exotic(cell))
    }
}

impl IntoMessageBody for Cell {
    fn into_message_body(self) -> anyhow::Result<OwnedCellSlice> {
        Ok(OwnedCellSlice::new_allow_exotic(self))
    }
}
