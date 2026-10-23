//! Live streams produced by backends.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// A live stream of items (log lines, samples) produced by a background task.
///
/// Dropping the stream stops the task, which in turn closes the `adb` socket or kills the
/// `simctl` child process that feeds it. Nothing keeps running after the reader goes away.
#[derive(Debug)]
pub struct Stream<T> {
    rx: mpsc::Receiver<T>,
    task: JoinHandle<()>,
}

impl<T: Send + 'static> Stream<T> {
    /// Spawns `producer` with the sending half of a channel that holds up to `capacity` items.
    pub fn spawn<F, Fut>(capacity: usize, producer: F) -> Self
    where
        F: FnOnce(mpsc::Sender<T>) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(capacity);
        let task = tokio::spawn(producer(tx));
        Stream { rx, task }
    }

    /// Waits for the next item. Returns `None` once the producer has finished.
    pub async fn next(&mut self) -> Option<T> {
        self.rx.recv().await
    }

    /// Takes every item that is already waiting, without blocking.
    pub fn drain_ready(&mut self, max: usize) -> Vec<T> {
        let mut out = Vec::new();
        while out.len() < max {
            match self.rx.try_recv() {
                Ok(item) => out.push(item),
                Err(_) => break,
            }
        }
        out
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished() && self.rx.is_empty()
    }
}

impl<T> Drop for Stream<T> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn yields_items_then_ends() {
        let mut s = Stream::spawn(4, |tx| async move {
            for i in 0..3 {
                tx.send(i).await.ok();
            }
        });
        let mut got = Vec::new();
        while let Some(i) = s.next().await {
            got.push(i);
        }
        assert_eq!(got, vec![0, 1, 2]);
    }

    #[tokio::test]
    async fn dropping_the_stream_stops_the_producer() {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let s = Stream::spawn(1, |tx| async move {
            let _guard = done_tx;
            loop {
                if tx.send(1u8).await.is_err() {
                    break;
                }
            }
        });
        drop(s);
        // The guard is dropped when the task is aborted, which closes the oneshot.
        assert!(done_rx.await.is_err());
    }
}
