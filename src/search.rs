include!("util/generated_macro.rs");

use std::sync::atomic::{AtomicUsize, Ordering};
use itertools::{max, Itertools};
use crate::chess_move::ChessMove;
use crate::game::{Game, GameStatus};
use crate::game::GameStatus::InProgress;
use crate::move_generator::{generate, king_attacks_finder};
use crate::position::Position;
use crate::move_formatter;

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
            .map(|(pos, cm)| { do_search(pos, &add_item(current_line, cm), depth + 1, max_depth) } )
            .collect::<Vec<_>>()
            .iter().max_by(|sr1, sr2| sr2.score.cmp(&sr1.score))
                    .unwrap_or(&SearchResults {score: MAXIMUM_SCORE-depth, best_line: vec!()}).clone();
        return SearchResults {score: -search_results.score, best_line: search_results.best_line}
    } else {
        return score_position(position, current_line, depth)
    }
    fn add_item(line: &Vec<ChessMove>, cm: &ChessMove) -> Vec<ChessMove> {
        let mut appended_line = line.clone();
        appended_line.push(*cm);
        appended_line
    }
}

fn score_position(position: &Position, current_line: &Vec<ChessMove>, depth: isize) -> SearchResults {
    if king_attacks_finder(position, position.side_to_move()) == 0 {
        return SearchResults {score: 0, best_line: current_line.clone()};
    }
    let game = Game::new(position);
    if game.get_game_status() != InProgress {
        if game.get_game_status() == GameStatus::Checkmate {
            SearchResults {score: depth - MAXIMUM_SCORE, best_line: current_line.clone()}
        } else {
            SearchResults {score: 0, best_line: current_line.clone()}
        }
    } else {
        SearchResults {score: 0, best_line: current_line.clone()}
    }
}

#[cfg(test)]
mod tests {
    use crate::chess_move::format_moves;
    use crate::move_formatter::FormatMove;
    use super::*;
    use crate::search::{search, MAXIMUM_SCORE};
    use crate::util::bit_indexes;

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
        println!("{}", search_results.best_line[0]);
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 3);
    }

    #[test]
    fn test_mate_in_three() {
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 5);
        println!("Node count (mate in 3) = {}", get_node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 5);
    }


    #[test]
    fn test_mate_in_four() {
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 7);
        println!("Node count (mate in 4) = {}", get_node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 77);
    }
}
