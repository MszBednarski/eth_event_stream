use crate::{
    events::event_from_declaration,
    log_sink::LogSink,
    rich_log::{MakesRichLog, RichLog},
};
use anyhow::Result;
use ethabi::Event;
use tokio::sync::broadcast;
use tokio::sync::watch;
use web3::transports::Http;
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, H256, U64};
use web3::{api, transports, Web3};

pub fn http_web3(http_url: &str) -> Result<api::Web3<web3::transports::Http>> {
    let transport = transports::Http::new(http_url)?;
    Ok(Web3::new(transport))
}

pub async fn ws_web3(ws_url: &str) -> Result<api::Web3<web3::transports::WebSocket>> {
    let transport = transports::WebSocket::new(ws_url).await?;
    Ok(Web3::new(transport))
}

#[derive(Debug)]
pub struct Stream {
    pub http_url: String,
    pub ws_url: String,
    pub contract_address: Address,
    pub from_block: U64,
    pub to_block: U64,
    confirmation_blocks: u8,
    block_step: u64,
    block_notify_subscription: watch::Receiver<U64>,
    sender: broadcast::Sender<(U64, Vec<RichLog>)>,
    event: Event,
    /// used for the filter builder
    f_contract_address: Vec<Address>,
    f_topic: Option<Vec<H256>>,
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
        ws_url: String,
        contract_address: Address,
        from_block: u64,
        to_block: u64,
        event_declaration: &'static str,
        sender: broadcast::Sender<(U64, Vec<RichLog>)>,
        block_notify_subscription: watch::Receiver<U64>,
    ) -> Result<Stream> {
        let f_contract_address = vec![contract_address];
        let event = event_from_declaration(event_declaration)?;
        let f_topic = Some(vec![H256::from_slice(event.signature().as_bytes())]);
        let s = Stream {
            block_notify_subscription,
            sender,
            block_step: 1000,
            http_url,
            ws_url,
            contract_address,
            from_block: U64::from(from_block),
            to_block: U64::from(to_block),
            // I found that 2 block delay usually feeds reliably
            confirmation_blocks: 2u8,
            event,
            f_contract_address,
            f_topic,
        };
        Ok(s)
    }

    pub fn block_step(&mut self, new_step: u64) {
        self.block_step = new_step
    }

    pub fn confirmation_blocks(&mut self, new_confirmation: u8) {
        self.confirmation_blocks = new_confirmation
    }

    async fn publish_blocks(&self, blocks: Vec<(U64, Vec<RichLog>)>) -> Result<()> {
        for l in blocks {
            self.sender.send(l)?;
        }
        Ok(())
    }

    /// this cannot get you logs on arbitrary range
    /// this does just one call to eth.logs
    /// sorts logs in ascending order the way they were emmited
    pub async fn get_logs(
        &self,
        web3: &web3::api::Web3<Http>,
        from_block: U64,
        to_block: U64,
    ) -> Result<Vec<RichLog>> {
        let filter = self.build_filter(from_block, to_block);
        let logs = web3.eth().logs(filter).await?;
        let parsed_log: Vec<RichLog> = logs
            .iter()
            .map(|l| l.make_rich_log(&self.event).unwrap())
            .collect();
        Ok(parsed_log)
    }

    /// gets logs that are old enough to be finalized for sure
    /// streams it through sender to consoomers
    async fn stream_historical_logs(&self, from_block: U64, to_block: U64) -> Result<()> {
        let web3 = http_web3(&self.http_url)?;
        let mut sink = LogSink::new();
        // get in batches of size block_step
        let mut start = from_block;
        while start < to_block {
            let mut end = start + self.block_step;
            if end > to_block {
                end = to_block
            }
            self.publish_blocks(sink.put_logs(self.get_logs(&web3, start, end).await?))
                .await?;
            start = start + self.block_step + 1u64;
        }
        self.publish_blocks(sink.flush_remaining()).await?;
        Ok(())
    }

    /// streams live blocks from_block inclusive
    async fn stream_live_logs(&mut self, from_block: U64, to_block: U64) -> Result<()> {
        let web3 = http_web3(&self.http_url)?;
        let mut sink = LogSink::new();

        let mut get_from = from_block;
        while self.block_notify_subscription.changed().await.is_ok() {
            let cur_block = *self.block_notify_subscription.borrow();
            let mut safe_block = cur_block - self.confirmation_blocks;
            if safe_block > to_block {
                safe_block = to_block;
            }
            if safe_block > get_from {
                self.publish_blocks(
                    sink.put_logs(self.get_logs(&web3, get_from, safe_block).await?),
                )
                .await?;
                // set new get from block
                get_from = safe_block + 1u64;
                // this is the end
                if safe_block == to_block {
                    self.publish_blocks(sink.flush_remaining()).await?;
                    return Ok(());
                }
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
        let mut safe_last_historical = self.to_block;
        // if we need to stream live too
        if cur_block_number < self.to_block {
            safe_last_historical = cur_block_number - self.confirmation_blocks;
        }
        self.stream_historical_logs(self.from_block, safe_last_historical)
            .await?;

        let new_from = safe_last_historical + 1u64;
        if new_from < self.to_block {
            println!("Streaming live");
            // stream
            self.stream_live_logs(new_from, self.to_block).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::Stream;
    use crate::{data_feed::block::BlockNotify, rich_log::RichLog};
    use anyhow::Result;
    use std::{borrow::BorrowMut, env};
    use tokio::sync::broadcast;
    use web3::types::Address;

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> Result<(
        Stream,
        broadcast::Receiver<(web3::types::U64, Vec<RichLog>)>,
    )> {
        let http_url = env::var("HTTP_NODE_URL")?;
        let ws_url = env::var("WS_NODE_URL")?;
        let contract_address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = 14658323u64;
        // from + 10
        let to_block = from_block + 10;
        let (sender, rx) = broadcast::channel(1000);
        let notify = BlockNotify::new(&http_url, &ws_url).await?;
        let mut stream = Stream::new(
            http_url,
            ws_url,
            contract_address,
            from_block,
            to_block,
            "event Transfer(address indexed from, address indexed to, uint value)",
            sender,
            notify.subscribe(),
        )
        .await?;
        stream.block_step(2);
        Ok((stream, rx))
    }

    fn test_ordering(logs: &Vec<RichLog>) {
        logs.iter()
            .map(|l| l.log_index.as_u128() as i128)
            // verify ordering of the logs
            .fold(-1i128, |prev, item| {
                assert!(item > prev);
                item
            });
    }

    #[test]
    fn test_contract_addr() -> Result<()> {
        // figure out how to have an eth address as the datatype
        Address::from_slice(hex::decode(USDC)?.as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_new_historical() -> Result<()> {
        test_stream().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_eth_logs_number() -> Result<()> {
        // check how to use eth logs
        let (mut stream, mut rx) = test_stream().await?;
        // run it as a separate task
        tokio::spawn(async move { stream.block_stream().await });

        let mut item = rx.recv().await;

        let mut all_logs = Vec::new();

        if !item.is_ok() {
            return Err(anyhow::anyhow!("No logs returned"));
        }

        while item.is_ok() {
            let (block_number, mut block_logs) = item?;
            println!("Got block {} size {}", block_number, block_logs.len());
            test_ordering(&block_logs);
            all_logs.append(block_logs.borrow_mut());
            // consoom the message
            item = rx.recv().await;
        }
        // if it reaches here it means the stream ended

        assert_eq!(all_logs.len(), 59);

        Ok(())
    }
}