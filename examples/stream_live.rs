use eth_event_stream::{
    data_feed::block::BlockNotify,
    sink::Sink,
    stream::{http_web3, Stream},
};
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use web3::types::Address;

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
            let logs = entry.get(&contract_address).unwrap();
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
        cur += 1;
    }
}
