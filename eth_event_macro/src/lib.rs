use eth_event_stream::events::event_from_declaration;
use ethabi::{Event, EventParam, ParamType};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::parse::Parser;
use syn::{parse_macro_input, DeriveInput};

/// matches the event params from declaration to corresponding types that need to be in the tuple
/// made by the tuple
/// TODO: extend to all solidity event types
fn match_event_param_to_rust_type(e: &EventParam) -> String {
    let eth_type = match e.kind {
        ParamType::Address => "Address".to_string(),
        ParamType::Uint(x) => format!("U{}", x),
        _ => panic!("param type {:?} is not supported", e),
    };
    format!("ethereum_types::{}", eth_type).to_string()
}

/// returns token stream to be parsed as a struct field that
/// that represents the struct data field holding a tuple of event arguments
/// in the same order as the event definition
fn dynamic_fields_from_data(event: &Event) -> String {
    let mut params_vec: Vec<String> = vec![];
    let mut doc_vec: Vec<String> = vec![];
    for e in &event.inputs {
        params_vec.push(match_event_param_to_rust_type(e));
        doc_vec.push(format!("{}: {:?}", e.name.clone(), e.kind))
    }
    format!(
        "
    /// This is the parsed data tuple of the event
    /// 
    /// Its signature is:
    /// 
    /// {}
    data: ({})",
        doc_vec.join(", "),
        params_vec.join(",")
    )
}

#[proc_macro_attribute]
pub fn event(declaration: TokenStream, input: TokenStream) -> TokenStream {
    // FIRST get the defined solidity ethereum event
    let declaration_str = declaration.to_string();
    println!("event {}", declaration_str);
    let event = event_from_declaration(declaration_str.replace("\"", "")).unwrap();

    // NEXT parse the input struct AST
    let mut ast = parse_macro_input!(input as DeriveInput);

    // ADD fields to the AST
    // dynamic fields:
    // - the user defined solidity event params (will be under `params`)
    // static fields:
    // - block_number: U64,
    // - transaction_hash: H256,
    // - address: Address,
    // - log_index: U256,
    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => match &mut struct_data.fields {
            syn::Fields::Named(fields) => {
                let fields_to_add = vec![
                    quote! {block_number: ethereum_types::U64},
                    quote! {transaction_hash: ethereum_types::H256},
                    quote! {address: ethereum_types::Address},
                    quote! {log_index: ethereum_types::U256},
                ];
                // add the static fields
                for field in fields_to_add {
                    fields
                        .named
                        .push(syn::Field::parse_named.parse2(field).unwrap());
                }
                let dynamic_data_field = dynamic_fields_from_data(&event);
                // add the dynamic fields
                let data_field = syn::Field::parse_named
                    .parse2(dynamic_data_field.parse().unwrap())
                    .unwrap();
                // add the dynamic field
                fields.named.push(data_field)
            }
            _ => (),
        },
        _ => panic!("`event` has to be used with structs."),
    }

    // let _type = _args_str.trim();
    // let types: Vec<&str> = _args_str.split(",").into_iter().map(|s| s.trim()).collect();
    // let mut pub_field = "pub a:".to_string();
    // let field_type: proc_macro::TokenStream = _args_str.parse().unwrap();
    // pub_field.push_str(_args_str.as_str());

    // println!("{:?}", types);
    // println!("{:?}", _type);
    // // println!("{:?}", tuple_tokenstream);
    // println!("ASEFASFE");
    // for a in &ast.attrs {
    //     println!("{}", a.tokens.to_string())
    // }
    // match &mut ast.data {
    //     syn::Data::Struct(ref mut struct_data) => {
    //         match &mut struct_data.fields {
    //             syn::Fields::Named(fields) => {
    //                 // add `pub a` field to the struct
    //                 fields.named.push(
    //                     syn::Field::parse_named
    //                         .parse2(pub_field.parse().unwrap())
    //                         .unwrap(),
    //                 );
    //             }
    //             _ => (),
    //         }
    //     }
    //     _ => panic!("`add_field` has to be used with structs "),
    // }

    // this works btw
    // let empty: TokenStream = quote! {impl Foo {fn say_hello() {println!("helo")}}}.into();
    // let mut impl_ast = parse_macro_input!(empty as DeriveInput);

    let mut output: TokenStream = quote! { #ast }.into();

    // output.extend(empty.into_iter());

    // match &mut token_stream {
    //     TokenStream(bridge) => {

    //     }
    // }
    output
}
