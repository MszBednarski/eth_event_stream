use clap::Parser;
use eth_event_stream::{
    address,
    sink::{EventReducer, StreamSignature},
    stream::StreamFactory,
};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use web3::{
    transports::Http,
    types::{Address, Log},
    Web3,
};

/// Event stream reduce example
#[derive(Parser, Debug)]
struct Args {
    // Number of previous blocks to get
    #[clap(long, default_value_t = 40)]
    diff_neg: u64,
    // Number of future blocks to get
    #[clap(long, default_value_t = 5)]
    diff_pos: u64,
}

#[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

struct USDCNetFlow {
    sig: StreamSignature,
    netflows: HashMap<Address, i128>,
    current_block: Option<u64>,
}

impl EventReducer for USDCNetFlow {
    fn new(sig: StreamSignature) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(USDCNetFlow {
            sig,
            netflows: HashMap::new(),
            current_block: None,
        }))
    }
    fn reduce(&mut self, block_number: u64, ordered_events: &[Log]) {
        match ordered_events {
            [first, ..] => {
                if self.sig.matches(first) {
                    self.current_block = Some(block_number);
                    // if this is the event pattern that we want
                    let transfer = Erc20Transfer::from(first.clone());
                    match &transfer.data {
                        (from, to, value) => {
                            // get the flows
                            let from_flow = self.netflows.get(from).unwrap_or(&0).clone();
                            let to_flow = self.netflows.get(to).unwrap_or(&0).clone();
                            let value_int = value.as_u128() as i128;
                            // update the flows
                            self.netflows.insert(*from, from_flow - value_int);
                            self.netflows.insert(*to, to_flow + value_int);
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags = Args::parse();
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = Web3::new(Http::new(&http_url).unwrap());
    let usdc_address = address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let weth_address = address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let cur_block = web3.eth().block_number().await?.as_u64();
    let from_block = cur_block - flags.diff_neg;
    // till the end of time pls
    let to_block = cur_block + flags.diff_pos;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    let confirmation_blocks = 2;
    let mut factory = StreamFactory::new(
        http_url,
        ws_url,
        from_block,
        to_block,
        confirmation_blocks,
        1000,
    )
    .await?;
    // make streams
    let usdc_stream = factory
        .add_stream(usdc_address, Erc20Transfer::event())
        .await?;
    let weth_stream = factory
        .add_stream(weth_address, Erc20Transfer::event())
        .await?;
    // get their signatures
    let usdc_signature = usdc_stream.signature;
    let weth_signature = weth_stream.signature;
    // get their sink

    // run the streams on separate threads
    usdc_stream.run_in_thread();
    weth_stream.run_in_thread();

    let reducer = USDCNetFlow::new(usdc_signature);
    // entrypoint of the program
    factory.reduce(reducer.clone());

    let mut new_blocks_sub = factory.notify.subscribe();

    while new_blocks_sub.changed().await.is_ok() {
        // lag a bit so reducer is updated
        tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;
        let cur_block = new_blocks_sub.borrow().as_u64();
        if (cur_block - confirmation_blocks as u64) > to_block {
            // stop if we reached `to_block`
            println!("finished.");
            return Ok(());
        }
        println!("{:<17} <=== Live block", cur_block);
        let locked = reducer.lock().await;
        println!(
            "Live - {:<10} <=== Netflows block",
            cur_block - locked.current_block.unwrap_or_default()
        );
        println!("{:<17} Addresses", locked.netflows.keys().len());
        let positive_flows = locked
            .netflows
            .values()
            .filter(|&a| a > &0i128)
            .collect::<Vec<&i128>>()
            .len();
        let negative_flows = locked
            .netflows
            .values()
            .filter(|&a| a < &0i128)
            .collect::<Vec<&i128>>()
            .len();
        println!("{positive_flows:<17} Positive flows");
        println!("{negative_flows:<17} Negative flows");
        println!("");
    }
    Ok(())
}
