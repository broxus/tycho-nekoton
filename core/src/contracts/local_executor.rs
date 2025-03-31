use anyhow::Result;
use everscale_types::cell::{Cell, CellBuilder, HashBytes};
use everscale_types::models::{
    Account, BlockchainConfig, ComputePhase, IntAddr, LibDescr, OutAction, OutActionsRevIter,
    OwnedMessage, OwnedRelaxedMessage,
};
use everscale_types::prelude::{CellFamily, Dict, Store};

use everscale_types::num::Tokens;
use tycho_executor::phase::{ComputePhaseContext, TransactionInput};
use tycho_executor::{ExecutorParams, ParsedConfig};
use tycho_vm::OwnedCellSlice;

pub struct ComputePhaseResult {
    pub exit_code: i32,
    pub success: bool,
    pub out_messages: Vec<OwnedRelaxedMessage>,
}
#[allow(clippy::too_many_arguments)]
pub fn execute_message(
    block_unixtime: u32,
    block_lt: u64,
    account: Account,
    message: OwnedMessage,
    config: BlockchainConfig,
    rand_seed: HashBytes,
    libraries: Dict<HashBytes, LibDescr>,
    vm_modifiers: tycho_vm::BehaviourModifiers,
) -> Result<ComputePhaseResult> {
    let mut builder = CellBuilder::new();
    message.store_into(&mut builder, Cell::empty_context())?;
    let in_msg_cell = builder.build()?;

    let executor_params = ExecutorParams {
        libraries,
        rand_seed,
        block_unixtime,
        block_lt,
        vm_modifiers,
        ..Default::default()
    };

    let config = ParsedConfig::parse(config, block_unixtime)?;

    let executor = tycho_executor::Executor::new(&executor_params, &config);

    let IntAddr::Std(std_addr) = &account.address.clone() else {
        anyhow::bail!("Invalid address type");
    };

    let mut state = executor.begin(std_addr, Some(account))?;
    let received_message = state.receive_in_msg(in_msg_cell)?;
    let compute_phase_result = state.compute_phase(ComputePhaseContext {
        input: TransactionInput::Ordinary(&received_message),
        storage_fee: Tokens::ZERO,
        force_accept: true,
    })?;

    let executed_compute_phase = match compute_phase_result.compute_phase {
        ComputePhase::Skipped(result) => {
            anyhow::bail!("Compute phase is skipped.Reason: {:?}", result.reason)
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
