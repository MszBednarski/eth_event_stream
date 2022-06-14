use anyhow;
use web3::types::{Address, U64};
use web3::{api, transports, Web3};
#[derive(Debug)]
pub enum StreamMode {
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
    mode: StreamMode,
}

impl Stream {
    pub fn http_web3(&self) -> anyhow::Result<api::Web3<web3::transports::Http>> {
        let transport = transports::Http::new(&self.http_url)?;
        Ok(Web3::new(transport))
    }

    pub async fn new(
        http_url: String,
        contract_address: Address,
        from_block: U64,
        to_block: U64,
    ) -> anyhow::Result<Stream> {
        let mut s = Stream {
            http_url,
            contract_address,
            from_block,
            to_block,
            // I found that 2 block delay usually feeds reliably
            confirmation_blocks: 2u8,
            mode: StreamMode::HistoricalStream,
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
}

#[cfg(test)]
mod tests {
    use super::Stream;
    use web3::types::{Address, U64};
    use std::env;

    const USDC: &str = "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    async fn test_stream() -> anyhow::Result<Stream> {
        let http_url= env::var("HTTP_NODE_URL")?;
        let contract_address = Address::from_slice(hex::decode(USDC)?.as_slice());
        let from_block = U64::from(14658323u64);
        let to_block = U64::from(14658343u64);
        Stream::new(http_url, contract_address, from_block, to_block).await
    }

    #[test]
    fn test_contract_addr() -> anyhow::Result<()> {
        // figure out how to have an eth address as the datatype
        Address::from_slice(hex::decode(USDC)?.as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_new_historical() -> anyhow::Result<()> {
        test_stream().await?;
        Ok(())
    }
}
