pub use transfer_mod::*;
#[allow(clippy::too_many_arguments, non_camel_case_types)]
pub mod transfer_mod {
    #![allow(clippy::enum_variant_names)]
    #![allow(dead_code)]
    #![allow(clippy::type_complexity)]
    #![allow(unused_imports)]
    ///Transfer was auto-generated with ethers-rs Abigen. More information at: https://github.com/gakonst/ethers-rs
    use std::sync::Arc;
    use ethers::core::{
        abi::{Abi, Token, Detokenize, InvalidOutputType, Tokenizable},
        types::*,
    };
    use ethers::contract::{
        Contract,
        builders::{ContractCall, Event},
        Lazy,
    };
    use ethers::providers::Middleware;
    pub static TRANSFER_ABI: ethers::contract::Lazy<ethers::core::abi::Abi> =
        ethers::contract::Lazy::new(|| {
            ethers :: core :: abi :: parse_abi_str ("[\n        event Transfer(address indexed from, address indexed to, uint value)\n    ]") . expect ("invalid abi")
        });
    pub struct Transfer<M>(ethers::contract::Contract<M>);
    impl<M> Clone for Transfer<M> {
        fn clone(&self) -> Self {
            Transfer(self.0.clone())
        }
    }
    impl<M> std::ops::Deref for Transfer<M> {
        type Target = ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M: ethers::providers::Middleware> std::fmt::Debug for Transfer<M> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_tuple("Transfer").field(&self.address()).finish()
        }
    }
    impl<M: ethers::providers::Middleware> Transfer<M> {
        /// Creates a new contract instance with the specified `ethers`
        /// client at the given `Address`. The contract derefs to a `ethers::Contract`
        /// object
        pub fn new<T: Into<ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            ethers::contract::Contract::new(address.into(), TRANSFER_ABI.clone(), client).into()
        }
        ///Gets the contract's `Transfer` event
        pub fn transfer_filter(&self) -> ethers::contract::builders::Event<M, TransferFilter> {
            self.0.event()
        }
        /// Returns an [`Event`](#ethers_contract::builders::Event) builder for all events of this contract
        pub fn events(&self) -> ethers::contract::builders::Event<M, TransferFilter> {
            self.0.event_with_filter(Default::default())
        }
    }
    impl<M: ethers::providers::Middleware> From<ethers::contract::Contract<M>> for Transfer<M> {
        fn from(contract: ethers::contract::Contract<M>) -> Self {
            Self(contract)
        }
    }
    #[ethevent(name = "Transfer", abi = "Transfer(address,address,uint256)")]
    pub struct TransferFilter {
        #[ethevent(indexed)]
        pub from: ethers::core::types::Address,
        #[ethevent(indexed)]
        pub to: ethers::core::types::Address,
        pub value: ethers::core::types::U256,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for TransferFilter {
        #[inline]
        fn clone(&self) -> TransferFilter {
            TransferFilter {
                from: ::core::clone::Clone::clone(&self.from),
                to: ::core::clone::Clone::clone(&self.to),
                value: ::core::clone::Clone::clone(&self.value),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for TransferFilter {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field3_finish(
                f,
                "TransferFilter",
                "from",
                &&self.from,
                "to",
                &&self.to,
                "value",
                &&self.value,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for TransferFilter {
        #[inline]
        fn default() -> TransferFilter {
            TransferFilter {
                from: ::core::default::Default::default(),
                to: ::core::default::Default::default(),
                value: ::core::default::Default::default(),
            }
        }
    }
    impl ::core::marker::StructuralEq for TransferFilter {}
    #[automatically_derived]
    impl ::core::cmp::Eq for TransferFilter {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<ethers::core::types::Address>;
            let _: ::core::cmp::AssertParamIsEq<ethers::core::types::Address>;
            let _: ::core::cmp::AssertParamIsEq<ethers::core::types::U256>;
        }
    }
    impl ::core::marker::StructuralPartialEq for TransferFilter {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for TransferFilter {
        #[inline]
        fn eq(&self, other: &TransferFilter) -> bool {
            self.from == other.from && self.to == other.to && self.value == other.value
        }
        #[inline]
        fn ne(&self, other: &TransferFilter) -> bool {
            self.from != other.from || self.to != other.to || self.value != other.value
        }
    }
    impl ethers::core::abi::AbiType for TransferFilter {
        fn param_type() -> ethers::core::abi::ParamType {
            ethers::core::abi::ParamType::Tuple(<[_]>::into_vec(
                #[rustc_box]
                ::alloc::boxed::Box::new([
                    <ethers::core::types::Address as ethers::core::abi::AbiType>::param_type(),
                    <ethers::core::types::Address as ethers::core::abi::AbiType>::param_type(),
                    <ethers::core::types::U256 as ethers::core::abi::AbiType>::param_type(),
                ]),
            ))
        }
    }
    impl ethers::core::abi::AbiArrayType for TransferFilter {}
    impl ethers::core::abi::Tokenizable for TransferFilter
    where
        ethers::core::types::Address: ethers::core::abi::Tokenize,
        ethers::core::types::Address: ethers::core::abi::Tokenize,
        ethers::core::types::U256: ethers::core::abi::Tokenize,
    {
        fn from_token(
            token: ethers::core::abi::Token,
        ) -> Result<Self, ethers::core::abi::InvalidOutputType>
        where
            Self: Sized,
        {
            if let ethers::core::abi::Token::Tuple(tokens) = token {
                if tokens.len() != 3usize {
                    return Err(ethers::core::abi::InvalidOutputType({
                        let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                            &["Expected ", " tokens, got ", ": "],
                            &[
                                ::core::fmt::ArgumentV1::new_display(&3usize),
                                ::core::fmt::ArgumentV1::new_display(&tokens.len()),
                                ::core::fmt::ArgumentV1::new_debug(&tokens),
                            ],
                        ));
                        res
                    }));
                }
                let mut iter = tokens.into_iter();
                Ok(Self {
                    from: ethers::core::abi::Tokenizable::from_token(iter.next().unwrap())?,
                    to: ethers::core::abi::Tokenizable::from_token(iter.next().unwrap())?,
                    value: ethers::core::abi::Tokenizable::from_token(iter.next().unwrap())?,
                })
            } else {
                Err(ethers::core::abi::InvalidOutputType({
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["Expected Tuple, got "],
                        &[::core::fmt::ArgumentV1::new_debug(&token)],
                    ));
                    res
                }))
            }
        }
        fn into_token(self) -> ethers::core::abi::Token {
            ethers::core::abi::Token::Tuple(<[_]>::into_vec(
                #[rustc_box]
                ::alloc::boxed::Box::new([
                    self.from.into_token(),
                    self.to.into_token(),
                    self.value.into_token(),
                ]),
            ))
        }
    }
    impl ethers::core::abi::TokenizableItem for TransferFilter
    where
        ethers::core::types::Address: ethers::core::abi::Tokenize,
        ethers::core::types::Address: ethers::core::abi::Tokenize,
        ethers::core::types::U256: ethers::core::abi::Tokenize,
    {
    }
    impl ethers::contract::EthEvent for TransferFilter {
        fn name() -> ::std::borrow::Cow<'static, str> {
            "Transfer".into()
        }
        fn signature() -> ethers::core::types::H256 {
            ethers::core::types::H256([
                221, 242, 82, 173, 27, 226, 200, 155, 105, 194, 176, 104, 252, 55, 141, 170, 149,
                43, 167, 241, 99, 196, 161, 22, 40, 245, 90, 77, 245, 35, 179, 239,
            ])
        }
        fn abi_signature() -> ::std::borrow::Cow<'static, str> {
            "Transfer(address,address,uint256)".into()
        }
        fn decode_log(log: &ethers::core::abi::RawLog) -> Result<Self, ethers::core::abi::Error>
        where
            Self: Sized,
        {
            let ethers::core::abi::RawLog { data, topics } = log;
            let event_signature = topics.get(0).ok_or(ethers::core::abi::Error::InvalidData)?;
            if event_signature != &Self::signature() {
                return Err(ethers::core::abi::Error::InvalidData);
            }
            let topic_types = <[_]>::into_vec(
                #[rustc_box]
                ::alloc::boxed::Box::new([
                    ethers::core::abi::ParamType::Address,
                    ethers::core::abi::ParamType::Address,
                ]),
            );
            let data_types = [ethers::core::abi::ParamType::Uint(256usize)];
            let flat_topics = topics
                .iter()
                .skip(1)
                .flat_map(|t| t.as_ref().to_vec())
                .collect::<Vec<u8>>();
            let topic_tokens = ethers::core::abi::decode(&topic_types, &flat_topics)?;
            if topic_tokens.len() != topics.len() - 1 {
                return Err(ethers::core::abi::Error::InvalidData);
            }
            let data_tokens = ethers::core::abi::decode(&data_types, data)?;
            let tokens: Vec<_> = topic_tokens
                .into_iter()
                .chain(data_tokens.into_iter())
                .collect();
            ethers::core::abi::Tokenizable::from_token(ethers::core::abi::Token::Tuple(tokens))
                .map_err(|_| ethers::core::abi::Error::InvalidData)
        }
        fn is_anonymous() -> bool {
            false
        }
    }
    impl ::std::fmt::Display for TransferFilter {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &[""],
                &[::core::fmt::ArgumentV1::new_debug(&self.from)],
            ))?;
            f.write_fmt(::core::fmt::Arguments::new_v1(&[", "], &[]))?;
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &[""],
                &[::core::fmt::ArgumentV1::new_debug(&self.to)],
            ))?;
            f.write_fmt(::core::fmt::Arguments::new_v1(&[", "], &[]))?;
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &[""],
                &[::core::fmt::ArgumentV1::new_debug(&self.value)],
            ))?;
            Ok(())
        }
    }
}