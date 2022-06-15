use anyhow::Result;
use ethabi::RawLog;
use web3::types::{H160, H256, U256, U64};

/// has ALL relevant log information
#[derive(Debug, Clone)]
pub struct RichLog {
    pub params: Vec<ethabi::LogParam>,
    pub address: H160,
    pub topics: Vec<H256>,
    pub block_number: U64,
    pub transaction_hash: H256,
    pub transaction_index: U64,
    pub log_index: U256,
    pub removed: bool,
}

/// implementors of this trait can make rich logs
pub trait MakesRichLog {
    /// uses the passed event to create a rich log
    fn make_rich_log(&self, event: &ethabi::Event) -> Result<RichLog>;
}

impl MakesRichLog for web3::types::Log {
    fn make_rich_log(&self, event: &ethabi::Event) -> Result<RichLog> {
        // log needed to parse Log.data
        let raw_log = RawLog {
            topics: self
                .topics
                .iter()
                .map(|o| ethabi::ethereum_types::H256::from(o.0))
                .collect(),
            data: self.data.0.clone(),
        };
        let parsed_log = event.parse_log(raw_log)?;

        Ok(RichLog {
            params: parsed_log.params,
            address: self.address,
            topics: self.topics.clone(),
            block_number: self.block_number.unwrap(),
            transaction_hash: self.transaction_hash.unwrap(),
            transaction_index: self.transaction_index.unwrap(),
            log_index: self.log_index.unwrap(),
            removed: self.removed.unwrap(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::MakesRichLog;
    use crate::events::erc20_transfer;
    use anyhow::Result;
    use web3::types::{Address, Bytes, Log, H256, U256, U64};

    #[test]
    fn test_make_rich_log() -> Result<()> {
        let event = erc20_transfer()?;
        let address = Address::from_slice(
            hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?.as_slice(),
        );
        let topics = vec![
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "00000000000000000000000092c25b7a591ed8367dc98629434d2c12af89af18",
            "000000000000000000000000885ccc87c41340e79425a28ba290f3a284985d8d",
        ]
        .iter()
        .map(|t| H256::from_slice(hex::decode(t).unwrap().as_slice()))
        .collect();
        let bytes: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            107, 73, 210, 0,
        ];
        let data = Bytes(bytes.to_vec());
        let block_number = Some(U64::from(14658333i32));
        let transaction_index = Some(U64::from(113i32));
        let log_index = Some(U256::from(287i32));
        let transaction_hash = Some(H256::from_slice(
            hex::decode("3fdd31638aee9b0b83565596a6efbd06a1937d78ec49fe21f7b22a67c87bb9c6")
                .unwrap()
                .as_slice(),
        ));
        let removed = Some(false);
        let log = Log {
            address,
            topics,
            data,
            block_number,
            log_index,
            transaction_hash,
            transaction_index,
            removed,
            block_hash: None,
            log_type: None,
            transaction_log_index: None,
        };
        log.make_rich_log(&event)?;
        Ok(())
    }
}
