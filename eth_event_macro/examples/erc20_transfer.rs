use eth_event_macro::event;

#[event("event Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

fn main() {
    use ethereum_types::{H256, U256, U64};
    use web3::types::Address;
    // let addr = Address::from("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into());
    // let zero = Address::zero();
    // println!("Str address {:?}", zero.to_fixed_bytes());
    println!("{}", Erc20Transfer::event().signature());
    let e = Erc20Transfer {
        data: (Address::zero(), Address::zero(), U256::zero()),
        block_number: U64::zero(),
        transaction_hash: H256::zero(),
        address: Address::zero(),
        log_index: U256::zero(),
    };
    println!("{:?}", e);
}
