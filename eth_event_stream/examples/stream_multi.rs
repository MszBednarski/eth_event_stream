use eth_event_stream::{
    data_feed::block::BlockNotify,
    sink::Sink,
    stream::{http_web3, Stream},
};
use ethereum_types::Address;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use web3::types::Log;

#[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

async fn process_batch(
    block_target: u64,
    address: &Address,
    sink: &Arc<Mutex<Sink<Address, Log>>>,
) {
    println!("{}", block_target);
    Sink::wait_until_included(sink.clone(), block_target).await;
    let res = sink.lock().await.flush_including(block_target);
    for (number, entry) in res {
        let transfers: Vec<Erc20Transfer> = entry
            .get(address)
            .unwrap()
            .iter()
            .map(|l| Erc20Transfer::from(l.to_owned()))
            .collect();
        println!("==> Block {}. Got logs. {}", number, transfers.len());
        println!("First log {:?}", transfers.first());
    }
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
    let to_block = cur_block + 4;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    let notify = BlockNotify::new(&http_url, &ws_url).await?;
    let sink = Arc::new(Mutex::new(Sink::new(vec![contract_address], from_block)));
    let mut stream = Stream::new(
        http_url,
        contract_address,
        from_block,
        to_block,
        Erc20Transfer::event(),
        notify.subscribe(),
        sink.clone(),
    )
    .await?;
    stream.confirmation_blocks(3);

    tokio::spawn(async move { stream.block_stream().await });

    let step = 5;
    for cur in ((from_block + step)..=to_block).step_by(5) {
        process_batch(cur, &contract_address, &sink).await
    }
    if to_block - from_block % step != 0 {
        process_batch(to_block, &contract_address, &sink).await
    }
    Ok(())
}
