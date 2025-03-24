use case::CaseExt;
use everscale_types::abi::NamedAbiType;

use crate::{quote_abi_type, quote_abi_value};
use quote::{format_ident, quote};

pub struct TraitImplGen;

impl TraitImplGen {
    pub fn new() -> Self {
        Self
    }

    pub fn implement_traits(
        &self,
        name: &str,
        properties: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let with_abi_type_impls = self.implement_with_abi_type(name, properties);
        let into_abi_impls = self.implement_into_abi(name, properties);
        let from_abi_impls = self.implement_from_abi(name, properties);

        quote! {
            //WithAbiType implementations
            #with_abi_type_impls

            //IntoAbi implementations
            #into_abi_impls

            //FromAbi implementations
            #from_abi_impls
        }
    }

    pub fn implement_with_abi_type(
        &self,
        struct_name: &str,
        properties: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let name_ident = format_ident!("{}", struct_name);

        let props_quote: Vec<_> = properties
            .iter()
            .map(|x| {
                let name = x.name.as_ref();
                let quote_abi_type = quote_abi_type(&x.ty);

                quote! {
                    NamedAbiType::new(#name, #quote_abi_type)
                }
            })
            .collect();

        let props = quote! {
            [ #(#props_quote),* ]
        };

        if !properties.is_empty() {
            let properties_count = properties.len();
            let tuple_tokens = quote! {
                let properties: [NamedAbiType; #properties_count] = #props;
            };

            return quote! {
                impl WithAbiType for #name_ident {
                    fn abi_type() -> AbiType {
                         #tuple_tokens
                         AbiType::Tuple(std::sync::Arc::new(properties))
                    }
                }
            };
        }

        proc_macro2::TokenStream::new()
    }

    pub fn implement_from_abi(
        &self,
        struct_name: &str,
        properites: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let struct_name_ident = format_ident!("{}", struct_name);
        let props: Vec<proc_macro2::TokenStream> = properites
            .iter()
            .map(|x| {
                let ident = format_ident!("{}", x.name.to_snake());
                quote! {
                    #ident: everscale_types::abi::FromAbiIter::<#struct_name_ident>::next_value(&mut iterator)?,
                }
            })
            .collect();

        let props_vec = quote! {
            #(#props)*
        };

        if !props.is_empty() {
            return quote! {
                impl FromAbi for #struct_name_ident {
                    fn from_abi(value: AbiValue) -> Result<Self> {
                        match value {
                            AbiValue::Tuple(properties) =>  {
                                let mut iterator = properties.into_iter();
                                Ok(
                                    #struct_name_ident {
                                        #props_vec
                                    }
                                )

                            },
                            _ => Err(anyhow::Error::from(
                                everscale_types::abi::error::AbiError::TypeMismatch {
                                    expected: std::boxed::Box::<str>::from("tuple"),
                                    ty: value.display_type().to_string().into(),
                                },
                            )),
                        }
                    }
                }
            };
        }

        proc_macro2::TokenStream::new()
    }

    pub fn implement_into_abi(
        &self,
        struct_name: &str,
        properties: &[NamedAbiType],
    ) -> proc_macro2::TokenStream {
        let mut props: Vec<proc_macro2::TokenStream> = Vec::new();
        let struct_name_ident = format_ident!("{}", struct_name);

        for prop in properties {
            let name = prop.name.clone();
            let quote_name = name.as_ref();
            let quote_abi_value = quote_abi_value(&name);
            let quote = quote! {
                NamedAbiValue {
                    name: {
                        let arc: std::sync::Arc<str> = std::sync::Arc::from(#quote_name);
                        arc
                    },
                    value: #quote_abi_value,
                }
            };
            props.push(quote);
        }

        if !props.is_empty() {
            return quote! {
                impl IntoAbi for #struct_name_ident {
                    fn as_abi(&self) -> AbiValue {
                        AbiValue::Tuple(vec![#(#props),*])
                    }

                    fn into_abi(self) -> AbiValue
                    where
                        Self: Sized,
                    {
                         AbiValue::Tuple(vec![#(#props),*])
                    }
                }
            };
        }

        proc_macro2::TokenStream::new()
    }
}
