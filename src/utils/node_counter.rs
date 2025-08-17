use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct NodeCountStats {
    pub node_count: usize,
    pub start_time: Instant,
    pub nodes_per_second: usize,
    pub elapsed_time: Duration,
}
pub(crate) struct NodeCounter {
    node_counter: AtomicUsize,
    start_time: Instant,
}

impl NodeCounter {
    pub(crate) fn new() -> Self {
        NodeCounter { node_counter: AtomicUsize::new(0), start_time: Instant::now() }
    }
    pub(crate) fn increment(&self) -> usize {
        self.node_counter.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn add(&self, count: usize) {
        self.node_counter.fetch_add(count, Ordering::Relaxed);
    }

    pub fn node_count(&self) -> usize {
        self.node_counter.load(Ordering::SeqCst)
    }

    pub(crate) fn reset(&mut self) {
        self.node_counter.store(0, Ordering::Relaxed);
        self.start_time = Instant::now();
    }

    pub(crate) fn stats(&self) -> NodeCountStats {
        let elapsed = self.start_time.elapsed();
        let elapsed_micros = elapsed.as_micros();
        let node_count_stats: NodeCountStats = NodeCountStats {
            node_count: self.node_count(),
            start_time: self.start_time,
            nodes_per_second: if elapsed_micros != 0 {
                (self.node_count() * 1000000) / elapsed_micros as usize
            } else {
                0
            },
            elapsed_time: elapsed,
        };
        node_count_stats
    }
}
