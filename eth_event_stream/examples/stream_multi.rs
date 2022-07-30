use clap::Parser;
use eth_event_stream::{
    sink::{EventReducer, StreamSignature},
    stream::StreamFactory,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, str::FromStr};
use tokio::sync::Mutex;

use anyhow::Result;
use ethers::providers::Provider;
use ethers::{
    abi::RawLog,
    prelude::{builders::Event, k256::ecdsa::SigningKey, Address, Middleware},
};
use ethers::{
    prelude::{abigen, LocalWallet, Log, SignerMiddleware, Wallet},
    providers::Http,
    utils::Ganache,
};
// use ethers::contract::Log;
use ethers::contract::EthEvent;
use std::{convert::TryFrom, path::PathBuf};

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

trait ConvertableToRaw {
    
}

impl ConvertableToRaw for Log {}

abigen!(
    Transfer,
    r#"[
        event Transfer(address indexed from, address indexed to, uint value)
    ]"#,
);

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
                    let transfer = TransferFilter::decode_log(first.clone());
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
    let provider = Provider::<Http>::try_from(http_url.clone())?;
    let usdc_address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse::<Address>()?;
    let weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>()?;
    let cur_block = provider.get_block_number().await?.as_u64();
    let from_block = cur_block - flags.diff_neg;
    let to_block = cur_block + flags.diff_pos;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    let confirmation_blocks = 2;
    let mut factory = StreamFactory::new(
        http_url.clone(),
        ws_url,
        from_block,
        to_block,
        confirmation_blocks,
        1000,
    )
    .await?;
    // Test::new(Transfer, usdc, client);

    // make streams
    let usdc_stream = factory
        .add_stream(usdc_address, TransferFilter::signature())
        .await?;
    let weth_stream = factory
        .add_stream(weth_address, TransferFilter::signature())
        .await?;
    // get their signatures
    let usdc_signature = usdc_stream.signature;
    let weth_signature = weth_stream.signature;
    // get their sink

    // // run the streams on separate threads
    // usdc_stream.run_in_thread();
    // weth_stream.run_in_thread();

    // // let reducer = USDCNetFlow::new(usdc_signature);
    // // // entrypoint of the program
    // // factory.reduce(reducer.clone());

    // // let mut new_blocks_sub = factory.notify.subscribe();

    // let wallet: LocalWallet = LocalWallet::new(&mut rand::thread_rng());
    // // 3. connect to the network
    // let provider = Provider::<Http>::try_from(http_url)?.interval(Duration::from_millis(10u64));

    // /// BELOWa A A A  A
    // // USE THISSSSSSS
    // provider.get_logs(filter);
    // let client = SignerMiddleware::new(provider, wallet);
    // let client = Arc::new(client);
    // // let t: Transfer<i32> = todo!();
    // let contract = Transfer::new(
    //     ethers::prelude::H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")?,
    //     client.clone(),
    // );
    // // contract is a nominal type
    // // it can return filter with contract.transfer_filter().filter
    // //
    // let filter: Event<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>, TransferFilter> =
    //     contract.transfer_filter();
    // filter.filter
    // USE THIS TO REPLACE WEB3
    // filter.parse_log(log)
    // let l = Log {
    //     address: val,
    //     topics: val,
    //     data: val,
    //     block_hash: val,
    //     block_number: val,
    //     transaction_hash: val,
    //     transaction_index: val,
    //     log_index: val,
    //     transaction_log_index: val,
    //     log_type: val,
    //     removed: Some(false),
    // };
    // let ev = filter.parse_log(todo!())?;
    // ev.from

    // let builder: ethers::contract::builders::Event<
    //     SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    //     TransferFilter,
    // > = contract.events();

    // builder.filter.event(event_name)
    // let builder = t.events();
    // builder.

    // t.

    // event_builder.

    // while new_blocks_sub.changed().await.is_ok() {
    //     // lag a bit so reducer is updated
    //     tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;
    //     let cur_block = new_blocks_sub.borrow().as_u64();
    //     if (cur_block - confirmation_blocks as u64) > to_block {
    //         // stop if we reached `to_block`
    //         println!("finished.");
    //         return Ok(());
    //     }
    //     println!("{:<17} <=== Live block", cur_block);
    //     let locked = reducer.lock().await;
    //     println!(
    //         "Live - {:<10} <=== Netflows block",
    //         cur_block - locked.current_block.unwrap_or_default()
    //     );
    //     println!("{:<17} Addresses", locked.netflows.keys().len());
    //     let positive_flows = locked
    //         .netflows
    //         .values()
    //         .filter(|&a| a > &0i128)
    //         .collect::<Vec<&i128>>()
    //         .len();
    //     let negative_flows = locked
    //         .netflows
    //         .values()
    //         .filter(|&a| a < &0i128)
    //         .collect::<Vec<&i128>>()
    //         .len();
    //     println!("{positive_flows:<17} Positive flows");
    //     println!("{negative_flows:<17} Negative flows");
    //     println!("");
    // }
    Ok(())
}
