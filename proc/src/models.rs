pub struct FunctionDescriptionTokens {
    pub body: proc_macro2::TokenStream,
    pub input: proc_macro2::TokenStream,
    pub output: proc_macro2::TokenStream,

    pub inner_models: Vec<proc_macro2::TokenStream>,
}
