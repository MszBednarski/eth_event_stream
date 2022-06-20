use eth_event_stream::{
    data_feed::block::BlockNotify,
    rich_log::RichLog,
    sink::Sink,
    stream::{http_web3, Stream},
};
use ethabi::Token;
use ethereum_types::{Address, H256, U256, U64};
use std::{collections::HashMap, env, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug)]
struct Erc20Transfer {
    from: Address,
    to: Address,
    value: U256,
    block_number: U64,
    transaction_hash: H256,
    address: Address,
    log_index: U256,
}

fn cast_addr(t: Token) -> Address {
    match t {
        ethabi::Token::Address(a) => Address::from_slice(a.as_bytes()),
        _ => panic!("Could not cast {:?} to address", t),
    }
}

fn cast_u256(t: Token) -> U256 {
    match t {
        ethabi::Token::Uint(v) => U256::from(v),
        _ => panic!("Could not cast {:?} to address", t),
    }
}

fn process_erc20_transfer(
    at: &Address,
    entries: HashMap<Address, Vec<RichLog>>,
) -> Vec<Erc20Transfer> {
    let logs = entries.get(at).unwrap();
    logs.to_owned()
        .iter()
        .map(|l| match &l {
            RichLog {
                params,
                address,
                block_number,
                transaction_hash,
                log_index,
                ..
            } => match &params[..] {
                [from, to, value] => Some(Erc20Transfer {
                    from: cast_addr(from.value.to_owned()),
                    to: cast_addr(to.value.to_owned()),
                    value: cast_u256(value.value.to_owned()),
                    address: *address,
                    block_number: *block_number,
                    transaction_hash: *transaction_hash,
                    log_index: *log_index,
                }),
                _ => None,
            },
        })
        .filter_map(|a| a)
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = http_web3(&http_url)?;
    let usdc: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    let contract_address = Address::from_slice(hex::decode(&usdc[2..])?.as_slice());
    let cur_block = web3.eth().block_number().await?.as_u64();
    let from_block = cur_block - 40;
    let to_block = cur_block + 6;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    let notify = BlockNotify::new(&http_url, &ws_url).await?;
    let sink = Arc::new(Mutex::new(Sink::new(vec![contract_address], from_block)));
    let mut stream = Stream::new(
        http_url,
        ws_url,
        contract_address,
        from_block,
        to_block,
        "event Transfer(address indexed from, address indexed to, uint value)",
        notify.subscribe(),
        sink.clone(),
    )
    .await?;
    stream.block_step(5);

    tokio::spawn(async move { stream.block_stream().await });

    let mut cur = from_block;
    loop {
        Sink::wait_until_at(sink.clone(), cur).await;
        let res = sink.lock().await.flush_up_to(cur);
        for (number, entry) in res {
            let transfers = process_erc20_transfer(&contract_address, entry);
            println!("==> Block {}. Got logs. {}", number, transfers.len());
            println!("First log {:?}", transfers.first());
        }
        cur += 1;
    }
}
