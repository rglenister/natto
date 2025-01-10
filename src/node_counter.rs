use crate::move_generator;
use crate::position::Position;

use std::sync::atomic::{AtomicUsize, Ordering};

// Define a static atomic counter
static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn increment_node_counter() {
    NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn get_node_count() -> usize {
    NODE_COUNTER.load(Ordering::SeqCst)
}

fn reset_node_counter() {
    NODE_COUNTER.store(0, Ordering::SeqCst);
}

pub fn count_nodes(position: &Position, max_depth: i32) -> usize{
    reset_node_counter();
    do_count_nodes(position, 0, max_depth);
    get_node_count()
}

fn do_count_nodes(position: &Position, depth: i32, max_depth: i32) -> () {
    if depth != 0 {
//        increment_node_counter();
    }
    if depth < max_depth {
        move_generator::generate(position)
            .iter()
            .filter_map(|cm| position.make_move(cm))
            .map(|pos| do_count_nodes( &pos, depth + 1, max_depth))
            .count();
    } else {
        increment_node_counter();
    }
    ()
}
