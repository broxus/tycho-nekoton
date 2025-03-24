use anyhow::{anyhow, Result};
use everscale_types::cell::HashBytes;
use everscale_types::models::{Account, AccountState, BlockchainConfig, LibDescr};
use everscale_types::prelude::Dict;
use tycho_vm::{
    BehaviourModifiers, GasParams, OwnedCellSlice, RcStackValue, SmcInfoBase, UnpackedConfig,
    VmStateBuilder,
};

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

    pub fn with_unpacked_config(mut self, config: BlockchainConfig) -> Result<Self> {
        self.unpacked_config = Some(UnpackedConfig {
            latest_storage_prices: config
                .get_raw_cell(18)?
                .map(|x| OwnedCellSlice::new_allow_exotic(x).into()),
            global_id: config.get_raw_cell(19)?,
            mc_gas_prices: config.get_raw_cell(20)?,
            gas_prices: config.get_raw_cell(21)?,
            mc_fwd_prices: config.get_raw_cell(24)?,
            fwd_prices: config.get_raw_cell(25)?,
            size_limits_config: config.get_raw_cell(43)?,
        });
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
    ) -> Result<VmGetterOutput> {
        tracing_subscriber::fmt::init();
        let state = match &account.state {
            AccountState::Active(state_init) => state_init,
            _ => anyhow::bail!("account is not active"),
        };
        let code = state.clone().code.ok_or(anyhow!("account has no code"))?;

        let smc = SmcInfoBase::new()
            .with_account_addr(account.address.clone())
            .with_account_balance(account.balance.clone())
            .with_block_lt(block_lt)
            .with_tx_lt(block_lt)
            .with_now(block_unixtime)
            .require_ton_v4()
            .require_ton_v6()
            .with_unpacked_config(self.config.clone().into_tuple())
            .require_ton_v9();

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
