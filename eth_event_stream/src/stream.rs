use crate::sink::Sink;
use anyhow::Result;
use ethabi::Event;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use web3::transports::Http;
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, Log, H256, U64};
use web3::{api, transports, Web3};

pub fn http_web3(http_url: &str) -> Result<api::Web3<web3::transports::Http>> {
    let transport = transports::Http::new(http_url)?;
    Ok(Web3::new(transport))
}

#[derive(Debug)]
pub struct Stream {
    pub http_url: String,
    pub address: Address,
    pub from_block: U64,
    pub to_block: U64,
    confirmation_blocks: u8,
    sink: Arc<Mutex<Sink<Address, Log>>>,
    block_step: u64,
    block_notify_subscription: watch::Receiver<U64>,
    event: Event,
    /// used for the filter builder
    f_contract_address: Vec<Address>,
    f_topic: Option<Vec<H256>>,
    web3: Web3<Http>,
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
        sink: Arc<Mutex<Sink<Address, Log>>>,
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
            sink,
            address,
            from_block: U64::from(from_block),
            to_block: U64::from(to_block),
            confirmation_blocks,
            event,
            f_contract_address,
            f_topic,
            web3,
        };
        Ok(s)
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
        Ok(logs)
    }

    /// end block inclusive
    async fn put(&self, vals: Vec<Log>, end_block: u64) -> Result<()> {
        self.sink.lock().await.put_multiple(
            &self.address,
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
        )
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
        while self.block_notify_subscription.changed().await.is_ok() {
            let cur_block = *self.block_notify_subscription.borrow();
            // the block that we can safely get finalized events
            let mut safe_block = cur_block - self.confirmation_blocks;
            if safe_block > to_block {
                safe_block = to_block;
            }
            if safe_block < get_from {
                panic!("Something went wrong with block order.");
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
        let web3 = http_web3(&self.http_url)?;
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
    use super::Stream;
    use crate::{data_feed::block::BlockNotify, sink::Sink};
    use anyhow::Result;
    use std::env;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use web3::types::{Address, Log};

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> Result<(Stream, Arc<Mutex<Sink<Address, Log>>>, u64)> {
        let http_url = env::var("HTTP_NODE_URL")?;
        let ws_url = env::var("WS_NODE_URL")?;
        let address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = 14658323u64;
        // from + 10
        let to_block = from_block + 8;
        let notify = BlockNotify::new(&http_url, &ws_url).await?;
        let sink = Arc::new(Mutex::new(Sink::new(vec![address], from_block)));
        #[eth_event_macro::event("Transfer(address indexed from, address indexed to, uint value)")]
        struct Erc20Transfer {}
        let mut stream = Stream::new(
            http_url,
            address,
            from_block,
            to_block,
            Erc20Transfer::event(),
            notify.subscribe(),
            sink.clone(),
        )
        .await?;
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
        let addr = stream.address.clone();

        tokio::spawn(async move { stream.block_stream().await });

        Sink::wait_until_included(sink.clone(), to_block).await;

        let mut all_logs = Vec::new();

        let results = sink.lock().await.flush_including(to_block);
        // println!("{:?}", sink.lock().await);
        for (block_number, mut entry) in results {
            let block_logs = entry.get_mut(&addr).unwrap();
            println!("Got block {} size {}", block_number, block_logs.len());
            all_logs.append(block_logs);
        }

        assert_eq!(all_logs.len(), 56);

        Ok(())
    }
}
