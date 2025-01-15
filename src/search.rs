use std::sync::atomic::{AtomicUsize, Ordering};
use crate::chess_move::ChessMove;
use crate::position::Position;

// Define a static atomic counter
static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);

static MAXIMUM_SCORE: isize = 10000;

struct SearchResults {
    score: isize,
    best_line: Vec<ChessMove>,
}

fn increment_node_counter() {
    NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn get_node_count() -> usize {
    NODE_COUNTER.load(Ordering::SeqCst)
}

fn reset_node_counter() {
    NODE_COUNTER.store(0, Ordering::SeqCst);
}

fn search(position: &Position, max_depth: isize) {
    reset_node_counter();
    let search_results = /*-*/do_search(position, 0, max_depth);
}

fn do_search(position: &Position, depth: isize, max_depth: isize) /*-> SearchResults*/ {
    increment_node_counter();
    let score = score_position(position, depth);
}

fn score_position(position: &Position, depth: isize) {
    // if position.game_status == CheckMate {
    //     depth - MAXIMUM_SCORE;
    // } else {
    //     0
    // }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use serde_derive::Deserialize;

    use std::error::Error;
    use crate::{fen, move_generator};

    use std::fs;
    use crate::position::{Position, NEW_GAME_FEN};

    #[derive(Deserialize, Debug)]

    struct FenTestCase {
        depth: usize,
        nodes: usize,
        fen: String,
    }
}
