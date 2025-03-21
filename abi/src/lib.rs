pub mod execution_context;
pub mod function_ext;
pub mod local_executor;
pub mod local_vm;
mod utils;

#[cfg(test)]
pub mod tests {
    use crate::execution_context::ExecutionContextBuilder;
    use everscale_types::abi::{AbiHeaderType, AbiType, AbiValue, AbiVersion, Function};
    use everscale_types::boc::Boc;
    use everscale_types::models::{
        BlockchainConfig, IntAddr, OptionalAccount, StdAddr, StdAddrFormat,
    };
    use everscale_types::prelude::{CellBuilder, Load};
    use nekoton_utils::time::SimpleClock;
    use num_bigint::BigUint;
    use num_traits::Zero;
    use tycho_vm::{tuple, OwnedCellSlice, SafeRc};

    #[test]
    fn local_executor_test() {
        let config_cell = Boc::decode_base64("te6ccgECjAEACdEAAUBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVQECA81AIwICAUgFAwEBtwQASgIAIAAAAAAgAAAAA+gCAAAA//8CAAABAAAD/wAAAAABAAAAAQACAUgIBgEBSAcBKxJn29wdZ9vfoQANAA0P/////////8AKAQFICQErEmfb2Jln29wdAA0ADQ//////////wAoCAswUCwIBIA0MAJvTnHQJPFDVaw0gpBKW4KLvlHk4muJ7DXIMx9rrMAF8SqwKM2VYwAnYnYnYnYnY1WsNIKQSluCi75R5OJriew1yDMfa6zABfEqsCjNlWMQCASARDgIBIBAPAJsc46BJ4o29TxaMfd0dgRmwy0xCO12cNXWna+BkJXqkxyqZzsq6wE7E7E7E7E7NvU8WjH3dHYEZsMtMQjtdnDV1p2vgZCV6pMcqmc7KuuAAmxzjoEnijuarW543FloKpnGsmsqEFR2EWHcTk+OORw7gGlgUkSxATsTsTsTsTs7mq1ueNxZaCqZxrJrKhBUdhFh3E5PjjkcO4BpYFJEsYAIBIBMSAJsc46BJ4poyedRm6soO+rtymuULxXD+LMQNWUybAxQQgR7j8jyIQE7E7E7E7E7aMnnUZurKDvq7cprlC8Vw/izEDVlMmwMUEIEe4/I8iGAAmxzjoEnim/wiTl4DrPp9Q31ew2a8g7LEubz9WVlpg2JtfA+O4EKATsTsTsTsTtv8Ik5eA6z6fUN9XsNmvIOyxLm8/VlZaYNibXwPjuBCoAIBIBwVAgEgGRYCASAYFwCbHOOgSeKdQG5lMBnnlWWgVolqZweFI850Dkph5YTa8QoxAZCwDkBOxOxOxOxO3UBuZTAZ55VloFaJamcHhSPOdA5KYeWE2vEKMQGQsA5gAJsc46BJ4qAgRTdO/zFcU7vtGYRzhNIHBaEJKXs1sLCT9I7JIErbwE7E7E7E7E7gIEU3Tv8xXFO77RmEc4TSBwWhCSl7NbCwk/SOySBK2+ACASAbGgCbHOOgSeKgh5tRm2VYuljOyzPozbASCWx1lTe29IZoOw/BvJfAWABOxOxOxOxO4IebUZtlWLpYzssz6M2wEglsdZU3tvSGaDsPwbyXwFggAJsc46BJ4qDjNXjH+NJSDBLCbLfAXZLMaKuI8uerjdSLZeLM16qYgE7E7E7E7E7g4zV4x/jSUgwSwmy3wF2SzGiriPLnq43Ui2XizNeqmKACASAgHQIBIB8eAJsc46BJ4qPAE/2psMUyKXMosxAu2aKoR+b8KPbDnjyEQomDF4o+wE7E7E7E7E7jwBP9qbDFMilzKLMQLtmiqEfm/Cj2w548hEKJgxeKPuAAmxzjoEnip+ycYsetYn48e/+trkcu4EntnaX39Rmn/myoMWhbPtkATsTsTsTsTufsnGLHrWJ+PHv/ra5HLuBJ7Z2l9/UZp/5sqDFoWz7ZIAIBICIhAJsc46BJ4rFH29jr0c5J03A4Ipr63JreP3DzVSE+NFnPtxpb6vD3wE7E7E7E7E7xR9vY69HOSdNwOCKa+tya3j9w81UhPjRZz7caW+rw9+AAmxzjoEniusB2bjpv5ukDfb8WmmsqUT3oHkB+AILEh1SYv5Gjb4cATsTsTsTsTvrAdm46b+bpA32/FpprKlE96B5AfgCCxIdUmL+Ro2+HIAIBIFIkAgEgOyUCASA2JgIBIC4nAQFYKAEBwCkCAUgrKgBCv7d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3AgEgLSwAQb9mZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZwAD37ACASAxLwEBIDAANNgTiAAMAAAAFACMANIDIAAAAJYAGQIBBANIAQEgMgHnpoAABOIAAHUwD4AAAAAjw0YAAIAAE4gAMgAFAB4ABQBMS0AATEtAQAAJxAAAACYloAAAAAAAfQTiAPoASwAAADeqCcQC7gAACcQE4gTiBOIABAABdwLuALuAu4ALcbABdwLuAAtxsAH0Au4AAAAAAAAAACAzAgLPNTQAAwKgAAMUIAIBSDk3AQEgOABC6gAAAAABycOAAAAAAHUwAAAAAAAtxsAAAAABgABVVVVVAQEgOgBC6gAAAAAR4aMAAAAABJPgAAAAAAHJw4AAAAABgABVVVVVAgEgRzwCASBCPQIBIEA+AQEgPwBQXcMAAgAAAAgAAAAQAADDAA27oAD0JAAExLQAwwAAA+gAABOIAAAnEAEBIEEAUF3DAAIAAAAIAAAAEAAAwwANu6AA5OHAATEtAMMAAAPoAAATiAAAJxACASBFQwEBIEQAlNEAAAAAAAAD6AAAAAADk4cA3gAAAADqYAAAAAAAAAAPQkAAAAAAAA9CQAAAAAAAACcQAAAAAACYloAAAAAAI8NGAAAAAOjUpRAAAQEgRgCU0QAAAAAAAAPoAAAAACPDRgDeAAAACSfAAAAAAAAAAA9CQAAAAAAF9eEAAAAAAAAAJxAAAAAAAKfYwAAAAAAjw0YAAAAA6NSlEAACASBNSAIBIEtJAQEgSgAI///ojwEBIEwATdBmAAAAAAAAAAAAAAADAAAAAAAABdwAAAAAAAALuAAAAAAAFuNgQAIBIFBOAQEgTwAxYJGE5yoAByOG8m/BAABlrzEHpAAAADAACAEBIFEADAPoAGQADQIBIIFTAgEgXVQCASBaVQIBIFhWAQEgVwAgAAADhAAAAcIAAAA8AAABwgEBIFkAFGtGVT8QBDuaygABAUhbAQHAXAC30FMAAAAAAAAAcAAPirB7YSr0qmhrx8eoLGJYRzM7d6jD2j+8u3UTTHwspQegJq/oR/FqSXsiwKvisZimExuGVkCZp3m1j3qXGqZTAAAAAAgAAAAAAAAAAAAAAAQCASBpXgIBIGNfAQEgYAICkWJhACo2BAcEAgBMS0ABMS0AAAAAAgAAA+gAKjYCAwICAA9CQACYloAAAAABAAAB9AEBIGQCA81AZ2UCAWJmcgIBIHt7AgEgdmgCAc5+fgIBIH9qAQEgawIDzUBvbAIBSG5tAAG3AAG1AgEgdnACASB0cQIBIHNyAAHUAgFIfn4CASB1dQIBIHl5AgEgfXcCASB6eAIBIHt5AgEgfn4CASB8ewABSAABWAIB1H5+AAEgAQEggAAaxAAAACAAAAAADAMWLgIBIISCAQH0gwABQAIBIIeFAQFIhgBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACASCKiAEBIIkAQDMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzAQEgiwBAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVU=").unwrap();
        let config = config_cell.parse::<BlockchainConfig>().unwrap();

        let account_cell = Boc::decode_base64("te6ccgECHwEABIcAAnCAGu/JZJZGN0YqhtnInCgdfa2DZTmHJ/jIfw+yhomjucEEfQ/XBn2w+JAAAAHXg4MMEoCBLTqnpggBAXHTW4gMB23vE519+GhxVz6Qw+TzzEhmgsuExZl1DYivtgAAAZWvtK5OgAAAAAAAAAAAAAAAAAAAAEACART/APSkE/S88sgLAwIBIAcEAubycdcBAcAA8nqDCNcY7UTQgwfXAdcLP8j4KM8WI88WyfkAA3HXAQHDAJqDB9cBURO68uBk3oBA1wGAINcBgCDXAVQWdfkQ8qj4I7vyeWa++COBBwiggQPoqFIgvLHydAIgghBM7mRsuuMPAcjL/8s/ye1UBgUAPoIQFp4+EbqOEfgAApMg10qXeNcB1AL7AOjRkzLyPOIAmDAC10zQ+kCDBtcBcdcBeNcB10z4AHCAEASqAhSxyMsFUAXPFlAD+gLLaSLQIc8xIddJoIQJuZgzcAHLAFjPFpcwcQHLABLM4skB+wAABNIwBCSK7VMg4wMgwP/jAiDA/uMC8gscCgkeAqDtRNDXScMB+GYh2zzTAAGOFIMI1xgg+CjIzs7J+QBY+EL5EPKo3tM/AfhDIbnytCD4I4ED6KiCCBt3QKC58rT4Y9MfAfgjvPK50x8B2zzyPBgLA0rtRNDXScMB+GYi0NcLA6k4ANwhxwDjAiHXDR/yvCHjAwHbPPI8GxsLBFAgghAciukbuuMCIIIQH7VEkbrjAiCCECujMwe64wIgghBfzAuTuuMCFhMRDANKMPhG8uBM+EJu4wAhl9M/03/U0dCU0z/Tf+LT/9M/0ds82zzyABkNFwGighA7msoAcPsCI4EAyKC1PyG2CFUDk1MBuY6A6DBTAbmOLFRxIyP4KHDIz4WAygDPhEDOcc8LblUwyM+RfzAuTss/y3/L/8s/zcmDBvsA3l8EDgEQVHBD2zyktT8PAYASyMv/yz/J+EvIz4SA9AD0AM+BySD5AMjPigBAy//J0BLIz4UIzgH6AnPPC2pRENs8zxTPgclx+wD4TKS1P/hsEAA00NIAAZPSBDHe0gABk9IBMd70BPQE9ATRXwMDiDD4RvLgTPhCbuMAIZXTP9TR0JLTP+LT/9HbPCGOHyPQ0wH6QDAxyM+HIM5xzwthAcjPkq6MzB7Ozclw+wCRMOLjAPIAGRIUADzIy//LP8n4S8jPhID0APQAz4HJ+QDIz4oAQMv/ydADaDD4RvLgTPhCbuMA0ds8IY4cI9DTAfpAMDHIz4cgzoIQn7VEkc8Lgcs/yXD7AJEw4uMA8gAZFRQAKO1E0NP/0z8x+ENYyMv/yz/Oye1UAAT4TAIoMPhCbuMA+Ebyc9TR+AD4a9s88gAYFwAy+Ez4S/hK+EP4QsjL/8s/z4PLP8zLP8ntVAIW7UTQ10nCAY6A4w0aGQA07UTQ0//TP9MAMdM/1NM/0fhs+Gv4avhj+GIBUnDtRND0BXEhgED0Dm+Rk9cLP96IcPhs+Gv4aoBA9A7yvdcL//hicPhjHgAK+Eby4EwCEPSkIPS98sBOHh0AFHNvbCAwLjcwLjAAAA==").unwrap();

        let mut builder = CellBuilder::new();
        builder.store_bit(true).unwrap();
        builder
            .store_slice(account_cell.as_slice().unwrap())
            .unwrap();
        let new_account_cell = builder.build().unwrap();

        let optional_account = new_account_cell.parse::<OptionalAccount>().unwrap();
        //
        let account = optional_account.0.unwrap();

        let inputs = vec![
            AbiType::Uint(64).named("_index"),
            AbiType::Uint(256).named("_publicKey"),
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
            AbiValue::Uint(256, BigUint::zero()).named("_publicKey"),
        ];
        match execution_context.run_local(&function, values.as_slice(), config) {
            Ok(output) => println!("{:?}", output),
            Err(e) => println!("error {:?}", e),
        };
    }

    #[test]
    fn wallet_address() -> anyhow::Result<()> {
        let account_cell = Boc::decode_base64("te6ccgECLwEAB4YAAm6AFe0/oSZXf0CdefSBA89p5cgZ/cjSo7/+/CB2bN5bhnekvRsDhnIRltAAAW6YCV7CEiEatKumIgECUXye1BbWVRSoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABEwIBFP8A9KQT9LzyyAsDAgFiBQQAG6D2BdqJofQB9IH0gahhAgLMEAYCASAIBwCD1AEGuQ9qJofQB9IH0gahgCaY/BCAvGooypEF1BCD3uy+8J3QlY+XFi6Z+Y/QAYCdAoEeQoAn0BLGeLAOeLZmT2qkAgEgDgkCASALCgDXO1E0PoA+kD6QNQwB9M/+gD6QDBRUaFSSccF8uLBJ8L/8uLCBYIJMS0AoBa88uLDghB73ZfeyMsfFcs/UAP6AiLPFgHPFslxgBjIywUkzxZw+gLLaszJgED7AEATyFAE+gJYzxYBzxbMye1UgAvc7UTQ+gD6QPpA1DAI0z/6AFFRoAX6QPpAU1vHBVRzbXBUIBNUFAPIUAT6AljPFgHPFszJIsjLARL0APQAywDJ+QBwdMjLAsoHy//J0FANxwUcsfLiwwr6AFGooYIImJaAZrYIoYIImJaAoBihJ5cQSRA4N18E4w0l1wsBgDQwAfMMAI8IAsI4hghDVMnbbcIAQyMsFUAjPFlAE+gIWy2oSyx8Syz/JcvsAkzVsIeIDyFAE+gJYzxYBzxbMye1UAHBSeaAYoYIQc2LQnMjLH1Iwyz9Y+gJQB88WUAfPFslxgBDIywUkzxZQBvoCFctqFMzJcfsAECQQIwHxUD0z/6APpAIfAB7UTQ+gD6QPpA1DBRNqFSKscF8uLBKML/8uLCVDRCcFQgE1QUA8hQBPoCWM8WAc8WzMkiyMsBEvQA9ADLAMkg+QBwdMjLAsoHy//J0AT6QPQEMfoAINdJwgDy4sR3gBjIywVQCM8WcPoCF8trE8yA8AnoIQF41FGcjLHxnLP1AH+gIizxZQBs8WJfoCUAPPFslQBcwjkXKRceJQCKgToIIJycOAoBS88uLFBMmAQPsAECPIUAT6AljPFgHPFszJ7VQCAdQSEQARPpEMHC68uFNgAMMIMcAkl8E4AHQ0wMBcbCVE18D8Azg+kD6QDH6ADFx1yH6ADH6ADBzqbQAAtMfghAPin6lUiC6lTE0WfAJ4IIQF41FGVIgupYxREQD8ArgNYIQWV8HvLqTWfAL4F8EhA/y8IAEDAMAUAgEgIBUCASAbFgIBIBkXAUG/XQH6XjwGkBxFBGxrLdzqWvdk/qDu1yoQ1ATyMSzrJH0YAAQAOQFBv1II3vRvWh1Pnc5mqzCfSoUTBfFm+R73nZI+9Y40+aIJGgBEACRVUCBpcyB0aGUgbmF0aXZlIHRva2VuIG9mIFRvblVQLgIBIB4cAUG/btT5QqeEjOLLBmt3oRKMah/4xD9Dii3OJGErqf+riwMdAAYAVVABQb9FRqb/4bec/dhrrT24dDE9zeL7BeanSqfzVS2WF8edEx8ADABUb25VUAFDv/CC62Y7V6ABkvSmrEZyiN8t/t252hvuKPZSHIvr0h8ewCEAtABodHRwczovL3B1YmxpYy1taWNyb2Nvc20uczMtYXAtc291dGhlYXN0LTEuYW1hem9uYXdzLmNvbS9kcm9wc2hhcmUvMTcwMjU0MzYyOS9VUC1pY29uLnBuZwEU/wD0pBP0vPLICyMCAWInJAIDemAmJQAfrxb2omh9AH0gamoYP6qQQAB9rbz2omh9AH0gamoYNhj8FAC4KhAJqgoB5CgCfQEsZ4sA54tmZJFkZYCJegB6AGWAZPyAODpkZYFlA+X/5OhAAgLMKSgAk7XwUIgG4KhAJqgoB5CgCfQEsZ4sA54tmZJFkZYCJegB6AGWAZJB8gDg6ZGWBZQPl/+ToO8AMZGWCrGeLKAJ9AQnltYlmZmS4/YBAvHZBjgEkvgfAA6GmBgLjYSS+B8H0gfSAY/QAYuOuQ/QAY/QAYAWmP6Z/2omh9AH0gamoYQAqpOF1HGZqamxsommOC+XAkgX0gfQBqGBBoQDBrkP0AGBKIGigheAUKUCgZ5CgCfQEsZ4tmZmT2qnBBCD3uy+8pOF1xgULSoBpoIQLHa5c1JwuuMCNTc3I8ADjhozUDXHBfLgSQP6QDBZyFAE+gJYzxbMzMntVOA1AsAEjhhRJMcF8uBJ1DBDAMhQBPoCWM8WzMzJ7VTgXwWED/LwKwH+Nl8DggiYloAVoBW88uBLAvpA0wAwlcghzxbJkW3ighDRc1QAcIAYyMsFUAXPFiT6AhTLahPLHxTLPyP6RDBwuo4z+ChEA3BUIBNUFAPIUAT6AljPFgHPFszJIsjLARL0APQAywDJ+QBwdMjLAsoHy//J0M8WlmwicAHLAeL0ACwACsmAQPsAAcA2NzcB+gD6QPgoVBIGcFQgE1QUA8hQBPoCWM8WAc8WzMkiyMsBEvQA9ADLAMn5AHB0yMsCygfL/8nQUAbHBfLgSqEDRUXIUAT6AljPFszMye1UAfpAMCDXCwHDAJFb4w0uAD6CENUydttwgBDIywVQA88WIvoCEstqyx/LP8mAQvsA").unwrap();

        let mut builder = CellBuilder::new();
        builder.store_bit(true)?;
        builder.store_slice(account_cell.as_slice()?)?;
        let new_account_cell = builder.build()?;

        let optional_account = new_account_cell.parse::<OptionalAccount>()?;
        //
        let account = optional_account.0.unwrap();

        let context = ExecutionContextBuilder::new(&account).build();

        let (owner, _) = StdAddr::from_str_ext(
            "EQC-D0YPvNUq92FeG7_ZGFQY-L-lZ0wayn8arc4AKElbSo6v",
            StdAddrFormat::any(),
        )?;

        let (expected, _) = StdAddr::from_str_ext(
            "EQBWqBJJQriSjGTOBXPPSZjZoTnESO3RqPLrO6enXSq--yes",
            StdAddrFormat::any(),
        )?;

        let owner = OwnedCellSlice::new_allow_exotic(CellBuilder::build_from(owner)?);
        let args = tuple![slice owner];

        let result = context.run_getter("get_wallet_address", args.as_slice())?;
        if result.success {
            match result.stack.len() {
                1 => {
                    let first = result.stack[0].clone().into_cell_slice()?;
                    let slice = SafeRc::unwrap_or_clone(first);
                    let addr = IntAddr::load_from(&mut slice.apply())?;
                    match addr {
                        IntAddr::Std(std) => assert_eq!(std, expected),
                        _ => anyhow::bail!("addresses do not match"),
                    }
                }
                x => println!("STACK LEN: {x}"),
            }
        } else {
            println!("ERR: {}", result.exit_code);
        }

        Ok(())
    }
}
