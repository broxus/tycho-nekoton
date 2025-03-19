mod execution_context;
mod function_ext;
mod local_executor;

pub mod tests {
    use crate::execution_context::ExecutionContextBuilder;
    use crate::function_ext::FunctionExt;
    use everscale_types::abi::{AbiVersion, Function};
    use everscale_types::cell::HashBytes;
    use everscale_types::models::BlockchainConfig;
    use nekoton_utils::time::SimpleClock;

    #[test]
    fn test() {
        let config = BlockchainConfig::new_empty(Default::default());
        let function = Function::builder(AbiVersion::V2_2, "test").build();
        let execution_context = ExecutionContextBuilder::new(Default::default())
            .with_rand_seed(HashBytes::default())
            .with_clock(&SimpleClock)
            .build();
        let values = vec![];
        let output = execution_context
            .run_local(&function, values.as_slice(), config)
            .unwrap();
        println!("{:?}", output);
    }
}
