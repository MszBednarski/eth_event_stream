mod events;
use crate::events::event_from_declaration;
use ethabi::{Event, EventParam, ParamType};
use proc_macro::{self, TokenStream};
use quote::quote;
use std::convert::Into;
use syn::parse::Parser;
use syn::{parse_macro_input, DeriveInput};


/// matches the event params from declaration to corresponding types that need to be in the tuple
/// made by the tuple
/// TODO: extend to all solidity event types
fn event_param_to_ethereum_type(e: &EventParam) -> String {
    let eth_type = match e.kind {
        ParamType::Address => "Address".to_string(),
        ParamType::Uint(x) => format!("U{}", x),
        _ => panic!("param type {:?} is not implemented", e),
    };
    format!("ethereum_types::{}", eth_type).to_string()
}

fn event_param_to_ethabi_type(e: &EventParam) -> String {
    let ethabi_type = match e.kind {
        ParamType::Address => "Address".to_string(),
        ParamType::Uint(x) => format!("Uint({})", x),
        _ => panic!("param type {:?} is not implemented", e),
    };
    format!("ethabi::ParamType::{}", ethabi_type)
}

/// outputs a syntax string representation of the instantiation of the struct
/// given struct instance to move an instance of event from function to macro output
trait IntoSyntaxString {
    fn into_syntax_string(&self) -> String;
}

impl IntoSyntaxString for ethabi::Event {
    fn into_syntax_string(&self) -> String {
        let mut event_inputs: Vec<String> = Vec::new();
        // create strings of all event params
        for input in &self.inputs {
            // push each param converted to string
            event_inputs.push(format!(
                "
ethabi::EventParam {{
    name: \"{}\".to_string(),
    kind: {},
    indexed: {},
}}",
                input.name,
                event_param_to_ethabi_type(input),
                input.indexed
            ))
        }
        let event_declaration = format!(
            "
pub fn event() -> ethabi::Event {{
    ethabi::Event {{
        name: \"{}\".to_string(),
        inputs: vec![{}],
        anonymous: {},
    }}
}}
",
            self.name,
            event_inputs.join(","),
            self.anonymous
        );
        event_declaration
    }
}

/// returns token stream to be parsed as a struct field that
/// that represents the struct data field holding a tuple of event arguments
/// in the same order as the event definition
fn dynamic_fields_from_data(event: &Event) -> String {
    let mut params_vec: Vec<String> = vec![];
    // used to tell the user what is the sig of the eth event on the data field
    let mut doc_vec: Vec<String> = vec![];
    for e in &event.inputs {
        params_vec.push(event_param_to_ethereum_type(e));
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
    // println!("event {}", declaration_str);
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

    // get the name of the struct
    let struct_name = ast.ident.to_string();

    // println!("{:?}", event);
    // println!("{}", event.into_syntax_string());

    // create the impl
    let implementation = format!(
        "
   impl {} {{
        {}
   }} 
    ",
        struct_name,
        event.into_syntax_string()
    );

    // add an impl for our event struct
    let _impl: TokenStream = implementation.parse().unwrap();
    // let mut impl_ast = parse_macro_input!(_impl as DeriveInput);
    let mut output: TokenStream = quote! { #ast }.into();
    // add the impl after the struct scope
    output.extend(_impl.into_iter());
    // return the transformed syntax
    output
}
