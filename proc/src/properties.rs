use case::CaseExt;

#[derive(Clone)]
pub enum StructProperty {
    Simple {
        name: Option<String>,
        type_name: Box<syn::Type>,
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
            StructProperty::Simple { type_name, .. } => *type_name.clone(),
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
