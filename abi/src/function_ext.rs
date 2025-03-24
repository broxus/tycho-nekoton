use anyhow::{anyhow, Result};
use everscale_types::abi::{AbiValue, Function, NamedAbiValue};
use everscale_types::cell::HashBytes;
use everscale_types::models::{Account, BlockchainConfig, LibDescr, RelaxedMsgInfo};
use everscale_types::num::Tokens;
use everscale_types::prelude::Dict;
use nekoton_utils::time::Clock;
use num_traits::cast::ToPrimitive;
use tycho_vm::{BehaviourModifiers, OwnedCellSlice};

use crate::execution_context::MessageBuilder;
use crate::local_executor;
use crate::utils::get_gen_timings;

pub trait FunctionExt {
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        account: &mut Account,
        input: &[NamedAbiValue],
        clock: &dyn Clock,
        responsible: bool,
        rand_seed: HashBytes,
        libraries: &Dict<HashBytes, LibDescr>,
        config: BlockchainConfig,
    ) -> Result<ExecutionOutput>;
}

impl FunctionExt for Function {
    fn run_local(
        &self,
        account: &mut Account,
        input: &[NamedAbiValue],
        clock: &dyn Clock,
        responsible: bool,
        rand_seed: HashBytes,
        libraries: &Dict<HashBytes, LibDescr>,
        config: BlockchainConfig,
    ) -> Result<ExecutionOutput> {
        let answer_id = if responsible {
            account.balance.tokens = Tokens::new(100_000_000_000_000u128); // 100 000 native tokens

            match input.first().map(|token| &token.value) {
                Some(AbiValue::Uint(32, number)) => {
                    let answer_id = number
                        .to_u32()
                        .ok_or_else(|| anyhow!("Invalid abi value"))?;
                    Some(answer_id)
                }
                _ => anyhow::bail!("Invalid abi"),
            }
        } else {
            None
        };

        let message = if responsible {
            MessageBuilder::new_internal_in(account.address.clone(), account.address.clone())
                .with_body(self.encode_internal_input(input)?)?
                .build()
        } else {
            let (_, payload) = self
                .encode_external(input)
                .with_expire_at(u32::MAX)
                .build_input_without_signature()?;
            MessageBuilder::new_external_in(account.address.clone())
                .with_body(payload)?
                .build()
        };

        let (gen_utime, gen_lt) = get_gen_timings(clock, account.last_trans_lt);

        let compute_phase_result = local_executor::execute_message(
            gen_utime,
            gen_lt,
            account.clone(),
            message,
            config,
            rand_seed,
            libraries,
            BehaviourModifiers::default(),
        )?;

        if !compute_phase_result.success {
            return Ok(ExecutionOutput {
                values: vec![],
                exit_code: compute_phase_result.exit_code,
            });
        }

        let mut output = vec![];
        if let Some(answer_id) = answer_id {
            for msg in compute_phase_result.out_messages {
                if let RelaxedMsgInfo::ExtOut(_) = msg.info {
                    continue;
                }

                let slice = OwnedCellSlice::from(msg.body);
                let mut slice = slice.apply();

                if !matches!(
                    slice.load_u32(),
                    Ok(target_answer_id) if target_answer_id == answer_id
                ) {
                    continue;
                }

                if let Ok(values) =
                    NamedAbiValue::load_tuple(self.outputs.as_ref(), self.abi_version, &mut slice)
                {
                    output = values;
                    break;
                }
            }
        } else {
            for msg in compute_phase_result.out_messages {
                if let RelaxedMsgInfo::Int(_) = msg.info {
                    continue;
                }

                let slice = OwnedCellSlice::from(msg.body);
                let slice = slice.apply();

                let output_id = slice.get_u32(slice.offset_bits())?;
                if output_id == self.output_id {
                    if let Ok(values) = self.decode_output(slice) {
                        output = values;
                        break;
                    }
                } else {
                    continue;
                };
            }
        };

        Ok(ExecutionOutput {
            values: output,
            exit_code: compute_phase_result.exit_code,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionOutput {
    pub values: Vec<NamedAbiValue>,
    pub exit_code: i32,
}
