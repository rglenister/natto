use env::var;
use crate::bit_board::BitBoard;
use crate::board::PieceColor;
use crate::board::PieceType::{King, Knight, Pawn, Queen};
use crate::chess_move::ChessMove;
use crate::game::GameStatus::{Checkmate, DrawnByInsufficientMaterial, InProgress, Stalemate};
use crate::game::{Game, GameStatus};
use crate::move_formatter::{FormatMove, LONG_FORMATTER};
use crate::move_generator::generate;
use crate::node_counter::NodeCountStats;
use crate::evaluation::piece_score_tables::{KING_SCORE_ADJUSTMENT_TABLE, PAWN_SCORE_ADJUSTMENT_TABLE, PIECE_SCORE_ADJUSTMENT_TABLE};
use crate::position::Position;
use crate::evaluation::sorted_move_list::SortedMoveList;
use crate::{uci, util};
use itertools::Itertools;
use log::{debug, info};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::ops::Neg;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use dotenv::dotenv;
use once_cell::sync::Lazy;
use GameStatus::{DrawnByFiftyMoveRule, DrawnByThreefoldRepetition};
use crate::evaluation::ttable;
use crate::evaluation::ttable::{BoundType, TTEntry, TranspositionTable};
use crate::evaluation::ttable::BoundType::Exact;

include!("../util/generated_macro.rs");

pub static TRANSPOSITION_TABLE: Lazy<TranspositionTable> = Lazy::new(|| {
    let default_size = 1 << 25;
    let transposition_table_size: usize = var("TRANSPOSITION_TABLE_SIZE")
        .unwrap_or_else(|_| default_size.to_string())
        .parse::<usize>()
        .unwrap_or(default_size);
    info!("Creating transposition table with size {} ({:#X})", transposition_table_size, transposition_table_size);
    let transposition_table = TranspositionTable::new(transposition_table_size);
    info!("Transposition table created");
    transposition_table
});

static NODE_COUNTER: LazyLock<RwLock<crate::node_counter::NodeCounter>> = LazyLock::new(|| {
    let node_counter = crate::node_counter::NodeCounter::new();
    RwLock::new(node_counter)
});

fn long_format_moves(position: &Position, search_results: &SearchResults) -> String {
    LONG_FORMATTER.format_move_list(position, &search_results.best_line).unwrap().join(",")
}

pub const MAXIMUM_SEARCH_DEPTH: isize = u8::MAX as isize;

pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 0];

pub const MAXIMUM_SCORE: isize = 100_000;


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResults {
    pub score: isize,
    pub depth: usize,
    pub best_line: Vec<(Position, ChessMove)>,
    pub game_status: GameStatus,
}

pub struct PositionWithSearchResults<'a> {
    pub position: &'a Position,
    pub search_results: &'a SearchResults,
}
impl Display for PositionWithSearchResults<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "score: {} depth: {} bestline: {} game_status: {:?}",
               self.search_results.score,
               self.search_results.depth,
               LONG_FORMATTER.format_move_list(self.position, &*self.search_results.best_line).unwrap().join(", "),
               self.search_results.game_status)
    }
}

#[derive(Clone, Debug)]
pub struct SearchParams {
    pub allocated_time_millis: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl Display for SearchParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "allocated_time_millis: {} max_depth: {} max_nodes: {}",
               self.allocated_time_millis,
               self.max_depth,
               self.max_nodes,
        )
    }
}
impl SearchParams {
    pub const DEFAULT_NUMBER_OF_MOVES_TO_GO : usize = 30;

    pub fn new(allocated_time_millis: usize, max_depth: usize, max_nodes: usize) -> SearchParams {
        SearchParams { allocated_time_millis, max_depth, max_nodes }
    }

    pub fn new_by_depth(max_depth: usize) -> SearchParams {
        SearchParams::new(usize::MAX, max_depth, usize::MAX)
    }
}
struct SearchContext<'a> {
    search_params: &'a SearchParams,
    stop_flag: Arc<AtomicBool>,
    sorted_root_moves: RefCell<SortedMoveList>,
    repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
}

impl SearchContext<'_> {
    pub fn new(
        search_params: &SearchParams,
        stop_flag: Arc<AtomicBool>,
        repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
        moves: Vec<ChessMove>,
    ) -> SearchContext {
        SearchContext {
            search_params,
            stop_flag,
            sorted_root_moves: RefCell::new(SortedMoveList::new(&moves)),
            repeat_position_counts,
        }
    }
}
impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "score: {} depth: {} bestline: {} game_status: {:?}",
               self.score,
               self.depth,
               self.best_line_moves().into_iter().join(", "),
               self.game_status)
    }
}

impl SearchResults {
    fn best_line_moves(&self) -> Vec<ChessMove> {
        self.best_line.clone().into_iter().map(|pm| pm.1).collect()
    }
    fn best_line_moves_as_string(&self) -> String {
        self.best_line_moves().iter().join(",")
    }
    pub fn negate_score(&mut self) {
        self.score = -self.score;
    }
}


pub fn iterative_deepening_search(
    position: &Position, search_params: &SearchParams,
    stop_flag: Arc<AtomicBool>,
    repeat_position_counts: Option<HashMap<u64, (Position, usize)>>
) -> SearchResults {
    reset_node_counter();
    let mut search_context = SearchContext::new(search_params, stop_flag, repeat_position_counts, generate(position));
    let mut score = -MAXIMUM_SCORE;
    for iteration_max_depth in 0..=search_params.max_depth {
       TRANSPOSITION_TABLE.clear();
        score = minimax(position, &vec!(), 0, iteration_max_depth, &mut search_context, -MAXIMUM_SCORE, MAXIMUM_SCORE);
        if !search_context.stop_flag.load(Ordering::Relaxed) {
            let (best_line, game_status) = retrieve_principal_variation(&TRANSPOSITION_TABLE, *position, None);
            debug!("Search results for depth {}: {}", iteration_max_depth, PositionWithSearchResults { position, search_results: &SearchResults { score, depth: iteration_max_depth, best_line, game_status } });
            let nps = node_counter_stats().nodes_per_second;
            uci::send_to_gui(format!("info nps {}", nps));
            if score.abs() >= MAXIMUM_SCORE - (iteration_max_depth as isize) {
                info!("Found mate at depth {} - stopping search", iteration_max_depth);
                break;
            }
        } else {
            break;
        }
    }
    info!("Search complete - pv table size is {}", TRANSPOSITION_TABLE.item_count());
    if let Some(entry) = TRANSPOSITION_TABLE.retrieve(position.hash_code()) {
        let variation = retrieve_principal_variation(&TRANSPOSITION_TABLE, *position, search_context.repeat_position_counts);
        SearchResults {
            score: score,
            depth: entry.depth,
            best_line: variation.0,
            game_status: variation.1,
        }
    } else {
        SearchResults {
            score,
            depth: 0,
            best_line: vec![],
            game_status: get_game_status(position, search_context.repeat_position_counts),
        }
    }
}

fn minimax(
    position: &Position,
    current_line: &[(Position, ChessMove)],
    depth: usize,
    max_depth: usize,
    search_context: &mut SearchContext,
    mut alpha: isize,
    mut beta: isize,
) -> isize {
    increment_node_counter();
    if used_allocated_move_time(search_context.search_params) {
        search_context.stop_flag.store(true, Ordering::Relaxed);
        return 0;
    }
    if let Some(entry) = TRANSPOSITION_TABLE.retrieve(position.hash_code()) {
        if entry.depth >= depth {
            match entry.bound {
                BoundType::Exact => return entry.score,
                BoundType::LowerBound => alpha = alpha.max(entry.score),
                BoundType::UpperBound =>  beta = beta.min(entry.score),
                _ => (),
            }
            if alpha >= beta {
                return entry.score;
            }
        }
    }
    if depth < max_depth {
        let mut best_score = -MAXIMUM_SCORE;
        let moves = if depth == 0 { search_context.sorted_root_moves.get_mut().get_all_moves() } else { generate(position) };
        let mut best_move: Option<ChessMove> = None;
        for chess_move in moves {
            if let Some(next_position) = position.make_move(&chess_move) {
                let next_score: isize = if get_repeat_position_count(&next_position.0, &add_item(current_line, &next_position), search_context.repeat_position_counts.as_ref()) >= 2 {
                    0
                } else {
                    -minimax(&next_position.0, &add_item(current_line, &next_position), depth + 1, max_depth, search_context, -beta, -alpha)
                };
                if depth == 0 {
                    search_context.sorted_root_moves.borrow_mut().update_score(&chess_move, next_score)
                };
                if next_score > best_score {
                    best_score = next_score;
                    best_move = Some(chess_move);
                }
                if next_score > alpha {
                    alpha = alpha.max(next_score);
                    if alpha >= beta /*|| (depth >= 2 && search_context.stop_flag.load(Ordering::Relaxed))*/ {
                        break;
                    }
                }
            }
        };
        if let Some(best_move) = best_move {
            let bound = if best_score <= alpha {
                BoundType::UpperBound
            } else if best_score >= beta {
                BoundType::LowerBound
            } else {
                BoundType::Exact
            };
            TRANSPOSITION_TABLE.store(position.hash_code(), best_move, (max_depth - depth) as u8, best_score as i32, Exact, InProgress);
            let entry = TRANSPOSITION_TABLE.retrieve(position.hash_code()).unwrap();
            assert_eq!(entry.zobrist, position.hash_code());
            assert_eq!(entry.best_move, best_move);
            assert_eq!(entry.depth, max_depth - depth);
            assert_eq!(entry.score, best_score);
            assert_eq!(entry.bound, Exact);
            assert_eq!(entry.game_status, InProgress);
        } else {
            return score_position(position, current_line, depth);
        }
//        write_uci_info(&best_search_results, depth);
        return best_score;
    } else {
        return score_position(position, current_line, depth);
    }
    fn add_item(line: &[(Position, ChessMove)], cm: &(Position, ChessMove)) -> Vec<(Position, ChessMove)> {
        let mut appended_line = line.to_owned();
        appended_line.push(*cm);
        appended_line
    }
}

pub fn get_repeat_position_count(current_position: &Position, current_line: &[(Position, ChessMove)], historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> usize {
    let maximum_moves_to_go_back = current_position.half_move_clock().min(current_line.len());
    let position_hash = current_position.hash_code();
    let mut result = 0;
    for i in (0..maximum_moves_to_go_back).rev() {
        let previous_position = &current_line[i];
        if previous_position.0.hash_code() == position_hash {
            result += 1;
        }
    }

    result += historic_repeat_position_counts
        .and_then(|historic_repeat_position_counts| historic_repeat_position_counts.get(&position_hash))
        .map(|pos_and_size| pos_and_size.1)
        .unwrap_or(0);
    result
}

fn score_position(position: &Position, current_line: &[(Position, ChessMove)], depth: usize) -> isize {
    let game = Game::new(position, None);
    let game_status = game.get_game_status();
    match game_status {
        InProgress => score_pieces(position),
        Checkmate => (depth as isize) - MAXIMUM_SCORE,
        _ => 0,
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
            util::process_bits(bitboards[piece_type], |square_index| {
                score += PIECE_SCORES[piece_type] + PIECE_SCORE_ADJUSTMENT_TABLE[piece_type][square_index as usize];
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

fn retrieve_principal_variation(transposition_table: &TranspositionTable, position: Position, repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> (Vec<(Position, ChessMove)>, GameStatus) {
    let mut pv = Vec::new();
    let mut current_position = position;

    while let Some(entry) = transposition_table.retrieve(current_position.hash_code()) {
        let next_pos = current_position.make_move(&entry.best_move).unwrap();
        pv.push(next_pos);
        // if entry.depth == 0 {
        //     break;
        // }
        current_position = next_pos.0;
    }
    info!("PV: {:?}", pv.len());
    (pv, get_game_status(&current_position, repeat_position_counts))
}

fn get_game_status(position: &Position, repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> GameStatus {
    let game = Game::new(&position, repeat_position_counts.as_ref());
    game.get_game_status()
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
    use super::*;
    use crate::chess_move::{BaseMove, RawChessMove};
    use crate::game::GameStatus::{DrawnByFiftyMoveRule};
    use crate::move_formatter::{format_move_list, FormatMove};
    use crate::position::NEW_GAME_FEN;
    use crate::evaluation::search::{iterative_deepening_search, MAXIMUM_SCORE};
    use crate::{move_formatter, uci};
    use crate::chess_move::ChessMove::Basic;

    fn test_eq(search_results: &SearchResults, expected: &SearchResults) {
        assert_eq!(search_results.score, expected.score);
        assert_eq!(search_results.depth, expected.depth);
        assert_eq!(search_results.game_status, expected.game_status);
    }

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
        assert_eq!(score_pieces(&all_black_no_white), 3780);

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
        let search_results = iterative_deepening_search(&position, &SearchParams { allocated_time_millis: usize::MAX, max_depth: 1, max_nodes: usize::MAX }, Arc::new(AtomicBool::new(false)), None);
        assert_eq!(search_results.score, -980);
        let best_line = move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.best_line).unwrap().join(", ");
        assert_eq!(best_line, "♞c4xe5");
    }

    #[test]
    fn test_retrieve_principal_variation() {
        let transposition_table = TranspositionTable::new(1 << 10);
        let position_1 = Position::new_game();
        let move_1 = Basic { base_move: BaseMove {from: sq!("e2"), to: sq!("e4"), capture: false} };
        transposition_table.store(position_1.hash_code(), move_1, 1, 0, BoundType::Exact, InProgress);
        let position_2 = position_1.make_move(&move_1).unwrap().0;
        let move_2 = Basic { base_move: BaseMove {from: sq!("e7"), to: sq!("e5"), capture: false} };
        transposition_table.store(position_2.hash_code(), move_2, 2, 0, BoundType::Exact, InProgress);
        let position_3 = position_2.make_move(&move_2).unwrap().0;
        let result = retrieve_principal_variation(&transposition_table, position_1, None);
        assert_eq!(result.0.len(), 2);
        assert_eq!(result.0[0], (position_2, move_1));
        assert_eq!(result.0[1], (position_3, move_2));
        assert_eq!(result.1, InProgress);
    }

    #[test]
    fn test_already_checkmated() {
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(
            search_results,
            SearchResults {
                score: -100_000,
                depth: 0,
                best_line: vec![],
                game_status: GameStatus::Checkmate,
            }
        );
    }

    #[test]
    fn test_already_stalemated() {
        let fen = "8/6n1/5k1K/6n1/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(
            search_results,
            SearchResults {
                score: 0,
                depth: 0,
                best_line: vec![],
                game_status: Stalemate,
            }
        );
    }

    #[test]
    fn test_mate_in_one() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_one_using_high_depth() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_two() {
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f5-g6+,h7xg6,♗c2xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 3,
                depth: 3,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_three() {
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♖f6-a6+,f7-f6,♗e5xf6+,♜g8-g7,♖a6xa8#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_four() {
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(7), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(long_format_moves(&position, &search_results), "♕g3-g6+,f7xg6,♗d5-g8+,♚h7-h8,♗g8-f7+,♚h8-h7,f5xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 7,
                depth: 7,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_three_fischer() {
        let fen = "8/8/8/8/4k3/8/8/2BQKB2 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♗f1-c4,♚e4-e5,♕d1-d5+,♚e5-f6,♕d5-g5#");
        test_eq(
            &search_results,
            &SearchResults {
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                best_line: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_hiarcs_game_engine_would_not_get_out_of_check() {
        let fen = "N7/pp6/8/1k6/2QR4/8/PPP4P/R1B1K3 b Q - 2 32";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(2), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }

    #[test]
    fn test_50_move_rule_is_recognised() {
        let fen = "4k3/8/R7/7n/7r/8/8/4K3 b - - 99 76";
        let in_progress_position: Position = Position::from(fen);
        let in_progress_search_results = iterative_deepening_search(&in_progress_position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(in_progress_search_results.best_line_moves_as_string(), "".to_string());
        test_eq(
            &in_progress_search_results,
            &SearchResults {
                score: 260,
                depth: 0,
                best_line: vec![],
                game_status: InProgress,
            }
        );

        let drawn_position = in_progress_position.make_raw_move(&RawChessMove::new(sq!("h5"), sq!("f4"), None)).unwrap().0;
        let drawn_position_search_results = iterative_deepening_search(&drawn_position, &SearchParams::new_by_depth(0), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(drawn_position_search_results.best_line_moves_as_string(), "".to_string());
        test_eq(
            &drawn_position_search_results,
            &SearchResults {
                score: 0,
                depth: 0,
                best_line: vec![],
                game_status: DrawnByFiftyMoveRule,
            }
        );
    }

    #[test]
    fn test_losing_side_plays_for_draw() {
        let go_for_draw_uci_position_str = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_for_win_uci_position_str = "position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        fn test_draw(uci_position_str: &str) -> SearchResults {
            let uci_position = uci::parse_position(uci_position_str).unwrap();
            let uci_go_options = uci::parse_uci_go_options(Some("depth 1".to_string()));
            let search_params = uci::create_search_params(&uci_go_options, &uci_position);
            let repeat_position_counts = Some(util::create_repeat_position_counts(uci_position.all_game_positions()));
            iterative_deepening_search(&uci_position.given_position, &search_params, Arc::new(AtomicBool::new(false)), repeat_position_counts)
        }
        
        let drawn_search_results = test_draw(go_for_draw_uci_position_str);
        assert_eq!(drawn_search_results.best_line_moves_as_string(), "f6-g8");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                score: 0,
                depth: 1,
                best_line: vec![],
                game_status: DrawnByThreefoldRepetition,
            }
        );
        
        let win_search_results = test_draw(go_for_win_uci_position_str);
        assert_eq!(win_search_results.best_line_moves_as_string(), "b8-c6".to_string());
        test_eq(
            &win_search_results,
            &SearchResults {
                score: 1000,
                depth: 1,
                best_line: vec![],
                game_status: InProgress,
            }
        );
    }
    #[test]
    fn test_li_chess_game() {
        // https://lichess.org/RZTYaEbP#87
        let uci_position_str = "position fen 4kb1Q/p4p2/2pp4/5Q2/P4PK1/4P3/3q4/4n3 b - - 10 40 moves d2g2 g4h5 g2h2 h5g4 h2g2 g4h5 g2h2 h5g4 h2h8";
        fn test_draw(uci_position_str: &str) -> SearchResults {
            let uci_position = uci::parse_position(uci_position_str).unwrap();
            let uci_go_options = uci::parse_uci_go_options(Some("depth 2".to_string()));
            let search_params = uci::create_search_params(&uci_go_options, &uci_position);
            let repeat_position_counts = Some(util::create_repeat_position_counts(uci_position.all_game_positions()));
            iterative_deepening_search(&uci_position.given_position, &search_params, Arc::new(AtomicBool::new(false)), repeat_position_counts)
        }
        let drawn_search_results = test_draw(uci_position_str);
        assert_eq!(drawn_search_results.best_line_moves_as_string(), "f5-c8,e8-e7");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                score: -660,
                depth: 2,
                best_line: vec![],
                game_status: InProgress,
            }
        );
    }

    // #[test]
    // fn test_perpetual_check() {
    //     let fen = "r1b5/ppp2Bpk/3p2Np/4p3/4P2q/3P1n1P/PPP2bP1/R1B4K w - - 0 1";
    //     let position: Position = Position::from(fen);
    //     let search_results = iterative_deepening_search(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
    //     assert_eq!(search_results.best_line_moves_as_string(), "g6-f8,h7-h8,f8-g6,h8-h7,g6-f8".to_string());
    //     test_eq(
    //         &search_results,
    //         &SearchResults {
    //             score: 0,
    //             depth: 4,
    //             best_line: vec![],
    //             game_status: DrawnByThreefoldRepetition,
    //         }
    //     );
    // }
}