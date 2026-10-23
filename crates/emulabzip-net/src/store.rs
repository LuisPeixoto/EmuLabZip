//! The flows the proxy has seen, bounded in memory, with a feed of changes for the interface.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::flow::{Flow, FlowId, FlowSummary};

pub const DEFAULT_CAPACITY: usize = 10_000;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FlowEvent {
    Added { flow: FlowSummary },
    Updated { flow: FlowSummary },
    Cleared,
}

pub struct FlowStore {
    flows: Mutex<VecDeque<Flow>>,
    capacity: usize,
    next_id: AtomicU64,
    tx: broadcast::Sender<FlowEvent>,
}

impl FlowStore {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, _) = broadcast::channel(4096);
        Arc::new(FlowStore {
            flows: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
            next_id: AtomicU64::new(1),
            tx,
        })
    }

    /// Adds a flow and returns its id. The oldest flow is dropped when the store is full.
    pub fn begin(&self, mut flow: Flow) -> FlowId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        flow.id = id;
        let summary = flow.summary();
        {
            let mut flows = self.flows.lock();
            if flows.len() >= self.capacity {
                flows.pop_front();
            }
            flows.push_back(flow);
        }
        let _ = self.tx.send(FlowEvent::Added { flow: summary });
        id
    }

    /// Changes a flow. Nothing happens when it was cleared or dropped meanwhile.
    pub fn update(&self, id: FlowId, change: impl FnOnce(&mut Flow)) {
        let summary = {
            let mut flows = self.flows.lock();
            // Ids only grow, so the flows are sorted by id.
            let Ok(i) = flows.binary_search_by_key(&id, |f| f.id) else { return };
            change(&mut flows[i]);
            flows[i].summary()
        };
        let _ = self.tx.send(FlowEvent::Updated { flow: summary });
    }

    pub fn get(&self, id: FlowId) -> Option<Flow> {
        let flows = self.flows.lock();
        flows.binary_search_by_key(&id, |f| f.id).ok().map(|i| flows[i].clone())
    }

    pub fn summaries(&self) -> Vec<FlowSummary> {
        self.flows.lock().iter().map(Flow::summary).collect()
    }

    /// Every flow, oldest first, for exports.
    pub fn all(&self) -> Vec<Flow> {
        self.flows.lock().iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.flows.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.flows.lock().clear();
        let _ = self.tx.send(FlowEvent::Cleared);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FlowEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowKind, FlowState};

    fn flow(path: &str) -> Flow {
        Flow::new(FlowKind::Http, "GET", "https", "a.com", 443, path)
    }

    #[test]
    fn assigns_increasing_ids_and_updates_in_place() {
        let store = FlowStore::new(10);
        let a = store.begin(flow("/a"));
        let b = store.begin(flow("/b"));
        assert!(b > a);
        store.update(a, |f| {
            f.status = Some(204);
            f.state = FlowState::Completed;
        });
        assert_eq!(store.get(a).unwrap().status, Some(204));
        assert_eq!(store.get(b).unwrap().status, None);
    }

    #[test]
    fn drops_the_oldest_flow_when_full() {
        let store = FlowStore::new(2);
        let a = store.begin(flow("/a"));
        store.begin(flow("/b"));
        let c = store.begin(flow("/c"));
        assert_eq!(store.len(), 2);
        assert!(store.get(a).is_none());
        assert!(store.get(c).is_some());
        store.update(a, |_| panic!("an evicted flow must not be updated"));
    }

    #[tokio::test]
    async fn publishes_changes() {
        let store = FlowStore::new(10);
        let mut rx = store.subscribe();
        let id = store.begin(flow("/a"));
        store.update(id, |f| f.status = Some(200));
        store.clear();
        assert!(matches!(rx.recv().await.unwrap(), FlowEvent::Added { .. }));
        assert!(matches!(rx.recv().await.unwrap(), FlowEvent::Updated { flow } if flow.status == Some(200)));
        assert!(matches!(rx.recv().await.unwrap(), FlowEvent::Cleared));
        assert!(store.is_empty());
    }
}
