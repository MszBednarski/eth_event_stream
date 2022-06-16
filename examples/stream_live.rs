use eth_event_stream::{http_web3, Stream};
use std::env;
use tokio::sync::broadcast;
use web3::types::Address;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = http_web3(&http_url)?;
    let usdc: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    let contract_address = Address::from_slice(hex::decode(&usdc[2..])?.as_slice());
    let confirmation_blocks = 2u8;
    let cur_block = web3.eth().block_number().await?.as_u64();
    let from_block = cur_block - 25;
    let to_block = cur_block + 6;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );
    let stream = Stream::new(
        http_url,
        ws_url,
        contract_address,
        from_block,
        to_block,
        confirmation_blocks,
        "event Transfer(address indexed from, address indexed to, uint value)",
    )
    .await?;

    let (snd, mut rx) = broadcast::channel(10);
    tokio::spawn(async move { stream.block_stream(&snd, 5).await });

    loop {
        let block = rx.recv().await;
        if block.is_err() {
            println!("{:?}", block);
            return Ok(());
        }
        match block.unwrap() {
            (number, logs) => {
                let parsed: Vec<(&ethabi::Token, &ethabi::Token, &ethabi::Token)> = logs
                    .iter()
                    .map(|l| match &l.params[..] {
                        [from, to, value] => Some((&from.value, &to.value, &value.value)),
                        _ => None,
                    })
                    .filter_map(|a| a)
                    .collect();
                println!("Block {}. Got logs. {}", number, parsed.len())
            }
        }
    }
}
