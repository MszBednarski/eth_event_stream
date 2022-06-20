use proc_macro::{self, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::parse::Parser;
use syn::{parse_macro_input, DeriveInput};