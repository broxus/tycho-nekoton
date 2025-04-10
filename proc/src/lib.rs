extern crate proc_macro;

use std::fs;
use everscale_types::abi::{AbiHeaderType, Contract};
use proc_macro::TokenStream;
use std::path::Path;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Result;
use syn::{parse_macro_input, ItemMod};

use crate::generator::{FunctionDescriptionTokens, StructGenerator};

mod generator;
mod properties;


struct ModuleParams {
    path: String,
}

impl Parse for ModuleParams {
    fn parse(input: ParseStream) -> Result<Self> {
        let path = input.parse::<syn::LitStr>()?.value();
        Ok(ModuleParams { path })
    }
}

#[proc_macro_attribute]
pub fn abi(params: TokenStream, input: TokenStream) -> TokenStream {
    let mut generated_structs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut generated_functions: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut generated_events: Vec<proc_macro2::TokenStream> = Vec::new();

    let params = parse_macro_input!(params as ModuleParams);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("project root dir not found");
    println!("root_path: {:?}", root);
    let file_path = root.join(params.path);
    println!("file_path: {:?}", file_path);

    let content = fs::read_to_string(file_path).unwrap();

    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    let input = parse_macro_input!(input as ItemMod);
    let mod_name = &input.ident;

    let mut struct_gen = StructGenerator::default();

    contract.functions.iter().for_each(|(_, function)| {
        let FunctionDescriptionTokens {
            body,
            input,
            output,
            inner_models,
        } = struct_gen.process_function(function);

        generated_functions.push(body);

        generated_structs.push(input);
        generated_structs.push(output);
        generated_structs.extend_from_slice(inner_models.as_slice());
    });

    contract.events.iter().for_each(|(_, event)| {
        let FunctionDescriptionTokens {
            body,
            input,
            output,
            inner_models,
        } = struct_gen.process_event(event);

        generated_events.push(body);

        generated_structs.push(input);
        generated_structs.push(output);
        generated_structs.extend_from_slice(inner_models.as_slice());
    });

    let header_type: syn::Type =
        syn::parse_str("everscale_types::abi::AbiHeaderType").unwrap();
    let abi_type: syn::Type = syn::parse_str("everscale_types::abi::AbiVersion").unwrap();

    let mut header_idents = Vec::<proc_macro2::TokenStream>::new();
    for i in contract.headers.iter() {
        let ty = match i {
            AbiHeaderType::Expire => "everscale_types::abi::AbiHeaderType::Expire",
            AbiHeaderType::PublicKey => "everscale_types::abi::AbiHeaderType::PublicKey",
            AbiHeaderType::Time => "everscale_types::abi::AbiHeaderType::Time",
        };
        let ty: syn::Type = syn::parse_str(ty).expect("Failed to parse header type");
        let quote = quote! {
            #ty
        };
        header_idents.push(quote);
    }

    let slice_token = quote! {
        [ #(#header_idents),* ]
    };

    let header_count = contract.headers.len();
    let major = contract.abi_version.major;
    let minor = contract.abi_version.minor;

    let quote = quote! {

        pub mod #mod_name {
            use anyhow::Result;
            use everscale_types::abi::{NamedAbiType, AbiType, WithAbiType, IntoAbi, IntoPlainAbi,
                FromAbiIter, FromAbi, AbiValue, NamedAbiValue, Function, Event
            };
            use num_bigint::{BigInt, BigUint};

            #(#generated_structs)*

            pub mod functions {
                use super::*;

                const HEADERS: [#header_type; #header_count] = #slice_token;
                const ABI_VERSION: #abi_type = <#abi_type>::new(#major, #minor);

                #(#generated_functions)*
            }

             pub mod events {
                use super::*;

                const ABI_VERSION: #abi_type = <#abi_type>::new(#major, #minor);

                #(#generated_events)*
            }
        }
    };

    quote.into()
}
