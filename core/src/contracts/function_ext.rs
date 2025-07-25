use num_traits::cast::ToPrimitive;
use tycho_executor::ParsedConfig;
use tycho_types::abi::{AbiType, AbiValue, Function, NamedAbiValue};
use tycho_types::models::{Account, RelaxedMsgInfo};
use tycho_types::num::Tokens;
use tycho_vm::OwnedCellSlice;

use super::blockchain_context::{BlockchainContext, MessageBuilder};
use super::local_executor;
use super::utils::get_gen_timings;
use crate::error::ExecutionError;
use crate::models::GenTimings;

pub trait FunctionExt {
    fn run_local(
        &self,
        account: &mut Account,
        input: &[NamedAbiValue],
        responsible: bool,
        context: &mut BlockchainContext,
    ) -> Result<ExecutionOutput, ExecutionError>;
}

impl FunctionExt for Function {
    fn run_local(
        &self,
        account: &mut Account,
        input: &[NamedAbiValue],
        responsible: bool,
        context: &mut BlockchainContext,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let answer_id = if responsible {
            account.balance.tokens = Tokens::new(100_000_000_000_000u128); // 100 000 native tokens

            match input.first().map(|token| &token.value) {
                Some(AbiValue::Uint(32, number)) => {
                    let answer_id =
                        number
                            .to_u32()
                            .ok_or_else(|| ExecutionError::UnexpectedAbiType {
                                expected: AbiType::Int(32),
                                actual: number.to_string(),
                            })?;
                    Some(answer_id)
                }
                _ => return Err(ExecutionError::InvalidContractStructure),
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

        let GenTimings { gen_utime, gen_lt } =
            get_gen_timings(context.clock(), account.last_trans_lt);

        let parsed_config = ParsedConfig::parse(context.config().clone(), gen_utime)?;

        let params = context.executor_params_mut();
        params.block_unixtime = gen_utime;
        params.block_lt = gen_lt;

        let compute_phase_result =
            local_executor::execute_message(account, &message, params, &parsed_config)?;

        if !compute_phase_result.success {
            return Ok(ExecutionOutput {
                values: vec![],
                exit_code: !compute_phase_result.exit_code,
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
            exit_code: !compute_phase_result.exit_code,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionOutput {
    pub values: Vec<NamedAbiValue>,
    pub exit_code: i32,
}
