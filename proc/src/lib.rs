extern crate proc_macro;

use std::fs;
use std::sync::Arc;

use case::CaseExt;
use everscale_types::abi::{
    AbiHeaderType, AbiType, Contract, Event, Function, NamedAbiType, PlainAbiType,
};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::Result;
use syn::{parse_macro_input, ItemMod};

use models::FunctionDescriptionTokens;
mod models;
mod train_gen;

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
    let path_ident = params.path.trim_matches('"');
    let path = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))
        .unwrap()
        .join(path_ident);

    let content = fs::read_to_string(path).unwrap();
    let contract = serde_json::from_str::<Contract>(&content).unwrap();

    let input = parse_macro_input!(input as ItemMod);
    let mod_name = &input.ident;

    let mut struct_gen = StructGen::new();

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

    let trait_gen = train_gen::TraitImplGen::new();

    let mut trait_implementations: Vec<proc_macro2::TokenStream> = Vec::new();

    for (name, properties) in struct_gen.generated_structs.iter() {
        let struct_traits = trait_gen.implement_traits(name, properties.as_slice());
        trait_implementations.push(struct_traits)
    }

    let header_type: syn::Type = syn::parse_str("everscale_types::abi::AbiHeaderType").unwrap();
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

            #(#trait_implementations)*

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

struct StructGen {
    generated_structs: std::collections::HashMap<String, Vec<NamedAbiType>>,

    unique_tokes: std::collections::HashMap<AbiType, StructProperty>,

    //used only for one function
    temporary_internal_structs_idents: Vec<proc_macro2::TokenStream>,
}

impl StructGen {
    fn new() -> Self {
        Self {
            unique_tokes: std::collections::HashMap::new(),
            temporary_internal_structs_idents: Vec::new(),

            generated_structs: std::collections::HashMap::new(),
        }
    }

    fn process_function(&mut self, function: &Function) -> FunctionDescriptionTokens {
        let input_token =
            self.make_function_input_struct(function.name.as_ref(), function.inputs.clone(), false);
        let output_token =
            self.make_function_output_struct(function.name.as_ref(), function.outputs.clone());

        let mut inner_modes = Vec::new();

        let func = self.generate_func_body(function.name.as_ref(), false);

        for i in self.temporary_internal_structs_idents.iter() {
            inner_modes.push(i.clone());
        }

        self.temporary_internal_structs_idents.clear();

        FunctionDescriptionTokens {
            body: func,
            input: input_token,
            output: output_token,
            inner_models: inner_modes,
        }
    }

    fn process_event(&mut self, event: &Event) -> FunctionDescriptionTokens {
        let input_token =
            self.make_function_input_struct(event.name.as_ref(), event.inputs.clone(), true);

        let mut inner_modes = Vec::new();

        let func = self.generate_func_body(event.name.as_ref(), true);
        for i in self.temporary_internal_structs_idents.iter() {
            inner_modes.push(i.clone());
        }

        self.temporary_internal_structs_idents.clear();

        FunctionDescriptionTokens {
            body: func,
            input: input_token,
            output: proc_macro2::TokenStream::default(), //event has no output
            inner_models: inner_modes,
        }
    }

    fn generate_func_body(&self, name: &str, is_event: bool) -> proc_macro2::TokenStream {
        let snake_function_name = name.to_snake();
        let camel_function_name = name.to_camel();

        let function_name_ident = if is_event {
            format_ident!("{}_event", snake_function_name)
        } else {
            format_ident!("{}", snake_function_name)
        };

        let input_name = if is_event {
            format!("{}EventInput", &camel_function_name)
        } else {
            format!("{}FunctionInput", &camel_function_name)
        };

        let inputs: Vec<_> = self
            .generated_structs
            .get(&input_name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|x| {
                let name = x.name.as_ref();
                let quote_abi_type = quote_abi_type(&x.ty);
                quote! {
                    NamedAbiType::new(#name, #quote_abi_type)
                }
            })
            .collect();

        let inputs_count = inputs.len();
        let inputs_array = quote! {
            [ #(#inputs),* ]
        };

        if is_event {
            quote! {
                pub fn #function_name_ident() -> &'static everscale_types::abi::Event {
                    static ONCE: std::sync::OnceLock<everscale_types::abi::Event> = std::sync::OnceLock::new();
                    ONCE.get_or_init(|| {
                        let inputs: [NamedAbiType; #inputs_count] = #inputs_array;
                        everscale_types::abi::Event::builder(ABI_VERSION, #name)
                            .with_inputs(inputs)
                            .build()
                    })
                }
            }
        } else {
            let outputs: Vec<_> = self
                .generated_structs
                .get(&format!("{}FunctionOutput", &camel_function_name))
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|x| {
                    let name = x.name.as_ref();
                    let quote_abi_type = quote_abi_type(&x.ty);
                    quote! {
                        NamedAbiType::new(#name, #quote_abi_type)
                    }
                })
                .collect();

            let outputs_count = outputs.len();
            let outputs_array = quote! {
                [ #(#outputs),* ]
            };

            quote! {
                pub fn #function_name_ident() -> &'static everscale_types::abi::Function {
                    static ONCE: std::sync::OnceLock<everscale_types::abi::Function> = std::sync::OnceLock::new();
                    ONCE.get_or_init(|| {
                        let inputs: [NamedAbiType; #inputs_count] = #inputs_array;
                        let outputs: [NamedAbiType; #outputs_count] = #outputs_array;
                        everscale_types::abi::Function::builder(ABI_VERSION, #name)
                            .with_headers(HEADERS)
                            .with_inputs(inputs)
                            .with_outputs(outputs)
                            .build()
                    })
                }
            }
        }
    }

    fn generate_model(
        &mut self,
        name: &str,
        values: Arc<[NamedAbiType]>,
    ) -> proc_macro2::TokenStream {
        let struct_name_ident = format_ident!("{}", name);
        let mut properties = Vec::<proc_macro2::TokenStream>::new();

        let mut inner_fields = Vec::new();

        let function_tuple = AbiType::Tuple(values.clone());

        for i in values.iter() {
            let struct_property = match self.unique_tokes.get(&i.ty) {
                Some(struct_property) => struct_property.clone(),
                None => self.make_struct_property_with_internal(i.name.to_string(), &i.ty),
            };

            self.unique_tokes
                .insert(i.ty.clone(), struct_property.clone());

            inner_fields.push(struct_property.clone());

            let rust_name = i.name.as_ref().to_snake();
            let rust_property_name_ident = format_ident!("{}", &rust_name);

            let ty_ident = struct_property.type_name_quote();

            let quote = quote! {
                pub #rust_property_name_ident: #ty_ident,
            };

            properties.push(quote);
        }

        if !inner_fields.is_empty() {
            self.unique_tokes.insert(
                function_tuple.clone(),
                StructProperty::Tuple {
                    name: name.to_string(),
                    //fields: inner_fields,
                },
            );
        }

        if properties.is_empty() {
            quote! {
                type #struct_name_ident = ();
            }
        } else {
            quote! {
                #[derive(Clone, Debug)]
                pub struct #struct_name_ident {
                    #(#properties)*
                }
            }
        }
    }

    fn make_function_input_struct(
        &mut self,
        name: &str,
        inputs: Arc<[NamedAbiType]>,
        is_event: bool,
    ) -> proc_macro2::TokenStream {
        let struct_name = if is_event {
            format!("{}EventInput", name.to_camel())
        } else {
            format!("{}EventOutput", name.to_camel())
        };
        let model = self.generate_model(&struct_name, inputs.clone());

        if !self.generated_structs.contains_key(&struct_name) {
            self.generated_structs
                .insert(struct_name.clone(), inputs.to_vec());
        }

        model
    }

    fn make_function_output_struct(
        &mut self,
        name: &str,
        outputs: Arc<[NamedAbiType]>,
    ) -> proc_macro2::TokenStream {
        let struct_name = format!("{}FunctionOutput", name.to_camel());
        let model = self.generate_model(&struct_name, outputs.clone());

        if !self.generated_structs.contains_key(&struct_name) {
            self.generated_structs
                .insert(struct_name.clone(), outputs.to_vec());
        }

        model
    }

    fn make_struct_property_with_internal(
        &mut self,
        initial_name: String,
        param: &AbiType,
    ) -> StructProperty {
        self.make_struct_property(Some(initial_name), param)
    }

    fn make_struct_property(
        &mut self,
        initial_name: Option<String>,
        param: &AbiType,
    ) -> StructProperty {
        let name = initial_name.map(|x| x.to_string());
        if let Some(st_property) = self.unique_tokes.get(param) {
            if name
                .clone()
                .map(|name| st_property.name().eq(&name))
                .unwrap_or(false)
            {
                return st_property.clone();
            }
        }

        match param {
            AbiType::Uint(a) => {
                let ty = match a {
                    8 => "u8",
                    16 => "u16",
                    32 => "u32",
                    64 => "u64",
                    128 => "u128",
                    160 => "[u8; 20]",
                    256 => "everscale_types::prelude::HashBytes",
                    _ => "num_bigint::BigUint",
                };
                StructProperty::Simple {
                    name,
                    type_name: syn::parse_str(ty).unwrap(),
                }
            }
            AbiType::Int(a) => {
                let ty = match a {
                    8 => "i8",
                    16 => "i16",
                    32 => "i32",
                    64 => "i64",
                    128 => "i128",
                    _ => "num_bigint::BigInt",
                };
                StructProperty::Simple {
                    name,
                    type_name: syn::parse_str(ty).unwrap(),
                }
            }
            AbiType::VarUint(_) | AbiType::VarInt(_) => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(num_bigint::BigUint),
            },
            AbiType::Bool => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(bool),
            },
            AbiType::Tuple(a) => {
                let name = name.unwrap_or_default();
                let camel_case_struct_name = name.to_camel();
                let struct_name_ident = format_ident!("{}", &camel_case_struct_name);

                let mut structs: Vec<StructProperty> = Vec::new();

                for i in a.iter() {
                    let property = self.make_struct_property(Some(i.name.to_string()), &i.ty);
                    structs.push(property);
                }

                let mut internal_properties: Vec<proc_macro2::TokenStream> = Vec::new();

                for p in &structs {
                    let p_name = p.name();
                    let rust_property_name_ident = format_ident!("{}", p_name.as_str().to_snake());
                    let internal_ident = p.type_name_quote();
                    let quote = quote! {
                        pub #rust_property_name_ident: #internal_ident,
                    };
                    internal_properties.push(quote);
                }

                let internal_struct = if !internal_properties.is_empty() {
                    quote! {
                        #[derive(Clone, Debug)]
                        pub struct #struct_name_ident {
                            #(#internal_properties)*
                        }
                    }
                } else {
                    quote! {
                        type #struct_name_ident = ();
                    }
                };

                self.temporary_internal_structs_idents.push(internal_struct);

                let property = StructProperty::Tuple {
                    name,
                    //fields: structs,
                };

                {
                    self.unique_tokes.insert(param.clone(), property.clone());
                    self.generated_structs
                        .entry(camel_case_struct_name)
                        .or_insert_with(|| a.to_vec());
                }
                property
            }
            AbiType::Array(a) | AbiType::FixedArray(a, _) => {
                let internal_struct = self.make_struct_property(None, a);
                StructProperty::Array {
                    name: name.unwrap_or_default(),
                    internal: Box::new(internal_struct),
                }
            }
            AbiType::Cell => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(everscale_types::prelude::Cell),
            },
            AbiType::Map(a, b) => {
                let key = match a {
                    PlainAbiType::Uint(_) | PlainAbiType::Int(_) | PlainAbiType::Address => {
                        self.make_struct_property(None, &(*a).into())
                    }
                    _ => panic!("Map key is not allowed type"),
                };

                let value = self.make_struct_property(None, b.as_ref());

                StructProperty::HashMap {
                    name: name.unwrap_or_default(),
                    key: Box::new(key),
                    value: Box::new(value),
                }
            }
            AbiType::Address => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(everscale_types::models::message::StdAddr),
            },
            AbiType::Bytes | AbiType::FixedBytes(_) => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(Vec<u8>),
            },
            AbiType::String => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(String),
            },
            AbiType::Token => StructProperty::Simple {
                name,
                type_name: syn::parse_quote!(everscale_types::num::Tokens),
            },
            AbiType::Optional(a) => {
                let internal_struct = self.make_struct_property(None, a.as_ref());
                StructProperty::Option {
                    name: name.unwrap_or_default(),
                    internal: Box::new(internal_struct),
                }
            }
            AbiType::Ref(a) => self.make_struct_property(name, a.as_ref()),
        }
    }
}

#[derive(Clone)]
enum StructProperty {
    Simple {
        name: Option<String>,
        type_name: syn::Type,
    },
    Tuple {
        name: String,
        //_fields: Vec<StructProperty>,
    },
    Array {
        name: String,
        internal: Box<StructProperty>,
    },
    Option {
        name: String,
        internal: Box<StructProperty>,
    },
    HashMap {
        name: String,
        key: Box<StructProperty>,
        value: Box<StructProperty>,
    },
}

impl StructProperty {
    pub fn type_name_quote(&self) -> syn::Type {
        match self {
            StructProperty::Simple { type_name, .. } => type_name.clone(),
            StructProperty::Tuple { name, .. } => syn::parse_str(&name.to_camel()).unwrap(),
            StructProperty::Array { internal, .. } => {
                let ty = internal.type_name_quote();
                syn::parse_quote!(Vec<#ty>)
            }
            StructProperty::Option { internal, .. } => {
                let ty = internal.type_name_quote();
                syn::parse_quote!(Option<#ty>)
            }
            StructProperty::HashMap { key, value, .. } => {
                let key = key.type_name_quote();
                let value = value.type_name_quote();
                syn::parse_quote!(std::collections::HashMap<#key, #value>)
            }
        }
    }

    pub fn name(&self) -> String {
        match self {
            StructProperty::Simple { name, .. } => {
                let name = name.clone();
                name.unwrap_or_default()
            }
            StructProperty::Tuple { name, .. } => name.clone(),
            StructProperty::Array { name, .. } => name.clone(),
            StructProperty::Option { name, .. } => name.clone(),
            StructProperty::HashMap { name, .. } => name.clone(),
        }
    }
}

fn quote_abi_type(ty: &AbiType) -> proc_macro2::TokenStream {
    let quote: proc_macro2::TokenStream = match ty.clone() {
        AbiType::String => {
            let ty: syn::Type = syn::parse_quote!(everscale_types::abi::AbiType::String);
            quote! {
                #ty
            }
        }
        AbiType::Address => {
            let ty: syn::Type = syn::parse_quote!(everscale_types::abi::AbiType::Address);
            quote! {
                #ty
            }
        }
        AbiType::Bool => syn::parse_quote!(everscale_types::abi::AbiType::Bool),
        AbiType::Bytes => syn::parse_quote!(everscale_types::abi::AbiType::Bytes),
        AbiType::FixedBytes(size) => {
            syn::parse_quote!(everscale_types::abi::AbiType::FixedBytes(#size))
        }
        AbiType::Cell => syn::parse_quote!(everscale_types::abi::AbiType::Cell),
        AbiType::Token => syn::parse_quote!(everscale_types::abi::AbiType::Token),
        AbiType::Int(value) => quote! {
            everscale_types::abi::AbiType::Int(#value)
        },
        AbiType::Uint(value) => {
            quote! {
                everscale_types::abi::AbiType::Uint(#value)
            }
        }
        AbiType::VarInt(value) => {
            let val = value.get();
            quote! {
                everscale_types::abi::AbiType::Int(core::num::nonzero::NonZeroU8(#val))
            }
        }
        AbiType::VarUint(value) => {
            let val = value.get();
            quote! {
                everscale_types::abi:AbiType::Uint(core::num::nonzero::NonZeroU8(#val))
            }
        }
        AbiType::Tuple(tuple) => {
            let mut tuple_properties = Vec::new();

            for i in tuple.iter() {
                let name_abi_quote = make_abi_type(i.name.as_ref(), i.ty.clone());
                tuple_properties.push(name_abi_quote);
            }

            quote! {
                everscale_types::abi::AbiType::Tuple(std::sync::Arc::new([ #(#tuple_properties),*]))
            }
        }
        AbiType::Array(ty) => {
            let ty = quote_abi_type(&ty);
            quote! {
                everscale_types::abi::AbiType::Array(std::sync::Arc::new(#ty))
            }
        }
        AbiType::FixedArray(ty, size) => {
            let ty = quote_abi_type(&ty);
            quote! {
                everscale_types::abi:AbiType::FixedArray(std::sync::Arc<#ty>, #size)
            }
        }
        AbiType::Map(key, value) => {
            let key_type: proc_macro2::TokenStream = match key {
                PlainAbiType::Address => {
                    let ty: syn::Type =
                        syn::parse_quote!(everscale_types::abi::PlainAbiType::Address);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Bool => {
                    let ty: syn::Type = syn::parse_quote!(everscale_types::abi::PlainAbiType::Bool);
                    quote! {
                        #ty
                    }
                }
                PlainAbiType::Uint(value) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Uint(#value)
                    }
                }
                PlainAbiType::Int(value) => {
                    quote! {
                        everscale_types::abi::PlainAbiType::Int(#value)
                    }
                }
            };

            let value_type = quote_abi_type(&value);
            syn::parse_quote!(everscale_types::abi::AbiType::Map(#key_type, std::sync::Arc::new(#value_type)))
        }
        AbiType::Optional(_) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Optional(std::sync::Arc<#ty>)
            }
        }
        AbiType::Ref(_) => {
            let ty = quote_abi_type(ty);
            quote! {
                everscale_types::abi::AbiType::Ref(std::sync::Arc<#ty>)
            }
        }
    };
    quote
}

fn quote_abi_value(name: &str) -> proc_macro2::TokenStream {
    let name_ident = format_ident!("{}", name.to_snake());
    quote! {
        everscale_types::abi::IntoAbi::as_abi(&self.#name_ident)
    }
}

fn make_abi_type(name: &str, abi_type: AbiType) -> proc_macro2::TokenStream {
    let abi_type = quote_abi_type(&abi_type);

    quote! {
        NamedAbiType::new(#name, #abi_type)
    }
}
