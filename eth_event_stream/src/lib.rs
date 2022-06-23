pub mod data_feed;
pub mod sink;
pub mod stream;

/// why is this not a thing?
/// just address from a human readable string please
pub fn address(_addr: &str) -> ethereum_types::Address {
    let addr_string = _addr.to_string();
    // copy hehe
    let mut addr = addr_string.as_str();
    if _addr.starts_with("0x") {
        addr = &addr[2..];
    }
    ethereum_types::Address::from_slice(hex::decode(addr).unwrap().as_slice())
}
