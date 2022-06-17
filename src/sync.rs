use crate::stream::Stream;
use web3::types::{Address, U64};

pub trait SubStream {
    fn address(&self) -> &Address;
    fn event_declaration(&self) -> &String;
}

pub struct SyncStreams<T: SubStream> {
    pub http_url: String,
    pub ws_url: String,
    pub from_block: U64,
    pub to_block: U64,
    pub streams: Vec<T>,
}

impl<T: SubStream> SyncStreams<T> {
    pub fn new(
        http_url: String,
        ws_url: String,
        from_block: U64,
        to_block: U64,
        streams: Vec<T>,
    ) -> Self {

        // for s in streams {
        //     s.
        // }

        SyncStreams {
            http_url,
            ws_url,
            from_block,
            to_block,
            streams,
        }
    }
}
