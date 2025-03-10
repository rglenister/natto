use std::sync::atomic::{AtomicUsize, Ordering};
use crate::position::Position;
use crate::move_generator::generate;
use std::time::{Duration, Instant};
use std::sync::{LazyLock, RwLock};

static NODE_COUNTER: LazyLock<RwLock<NodeCounter>> = LazyLock::new(|| {
    let node_counter = NodeCounter::new();
    RwLock::new(node_counter)
});

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
        NodeCounter {  node_counter: AtomicUsize::new(0), start_time: Instant::now() }
    }
    pub(crate) fn increment(&self) -> usize {
        self.node_counter.fetch_add(1, Ordering::Relaxed)
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
            nodes_per_second: if elapsed_micros != 0 { (self.node_count() * 1000000) / elapsed_micros as usize } else { 0 },
            elapsed_time: elapsed,
        };
        node_count_stats
    }
}



pub fn count_nodes(position: &Position, max_depth: i32) -> NodeCountStats {
    NODE_COUNTER.write().unwrap().reset();
    do_count_nodes(position, 0, max_depth);
    NODE_COUNTER.read().unwrap().stats()
}

fn do_count_nodes(position: &Position, depth: i32, max_depth: i32) {
    if depth != 0 {
        //        increment_node_counter();
    }
    if depth < max_depth {
        generate(position)
            .iter()
            .filter_map(|cm| position.make_move(cm))
            .map(|(pos, _)| do_count_nodes( &pos, depth + 1, max_depth))
            .count();
    } else {
        NODE_COUNTER.read().unwrap().increment();
    }
}
