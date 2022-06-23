use eth_event_stream::{
    address, data_feed::block::BlockNotify, sink::stream_synced_blocks, stream::StreamFactory,
};
use std::env;
use web3::{transports::Http, Web3};

#[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = Web3::new(Http::new(&http_url).unwrap());
    let usdc_address = address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let weth_address = address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let cur_block = web3.eth().block_number().await?.as_u64();
    let from_block = cur_block - 40;
    let to_block = cur_block + 4;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    let notify = BlockNotify::new(&http_url, &ws_url).await?;
    let mut factory = StreamFactory::new(http_url, from_block, to_block, 2, 1000);
    // make streams
    let mut usdc_stream = factory
        .make(usdc_address, Erc20Transfer::event(), notify.subscribe())
        .await?;
    let mut weth_stream = factory
        .make(weth_address, Erc20Transfer::event(), notify.subscribe())
        .await?;
    // get their signatures
    let usdc_signature = usdc_stream.signature;
    let weth_signature = weth_stream.signature;
    // get their sink
    let sink = factory.get_sink();

    // run the streams on separate threads
    tokio::spawn(async move { usdc_stream.block_stream().await });
    tokio::spawn(async move { weth_stream.block_stream().await });

    stream_synced_blocks(sink, to_block, |(number, entry)| async move {
        let usdc_transfers: Vec<Erc20Transfer> = entry
            .get(&usdc_signature)
            .unwrap()
            .iter()
            .map(|l| Erc20Transfer::from(l.to_owned()))
            .collect();
        let weth_transfers: Vec<Erc20Transfer> = entry
            .get(&weth_signature)
            .unwrap()
            .iter()
            .map(|l| Erc20Transfer::from(l.to_owned()))
            .collect();
        println!(
            "==> Block {}. USDC transfers {} || WETH transfers {}",
            number,
            usdc_transfers.len(),
            weth_transfers.len()
        );
        // println!("First log {:?}", transfers.first());
    })
    .await;
    Ok(())
}
