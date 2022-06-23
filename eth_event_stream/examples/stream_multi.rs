use eth_event_stream::{
    data_feed::block::BlockNotify,
    sink::{stream_synced_blocks, Sink},
    stream::Stream,
};
use ethereum_types::Address;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use web3::{transports::Http, Web3};

#[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = Web3::new(Http::new(&http_url).unwrap());
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
    let mut stream = Stream::new(
        http_url,
        contract_address,
        from_block,
        to_block,
        Erc20Transfer::event(),
        notify.subscribe(),
    )
    .await?;
    let signature = stream.signature;
    let sink = Arc::new(Mutex::new(Sink::new(vec![stream.signature], from_block)));
    stream.bind_sink(sink.clone());
    stream.confirmation_blocks(3);

    tokio::spawn(async move { stream.block_stream().await });

    stream_synced_blocks(sink, to_block, |(number, entry)| async move {
        let transfers: Vec<Erc20Transfer> = entry
            .get(&signature)
            .unwrap()
            .iter()
            .map(|l| Erc20Transfer::from(l.to_owned()))
            .collect();
        println!("==> Block {}. Got logs. {}", number, transfers.len());
        // println!("First log {:?}", transfers.first());
    })
    .await;
    Ok(())
}
