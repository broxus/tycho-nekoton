use anyhow::{anyhow, Result};
use everscale_types::cell::HashBytes;
use everscale_types::models::{Account, AccountState, LibDescr};
use everscale_types::prelude::Dict;
use tycho_vm::{BehaviourModifiers, GasParams, RcStackValue, SmcInfoBase, VmStateBuilder};

pub fn call_getter(
    block_unixtime: u32,
    block_lt: u64,
    account: &Account,
    stack: Vec<RcStackValue>,
    libraries: &Dict<HashBytes, LibDescr>,
    behaviour_modifiers: BehaviourModifiers,
) -> Result<VmGetterOutput> {
    tracing_subscriber::fmt::init();
    let state = match &account.state {
        AccountState::Active(state_init) => state_init,
        _ => anyhow::bail!("account is not active"),
    };

    let smc = SmcInfoBase::new()
        .with_account_addr(account.address.clone())
        .with_account_balance(account.balance.clone())
        .with_block_lt(block_lt)
        .with_tx_lt(block_lt)
        .with_now(block_unixtime)
        .require_ton_v4();

    let code = state.clone().code.ok_or(anyhow!("account has no code"))?;
    let data = state.clone().data.unwrap_or_default();

    let mut vm_state = VmStateBuilder::new()
        .with_code(code)
        .with_data(data)
        .with_stack(stack)
        .with_smc_info(smc)
        .with_libraries(libraries)
        .with_modifiers(behaviour_modifiers)
        .with_gas(GasParams::getter())
        .build();

    let exit_code = !vm_state.run();

    Ok(VmGetterOutput {
        exit_code,
        stack: vm_state.stack.items.clone(),
        success: exit_code == 0 || exit_code == 1,
    })
}

pub struct VmGetterOutput {
    pub exit_code: i32,
    pub stack: Vec<RcStackValue>,
    pub success: bool,
}
