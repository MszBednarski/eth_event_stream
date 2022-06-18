use crate::data_feed::pubsub::PubSub;
use anyhow::Result;
use std::borrow::Borrow;
use std::cmp::{max, Ord};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::watch;

use tokio::sync::{Mutex, MutexGuard};

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
struct Sink<SourceKey, T> {
    store: BTreeMap<SourceKey, BTreeMap<u64, BTreeMap<u128, T>>>,
    sources: Vec<SourceKey>,
    source_vals: HashMap<SourceKey, u64>,
    /// needed so that the sink knows how much it needs to backfill for some sources
    from_block: u64,
    ps: PubSub<u64>,
    sender: Arc<watch::Sender<u64>>,
}

impl<A: Ord + Clone + Hash, D> Sink<A, D> {
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
            from_block,
            source_vals,
            sender,
        }
    }

    /// returns non-inclusive block up to which the synced data can be flushed / was flushed
    pub fn synced_up_to(&mut self) -> u64 {
        // we can flush up to the min(of all maximums) - 1
        // thus what we output is the min(of all maximums)
        self.source_vals.values().min().unwrap().clone()
    }

    /// non-inclusive block target (all blocks below that block)
    pub async fn wait_until_at(target: Arc<Mutex<Self>>, up_to: u64) {
        let cur = target.lock().await.synced_up_to();
        if cur >= up_to {
            return;
        }
        let mut rx = target.lock().await.ps.subscribe();
        while rx.changed().await.borrow().is_ok() {
            let current = rx.borrow().clone();
            if current >= up_to {
                return;
            }
        }
        panic!("Failed to wait and pause")
    }

    /// don't feed it source keys that you did not register
    pub fn put(&mut self, source_key: &A, block_key: u64, log_key: u128, data: D) -> Result<()> {
        let store_for_source = self.store.get_mut(source_key).unwrap();
        // set the max known log value for the source
        self.source_vals.insert(
            source_key.clone(),
            max(block_key, self.source_vals.get(source_key).unwrap().clone()),
        );

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
        let up_to = self.synced_up_to();
        self.sender.send(up_to)?;
        Ok(())
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
        sink.put(&0, 1, 2, "hello").unwrap();
        // sanity check
        println!("{:?}", sink);
    }

    #[test]
    fn test_up_to() {
        let mut sink = Sink::new(vec![1, 2], 0);
        sink.put(&1, 3, 0, "yo").unwrap();
        assert_eq!(sink.synced_up_to(), 0);
        sink.put(&2, 4, 0, "hi").unwrap();
        assert_eq!(sink.synced_up_to(), 3);
    }

    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_wait_util() -> Result<()> {
        let sink = Arc::new(Mutex::new(Sink::new(vec![1, 2], 0)));
        let sink1 = sink.clone();
        let sink2 = sink.clone();
        spawn(async move {
            sleep(Duration::new(0, 100)).await;
            for i in 1..10 {
                sink1.lock().await.put(&1, i, 0, i).unwrap();
            }
        });
        spawn(async move {
            sleep(Duration::new(0, 100)).await;
            for i in 1..8 {
                sink2.lock().await.put(&2, i, 0, i).unwrap();
            }
        });
        assert_eq!(sink.lock().await.synced_up_to(), 0);
        println!("Before waiting {}", sink.lock().await.synced_up_to());
        Sink::wait_until_at(sink.clone(), 7).await;
        assert_eq!(sink.lock().await.synced_up_to(), 7);
        Ok(())
    }
}
