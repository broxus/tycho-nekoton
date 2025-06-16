use anyhow::Result;
use everscale_types::cell::HashBytes;
use everscale_types::models::{Account, AccountState, BlockchainConfig, LibDescr};
use everscale_types::prelude::Dict;
use tycho_vm::{
    BehaviourModifiers, GasParams, RcStackValue, SmcInfoBase, SmcInfoTonV6, UnpackedConfig,
    VmStateBuilder,
};

use crate::error::ExecutionError;

pub struct LocalVmBuilder {
    libraries: Dict<HashBytes, LibDescr>,
    behaviour_modifiers: Option<BehaviourModifiers>,
    unpacked_config: Option<UnpackedConfig>,
}

impl Default for LocalVmBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalVmBuilder {
    pub fn new() -> Self {
        Self {
            libraries: Dict::default(),
            behaviour_modifiers: None,
            unpacked_config: None,
        }
    }

    pub fn with_libraries(mut self, libraries: Dict<HashBytes, LibDescr>) -> Self {
        self.libraries = libraries;
        self
    }
    pub fn with_behaviour_modifiers(mut self, modifiers: BehaviourModifiers) -> Self {
        self.behaviour_modifiers = Some(modifiers);
        self
    }

    pub fn with_unpacked_config(mut self, config: BlockchainConfig, now: u32) -> Result<Self> {
        let partial_config = SmcInfoTonV6::unpack_config_partial(&config, now)?;
        self.unpacked_config = Some(partial_config);
        Ok(self)
    }

    pub fn build(self) -> Result<LocalVm> {
        Ok(LocalVm {
            libraries: self.libraries,
            behaviour_modifiers: self.behaviour_modifiers.unwrap_or_default(),
            config: match self.unpacked_config {
                Some(config) => config,
                None => anyhow::bail!("failed to build unpacked config"),
            },
        })
    }
}

pub struct LocalVm {
    libraries: Dict<HashBytes, LibDescr>,
    behaviour_modifiers: BehaviourModifiers,
    config: UnpackedConfig,
}

impl LocalVm {
    pub fn call_getter(
        &self,
        block_unixtime: u32,
        block_lt: u64,
        account: &Account,
        stack: Vec<RcStackValue>,
    ) -> Result<VmGetterOutput, ExecutionError> {
        let state = match &account.state {
            AccountState::Active(state_init) => state_init,
            _ => return Err(ExecutionError::AccountNotActive(account.address.clone())),
        };
        let code = state
            .clone()
            .code
            .ok_or(ExecutionError::AccountHasNoCode(account.address.clone()))?;

        let smc = SmcInfoBase::new()
            .with_account_addr(account.address.clone())
            .with_account_balance(account.balance.clone())
            .with_block_lt(block_lt)
            .with_tx_lt(block_lt)
            .with_now(block_unixtime)
            .require_ton_v4()
            .require_ton_v6()
            .with_unpacked_config(self.config.clone().into_tuple())
            .require_ton_v11();

        let data = state.clone().data.unwrap_or_default();

        let mut vm_state = VmStateBuilder::new()
            .with_code(code)
            .with_data(data)
            .with_stack(stack)
            .with_smc_info(smc)
            .with_libraries(&self.libraries)
            .with_modifiers(self.behaviour_modifiers)
            .with_gas(GasParams::getter())
            .build();

        let exit_code = !vm_state.run();

        Ok(VmGetterOutput {
            exit_code,
            stack: vm_state.stack.items.clone(),
            success: exit_code == 0 || exit_code == 1,
        })
    }
}

pub struct VmGetterOutput {
    pub exit_code: i32,
    pub stack: Vec<RcStackValue>,
    pub success: bool,
}
