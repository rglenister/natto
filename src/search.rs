use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, LazyLock, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use itertools::Itertools;
use log::{debug, error, info};
use GameStatus::{DrawnByFiftyMoveRule, DrawnByThreefoldRepetition};
use crate::bit_board::BitBoard;
use crate::board::{Board, PieceColor};
use crate::board::PieceType::{King, Knight, Pawn, Queen};
use crate::chess_move::ChessMove;
use crate::game::{Game, GameStatus};
use crate::game::GameStatus::{Checkmate, InProgress, Stalemate};
use crate::move_generator::generate;
use crate::node_counter::NodeCountStats;
use crate::piece_score_tables::{KING_SCORE_ADJUSTMENT_TABLE, PAWN_SCORE_ADJUSTMENT_TABLE, PIECE_SCORE_ADJUSTMENT_TABLE};
use crate::position::Position;
use crate::util;

include!("util/generated_macro.rs");

static NODE_COUNTER: LazyLock<RwLock<crate::node_counter::NodeCounter>> = LazyLock::new(|| {
    let node_counter = crate::node_counter::NodeCounter::new();
    RwLock::new(node_counter)
});


pub const MAXIMUM_SEARCH_DEPTH: isize = isize::MAX;

pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 0];

const MAXIMUM_SCORE: isize = 100000;


#[derive(Clone, Debug)]
pub struct SearchResults {
    pub score: isize,
    pub depth: isize,
    pub best_line: Vec<(Position, ChessMove)>,
    pub game_status: GameStatus,
}

#[derive(Clone, Debug)]
pub struct SearchParams {
    pub allocated_time_millis: usize,
    pub max_depth: isize,
    pub max_nodes: usize,
    pub repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
}

impl SearchParams {
    pub const DEFAULT_NUMBER_OF_MOVES_TO_GO : usize = 20;

    pub fn new(allocated_time_millis: usize, max_depth: isize, max_nodes: usize, repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> SearchParams {
        SearchParams { allocated_time_millis, max_depth, max_nodes, repeat_position_counts }
    }

    pub fn new_by_depth(max_depth: isize) -> SearchParams {
        SearchParams::new(usize::MAX, max_depth, usize::MAX, None)
    }
}

impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "score: {} depth: {} bestline: {} game_status: {:?}", self.score, self.depth, self.best_line.clone().into_iter().map(|pm| pm.1).join(", "), self.game_status)
    }
}


pub fn search(position: &Position, search_params: &SearchParams, stop_flag: Arc<AtomicBool>) -> SearchResults {
    reset_node_counter();
    let mut search_results = SearchResults { score: 0, depth: 0, best_line: vec!(), game_status: GameStatus::InProgress };
    for iteration_max_depth in 0..=search_params.max_depth {
        let iteration_search_results = do_search(&position,&vec!(), 0, iteration_max_depth, search_params, -MAXIMUM_SCORE, MAXIMUM_SCORE, stop_flag.clone());
        if !stop_flag.load(Ordering::Relaxed) {
            debug!("Search results for depth {}: {}", iteration_max_depth, iteration_search_results);
            let nps = node_counter_stats().nodes_per_second;
            println!("info nps {}", nps);
            info!("info nps {}", nps);
            search_results = iteration_search_results;
            match search_results.game_status {
                Checkmate => {
                    info!("Found mate at depth {} - stopping search", iteration_max_depth);
                    break;
                }
                _ => {}
            }
        } else {
            break;
        }
    }
    search_results
}

fn do_search(position: &Position, current_line: &Vec<(Position, ChessMove)>, depth: isize, max_depth: isize, search_params: &SearchParams, mut alpha: isize, beta: isize, stop_flag: Arc<AtomicBool>) -> SearchResults {
    increment_node_counter();
    if used_allocated_move_time(search_params) {
        stop_flag.store(true, Ordering::Relaxed);
        return SearchResults { score: 0, depth, best_line: vec!(), game_status: GameStatus::InProgress };
    }
    if depth < max_depth {
        let mut best_search_results = SearchResults { score: -MAXIMUM_SCORE, depth, best_line: current_line.clone(), game_status: GameStatus::InProgress };
        let moves = generate(position);
        let mut has_legal_move = false;
        for chess_move in moves {
            if let Some(mut next_position) = position.make_move(&chess_move) {
                // there isn't a checkmate or a stalemate
                has_legal_move = true;
                let repeat_position_count = get_repeat_position_count(&next_position.0, current_line, search_params.repeat_position_counts.as_ref());
                if repeat_position_count >= 1 {
                    return SearchResults { score: 0, depth, best_line: vec!(), game_status: DrawnByThreefoldRepetition };
                }
                let mut next_result = do_search(&next_position.0, &add_item(&current_line, &next_position), depth + 1, max_depth, search_params, -beta, -alpha, stop_flag.clone());
                next_result.score = -next_result.score;
                if next_result.score > best_search_results.score {
                    best_search_results = next_result.clone();
                }
                alpha = alpha.max(next_result.score);
                if alpha >= beta || (depth >= 2 && stop_flag.load(Ordering::Relaxed)) {
                    break;
                }
            }
        };
        if !has_legal_move {
           return score_position(position, &current_line, depth);
        }
//        write_uci_info(&best_search_results, depth);
        return best_search_results;
    } else {
        return score_position(&position, &current_line, depth);
    }
    fn add_item(line: &Vec<(Position, ChessMove)>, cm: &(Position, ChessMove)) -> Vec<(Position, ChessMove)> {
        let mut appended_line = line.clone();
        appended_line.push(*cm);
        appended_line
    }
}

fn get_repeat_position_count(current_position: &Position, current_line: &Vec<(Position, ChessMove)>, historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> usize {
    let maximum_moves_to_go_back = current_position.half_move_clock().min(current_line.len());
    let position_hash = current_position.hash_code();
    let mut result = 0;
    for i in (0..maximum_moves_to_go_back).rev() {
        let previous_position = &current_line[i];
        // let previous_move = previous_position.1.get_base_move();
        // if previous_move.capture || previous_position.0.board().get_piece(previous_position.1.get_base_move().to).unwrap() == Pawn {
        //     break;
        // }
        if previous_position.0.hash_code() == position_hash {
            result += 1;
        }
    }
    result += historic_repeat_position_counts.map(|historic_repeat_position_counts| historic_repeat_position_counts.get(&position_hash).map(|(_, count)| *count).unwrap_or(0)).unwrap_or(0);
    result
}

fn score_position(position: &Position, current_line: &Vec<(Position, ChessMove)>, depth: isize) -> SearchResults {
    let game = Game::new(position);
    match game.get_game_status() {
        InProgress => { SearchResults {score: score_pieces(position), depth, best_line: current_line.clone(), game_status: InProgress }}
        Checkmate => { SearchResults {score: depth - MAXIMUM_SCORE, depth, best_line: current_line.clone(), game_status: Checkmate }}
        Stalemate => { SearchResults {score: 0, depth, best_line: current_line.clone(), game_status: Stalemate }}
        DrawnByFiftyMoveRule => { SearchResults {score: 0, depth, best_line: current_line.clone(), game_status: DrawnByFiftyMoveRule }}
        DrawnByThreefoldRepetition => { SearchResults {score: 0, depth, best_line: current_line.clone(), game_status: DrawnByThreefoldRepetition }}
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

fn used_allocated_move_time(search_params: &SearchParams) -> bool {
    let stats = node_counter_stats();
    stats.elapsed_time.as_millis() > search_params.allocated_time_millis.try_into().unwrap()
}

fn increment_node_counter() -> NodeCountStats {
    let node_counter = NODE_COUNTER.read().unwrap();
    node_counter.increment();
    node_counter_stats()
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
    use crate::chess_move::{format_moves, RawChessMove};
    use crate::game::GameStatus::DrawnByFiftyMoveRule;
    use crate::{move_formatter, uci};
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
        assert_eq!(score_pieces(&black_pawn_on_seventh_rank), -260);
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
        let search_results = search(&position, &SearchParams { allocated_time_millis: usize::MAX, max_depth: 1, max_nodes: usize::MAX, repeat_position_counts: None }, Arc::new(AtomicBool::new(false)));
        assert_eq!(search_results.score, -980);
        let best_line = move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", ");
        assert_eq!(best_line, "♞c4xe5");
    }

    #[test]
    fn test_already_checkmated() {
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)));
        println!("Node count (mated already) = {}", node_count());
        assert_eq!(search_results.score, -MAXIMUM_SCORE);
    }

    #[test]
    fn test_already_stalemated() {
        let fen = "8/6n1/5k1K/6n1/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)));
        println!("Node count (mated already) = {}", node_count());
        assert_eq!(search_results.score, 0);
    }

    #[test]
    fn test_mate_in_one() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 1) = {}", node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 1);
    }

    #[test]
    fn test_mate_in_one_using_high_depth() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 1) = {}", node_count());
        assert_eq!(search_results.score, MAXIMUM_SCORE - 1);
    }

    #[test]
    fn test_mate_in_two() {
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 2) = {}", node_count());
        println!("{}", search_results.best_line[0].1);
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 3);
    }

    #[test]
    fn test_mate_in_three() {
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 3) = {}", node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 5);
    }

    #[test]
    fn test_mate_in_four() {
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(7), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 4) = {}", node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        assert_eq!(search_results.score, MAXIMUM_SCORE - 7);
    }

    #[test]
    fn test_mate_in_three_fischer() {
        let fen = "8/8/8/8/4k3/8/8/2BQKB2 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(7), Arc::new(AtomicBool::new(false)));
        println!("Node count (mate in 3) = {}", node_count());
        println!("best line = {:?}", format_moves(&search_results.best_line));
        println!("best line++ = {}", move_formatter::SHORT_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", "));
        println!("search_results = {}", search_results);
        assert_eq!(search_results.score, MAXIMUM_SCORE - 5);
    }

    #[test]
    fn test_hiarcs_game_engine_would_not_get_out_of_check() {
        let fen = "N7/pp6/8/1k6/2QR4/8/PPP4P/R1B1K3 b Q - 2 32";
        let position: Position = Position::from(fen);
        let search_results = search(&position, &SearchParams::new_by_depth(2), Arc::new(AtomicBool::new(false)));
        println!("best line++ = {}", move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(","));
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }

    #[test]
    fn test_50_move_rule_is_recognised() {
        let fen = "4k3/8/R7/7n/7r/8/8/4K3 b - - 49 76";
        let in_progress_position: Position = Position::from(fen);
        let in_progress_search_results = search(&in_progress_position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)));
        assert_eq!(in_progress_search_results.game_status, InProgress);
        assert_eq!(in_progress_search_results.score, 260);

        let drawn_position = in_progress_position.make_raw_move(&RawChessMove::new(sq!("h5"), sq!("f4"), None)).unwrap().0;
        let drawn_position_search_results = search(&drawn_position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)));
        assert_eq!(drawn_position_search_results.game_status, DrawnByFiftyMoveRule);
        assert_eq!(drawn_position_search_results.score, 0);
    }

    #[test]
    fn test_losing_side_plays_for_draw() {
        let uci_position = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1 moves g8f6 g1f3 f6g8 f3g1 g8f6 g1f3 f6g8 f3g1";
        let game_history = uci::parse_position(uci_position).unwrap();
        let go_options = uci::parse_uci_go_options(Some("depth 3".to_string()));
        let search_params = uci::create_search_params(&go_options, &game_history);
        let drawn_position_search_results = search(&game_history.given_position, &search_params, Arc::new(AtomicBool::new(false)));
        assert_eq!(drawn_position_search_results.game_status, DrawnByThreefoldRepetition);
        assert_eq!(drawn_position_search_results.score, 0);
    }
}
