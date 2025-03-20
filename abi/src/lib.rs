mod execution_context;
mod function_ext;
mod local_executor;

pub mod tests {
    use crate::execution_context::ExecutionContextBuilder;
    use crate::function_ext::FunctionExt;
    use everscale_types::abi::{AbiHeaderType, AbiType, AbiValue, AbiVersion, Function};
    use everscale_types::boc::Boc;
    use everscale_types::cell::HashBytes;
    use everscale_types::models::{Account, BlockchainConfig, OptionalAccount};
    use everscale_types::prelude::CellBuilder;
    use num_traits::Zero;
    use tycho_vm::__export::num_bigint::BigUint;

    use base64::prelude::{Engine as _, BASE64_STANDARD};

    use nekoton_utils::time::SimpleClock;

    #[test]
    fn local_executor_test() {

        let config_cell = Boc::decode_base64("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").unwrap();
        let config = config_cell.parse::<BlockchainConfig>().unwrap();

        let account_cell = Boc::decode_base64("te6ccgECHwEABIcAAnCAGu/JZJZGN0YqhtnInCgdfa2DZTmHJ/jIfw+yhomjucEEfQ/XBn2w+JAAAAHXg4MMEoCBLTqnpggBAXHTW4gMB23vE519+GhxVz6Qw+TzzEhmgsuExZl1DYivtgAAAZWvtK5OgAAAAAAAAAAAAAAAAAAAAEACART/APSkE/S88sgLAwIBIAcEAubycdcBAcAA8nqDCNcY7UTQgwfXAdcLP8j4KM8WI88WyfkAA3HXAQHDAJqDB9cBURO68uBk3oBA1wGAINcBgCDXAVQWdfkQ8qj4I7vyeWa++COBBwiggQPoqFIgvLHydAIgghBM7mRsuuMPAcjL/8s/ye1UBgUAPoIQFp4+EbqOEfgAApMg10qXeNcB1AL7AOjRkzLyPOIAmDAC10zQ+kCDBtcBcdcBeNcB10z4AHCAEASqAhSxyMsFUAXPFlAD+gLLaSLQIc8xIddJoIQJuZgzcAHLAFjPFpcwcQHLABLM4skB+wAABNIwBCSK7VMg4wMgwP/jAiDA/uMC8gscCgkeAqDtRNDXScMB+GYh2zzTAAGOFIMI1xgg+CjIzs7J+QBY+EL5EPKo3tM/AfhDIbnytCD4I4ED6KiCCBt3QKC58rT4Y9MfAfgjvPK50x8B2zzyPBgLA0rtRNDXScMB+GYi0NcLA6k4ANwhxwDjAiHXDR/yvCHjAwHbPPI8GxsLBFAgghAciukbuuMCIIIQH7VEkbrjAiCCECujMwe64wIgghBfzAuTuuMCFhMRDANKMPhG8uBM+EJu4wAhl9M/03/U0dCU0z/Tf+LT/9M/0ds82zzyABkNFwGighA7msoAcPsCI4EAyKC1PyG2CFUDk1MBuY6A6DBTAbmOLFRxIyP4KHDIz4WAygDPhEDOcc8LblUwyM+RfzAuTss/y3/L/8s/zcmDBvsA3l8EDgEQVHBD2zyktT8PAYASyMv/yz/J+EvIz4SA9AD0AM+BySD5AMjPigBAy//J0BLIz4UIzgH6AnPPC2pRENs8zxTPgclx+wD4TKS1P/hsEAA00NIAAZPSBDHe0gABk9IBMd70BPQE9ATRXwMDiDD4RvLgTPhCbuMAIZXTP9TR0JLTP+LT/9HbPCGOHyPQ0wH6QDAxyM+HIM5xzwthAcjPkq6MzB7Ozclw+wCRMOLjAPIAGRIUADzIy//LP8n4S8jPhID0APQAz4HJ+QDIz4oAQMv/ydADaDD4RvLgTPhCbuMA0ds8IY4cI9DTAfpAMDHIz4cgzoIQn7VEkc8Lgcs/yXD7AJEw4uMA8gAZFRQAKO1E0NP/0z8x+ENYyMv/yz/Oye1UAAT4TAIoMPhCbuMA+Ebyc9TR+AD4a9s88gAYFwAy+Ez4S/hK+EP4QsjL/8s/z4PLP8zLP8ntVAIW7UTQ10nCAY6A4w0aGQA07UTQ0//TP9MAMdM/1NM/0fhs+Gv4avhj+GIBUnDtRND0BXEhgED0Dm+Rk9cLP96IcPhs+Gv4aoBA9A7yvdcL//hicPhjHgAK+Eby4EwCEPSkIPS98sBOHh0AFHNvbCAwLjcwLjAAAA==").unwrap();

        let mut builder =  CellBuilder::new();
        builder.store_bit(true).unwrap();
        builder.store_slice(account_cell.as_slice().unwrap()).unwrap();
        let new_account_cell = builder.build().unwrap();

        let optional_account = new_account_cell.parse::<OptionalAccount>().unwrap();
        //
        let account = optional_account.0.unwrap();

        let inputs = vec![
            AbiType::Uint(64).named("_index") ,
            AbiType::Uint(256).named("_publicKey")
        ];

        let outputs = vec![AbiType::Address.named("receiver")];

        let headers = vec![AbiHeaderType::Time, AbiHeaderType::Expire];
        let function = Function::builder(AbiVersion::V2_3, "get_wallet")
            .with_headers(headers)
            .with_inputs(inputs)
            .with_outputs(outputs)
            .build();

        let mut execution_context = ExecutionContextBuilder::new(&account)
            .with_clock(&SimpleClock)
            .build();
        let values = vec![
            AbiValue::Uint(64, BigUint::zero()).named("_index"),
            AbiValue::Uint(256, BigUint::zero()).named("_publicKey")
        ];
         match execution_context
            .run_local(&function, values.as_slice(), config) {
            Ok(output) => println!("{:?}", output),
            Err(e) => println!("error {:?}", e)
        };
    }
}
