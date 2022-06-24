use crate::sink::{Sink, StreamSignature, StreamSink};
use anyhow::Result;
use ethabi::Event;
use tokio::sync::watch;
use web3::transports::Http;
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, Log, H256, U64};
use web3::Web3;

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
    block_notify_subscription: watch::Receiver<U64>,
    /// used for the filter builder
    f_contract_address: Vec<Address>,
    f_topic: Option<Vec<H256>>,
    web3: Web3<Http>,
}

/// Used to create multiple synchronized streams faster
pub struct StreamFactory {
    pub http_url: String,
    pub from_block: u64,
    pub to_block: u64,
    pub confirmation_blocks: u8,
    pub block_step: u64,
    sink: StreamSink,
}

impl StreamFactory {
    pub fn new(
        http_url: String,
        from_block: u64,
        to_block: u64,
        confirmation_blocks: u8,
        block_step: u64,
    ) -> Self {
        StreamFactory {
            http_url,
            from_block,
            to_block,
            confirmation_blocks,
            block_step,
            sink: Sink::new_threadsafe(Vec::new(), from_block),
        }
    }

    /// makes a new stream
    pub async fn make(
        &mut self,
        address: Address,
        event: Event,
        block_notify_subscription: watch::Receiver<U64>,
    ) -> Result<Stream> {
        let mut stream = Stream::new(
            self.http_url.clone(),
            address,
            self.from_block,
            self.to_block,
            event,
            block_notify_subscription,
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

    /// consumes the factory and returns the sink to which the events go
    /// call this at the end to get the sink to which the events will be trickling down
    pub fn get_sink(self) -> StreamSink {
        self.sink
    }
}

impl Stream {
    /// builds filter for one time call to eth.logs
    fn build_filter(&self, from_block: U64, to_block: U64) -> Filter {
        FilterBuilder::default()
            .address(self.f_contract_address.clone())
            .from_block(BlockNumber::Number(from_block))
            // just get 10 blocks to make sure this returns
            .to_block(BlockNumber::Number(to_block))
            .topics(self.f_topic.clone(), None, None, None)
            .build()
    }

    pub async fn new(
        http_url: String,
        address: Address,
        from_block: u64,
        to_block: u64,
        event: Event,
        block_notify_subscription: watch::Receiver<U64>,
    ) -> Result<Stream> {
        let f_contract_address = vec![address];
        let f_topic = Some(vec![H256::from_slice(event.signature().as_bytes())]);
        let web3 = Web3::new(Http::new(http_url.as_str())?);
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
            signature: StreamSignature(address, event.signature().clone()),
            f_contract_address,
            f_topic,
            web3,
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

    /// this cannot get you logs on arbitrary range
    /// this does just one call to eth.logs
    pub async fn get_logs(&self, from_block: &U64, to_block: &U64) -> Result<Vec<Log>> {
        let filter = self.build_filter(from_block.clone(), to_block.clone());
        let logs = self.web3.eth().logs(filter).await?;
        for log in &logs {
            if log.is_removed() {
                println!(
                    "Encountered removed block, increase confirmation blocks number. {:?}",
                    log
                );
                // we are not fucking around here
                // we don't want the consumers to work with bad data
                // you should have set confirmation blocks to a higher value
                std::process::exit(-1)
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
            None => panic!("Sink has to be bound to stream in order to stream logs."),
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
            let cur_block = *self.block_notify_subscription.borrow();
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
        panic!("Block notify subscription failed.");
    }

    /// uses parameter sender to send blocks of eth logs to all recievers
    /// on broadcast the logs are sorted in ascending order the way they were emmited
    /// in the blockchain EVM
    pub async fn block_stream(&mut self) -> Result<()> {
        let web3 = Web3::new(Http::new(&self.http_url).unwrap());
        // set the stream mode to live or historical
        // based on the users request
        let cur_block_number = web3.eth().block_number().await?;
        // bool if we need to stream live logs too
        let need_live = (cur_block_number - self.confirmation_blocks) < self.to_block;
        if !need_live {
            self.stream_historical_logs(self.from_block, self.to_block)
                .await?;
            return Ok(());
        }
        // safe last historical block we can get
        let safe_last_historical = cur_block_number - self.confirmation_blocks;
        self.stream_historical_logs(self.from_block, safe_last_historical)
            .await?;
        // now stream the live logs
        // have to get one more block to ensure sink flushes inclusive
        self.stream_live_logs(safe_last_historical + 1u64, self.to_block)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Stream, StreamSink};
    use crate::{data_feed::block::BlockNotify, sink::Sink};
    use anyhow::Result;
    use std::env;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use web3::types::{Address, Log};

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> Result<(Stream, StreamSink, u64)> {
        let http_url = env::var("HTTP_NODE_URL")?;
        let ws_url = env::var("WS_NODE_URL")?;
        let address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = 14658323u64;
        // from + 10
        let to_block = from_block + 8;
        let notify = BlockNotify::new(&http_url, &ws_url).await?;
        #[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
        struct Erc20Transfer {}
        let mut stream = Stream::new(
            http_url,
            address,
            from_block,
            to_block,
            Erc20Transfer::event(),
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
