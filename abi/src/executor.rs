use std::rc::Rc;
use std::sync::Arc;
use anyhow::Result;
use everscale_types::cell::{Cell, CellBuilder};
use everscale_types::error::Error;
use everscale_types::models::{Account, AccountState, BlockchainConfig, Message, MsgInfo, OutAction, OutActionsRevIter, OwnedMessage};
use everscale_types::prelude::{CellFamily, Store};
use tycho_executor::{ExecutorParams, ParsedConfig};
use tycho_vm::__export::num_bigint::BigInt;
use tycho_vm::{tuple, GasParams, OwnedCellSlice, RcStackValue, SmcInfoBase, Stack, VmState};

pub fn execute_message() -> Result<(), ExecutionError> {
    tycho_executor::ExecutorState::receive_in_msg()
}
pub fn run_vm(
    block_utime: u32,
    block_lt: u64,
    account: &mut Account,
    message: &OwnedMessage,
    config: BlockchainConfig,
    vm_modifiers: tycho_vm::BehaviourModifiers) -> Result<i32, ExecutionError> {
    let executor_params = ExecutorParams {
        libraries: Default::default(),
        rand_seed: Default::default(),
        block_unixtime: block_utime,
        block_lt,
        vm_modifiers,
        disable_delete_frozen_accounts: false,
        charge_action_fees_on_fail: false,
        full_body_in_bounced: false,
    };

    let mut builder = CellBuilder::new();
    message.store_into(&mut builder, &mut Cell::empty_context())?;
    let message_cell = builder.build()?;
    let balance = account.balance.tokens.into_inner();
    let (function_selector, msg_balance) = match message.info {
        MsgInfo::Int(_) => (BigInt::default(), BigInt::from(1_000_000_000_000u64)),
        MsgInfo::ExtIn(_) => (BigInt::from(-1), BigInt::default()),
        MsgInfo::ExtOut(_) => return Err(ExecutionError::InvalidMessageType)
    };
    let message_body = OwnedCellSlice::from(message.body.clone());

    let stack_values = tuple![
        int balance,
        int msg_balance,
        cell message_cell,
        slice message_body,
        int function_selector
    ];

    run_vm_with_stack(block_utime, block_lt, account, stack_values, config)

}
fn run_vm_with_stack(
    block_utime: u32,
    block_lt: u64,
    account: &mut Account,
    init_stack_values: Vec<RcStackValue>,
    config: BlockchainConfig,
) -> Result<i32, ExecutionError> {
    let state_init = match &account.state {
        AccountState::Active(state_init) => Ok(state_init),
        _ => Err(ExecutionError::AccountIsNotActive)?,
    }?;

    let code = state_init.clone().code.ok_or(ExecutionError::AccountHasNoCode)?;


    let smc_info = SmcInfoBase::new()
        .with_now(block_utime)
        .with_block_lt(block_lt)
        .with_tx_lt(block_lt)
        .with_config(config.params)
        .with_account_balance(account.balance.clone())
        .with_account_addr(account.address.clone());

    let mut vm_state = VmState::builder()
        .with_smc_info(smc_info)
        .with_stack(init_stack_values)
        .with_code(code)
        .with_data(state_init.data.clone().unwrap_or_default())
        .with_gas(GasParams::getter())
        .build();


    let exit_code = !vm_state.run();
    let output_actions = vm_state.cr.get_d(1)//c5
        .ok_or(|_| ExecutionError::FailedToRetrieveActions)?;
    for i in OutActionsRevIter::new(output_actions.as_slice()?) {

    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ExecutionError {
    #[error("Cell error")]
    CellError(#[from] Error),
    #[error("Failed to serialize message")]
    FailedToSerializeMessage,
    #[error("Invalid message type")]
    InvalidMessageType,
    #[error("Account is not active")]
    AccountIsNotActive,
    #[error("Account has not code")]
    AccountHasNoCode,
    #[error("Failed to put data into registers")]
    FailedToPutDataIntoRegisters,
    #[error("Failed to put SCI into registers")]
    FailedToPutSciIntoRegisters,
    #[error("Failed to parse exception")]
    FailedToParseException,
    #[error("Failed to retrieve actions")]
    FailedToRetrieveActions,
}