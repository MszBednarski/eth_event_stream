use eth_event_macro::event;

#[event("event Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

impl std::convert::From<web3::types::Log> for Erc20Transfer {
    fn cast_addr(t : ethabi :: Token) -> ethabi :: Address
{
match t
{
    ethabi :: Token :: Address(a) => ethabi :: Address ::
    from_slice(a.as_bytes()), _ => panic!
    ("Could not cast {:?} to address", t),
}
}
fn cast_addr(t : ethabi :: Token) -> ethabi :: Address
{
match t
{
    ethabi :: Token :: Address(a) => ethabi :: Address ::
    from_slice(a.as_bytes()), _ => panic!
    ("Could not cast {:?} to address", t),
}
}
fn cast_u256(t : ethabi :: Token) -> web3 :: U256
{
match t
{
    ethabi :: Token :: Uint(v) => web3 :: U256 :: from(v), _ => panic!
    ("Could not cast {:?} to address", t),
}
}
    fn from(log: web3::types::Log) -> Self {
        let raw_log = ethabi::RawLog {
            topics: log.topics.clone(),
            data: log.data.0.clone(),
        };
        let parsed_log = Erc20Transfer::event().parse_log(raw_log).unwrap();
        let data = (
            Erc20Transfer::cast_addr(parsed_log.params.get(0).unwrap().value),Erc20Transfer::cast_addr(parsed_log.params.get(1).unwrap().value),Erc20Transfer::cast_u256(parsed_log.params.get(2).unwrap().value)
        );
        Erc20Transfer {
            block_number: log.block_number.unwrap(),
            transaction_hash: log.transaction_hash.unwrap(),
            address: log.address,
            log_index: log.log_index.unwrap(),
            data,
        }
    }
}

fn main() {
    use ethereum_types::{H256, U256, U64};
    use web3::types::Address;
    // let addr = Address::from("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into());
    let zero = Address::zero();
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
