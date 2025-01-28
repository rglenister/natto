use std::os::raw::c_double;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::position::Position;
use crate::move_generator::generate;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::LazyLock;

static NODE_COUNTER: LazyLock<NodeCounter> = LazyLock::new(|| {
    let node_counter = NodeCounter::new();
    node_counter
});

pub struct NodeCountStats {
    pub node_count: usize,
    pub start_time: Instant,
    pub nodes_per_second: usize,
}
struct NodeCounter {
    node_counter: AtomicUsize,
    start_time: Instant,
}

impl NodeCounter {
    fn new() -> Self {
        NodeCounter {  node_counter: AtomicUsize::new(0), start_time: Instant::now() }
    }
    fn increment(&self) {
        self.node_counter.fetch_add(1, Ordering::Relaxed);
    }

    fn node_count(&self) -> usize {
        self.node_counter.load(Ordering::SeqCst)
    }

    // fn reset(&mut self) {
    //     self.node_counter.store(0, Ordering::Relaxed);
    //     self.start_time = Instant::now();
    // }

    fn stats(&self) -> NodeCountStats {
        let node_count_stats: NodeCountStats = NodeCountStats {
            node_count: self.node_count(),
            start_time: self.start_time,
            nodes_per_second: self.node_count() / self.start_time.elapsed().as_micros() as usize / 1000000,
        };
        node_count_stats
    }
}



pub fn count_nodes(position: &Position, max_depth: i32) -> usize {
//    NODE_COUNTER.reset();
    do_count_nodes(position, 0, max_depth);
    NODE_COUNTER.stats().node_count
}

fn do_count_nodes(position: &Position, depth: i32, max_depth: i32) -> () {
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
        NODE_COUNTER.increment();
    }
    ()
}
