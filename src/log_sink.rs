use crate::rich_log::RichLog;
use anyhow::Result;
use std::collections::BTreeMap;
use tokio::sync::broadcast;
use web3::types::{U256, U64};

/// Stores RichLogs in a datastructure optimized for flushing logs,
/// once we are sure that they are finalized based on confirmation_blocks number.
pub struct LogSink<'a> {
    confirmation_blocks: u8,
    // block number -> log index -> log
    log_store: BTreeMap<U64, BTreeMap<U256, RichLog>>,
    sender: &'a broadcast::Sender<(U64, Vec<RichLog>)>,
    min_block: U64,
    max_block: U64,
}

impl<'a> LogSink<'a> {
    pub fn new(
        sender: &'a broadcast::Sender<(U64, Vec<RichLog>)>,
        confirmation_blocks: u8,
    ) -> Self {
        let log_store: BTreeMap<U64, BTreeMap<U256, RichLog>> = BTreeMap::new();
        LogSink {
            confirmation_blocks,
            log_store,
            sender,
            min_block: U64::MAX,
            max_block: U64::from(0u8),
        }
    }

    /// flushes block range inclusive
    fn flush_blocks_range(&mut self, from: U64, to: U64) -> Result<()> {
        let from_u64 = from.as_u64();
        // range is inclusive
        let up_to_u64 = to.as_u64() + 1;
        for block_number in from_u64..up_to_u64 {
            let block_number_ = U64::from(block_number);
            match self.log_store.get_mut(&block_number_) {
                Some(entry) => {
                    // here we aim to flush and delete the entry
                    // already sorted logs due to the btreemap
                    let logs = entry.values().cloned().collect();
                    // delete the entry
                    self.log_store.remove(&block_number_);
                    // send it to the consoomers
                    self.sender.send((block_number_, logs))?;
                }
                None => {} // do nothing
            }
        }
        // set min blocks to valid value
        self.min_block = to + 1;
        // max does not need to be updated because it always leads
        Ok(())
    }

    /// puts multiple logs into the datastructure then
    /// flushes the finalized ones
    pub fn put_logs(&mut self, logs: Vec<RichLog>) -> Result<()> {
        if logs.len() > 0 {
            for log in logs {
                self.put_log(log)
            }
            // since logs.len > 0 we can safely flush
            // flush only finalized
            let flush_up_to = self.max_block - self.confirmation_blocks;
            self.flush_blocks_range(self.min_block, flush_up_to)?
        }
        Ok(())
    }

    pub fn flush_remaining(&mut self) -> Result<()> {
        self.flush_blocks_range(self.min_block, self.max_block)
    }

    /// puts one log into the datastructure
    /// keeps track of current min and max block
    fn put_log(&mut self, log: RichLog) {
        match log {
            RichLog {
                block_number,
                log_index,
                removed,
                ..
            } => match removed {
                true => {
                    // if log is removed we need to make sure we don't have it in our datastructure
                    match self.log_store.get_mut(&block_number) {
                        Some(entry) => {
                            // check if entry exists
                            match entry.get(&log_index) {
                                Some(_) => {
                                    // delete it from our datastructure
                                    entry.remove(&log_index);
                                }
                                None => {} // do nothing
                            }
                        }
                        None => {} // do nothing
                    }
                }
                false => {
                    // keep track of min/max block
                    if block_number > self.max_block {
                        self.max_block = block_number
                    }
                    if block_number < self.min_block {
                        self.min_block = block_number
                    }
                    // insert
                    match self.log_store.get_mut(&block_number) {
                        Some(entry) => {
                            // put log to existing block
                            entry.insert(log_index, log);
                        }
                        None => {
                            // just make a new entry
                            let mut map = BTreeMap::new();
                            map.insert(log_index, log);
                            self.log_store.insert(block_number, map);
                        }
                    }
                }
            },
        }
    }
}
