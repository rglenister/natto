include!("util/generated_macro.rs");

use crate::piece_score_tables::{KING_SCORE_ADJUSTMENT_TABLE, PAWN_SCORE_ADJUSTMENT_TABLE, PIECE_SCORE_ADJUSTMENT_TABLE};
use std::fmt::Display;
use std::sync::{LazyLock, RwLock};
use itertools::{max, Itertools};
use strum::IntoEnumIterator;
use crate::bit_board::BitBoard;
use crate::board::{Board, PieceColor, PieceType};
use crate::chess_move::ChessMove;
use crate::game::{Game, GameStatus};
use crate::game::GameStatus::InProgress;
use crate::move_generator::{generate, king_attacks_finder};
use crate::position::Position;
use crate::{move_formatter, util};
use crate::board::PieceType::{King, Knight, Pawn, Queen};
use crate::node_counter::NodeCountStats;
use crate::util::format_square;

// Define a static atomic counter
static NODE_COUNTER: LazyLock<RwLock<crate::node_counter::NodeCounter>> = LazyLock::new(|| {
    let node_counter = crate::node_counter::NodeCounter::new();
    RwLock::new(node_counter)
});


static MAXIMUM_SCORE: isize = 100000;

pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 0];

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


pub fn search(position: &Position, depth: isize, max_depth: isize) -> SearchResults {
    reset_node_counter();
    let search_results = do_search(&position,&vec!(), depth, max_depth, -MAXIMUM_SCORE, MAXIMUM_SCORE);
    eprintln!("{}", search_results);
    search_results
}

fn do_search(position: &Position, current_line: &Vec<ChessMove>, depth: isize, max_depth: isize, alpha: isize, beta: isize) -> SearchResults {
    increment_node_counter();
    if depth < max_depth {
        let moves = generate(position);
        let legal_moves: Vec<_> = moves.iter().filter_map(|m| position.make_move(m)).collect();
        let search_results = legal_moves.iter()
            .map(|(pos, cm)| { do_search(pos, &add_item(current_line, cm), depth + 1, max_depth, beta, alpha) } )
            .collect::<Vec<_>>()
            .iter().max_by(|sr1, sr2| sr2.score.cmp(&sr1.score))
                    .unwrap_or(&SearchResults {score: MAXIMUM_SCORE-depth, best_line: current_line.clone()}).clone();
        let results =  SearchResults {score: -search_results.score, best_line: search_results.best_line};
        write_uci_info(&results, depth);
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
        let mut score: isize = 0;
        util::process_bits(bitboards[Pawn as usize], |square_index| {
            score += PIECE_SCORES[Pawn as usize] + PAWN_SCORE_ADJUSTMENT_TABLE[color as usize][square_index as usize];
        });
        for piece_type in Knight as usize ..=Queen as usize {
            util::process_bits(bitboards[piece_type as usize], |square_index| {
                score += PIECE_SCORES[piece_type as usize] + PIECE_SCORE_ADJUSTMENT_TABLE[piece_type][square_index as usize];
            });
        }
        util::process_bits(bitboards[King as usize], |square_index| {
            score += PIECE_SCORES[King as usize] + KING_SCORE_ADJUSTMENT_TABLE[color as usize][square_index as usize];
        });
        score
    }

    score_board_for_color(position.board(), position.side_to_move())
        - score_board_for_color(position.board(), position.opposing_side())
}

fn write_uci_info(results: &SearchResults, depth: isize) {
    let stats = node_counter_stats();
    eprintln!("info depth {} score {} nodes {} nps {} time {}", depth, results.score, node_count(), stats.nodes_per_second, stats.elapsed_time.as_secs());
}

fn increment_node_counter() {
    NODE_COUNTER.read().unwrap().increment();
}

fn reset_node_counter() {
    NODE_COUNTER.write().unwrap().reset();
}

fn node_count() -> usize {
    NODE_COUNTER.read().unwrap().node_count()
}

fn node_counter_stats() -> NodeCountStats {
    NODE_COUNTER.read().unwrap().stats()
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


        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_pieces(&all_black_no_white), 3760);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_pieces(&all_black_no_white), 3760);
    }

    #[test]
    fn test_pawn_scores() {
        let position: Position = Position::from("4k3/P7/8/8/8/6p1/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 80);
    }

    #[test]
    fn test_knight_scores() {
        let position: Position = Position::from("N3k3/8/8/4n3/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -60);
    }

    #[test]
    fn test_bishop_scores() {
        let position: Position = Position::from("b3k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -30);
    }

    #[test]
    fn test_rook_scores() {
        let position: Position = Position::from("4k1r1/8/R7/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 0);
    }

    #[test]
    fn test_king_scores() {
        let position: Position = Position::from("8/7k/8/8/8/2K5/8/8 w - - 0 1");
        assert_eq!(score_pieces(&position), -40);
    }

    #[test]
    fn test_piece_captured() {
        let fen = "4k3/8/1P6/R3Q3/2n5/4N3/1B6/4K3 b - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 1, 2);
        assert_eq!(search_results.score, -980);
        let best_line = move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", ");
        assert_eq!(best_line, "♞c4xe5");
    }

    #[test]
    fn test_already_checkmated() {
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 0);
        println!("Node count (mated already) = {}", node_count());
        assert_eq!(search_results.score, -MAXIMUM_SCORE);
    }

    #[test]
    fn test_mate_in_one() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 1);
        println!("Node count (mate in 1) = {}", node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 1);
    }

    #[test]
    fn test_mate_in_two() {
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 3);
        println!("Node count (mate in 2) = {}", node_count());
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
        println!("Node count (mate in 3) = {}", node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 5);
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
    //     assert_eq!(search_results.score, MAXIMUM_SCORE - 7);
    // }

    #[test]
    fn test_hiarcs_game_engine_would_not_get_out_of_check() {
        let fen = "N7/pp6/8/1k6/2QR4/8/PPP4P/R1B1K3 b Q - 2 32";
        let position: Position = Position::from(fen);
        let search_results = search(&position, 0, 2);
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }
}
