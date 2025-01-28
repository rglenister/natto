include!("util/generated_macro.rs");

use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};
use itertools::{max, Itertools};
use strum::IntoEnumIterator;
use crate::bit_board::BitBoard;
use crate::board::{Board, PieceColor, PieceType};
use crate::chess_move::ChessMove;
use crate::game::{Game, GameStatus};
use crate::game::GameStatus::InProgress;
use crate::move_generator::{generate, king_attacks_finder};
use crate::position::Position;
use crate::move_formatter;
use crate::util::format_square;

// Define a static atomic counter
static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);

static MAXIMUM_SCORE: isize = 100000;

pub const PIECE_SCORES: [usize; 6] = [100, 300, 300, 500, 900, 0];

#[derive(Clone, Debug)]
pub struct SearchResults {
    pub score: isize,
    pub best_line: Vec<ChessMove>,
}

impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "score: {} bestline: {}", self.score, self.best_line.clone().into_iter().join(", "))
    }
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

pub fn search(position: &Position, depth: isize, max_depth: isize) -> SearchResults {
    reset_node_counter();
    let search_results = do_search(&position,&vec!(), depth, max_depth);
    eprintln!("{}", search_results);
    search_results
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
        let results =  SearchResults {score: -search_results.score, best_line: search_results.best_line};
        eprintln!("info depth {} seldepth {} score cp {} nodes {} nps {} time {}", depth, 0, results.score, get_node_count(), "?nps?", "?time?");
        // info depth 20 seldepth 32 score cp 38 nodes 105456 nps 5230 time 201 pv e2e4 e7e5
        return results;
    } else {
        return score_position(position, current_line, depth);
    }
    fn add_item(line: &Vec<ChessMove>, cm: &ChessMove) -> Vec<ChessMove> {
        let mut appended_line = line.clone();
        appended_line.push(*cm);
        appended_line
    }
}

fn score_position(position: &Position, current_line: &Vec<ChessMove>, depth: isize) -> SearchResults {
    if king_attacks_finder(position, position.side_to_move()) == 0 {
        return SearchResults {score: score_pieces(position), best_line: current_line.clone()};
    }
    let game = Game::new(position);
    if game.get_game_status() != InProgress {
        if game.get_game_status() == GameStatus::Checkmate {
            SearchResults {score: depth - MAXIMUM_SCORE, best_line: current_line.clone()}
        } else {
            SearchResults {score: score_pieces(position), best_line: current_line.clone()}
        }
    } else {
        SearchResults {score: score_pieces(position), best_line: current_line.clone()}
    }
}

fn score_pieces(position: &Position) -> isize {
    fn score_board_for_color(board: &BitBoard, color: PieceColor) -> isize {
        let bitboards = board.bitboards_for_color(color);
        PieceType::iter().map(|piece_type| {
            let piece_count = bitboards[piece_type as usize].count_ones() as isize;
            piece_count * PIECE_SCORES[piece_type as usize] as isize
        }).sum()
    }

    score_board_for_color(position.board(), position.side_to_move())
        - score_board_for_color(position.board(), position.opposing_side())
}

#[cfg(test)]
mod tests {
    use crate::chess_move::format_moves;
    use crate::move_formatter::FormatMove;
    use crate::position::NEW_GAME_FEN;
    use super::*;
    use crate::search::{search, MAXIMUM_SCORE};

    #[test]
    fn test_score_pieces() {
        let position: Position = Position::from(NEW_GAME_FEN);
        assert_eq!(score_pieces(&position), 0);

        let missing_white_pawn: Position = Position::from("rnbqkbnr/pppppppp/8/8/8/8/PPP1PPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_pieces(&missing_white_pawn), -100);

        let missing_black_pawn: Position = Position::from("rnbqkbnr/1ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_pieces(&missing_black_pawn), 100);

        let fen = "7K/5k2/8/7r/8/8/8/8 b - - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(score_pieces(&position), 500);

        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_pieces(&all_black_no_white), 3900);
    }

    #[test]
    fn test_piece_captured() {
        let fen = "4k3/8/1P6/R3Q3/2n5/4N3/1B6/4K3 b - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 1, 2);
        assert_eq!(search_results.score, -900);
        let best_line = move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", ");
        assert_eq!(best_line, "♞c4xe5");
    }

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
        assert_eq!(search_results.score, MAXIMUM_SCORE - 55);
    }


    // #[test]
    // fn test_mate_in_four() {
    //     let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
    //     let position: Position = Position::from(fen);
    //     let search_results = search(&position, 0, 7);
    //     println!("Node count (mate in 4) = {}", get_node_count());
    //     println!("best line = {:?}", format_moves(&search_results.best_line));
    //     println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
    //     println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
    //     assert_eq!(search_results.score, MAXIMUM_SCORE - 77);
    // }
}
