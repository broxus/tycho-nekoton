use anyhow::Result;
use everscale_types::abi::{Function, NamedAbiValue};
use everscale_types::cell::HashBytes;
use everscale_types::crc::crc_16;
use everscale_types::models::{
    Account, BlockchainConfig, ExtInMsgInfo, IntAddr, IntMsgInfo, LibDescr, MsgInfo, OwnedMessage,
};
use everscale_types::prelude::{Cell, CellBuilder, CellFamily, Dict};
use nekoton_utils::time::{Clock, SimpleClock};
use num_bigint::BigInt;
use tycho_vm::{BehaviourModifiers, OwnedCellSlice, RcStackValue, SafeRc};

use crate::function_ext::{ExecutionOutput, FunctionExt};
use crate::local_vm;
use crate::local_vm::VmGetterOutput;
use crate::utils::get_gen_timings;

#[derive(Clone)]
pub struct ExecutionContext<'a> {
    clock: &'a dyn Clock,
    rand_seed: HashBytes,
    libraries: Dict<HashBytes, LibDescr>,
    account: Account,
}

impl ExecutionContext<'_> {
    pub fn run_local(
        &mut self,
        function: &Function,
        values: &[NamedAbiValue],
        config: BlockchainConfig,
    ) -> Result<ExecutionOutput> {
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
    ) -> Result<ExecutionOutput> {
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

    pub fn run_getter<M>(&self, method_id: &M, args: &[RcStackValue]) -> Result<VmGetterOutput>
    where
        M: AsGetterMethodId + ?Sized,
    {
        self.run_getter_ext(method_id, args)
    }

    fn run_getter_ext<M>(&self, method_id: &M, args: &[RcStackValue]) -> Result<VmGetterOutput>
    where
        M: AsGetterMethodId + ?Sized,
    {
        let (gen_utime, gen_lt) = get_gen_timings(self.clock, self.account.last_trans_lt);
        let mut stack_values = Vec::with_capacity(args.len() + 1);
        for i in args {
            stack_values.push(i.clone())
        }
        stack_values.push(SafeRc::new_dyn_value(BigInt::from(
            method_id.as_getter_method_id(),
        )));

        println!("{:?}", stack_values);

        local_vm::call_getter(
            gen_utime,
            gen_lt,
            &self.account,
            stack_values,
            &self.libraries,
            BehaviourModifiers::default(),
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
    pub fn new(account: &'a Account) -> ExecutionContextBuilder<'a> {
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

    pub fn build_cell(&self) -> Result<Cell> {
        let cell = CellBuilder::build_from(self.build())?;
        Ok(cell)
    }
}

pub trait IntoMessageBody {
    fn into_message_body(self) -> Result<OwnedCellSlice>;
}

impl IntoMessageBody for CellBuilder {
    fn into_message_body(self) -> Result<OwnedCellSlice> {
        let cell = self.build()?;
        Ok(OwnedCellSlice::new_allow_exotic(cell))
    }
}

impl IntoMessageBody for Cell {
    fn into_message_body(self) -> Result<OwnedCellSlice> {
        Ok(OwnedCellSlice::new_allow_exotic(self))
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
