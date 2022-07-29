/// Broadcast the current block to all freely joining/leaving subscribers
use crate::data_feed::pubsub::PubSub;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::watch::Sender;
use tokio::{spawn, sync::watch::Receiver};
use tokio_retry::{
    strategy::{jitter, ExponentialBackoff},
    Retry,
};
use web3::futures::TryStreamExt;
use web3::transports::{Http, WebSocket};
use web3::Web3;

/// it maintains a tokio watch channel that all processes can listen to to know what is the current block
pub struct BlockNotify {
    ps: PubSub<u64>,
}

impl BlockNotify {
    async fn stream_blocks(block_sender: &Arc<Sender<u64>>, ws_url: &str) -> Result<()> {
        // tryhard to make sure websocket is connected
        // do it indefinitely
        let retry_strategy = ExponentialBackoff::from_millis(10).map(jitter).take(5);
        let ws_t = Retry::spawn(retry_strategy, || WebSocket::new(ws_url)).await?;
        let ws = Web3::new(ws_t);
        let mut subscription = ws.eth_subscribe().subscribe_new_heads().await?;
        loop {
            let block = subscription.try_next().await?;
            if block.is_none() {
                return Err(anyhow!("Block Header is None."));
            }
            let block = block.unwrap();
            if block.number.is_none() {
                return Err(anyhow!("Block number is None."));
            }
            let block_number = block.number.unwrap();
            block_sender.send(block_number.as_u64()).unwrap();
        }
    }

    pub async fn new(http_url: &String, ws_url: &String) -> Result<Self> {
        let http_t = Http::new(&http_url.as_str())?;
        let http = Web3::new(http_t);
        let init_block = http.eth().block_number().await?.as_u64();
        let ps = PubSub::new(init_block);

        // sender we move into the task
        let block_sender = ps.sender();
        let ws_url_ = ws_url.clone();

        // spawn a separate task that always gets us the up to date block number
        spawn(async move {
            loop {
                let must_err = BlockNotify::stream_blocks(&block_sender, &ws_url_).await;
                if must_err.is_err() {
                    println!("Issue with block stream {:?}.", must_err.err());
                }
            }
        });

        Ok(BlockNotify { ps })
    }

    pub fn subscribe(&self) -> Receiver<u64> {
        self.ps.subscribe()
    }
}
