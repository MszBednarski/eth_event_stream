use crate::data_feed::block;
use crate::rich_log::RichLog;
use std::cmp;
use std::collections::{BTreeMap, HashMap};
use web3::types::{Address, U256, U64};

/// Stores RichLogs in a datastructure optimized for flushing logs,
/// once we are sure that they are finalized
#[derive(Debug)]
pub struct LogSink {
    /// address -> block number -> log index -> log
    log_store: BTreeMap<Address, BTreeMap<U64, BTreeMap<U256, RichLog>>>,
    min_block: U64,
    max_block: U64,
    /// addressses that this sink tracks
    registered_addresses: Vec<Address>,
}

impl LogSink {
    pub fn new(registered_addresses: Vec<Address>) -> Self {
        let mut log_store: BTreeMap<Address, BTreeMap<U64, BTreeMap<U256, RichLog>>> =
            BTreeMap::new();
        for a in registered_addresses.clone() {
            log_store.insert(a, BTreeMap::new());
        }
        LogSink {
            log_store,
            min_block: U64::MAX,
            max_block: U64::from(0u8),
            registered_addresses,
        }
    }

    /// returns whether the sink should flush
    /// answers if all addresses have entry for a block
    fn should_flush(&self, block_number: &U64) -> bool {
        for a in self.registered_addresses.clone() {
            if !self.log_store.get(&a).unwrap().contains_key(block_number) {
                return false;
            }
        }
        return true;
    }

    /// flushes block range non-inclusive on last it also publishes empty blocks
    /// it makes it easy to synchronize multiple event streams
    /// UNSAFE should be called by its higher order counterpart
    fn __flush_blocks_range(
        &mut self,
        from: U64,
        to: U64,
    ) -> Vec<(U64, HashMap<Address, Vec<RichLog>>)> {
        // range is inclusive
        let mut to_send = Vec::new();
        let mut biggest_flushed = U64::from(0u64);
        for block_number in from.as_u64()..to.as_u64() {
            let block_number_ = U64::from(block_number);
            if !self.should_flush(&block_number_) {
                // we are done flushing, the blocks after this are not done
                break;
            }
            // every address has an entry for the current block
            let mut block_hashmap: HashMap<Address, Vec<RichLog>> = HashMap::new();
            // for each address
            for a in self.registered_addresses.clone() {
                let store_for_address = self.log_store.get_mut(&a).unwrap();
                // due to should_flush we know that each address has an entry for this block
                let block_entry = store_for_address.get_mut(&block_number_).unwrap();
                // here we delete the entry then add to block_hashmap
                // already sorted logs due to the btreemap
                let logs = block_entry.values().cloned().collect();
                // delete the entry
                store_for_address.remove(&block_number_);
                // add the address to our block_hashmap
                block_hashmap.insert(a, logs);
            }
            // finally we can add the block_hashmap to the to_send
            to_send.push((block_number_, block_hashmap));
            biggest_flushed = block_number_
        }
        if biggest_flushed != U64::from(0u8) {
            // if we did flush something
            self.min_block = biggest_flushed + 1;
        }
        // set min blocks to valid value
        // max does not need to be updated because it always leads
        to_send
    }

    /// puts multiple logs into the datastructure then
    /// flushes the finalized ones
    pub fn put_logs(
        &mut self,
        address: &Address,
        logs: Vec<RichLog>,
    ) -> Vec<(U64, HashMap<Address, Vec<RichLog>>)> {
        if logs.len() > 0 {
            // get the store specific to the address we are working with
            let store_for_address = self.log_store.get_mut(address).unwrap();
            let mut min_block = U64::max_value();
            let mut max_block = U64::from(0u8);
            // not abstracting it to a separate function for speed
            // and to gather min max info
            for log in logs {
                // puts a SINGLE log into the datastructure
                match log {
                    RichLog {
                        block_number,
                        log_index,
                        removed,
                        ..
                    } => {
                        // put the log into the datastructure
                        match removed {
                            true => {
                                // if log is removed we need to make sure we don't have it in our datastructure
                                match store_for_address.get_mut(&block_number) {
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
                                // keep track of min/max block globally for all addresses
                                self.max_block = cmp::max(block_number, self.max_block);
                                self.min_block = cmp::min(block_number, self.min_block);
                                // keep track of min/max block locally for current address
                                min_block = cmp::min(block_number, min_block);
                                max_block = cmp::max(block_number, max_block);

                                // insert
                                match store_for_address.get_mut(&block_number) {
                                    Some(entry) => {
                                        // put log to existing block
                                        entry.insert(log_index, log);
                                    }
                                    None => {
                                        // just make a new entry
                                        let mut map = BTreeMap::new();
                                        map.insert(log_index, log);
                                        store_for_address.insert(block_number, map);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // now we have inserted a bunch of logs into the datastructure
            // it might look something like this [_log__log____log____log]
            // we need to backfill all of the empty blocks to say that there was no logs on that range
            // into something like [elogeelogeeeeelogeee]
            // so that the flush function knows that it can flush all of the events safely
            for blk in min_block.as_u64()..max_block.as_u64() {
                let block = U64::from(blk);
                if !store_for_address.contains_key(&block) {
                    store_for_address.insert(block, BTreeMap::new());
                }
            }

            // now that we put all of our logs in
            // since logs.len > 0 we can safely flush
            // flush only finalized the logs on max_block could be missing data
            return self.__flush_blocks_range(self.min_block, self.max_block);
        }
        Vec::new()
    }

    pub fn flush_remaining(&mut self) -> Vec<(U64, HashMap<Address, Vec<RichLog>>)> {
        return self.__flush_blocks_range(self.min_block, self.max_block + 1);
    }
}
