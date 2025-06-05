use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::models::{
    Account, ComputePhase, IntAddr, MsgType, OutAction, OutActionsRevIter, OwnedMessage,
    OwnedRelaxedMessage, ShardAccount, Transaction,
};
use tycho_types::prelude::{CellFamily, Store};

use tycho_executor::phase::{ComputePhaseContext, TransactionInput};
use tycho_executor::{ExecutorParams, ParsedConfig};
use tycho_types::num::Tokens;
use tycho_vm::OwnedCellSlice;

use crate::error::ExecutionError;

pub struct ComputePhaseResult {
    pub exit_code: i32,
    pub success: bool,
    pub out_messages: Vec<OwnedRelaxedMessage>,
}
#[allow(clippy::too_many_arguments)]
pub fn execute_message(
    account: &Account,
    message: &OwnedMessage,
    executor_params: &ExecutorParams,
    config: &ParsedConfig,
) -> Result<ComputePhaseResult, ExecutionError> {
    let mut builder = CellBuilder::new();
    message.store_into(&mut builder, Cell::empty_context())?;
    let in_msg_cell = builder.build()?;

    let executor = tycho_executor::Executor::new(executor_params, config);

    let IntAddr::Std(std_addr) = &account.address else {
        return Err(ExecutionError::InvalidAddressType);
    };

    let mut state = executor.begin(std_addr, Some(account.clone()))?;
    let received_message = state.receive_in_msg(in_msg_cell)?;

    let compute_phase_result = state.compute_phase(ComputePhaseContext {
        input: TransactionInput::Ordinary(&received_message),
        storage_fee: Tokens::ZERO,
        force_accept: true,
        inspector: None,
    })?;

    let executed_compute_phase = match compute_phase_result.compute_phase {
        ComputePhase::Skipped(result) => {
            return Err(ExecutionError::ComputePhaseSkipped(result.reason))
        }
        ComputePhase::Executed(executed_result) => executed_result,
    };

    let mut msgs = Vec::new();
    let actions_slice = OwnedCellSlice::new_allow_exotic(compute_phase_result.actions);
    let out_actions_iter = OutActionsRevIter::new(actions_slice.apply());
    for action in out_actions_iter.flatten() {
        if let OutAction::SendMsg { out_msg, .. } = action {
            msgs.push(out_msg.load()?);
        }
    }

    msgs.reverse();

    Ok(ComputePhaseResult {
        exit_code: !executed_compute_phase.exit_code,
        success: executed_compute_phase.success,
        out_messages: msgs,
    })
}

pub fn execute_ordinary_transaction(
    shard_account: &ShardAccount,
    message: &OwnedMessage,
    executor_params: &ExecutorParams,
    config: &ParsedConfig,
) -> Result<Transaction, ExecutionError> {
    let is_external = !matches!(message.ty(), MsgType::Int);

    let optional = shard_account.load_account()?;
    let Some(account) = optional else {
        return Err(ExecutionError::AccountDoesNotExist);
    };
    let address = account.address.as_std().unwrap();

    let executor = tycho_executor::Executor::new(executor_params, config);
    let uncommited = executor.begin_ordinary(address, is_external, message, shard_account)?;
    let tx = uncommited.build_uncommitted()?;
    Ok(tx)
}
