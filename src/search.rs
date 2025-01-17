use std::sync::atomic::{AtomicUsize, Ordering};
use itertools::{max, Itertools};
use crate::chess_move::ChessMove;
use crate::chess_move::ChessMove::BasicMove;
use crate::game::{Game, GameStatus};
use crate::game::GameStatus::InProgress;
use crate::move_generator::generate;
use crate::position::Position;

include!("util/generated_macro.rs");

// Define a static atomic counter
static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);

static MAXIMUM_SCORE: isize = 10000;

#[derive(Clone)]
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

fn search(position: &Position, depth: isize, max_depth: isize) -> SearchResults {
    reset_node_counter();
    do_search(position,&vec!(), depth, max_depth)
}

fn do_search(position: &Position, current_line: &Vec<ChessMove>, depth: isize, max_depth: isize) -> SearchResults {
    increment_node_counter();
    if depth < max_depth {
        let moves = generate(position);
        let legal_moves: Vec<_> = moves.iter().filter_map(|m| position.make_move(m)).collect();
        let search_results = legal_moves.iter()
            .map(|(pos, cm)| { do_search(pos, /* cm + */ current_line, depth + 1, max_depth) } )
            .collect::<Vec<_>>()
            .iter().max_by(|sr1, sr2| sr2.score.cmp(&sr1.score)).unwrap_or(&SearchResults {score: MAXIMUM_SCORE-depth, best_line: vec!()}).clone();
        negate_search_results(&search_results)
    } else {
        score_position(position, depth)
    }
}

fn score_position(position: &Position, depth: isize) -> SearchResults {
    let game = Game::new(position);
    if game.get_game_status() != InProgress {
        if game.get_game_status() == GameStatus::Checkmate {
            SearchResults {score: depth - MAXIMUM_SCORE, best_line: Vec::new()}
        } else {
            SearchResults {score: 0, best_line: Vec::new()}
        }
    } else {
        SearchResults {score: 0, best_line: Vec::new()}
    }
}

fn negate_search_results(results: &SearchResults) -> SearchResults {
    SearchResults {score: -results.score, best_line: Vec::new()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{search, MAXIMUM_SCORE};

    #[test]
    fn test_already_checkmated() {
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 0);
        println!("Node count (mated already) = {}", get_node_count());
        assert_eq!(search_results.score, -MAXIMUM_SCORE);
    }

    #[test]
    fn test_mate_in_one() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 1);
        println!("Node count (mate in 1) = {}", get_node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 1);
    }

    #[test]
    fn test_mate_in_two() {
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 3);
        println!("Node count (mate in 2) = {}", get_node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 3);
    }

    #[test]
    fn test_mate_in_three() {
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 5);
        println!("Node count (mate in 3) = {}", get_node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 5);
    }
}
