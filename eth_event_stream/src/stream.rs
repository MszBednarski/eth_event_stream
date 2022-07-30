use crate::{
    data_feed::block::BlockNotify,
    sink::{reduce_synced_events, EventReducer, Sink, StreamSignature, StreamSink},
};
use anyhow::{anyhow, Result};
use ethers::prelude::{
    Address, BlockNumber, Filter, FilterBlockOption, Http, Log, Provider, ValueOrArray, H160, H256,
    U64,
};
use ethers::{abi::Event, prelude::Middleware};
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio_retry::{
    strategy::{jitter, ExponentialBackoff},
    Retry,
};

#[derive(Debug)]
pub struct Stream {
    pub http_url: String,
    pub address: Address,
    pub from_block: U64,
    pub to_block: U64,
    // Stream signature that is the composition of the
    // watched address and the event signature
    pub signature: StreamSignature,
    confirmation_blocks: u8,
    sink: Option<StreamSink>,
    block_step: u64,
    block_notify_subscription: watch::Receiver<u64>,
    /// used for the filter builder
    f_contract_address: ValueOrArray<Address>,
    f_topic: H256,
    provider: Provider<Http>,
}

/// Used to create multiple synchronized streams faster
/// Going to be opinionated here. This allows you to create some service that is based on blockchain state.
/// Use only one StreamFactory per process/service. One stream factory can be used to create some complex service.
/// Since it can dynamically sync N different streams. Those streams should be coupled and work for some purpose.
/// For instance you can use one StreamFactory to track all of uniswap by watching the pair factory and generating
/// dynamic streams from there. If you need to replicate the same service to handle more traffic make a separate service using
/// StreamFactory that has geth eth API and acts as a caching layer for the same requests.
pub struct StreamFactory {
    pub http_url: String,
    pub ws_url: String,
    pub from_block: u64,
    pub to_block: u64,
    pub confirmation_blocks: u8,
    pub block_step: u64,
    /// the `clock` of this struct it makes all of the update functions and triggers tick based on the current
    /// newest block on the blockchain
    pub notify: BlockNotify,
    pub sink: StreamSink,
}

impl StreamFactory {
    pub async fn new(
        http_url: String,
        ws_url: String,
        from_block: u64,
        to_block: u64,
        confirmation_blocks: u8,
        block_step: u64,
    ) -> Result<Self> {
        let notify = BlockNotify::new(&http_url, &ws_url).await?;
        Ok(StreamFactory {
            http_url,
            ws_url,
            from_block,
            to_block,
            confirmation_blocks,
            block_step,
            sink: Sink::new_threadsafe(Vec::new(), from_block),
            notify,
        })
    }

    /// Stream and reduce events from multiple sources in batches of 1 block.
    /// Imagine this as a functional reduce over an infinite array of events in synced batches of 1 block.
    /// calls `reduce_synced_events` under the hood
    pub fn reduce(&self, reducer: Arc<Mutex<impl EventReducer + std::marker::Send + 'static>>) {
        let sink = self.sink.clone();
        let to_block = self.to_block;
        tokio::spawn(async move { reduce_synced_events(sink, to_block, &vec![reducer]).await });
    }

    /// creates a new stream
    /// adds it to the sink and syncs it with the rest
    pub async fn add_stream(&mut self, address: H160, signature: H256) -> Result<Stream> {
        let mut stream = Stream::new(
            self.http_url.clone(),
            address,
            self.from_block,
            self.to_block,
            signature,
            self.notify.subscribe(),
        )
        .await?;
        stream.confirmation_blocks(self.confirmation_blocks);
        stream.block_step(self.block_step);
        // register source
        self.sink.lock().await.add_source(stream.signature);
        // register where to send events
        stream.bind_sink(self.sink.clone());
        Ok(stream)
    }
}

impl Stream {
    /// builds filter for one time call to eth.logs
    fn build_filter(&self, from_block: U64, to_block: U64) -> Filter {
        Filter::new()
            .address(self.f_contract_address.clone())
            .from_block(from_block)
            .to_block(to_block)
            .topic0(self.f_topic.clone())
    }

    /// runs the stream in thread the stream will be feeding the incoming data into the `sink`
    /// consumes the stream as you will not be able to reference it anymore since it is in a
    /// new thread
    pub fn run_in_thread(mut self) {
        tokio::spawn(async move { self.block_stream().await });
    }

    /// * `ftopic` - the topic returned by EventName::signature()
    pub async fn new(
        http_url: String,
        address: Address,
        from_block: u64,
        to_block: u64,
        f_topic: H256,
        block_notify_subscription: watch::Receiver<u64>,
    ) -> Result<Stream> {
        let f_contract_address = ValueOrArray::Value(address);
        let provider = Provider::<Http>::try_from(http_url.clone())?;
        // I found that 2 block delay usually feeds reliably
        let confirmation_blocks = 2u8;
        let s = Stream {
            block_notify_subscription,
            block_step: 1000,
            http_url,
            sink: None as Option<StreamSink>,
            address,
            from_block: U64::from(from_block),
            to_block: U64::from(to_block),
            confirmation_blocks,
            signature: StreamSignature(address, f_topic.clone()),
            f_contract_address,
            f_topic,
            provider,
        };
        Ok(s)
    }

    /// stream sink has to be bound before any event streaming can be done
    pub fn bind_sink(&mut self, sink: StreamSink) {
        self.sink = Some(sink)
    }

    pub fn block_step(&mut self, new_step: u64) {
        self.block_step = new_step
    }

    pub fn confirmation_blocks(&mut self, new_confirmation: u8) {
        self.confirmation_blocks = new_confirmation;
    }

    async fn get_cur_block(&self) -> Result<U64> {
        let retry_strategy = ExponentialBackoff::from_millis(10).map(jitter).take(4);
        // retry to get the logs
        let res = Retry::spawn(retry_strategy, || self.provider.get_block_number()).await;
        if res.is_err() {
            return Err(anyhow!("Failed getting current block {:?}", res.err()));
        }
        Ok(U64([res.unwrap().as_u64()]))
    }

    /// this cannot get you logs on arbitrary range
    /// this does just one call to eth.logs
    pub async fn get_logs(&self, from_block: &U64, to_block: &U64) -> Result<Vec<Log>> {
        let retry_strategy = ExponentialBackoff::from_millis(10).map(jitter).take(4);
        // retry to get the logs
        let filter = self.build_filter(from_block.clone(), to_block.clone());
        let logs_res = Retry::spawn(retry_strategy, || self.provider.get_logs(&filter)).await;
        if logs_res.is_err() {
            return Err(anyhow!(
                "Fatal error when fetching logs {:?}",
                logs_res.err()
            ));
        }
        let logs = logs_res.unwrap();
        for log in &logs {
            if log.removed.is_some() && log.removed.unwrap() {
                return Err(anyhow!(
                    "Encountered removed block, increase confirmation blocks number. {:?}",
                    log
                ));
            }
        }
        Ok(logs)
    }

    /// end block inclusive
    async fn put(&self, vals: Vec<Log>, end_block: u64) -> Result<()> {
        match &self.sink {
            Some(sink) => sink.lock().await.put_multiple(
                &self.signature,
                vals.iter()
                    .map(|l| {
                        (
                            l.block_number.unwrap().as_u64(),
                            l.log_index.unwrap().as_u128(),
                            l.to_owned(),
                        )
                    })
                    .collect(),
                end_block,
            ),
            None => Err(anyhow!(
                "Sink has to be bound to stream in order to stream logs."
            )),
        }
    }

    async fn get_and_put_logs(&self, start: &U64, end: &U64) -> Result<()> {
        let logs = self.get_logs(start, end).await?;
        self.put(logs, end.as_u64()).await
    }

    /// gets logs that are old enough to be finalized for sure
    /// streams it through sender to consoomers
    async fn stream_historical_logs(&self, from_block: U64, to_block: U64) -> Result<()> {
        // get in batches of size block_step
        let mut start = from_block;
        while start <= to_block {
            let mut end = start + self.block_step;
            if end > to_block {
                end = to_block
            }
            self.get_and_put_logs(&start, &end).await?;
            start = start + self.block_step + 1u64;
        }
        Ok(())
    }

    /// streams live blocks from_block inclusive
    async fn stream_live_logs(&mut self, from_block: U64, to_block: U64) -> Result<()> {
        let mut get_from = from_block;
        // due to uncles we need to track also current block
        // to be sure that when the block is resubmitted
        // the safe_block < get_from check does not fail
        let mut previous_block = from_block;
        while self.block_notify_subscription.changed().await.is_ok() {
            let cur_block = U64([*self.block_notify_subscription.borrow()]);
            // this is going to protect agains uncle blocks
            // and resubmissions
            if cur_block <= previous_block {
                continue;
            }
            previous_block = cur_block;
            // the block that we can safely get finalized events
            let mut safe_block = cur_block - self.confirmation_blocks;
            if safe_block > to_block {
                safe_block = to_block;
            }
            self.get_and_put_logs(&get_from, &safe_block).await?;
            // set new get from block
            get_from = safe_block + 1u64;
            // this is the end
            if safe_block == to_block {
                return Ok(());
            }
        }
        println!("Block notify subscription failed.");
        std::process::exit(-1);
    }

    /// uses parameter sender to send blocks of eth logs to all recievers
    /// on broadcast the logs are sorted in ascending order the way they were emmited
    /// in the blockchain EVM
    pub async fn block_stream(&mut self) {
        // set the stream mode to live or historical
        // based on the users request
        let cur_block_res = self.get_cur_block().await;
        if cur_block_res.is_err() {
            println!("{:?}", cur_block_res.err());
            std::process::exit(-1);
        }
        let cur_block_number = cur_block_res.unwrap();
        // bool if we need to stream live logs too
        let need_live = (cur_block_number - self.confirmation_blocks) < self.to_block;
        if !need_live {
            let res = self
                .stream_historical_logs(self.from_block, self.to_block)
                .await;
            if res.is_err() {
                println!("{:?}", res.err());
                std::process::exit(-1)
            }
        }
        // safe last historical block we can get
        let safe_last_historical = cur_block_number - self.confirmation_blocks;
        let hist_res = self
            .stream_historical_logs(self.from_block, safe_last_historical)
            .await;
        if hist_res.is_err() {
            println!("{:?}", hist_res.err());
            std::process::exit(-1);
        }
        // now stream the live logs
        // have to get one more block to ensure sink flushes inclusive
        let live_res = self
            .stream_live_logs(safe_last_historical + 1u64, self.to_block)
            .await;
        if live_res.is_err() {
            println!("{:?}", live_res.err());
            std::process::exit(-1);
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Stream, StreamSink};
    use crate::{data_feed::block::BlockNotify, sink::Sink};
    use anyhow::Result;
    use ethers::contract::EthEvent;
    use ethers::prelude::{abigen, Address};
    use std::env;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> Result<(Stream, StreamSink, u64)> {
        let http_url = env::var("HTTP_NODE_URL")?;
        let ws_url = env::var("WS_NODE_URL")?;
        let address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = 14658323u64;
        // from + 10
        let to_block = from_block + 8;
        let notify = BlockNotify::new(&http_url, &ws_url).await?;
        abigen!(
            Transfer,
            r#"[
                event Transfer(address indexed from, address indexed to, uint value)
            ]"#,
        );
        let mut stream = Stream::new(
            http_url,
            address,
            from_block,
            to_block,
            TransferFilter::signature(),
            notify.subscribe(),
        )
        .await?;
        let sink: StreamSink = Arc::new(Mutex::new(Sink::new(
            vec![stream.signature.clone()],
            from_block,
        )));
        stream.bind_sink(sink.clone());
        stream.block_step(2);
        Ok((stream, sink, to_block))
    }

    #[test]
    fn test_contract_addr() -> Result<()> {
        // figure out how to have an eth address as the datatype
        Address::from_slice(hex::decode(USDC)?.as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_eth_logs_number() -> Result<()> {
        // check how to use eth logs
        let (mut stream, sink, to_block) = test_stream().await?;
        let sig = stream.signature;

        tokio::spawn(async move { stream.block_stream().await });

        Sink::wait_until_included(sink.clone(), to_block).await;

        let mut all_logs = Vec::new();

        let results = sink.lock().await.flush_including(to_block);
        // println!("{:?}", sink.lock().await);
        for (block_number, mut entry) in results {
            let block_logs = entry.get_mut(&sig).unwrap();
            println!("Got block {} size {}", block_number, block_logs.len());
            all_logs.append(block_logs);
        }

        assert_eq!(all_logs.len(), 56);

        Ok(())
    }
}
