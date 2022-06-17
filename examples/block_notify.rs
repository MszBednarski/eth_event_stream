use anyhow::Result;
use eth_event_stream::data_feed::block::BlockNotify;
use std::env;
use tokio::spawn;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Block notify");
    let http_url = env::var("HTTP_NODE_URL")?;
    let ws_url = env::var("WS_NODE_URL")?;

    let notify = BlockNotify::new(&http_url, &ws_url).await?;

    let mut rx = notify.subscribe();

    // as proof of concept
    spawn(async move {
        while rx.changed().await.is_ok() {
            let val = *rx.borrow();
            println!("At block {:?}", val)
        }
    })
    .await?;

    Ok(())
}
