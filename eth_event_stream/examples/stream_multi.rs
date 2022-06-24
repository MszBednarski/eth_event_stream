use eth_event_stream::{
    address,
    data_feed::block::BlockNotify,
    sink::{reduce_synced_events, stream_synced_events, EventReducer, StreamSignature},
    stream::StreamFactory,
};
use std::env;
use std::sync::Arc;
use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
};
use tokio::sync::Mutex;
use web3::{
    transports::Http,
    types::{Address, Log},
    Web3,
};

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
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;
    let web3 = Web3::new(Http::new(&http_url).unwrap());
    let usdc_address = address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let weth_address = address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let cur_block = web3.eth().block_number().await?.as_u64();
    let from_block = cur_block - 40;
    // till the end of time pls
    let to_block = u64::max_value();
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

    let reducer = USDCNetFlow::new(usdc_signature);

    let reducer_in_thread = reducer.clone();
    // stream and reduce events from multiple sources in batches of 1 block
    tokio::spawn(
        async move { reduce_synced_events(sink, to_block, &vec![reducer_in_thread]).await },
    );

    let mut new_blocks_sub = notify.subscribe();

    while new_blocks_sub.changed().await.is_ok() {
        tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;
        let cur_block = new_blocks_sub.borrow().as_u64();
        println!("{:<10} <=== Live block", cur_block);
        let locked = reducer.lock().await;
        println!(
            "{:<10} <=== Netflows block",
            locked.current_block.unwrap_or_default()
        );
        println!("{:<10} Addresses", locked.netflows.keys().len());
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
        println!("{positive_flows:<10} Positive flows");
        println!("{negative_flows:<10} Negative flows");
        println!("");
    }

    // stream ordered events from multiple sources in batches of 1 block
    // alternative is to use `stream_synced_blocks` that separates events and
    // they need to be got by signature
    // stream_synced_events(sink, to_block, |(number, logs)| async move {
    //     if logs.len() != 0 {
    //         let mut index = logs.first().unwrap().log_index.unwrap();
    //         for i in 1..logs.len() {
    //             let log = logs.get(i).unwrap();
    //             if log.log_index.unwrap() <= index {
    //                 panic!("events are unordered");
    //             }
    //             index = log.log_index.unwrap();
    //         }
    //     }
    //     println!("==> Block {}. Events {}.", number, logs.len());
    // })
    // .await;
    // stream_synced_blocks(sink, to_block, |(number, entry)| async move {
    //     let usdc_transfers: Vec<Erc20Transfer> = entry
    //         .get(&usdc_signature)
    //         .unwrap()
    //         .iter()
    //         .map(|l| Erc20Transfer::from(l.to_owned()))
    //         .collect();
    //     let weth_transfers: Vec<Erc20Transfer> = entry
    //         .get(&weth_signature)
    //         .unwrap()
    //         .iter()
    //         .map(|l| Erc20Transfer::from(l.to_owned()))
    //         .collect();
    //     println!(
    //         "==> Block {}. USDC transfers {} || WETH transfers {}",
    //         number,
    //         usdc_transfers.len(),
    //         weth_transfers.len()
    //     );
    //     // println!("First log {:?}", transfers.first());
    // })
    // .await;
    Ok(())
}
