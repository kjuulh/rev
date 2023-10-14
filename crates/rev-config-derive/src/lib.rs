use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(AppConfig)]
pub fn app_config_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as TokenStream2);

    let ast = syn::parse2::<DeriveInput>(input.clone()).unwrap();

    let struct_name = &ast.ident;

    let expanded = quote! {
        impl AppConfig for #struct_name {
            fn my_method(&self) {
                println!("hello from: {}", stringify!(#struct_name));
            }
        }
    };

    TokenStream::from(expanded)
}
