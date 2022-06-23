mod events;
use crate::events::event_from_declaration;
use ethabi::{Event, EventParam, ParamType};
use proc_macro::{self, TokenStream};
use quote::quote;
use std::collections::HashSet;
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
        _ => todo!(),
    };
    format!("ethereum_types::{}", eth_type).to_string()
}

fn event_param_to_ethabi_type(e: &EventParam) -> String {
    let ethabi_type = match e.kind {
        ParamType::Address => "Address".to_string(),
        ParamType::Uint(x) => format!("Uint({})", x),
        _ => todo!(),
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

/// gets data field in parsing of the event
/// Returns a vector of data getting expressions
fn get_data_field_parsing(struct_name: &String, e: &Event) -> (Vec<String>, Vec<String>) {
    let mut data_expressions: Vec<String> = Vec::new();
    let mut cast_definitions_set: HashSet<String> = HashSet::new();
    for (index, param) in e.inputs.iter().enumerate() {
        let (name, stream) = match_token_to_cast_fun(&param.kind);
        cast_definitions_set.insert(stream);

        data_expressions.push(format!(
            "{}::{}(&parsed_log.params.get({}).unwrap().value)",
            struct_name, name, index
        ));
    }
    (data_expressions, cast_definitions_set.into_iter().collect())
}

fn match_token_to_cast_fun(t: &ethabi::ParamType) -> (String, String) {
    let cast_addr = quote! {
        fn cast_addr(t: &ethabi::Token) -> ethabi::Address {
            match t {
                ethabi::Token::Address(a) => ethabi::Address::from_slice(a.as_bytes()),
                _ => panic!("Could not cast {:?} to address", t),
            }
        }
    };
    let cast_u256 = quote! {
        fn cast_u256(t: &ethabi::Token) -> web3::types::U256 {
            match t {
                ethabi::Token::Uint(v) => web3::types::U256::from(v),
                _ => panic!("Could not cast {:?} to address", t),
            }
        }
    };
    let res = match t {
        ParamType::Address => ("cast_addr", cast_addr),
        ParamType::Uint(256) => ("cast_u256", cast_u256),
        _ => todo!(),
    };
    match res {
        (s, t) => (s.to_string(), format!("{}", t)),
    }
}

/// impl From web3 log implementation
/// Returns the From<web3::types::Log> impl and additional function definitions
fn get_from_web3_log_impl(struct_name: &String, e: &Event) -> (String, Vec<String>) {
    let (data_expressions, cast_definitions) = get_data_field_parsing(struct_name, e);

    // the implementation of from
    // a lot of boilerplate
    (
        format!(
            "
   impl std::convert::From<web3::types::Log> for {} {{
        fn from(log: web3::types::Log) -> Self {{
            let raw_log = ethabi::RawLog {{
                topics: log.topics.clone(),
                data: log.data.0.clone(),
            }};
            let parsed_log = {}::event().parse_log(raw_log).unwrap();
            let data = (
                {}
            );
            {} {{
                block_number: log.block_number.unwrap(),
                transaction_hash: log.transaction_hash.unwrap(),
                address: log.address,
                log_index: log.log_index.unwrap(),
                data,
            }}
        }}
    }}",
            struct_name,
            struct_name,
            data_expressions.join(","),
            struct_name
        ),
        cast_definitions,
    )
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

    let (from_impl, cast_definitions) = get_from_web3_log_impl(&struct_name, &event);
    // create the impl
    let implementation = format!(
        "
   impl {} {{
        {}
        {}
   }} 
    ",
        struct_name,
        event.into_syntax_string(),
        cast_definitions.join("\n")
    );

    // println!("{}", from_impl);
    // println!("{}", res.join("\n"));

    // add an impl for our event struct
    let _impl: TokenStream = implementation.parse().unwrap();
    // let mut impl_ast = parse_macro_input!(_impl as DeriveInput);
    let mut output: TokenStream = quote! { #ast }.into();
    // add the impl after the struct scope
    output.extend(_impl.into_iter());
    let from_impl_token_stream: TokenStream = from_impl.parse().unwrap();
    // add the from implementation
    output.extend(from_impl_token_stream.into_iter());
    // return the transformed syntax
    output
}
