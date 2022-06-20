use std::sync::Arc;
use tokio::sync::watch::{self, Receiver, Sender};

/// gives a pub sub interface to watch what is the current state of variable
#[derive(Debug)]
pub struct PubSub<T> {
    /// basically owns the entire watch channel
    arc: Arc<Sender<T>>,
    /// we need this receiver so that the channel does not get closed
    _rx: Receiver<T>,
}

impl<A> PubSub<A> {
    pub fn new(init: A) -> Self {
        let (s, _rx) = watch::channel(init);
        PubSub {
            arc: Arc::new(s),
            _rx,
        }
    }

    pub fn sender(&self) -> Arc<Sender<A>> {
        self.arc.clone()
    }

    pub fn subscribe(&self) -> Receiver<A> {
        self.arc.clone().subscribe()
    }
}

#[cfg(test)]
mod test {
    use super::PubSub;
    use tokio::spawn;

    #[tokio::test]
    async fn test_pubsub() {
        let ps = PubSub::new(-1);

        let sender = ps.sender();
        let sender2 = ps.sender();
        let mut r1 = ps.subscribe();

        spawn(async move {
            sender.send(0).unwrap();
            sender.send(1).unwrap()
        })
        .await
        .unwrap();

        spawn(async move {
            assert_eq!(*r1.borrow(), 1);
            sender2.send(2).unwrap();
            if r1.changed().await.is_ok() {
                println!("I triggered. ");
                assert_eq!(*r1.borrow(), 2);
            }
        })
        .await
        .unwrap();
    }
}
