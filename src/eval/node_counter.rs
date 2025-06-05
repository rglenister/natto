use std::os::macos::raw::stat;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};
use crate::core::move_generator::generate_moves;
use rayon::prelude::*;
use crate::core::position::Position;

static NODE_COUNTER: LazyLock<RwLock<NodeCounter>> = LazyLock::new(|| {
    let node_counter = NodeCounter::new();
    RwLock::new(node_counter)
});

const NODE_COUNTS_AT_DEPTH: [usize; 11] = [
    1,
    20,
    400,
    8902,
    197281,
    4865609,
    119060324,
    3195901860,
    84998978956,
    2439530234167,
    69352859712417,
];
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
            nodes_per_second: if elapsed_micros != 0 { (self.node_count() * 1000000) / elapsed_micros as usize } else { 0 },
            elapsed_time: elapsed,
        };
        node_count_stats
    }
}

pub fn perf_t() {
    for depth in 0..NODE_COUNTS_AT_DEPTH.len() {
        let stats = count_nodes(&Position::new_game(), depth);
        assert_eq!(NODE_COUNTS_AT_DEPTH[depth], stats.node_count);
        println!("Depth {} nodes {} nps {}", depth, stats.node_count, stats.nodes_per_second);
    }
}

pub fn count_nodes(position: &Position, max_depth: usize) -> NodeCountStats {
    let mut node_counter = NodeCounter::new();
    node_counter.reset();
    let count = do_count_nodes::<true>(position, 0, max_depth);
    node_counter.add(count);
    node_counter.stats()
}

fn do_count_nodes<const USE_PARALLEL_ITERATOR: bool>(position: &Position, depth: usize, max_depth: usize) -> usize {
    if depth < max_depth {
        let moves = generate_moves(position);
        if USE_PARALLEL_ITERATOR {
            moves
                .par_iter()
                .filter_map(|cm| position.make_move(cm))
                .map(|(pos, _)| do_count_nodes::<false>( &pos, depth + 1, max_depth))
                .sum()
        } else {
            moves
                .iter()
                .filter_map(|cm| position.make_move(cm))
                .map(|(pos, _)| do_count_nodes::<false>( &pos, depth + 1, max_depth))
                .sum()
        }
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_perft() {
        let position = Position::new_game();
        let node_count_stats = count_nodes(&position, 5);
        assert_eq!(node_count_stats.node_count, 4865609);
    }
}