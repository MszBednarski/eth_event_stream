use crate::data_feed::pubsub::PubSub;
use anyhow::Result;
use std::borrow::Borrow;
use std::cmp::{max, Ord};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::Mutex;

/// The stream is feeding us events in block order
/// This datastructure needs to be able to:
/// - sychronize multiple event streams
/// - tell what is the current synchronized block (to which block non inclusive it flushed)
/// - needs to be pausable
/// - accept data from multiple threads at the same time
/// - expose api like .wait_till_synchronized_to(block_num) panic if block_num is lower than current synchronized
/// - panic the moment it gets an event from a block it already published
/// - sort incoming on block level and log level
#[derive(Debug)]
pub struct Sink<SourceKey, T> {
    store: BTreeMap<SourceKey, BTreeMap<u64, BTreeMap<u128, T>>>,
    sources: Vec<SourceKey>,
    source_vals: HashMap<SourceKey, u64>,
    /// needed so that the sink knows how much it needs to backfill for some sources
    bottom: u64,
    ps: PubSub<u64>,
    sender: Arc<watch::Sender<u64>>,
}

impl<A: Ord + Clone + Hash, D: Clone> Sink<A, D> {
    pub fn new(sources: Vec<A>, from_block: u64) -> Self {
        let mut store = BTreeMap::new() as BTreeMap<A, BTreeMap<u64, BTreeMap<u128, D>>>;
        let mut source_vals = HashMap::new();
        for s in sources.clone() {
            store.insert(s.clone(), BTreeMap::new());
            source_vals.insert(s, from_block);
        }
        let ps = PubSub::new(from_block);
        let sender = ps.sender();
        Sink {
            ps,
            store,
            sources,
            bottom: from_block,
            source_vals,
            sender,
        }
    }

    /// returns inclusive block up to which the synced data can be flushed / was flushed
    pub fn synced_including(&mut self) -> Option<u64> {
        // we can flush up to the min(of all maximums) - 1
        match self.source_vals.values().min() {
            Some(val) => match val {
                0 => None,
                _ => Some(val.clone() - 1),
            },
            None => None,
        }
    }

    /// inclusive block target (all blocks including that block)
    pub async fn wait_until_included(target: Arc<Mutex<Self>>, to_block_inclusive: u64) {
        let cur = target.lock().await.synced_including();
        if cur.is_some() && cur >= Some(to_block_inclusive) {
            return;
        }
        let mut rx = target.lock().await.ps.subscribe();
        while rx.changed().await.borrow().is_ok() {
            let current = rx.borrow().clone();
            if current >= to_block_inclusive {
                return;
            }
        }
        panic!("Failed to wait and pause")
    }

    // flush inclusive given block
    pub fn flush_including(&mut self, include_target: u64) -> Vec<(u64, HashMap<A, Vec<D>>)> {
        let synced = self.synced_including();
        if synced.is_none() || include_target > synced.unwrap() {
            panic!("Tried to flush above synced val.");
        }
        let mut results = Vec::new();

        for blk in self.bottom..=include_target {
            let mut blk_result = HashMap::new();
            for a in self.sources.clone() {
                let source_store = self.store.get_mut(&a).unwrap();
                let blk_store = source_store.get_mut(&blk);
                match blk_store {
                    Some(res) => {
                        // get the logs
                        let logs = res.values().cloned().collect();
                        // delete logs from store
                        source_store.remove(&blk);
                        // put them to flush
                        blk_result.insert(a, logs);
                    }
                    None => {
                        // put empty vec
                        blk_result.insert(a, Vec::new());
                        ()
                    }
                };
            }
            results.push((blk, blk_result));
        }
        self.bottom = include_target + 1;

        results
    }

    /// needs to be aware of the end_block inclusive in the range inserted to make an empty entry
    /// for the purpose of accurate flushing
    pub fn put_multiple(
        &mut self,
        source_key: &A,
        vals: Vec<(u64, u128, D)>,
        end_block: u64,
    ) -> Result<()> {
        for (b, l, d) in vals {
            self.put(source_key, b, l, d)?;
        }
        self.sync(source_key, end_block)
    }

    /// syncs the sinks flush data
    fn sync(&mut self, source_key: &A, block_key: u64) -> Result<()> {
        // set the max known log value for the source
        self.source_vals.insert(
            source_key.clone(),
            max(block_key, self.source_vals.get(source_key).unwrap().clone()),
        );
        let last_synced = self.synced_including();
        if last_synced.is_some() {
            self.sender.send(last_synced.unwrap())?;
        }
        Ok(())
    }

    /// don't feed it source keys that you did not register
    fn put(&mut self, source_key: &A, block_key: u64, log_key: u128, data: D) -> Result<()> {
        let store_for_source = self.store.get_mut(source_key).unwrap();

        match store_for_source.get_mut(&block_key) {
            Some(entry) => {
                // put into existing log
                entry.insert(log_key, data);
            }
            None => {
                // just make a new entry
                let mut map = BTreeMap::new();
                map.insert(log_key, data);
                store_for_source.insert(block_key, map);
            }
        }
        Ok(())
    }

    /// put key into sink and then sync its state
    pub fn put_sync(
        &mut self,
        source_key: &A,
        block_key: u64,
        log_key: u128,
        data: D,
    ) -> Result<()> {
        self.put(source_key, block_key, log_key, data)?;
        self.sync(source_key, block_key)
    }
}

#[cfg(test)]
mod tests {
    use super::Sink;
    use anyhow::Result;
    use tokio::spawn;

    #[test]
    fn test_sink_print() {
        let mut sink = Sink::new(vec![0], 0);
        sink.put_sync(&0, 1, 2, "hello").unwrap();
        // sanity check
        println!("{:?}", sink);
    }

    #[test]
    fn test_up_to() {
        let mut sink = Sink::new(vec![1, 2], 0);
        sink.put_sync(&1, 3, 0, "yo").unwrap();
        assert!(sink.synced_including().is_none());
        sink.put_sync(&2, 4, 0, "hi").unwrap();
        assert_eq!(sink.synced_including().unwrap(), 2);
    }

    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_wait_until_included() -> Result<()> {
        let sink = Arc::new(Mutex::new(Sink::new(vec![1, 2], 0)));
        let sink1 = sink.clone();
        let sink2 = sink.clone();
        spawn(async move {
            sleep(Duration::new(0, 100)).await;
            for i in 1..10 {
                sink1.lock().await.put_sync(&1, i, 0, i).unwrap();
            }
        });
        spawn(async move {
            sleep(Duration::new(0, 100)).await;
            for i in 1..8 {
                sink2.lock().await.put_sync(&2, i, 0, i).unwrap();
            }
        });
        assert!(sink.lock().await.synced_including().is_none());
        println!("Before waiting {:?}", sink.lock().await.synced_including());
        Sink::wait_until_included(sink.clone(), 6).await;
        assert_eq!(sink.lock().await.synced_including().unwrap(), 6);
        Ok(())
    }

    use std::collections::HashMap;

    #[test]
    fn test_flush() {
        let mut sink = Sink::new(vec![-7, -5], 0);
        sink.put_sync(&-7, 1, 0, 0).unwrap();
        sink.put_sync(&-5, 2, 0, 0).unwrap();
        let synced = sink.synced_including().unwrap();
        assert_eq!(
            sink.flush_including(synced),
            vec![(0, HashMap::from([(-5, vec![]), (-7, vec![])]))]
        );

        sink.put_sync(&-7, 1, 1, 42).unwrap();
        sink.put_sync(&-7, 3, 0, 0).unwrap();
        sink.put_sync(&-5, 4, 0, 0).unwrap();

        let synced = sink.synced_including().unwrap();
        assert_eq!(
            sink.flush_including(synced),
            vec![
                (1, HashMap::from([(-5, vec![]), (-7, vec![0, 42])])),
                (2, HashMap::from([(-5, vec![0]), (-7, vec![])]))
            ]
        );
    }
}
