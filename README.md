# eth_event_stream

[![CI](https://github.com/MszBednarski/eth_event_stream/actions/workflows/main.yml/badge.svg)](https://github.com/MszBednarski/eth_event_stream/actions/workflows/main.yml)

*Synchronized multiple type event streaming for arbitrary time frames*

So you want to get events from the ethereum like blockchains. You wrote some event streaming, live and historical getters, but you want more. You tried writing some in typescript but you needed more threads, you wrote in python is also not it. Welcome you can rest here.

# You want
- To know more complex products of blockchain state over time? No problem just stream synchronized events and monitor the state changes.
- To build an exchange? No problem just stream synchronized events and monitor the state changes.
- To stream events into a timeseries database? No problem just stream synchronized events and monitor the state changes.

# Example
For instance I want all events from the block 15013349 till now + 10000. No problem. The API is the same you will just have to wait longer. No more getting in batches with the Alchemy limit of 2k blocks and even smaller limits for other providers. No more disjoint ways of handling historical events and events coming in live. Just use this, set the from_block and to_block and events will stream from separate threads and be published as synchronized blocks.

# Install
```
eth_event_macro = "0.1.0"
eth_event_stream = "0.1.0"
web3 = "0.18.0"
ethabi = "16.0.0"
ethereum-types = "0.12.1"
```

# API

```rust
// This will derive all needed utility functions
#[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
#[derive(Debug)]
struct Erc20Transfer {}

// We will track usdc netflow between addresses from some block
struct USDCNetFlow {
    sig: StreamSignature,
    netflows: HashMap<Address, i128>,
    current_block: Option<u64>,
}

// EventReducer trait allows us to have an object that reduces the incoming event stream
// to some datastructure that we are interested in
impl EventReducer for USDCNetFlow {
    fn new(sig: StreamSignature) -> Arc<Mutex<Self>> {
        // we return a thread safe reducer so that we can access
        // it from main thread
        Arc::new(Mutex::new(USDCNetFlow {
            sig,
            netflows: HashMap::new(),
            current_block: None,
        }))
    }
    // this is where the magic happens
    // given ordered events in blocks of 1 block
    // we can track what is happening
    fn reduce(&mut self, block_number: u64, ordered_events: &[Log]) {
        match ordered_events {
            [first, ..] => {
                // if the signature of the event is of the type we want we can process it
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
    // get previous 100 blocks
    let from_block = cur_block - 100;
    // get future 10 blocks
    let to_block = cur_block + 10;
    println!(
        "Going to stream from block {} to {} inclusive",
        from_block, to_block
    );

    // separate thread that will notify workers of current block number
    let notify = BlockNotify::new(&http_url, &ws_url).await?;
    let mut factory = StreamFactory::new(http_url, from_block, to_block, 2, 1000);
    // make streams
    let mut usdc_stream = factory
        .make(usdc_address, Erc20Transfer::event(), notify.subscribe())
        .await?;
    // make also weth stream for the example
    let mut weth_stream = factory
        .make(weth_address, Erc20Transfer::event(), notify.subscribe())
        .await?;
    // get their signatures
    // we use those to tell what Log is of what type
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

```