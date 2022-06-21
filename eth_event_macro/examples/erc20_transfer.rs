use eth_event_macro::event;

#[event("event Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

fn main() {
    use ethereum_types::{Address, H256, U256, U64};
    Erc20Transfer::event();
    let e = Erc20Transfer {
        data: (Address::zero(), Address::zero(), U256::zero()),
        block_number: U64::zero(),
        transaction_hash: H256::zero(),
        address: Address::zero(),
        log_index: U256::zero(),
    };
    println!("{:?}", e);
}
