use std::sync::Arc;

use crate::error::ExecutionError;
use crate::models::{ContractState, GenTimings};
use crate::transport::Transport;
use everscale_types::abi::{Function, NamedAbiValue};
use everscale_types::crc::crc_16;
use everscale_types::models::{
    Account, BlockchainConfig, ExtInMsgInfo, IntAddr, IntMsgInfo, MsgInfo, OwnedMessage, StdAddr,
    Transaction,
};
use everscale_types::prelude::{Cell, CellBuilder, CellFamily, DynCell};
use nekoton_utils::time::{Clock, SimpleClock};
use num_bigint::BigInt;
use tycho_executor::ExecutorParams;
use tycho_vm::{BehaviourModifiers, OwnedCellSlice, RcStackValue, SafeRc};

use super::function_ext::{ExecutionOutput, FunctionExt};
use super::local_vm::{LocalVmBuilder, VmGetterOutput};
use super::utils::get_gen_timings;

#[derive(Clone)]
pub struct BlockchainContext {
    desc: BlockchainDesc,
    transport: Arc<dyn Transport>,
    clock: Arc<dyn Clock>,
}

impl BlockchainContext {
    pub async fn get_account(self, address: &StdAddr) -> anyhow::Result<BlockchainAccount> {
        let state = self.transport.get_contract_state(address, None).await?;
        let account = match state {
            ContractState::Exists { account, .. } => account,
            ContractState::NotExists { .. } => anyhow::bail!("Account does not exist"),
            _ => unreachable!(),
        };

        Ok(BlockchainAccount {
            context: self,
            account: account.as_ref().clone(),
        })
    }

    pub fn get_account_from_cell(
        self,
        account_cell: &DynCell,
    ) -> anyhow::Result<BlockchainAccount> {
        let account = account_cell.parse::<Account>()?;
        Ok(BlockchainAccount {
            context: self,
            account,
        })
    }

    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    pub fn config(&self) -> &BlockchainConfig {
        &self.desc.config
    }

    pub fn executor_params(&self) -> &ExecutorParams {
        &self.desc.executor_params
    }

    pub fn executor_params_mut(&mut self) -> &mut ExecutorParams {
        &mut self.desc.executor_params
    }
}

pub struct BlockchainAccount {
    context: BlockchainContext,
    account: Account,
}

#[derive(Clone)]
pub struct BlockchainDesc {
    pub config: BlockchainConfig,
    pub executor_params: ExecutorParams,
}

impl BlockchainAccount {
    pub fn run_local(
        &mut self,
        function: &Function,
        values: &[NamedAbiValue],
    ) -> Result<ExecutionOutput, ExecutionError> {
        function.run_local(&mut self.account, values, false, &mut self.context)
    }

    pub fn run_local_responsible(
        &mut self,
        function: &Function,
        values: &[NamedAbiValue],
    ) -> Result<ExecutionOutput, ExecutionError> {
        function.run_local(&mut self.account, values, true, &mut self.context)
    }

    pub async fn execute_message(
        &self,
        message: &OwnedMessage,
    ) -> Result<Transaction, ExecutionError> {
        self.context
            .transport
            .send_message_reliable(message)
            .await
            .map_err(Into::into)
    }

    pub fn run_getter<M>(
        &self,
        method_id: &M,
        args: &[RcStackValue],
    ) -> Result<VmGetterOutput, ExecutionError>
    where
        M: AsGetterMethodId + ?Sized,
    {
        self.run_getter_ext(method_id, args)
    }

    fn run_getter_ext<M>(
        &self,
        method_id: &M,
        args: &[RcStackValue],
    ) -> Result<VmGetterOutput, ExecutionError>
    where
        M: AsGetterMethodId + ?Sized,
    {
        let GenTimings { gen_utime, gen_lt } =
            get_gen_timings(self.context.clock.as_ref(), self.account.last_trans_lt);
        let mut stack_values = Vec::with_capacity(args.len() + 1);
        for i in args {
            stack_values.push(i.clone())
        }
        stack_values.push(SafeRc::new_dyn_value(BigInt::from(
            method_id.as_getter_method_id(),
        )));

        let local_vm = LocalVmBuilder::new()
            .with_behaviour_modifiers(BehaviourModifiers::default())
            .with_libraries(self.context.executor_params().libraries.clone())
            .with_unpacked_config(self.context.desc.config.clone(), gen_utime)?
            .build()?;

        local_vm.call_getter(gen_utime, gen_lt, &self.account, stack_values)
    }
}

pub struct BlockchainContextBuilder {
    pub clock: Arc<dyn Clock>,
    pub executor_params: ExecutorParams,
    pub transport: Option<Arc<dyn Transport>>,
    pub config: Option<BlockchainConfig>,
}

impl BlockchainContextBuilder {
    pub fn new() -> BlockchainContextBuilder {
        Self {
            clock: Arc::new(SimpleClock),
            executor_params: ExecutorParams::default(),
            transport: None,
            config: None,
        }
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_config(mut self, config: BlockchainConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_executor_params(mut self, executor_params: ExecutorParams) -> Self {
        self.executor_params = executor_params;
        self
    }

    pub fn with_transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    pub fn build(self) -> anyhow::Result<BlockchainContext> {
        let Some(config) = self.config else {
            anyhow::bail!("Failed to build BlockchainContext. Config is missing");
        };

        let Some(transport) = self.transport else {
            anyhow::bail!("Failed to build BlockchainContext. Transport is missing");
        };

        Ok(BlockchainContext {
            desc: BlockchainDesc {
                config: config.clone(),
                executor_params: self.executor_params,
            },
            transport,
            clock: self.clock,
        })
    }
}

impl Default for BlockchainContextBuilder {
    fn default() -> Self {
        Self::new()
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
            body: OwnedCellSlice::new_allow_exotic(Cell::empty_cell()),
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

pub trait IntoMessageBody {
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

impl IntoMessageBody for OwnedCellSlice {
    fn into_message_body(self) -> anyhow::Result<OwnedCellSlice> {
        Ok(self)
    }
}

pub trait AsGetterMethodId {
    fn as_getter_method_id(&self) -> u32;
}

impl<T: AsGetterMethodId + ?Sized> AsGetterMethodId for &T {
    fn as_getter_method_id(&self) -> u32 {
        T::as_getter_method_id(*self)
    }
}

impl<T: AsGetterMethodId + ?Sized> AsGetterMethodId for &mut T {
    fn as_getter_method_id(&self) -> u32 {
        T::as_getter_method_id(*self)
    }
}

impl AsGetterMethodId for u32 {
    fn as_getter_method_id(&self) -> u32 {
        *self
    }
}

impl AsGetterMethodId for str {
    fn as_getter_method_id(&self) -> u32 {
        let crc = crc_16(self.as_bytes());
        crc as u32 | 0x10000
    }
}
