mod events;
mod rich_log;
use std::cmp::Ordering;

use anyhow::Result;
use ethabi::Event;
use rich_log::{MakesRichLog, RichLog};
use tokio::sync::broadcast;
use web3::transports::Http;
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, H160, H256, U256, U64};
use web3::{api, transports, Web3};

#[derive(Debug)]
enum StreamMode {
    /// stream blocks that are going to be mined
    /// and are not mined yet
    LiveStream,
    /// stream only historical blocks
    HistoricalStream,
}

#[derive(Debug)]
pub struct Stream {
    pub http_url: String,
    pub contract_address: Address,
    pub confirmation_blocks: u8,
    pub from_block: U64,
    pub to_block: U64,
    pub event: Event,
    /// used for the filter builder
    f_contract_address: Vec<Address>,
    f_topic: Option<Vec<H256>>,
    mode: StreamMode,
}

impl Stream {
    pub fn http_web3(&self) -> Result<api::Web3<web3::transports::Http>> {
        let transport = transports::Http::new(&self.http_url)?;
        Ok(Web3::new(transport))
    }

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
        contract_address: Address,
        from_block: U64,
        to_block: U64,
        event: Event,
    ) -> Result<Stream> {
        let f_contract_address = vec![contract_address];
        let f_topic = Some(vec![H256::from_slice(event.signature().as_bytes())]);
        let mut s = Stream {
            http_url,
            contract_address,
            from_block,
            to_block,
            // I found that 2 block delay usually feeds reliably
            confirmation_blocks: 2u8,
            mode: StreamMode::HistoricalStream,
            event,
            f_contract_address,
            f_topic,
        };
        let web3 = s.http_web3()?;
        // set the stream mode to live or historical
        // based on the users request
        let block_number = web3.eth().block_number().await?;
        s.mode = match block_number >= s.to_block {
            true => StreamMode::HistoricalStream,
            false => StreamMode::LiveStream,
        };
        Ok(s)
    }

    /// this cannot get you logs on arbitrary range
    /// this does just one call to eth.logs
    /// sorts logs in ascending order the way they were emmited
    pub async fn get_logs(
        &self,
        web3: web3::api::Web3<Http>,
        from_block: U64,
        to_block: U64,
    ) -> Result<Vec<RichLog>> {
        let filter = self.build_filter(from_block, to_block);
        let logs = web3.eth().logs(filter).await?;
        let mut parsed_log: Vec<RichLog> = logs
            .iter()
            .map(|l| l.make_rich_log(&self.event).unwrap())
            .collect();
        parsed_log.sort_by(|a, b| match a.log_index > b.log_index {
            true => Ordering::Greater,
            false => match a.log_index == b.log_index {
                true => Ordering::Equal,
                false => Ordering::Less,
            },
        });
        Ok(parsed_log)
    }

    /// uses parameter sender to send blocks of eth logs to all recievers
    /// on broadcast the logs are sorted in ascending order the way they were emmited
    /// in the blockchain EVM
    pub async fn block_stream(
        &self,
        sender: broadcast::Sender<(BlockNumber, Vec<RichLog>)>,
    ) -> Result<()> {
        // we use alchemy and they are gigachads that do not allow ranges bigger than 2k on the eth.logs call
        // hence motivation for this entire lib
        let web3 = self.http_web3()?;
        let logs = self.get_logs(web3, self.from_block, self.to_block).await?;
        sender.send((BlockNumber::from(9i8), logs))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::Stream;
    use crate::{events::erc20_transfer, rich_log::RichLog};
    use anyhow::Result;
    use std::{borrow::BorrowMut, env};
    use tokio::sync::broadcast;
    use web3::types::{Address, BlockNumber, FilterBuilder, H256, U256, U64};

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> Result<Stream> {
        let http_url = env::var("HTTP_NODE_URL")?;
        let contract_address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = U64::from(14658323u64);
        // from + 10
        let to_block = from_block + U64::from(10u64);
        let event = erc20_transfer()?;
        Stream::new(http_url, contract_address, from_block, to_block, event).await
    }

    fn test_ordering(logs: Vec<RichLog>) {
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
        let stream = test_stream().await?;
        let (snd, mut rx) = broadcast::channel(1000);
        // run it as a separate task
        tokio::spawn(async move { stream.block_stream(snd).await });

        let mut item = rx.recv().await;

        let mut all_logs = Vec::new();

        if !item.is_ok() {
            return Err(anyhow::anyhow!("No logs returned"));
        }

        while item.is_ok() {
            let (_block_number, mut block_logs) = item?;
            all_logs.append(block_logs.borrow_mut());
            // consoom the message
            item = rx.recv().await;
        }
        // if it reaches here it means the stream ended

        assert_eq!(all_logs.len(), 59);
        test_ordering(all_logs);

        Ok(())
    }
}
