use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::core::{move_gen, r#move};
use crate::engine::uci;
use crate::eval::evaluation;
use crate::eval::evaluation::GameStatus;
use crate::search::move_ordering::MoveOrderer;
use crate::search::transposition_table::{BoundType, TranspositionTable};
use crate::search::{move_ordering, quiescence};
use crate::utils::move_formatter;
use crate::utils::move_formatter::FormatMove;
use crate::utils::node_counter::{NodeCountStats, NodeCounter};
use crate::utils::{fen, util};
use arrayvec::ArrayVec;
use itertools::Itertools;
use log::{debug, error, info};
use std::collections::HashSet;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, RwLock};

include!("../utils/generated_macro.rs");

static NODE_COUNTER: LazyLock<RwLock<NodeCounter>> = LazyLock::new(|| {
    let node_counter = NodeCounter::new();
    RwLock::new(node_counter)
});

pub const MAXIMUM_SEARCH_DEPTH: usize = 63;

pub const MAXIMUM_SCORE: i32 = 100000;

pub const DRAW_SCORE: i32 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResults {
    pub position: Position,
    pub score: i32,
    pub depth: u8,
    pub pv: Vec<Move>,
    pub game_status: GameStatus,
}

impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "score: {} depth: {} bestline: {} game_status: {:?}",
            self.score,
            self.depth,
            move_formatter::LONG_FORMATTER.format_move_list(&self.position, &self.pv).unwrap().join(", "),
            self.game_status
        )
    }
}

#[derive(Clone, Debug)]
pub struct SearchParams {
    pub allocated_time_millis: usize,
    pub max_depth: u8,
    pub max_nodes: usize,
}

impl Display for SearchParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "allocated_time_millis: {} max_depth: {} max_nodes: {}",
            self.allocated_time_millis, self.max_depth, self.max_nodes,
        )
    }
}
impl SearchParams {
    pub const DEFAULT_NUMBER_OF_MOVES_TO_GO: usize = 30;

    pub fn new(allocated_time_millis: usize, max_depth: isize, max_nodes: usize) -> SearchParams {
        SearchParams { allocated_time_millis, max_depth: max_depth.try_into().unwrap(), max_nodes }
    }

    pub fn new_by_depth(max_depth: isize) -> SearchParams {
        SearchParams::new(usize::MAX, max_depth, usize::MAX)
    }
}
pub struct SearchContext<'a> {
    pub transposition_table: &'a mut TranspositionTable,
    pub search_params: &'a SearchParams,
    pub stop_flag: Arc<AtomicBool>,
    pub repetition_key_stack: Vec<RepetitionKey>,
    pub move_orderer: MoveOrderer,
    pub max_depth: u8,
}

impl<'a> SearchContext<'a> {
    pub fn new(
        transposition_table: &'a mut TranspositionTable,
        search_params: &'a SearchParams,
        stop_flag: Arc<AtomicBool>,
        repetition_keys: Vec<RepetitionKey>,
        move_orderer: MoveOrderer,
        max_depth: u8,
    ) -> Self {
        Self {
            transposition_table,
            search_params,
            stop_flag,
            repetition_key_stack: repetition_keys,
            move_orderer,
            max_depth,
        }
    }

    fn stop_search_requested(&self) -> bool {
        self.stop_flag.load(Ordering::Relaxed)
    }

    fn request_stop_search(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl SearchResults {
    #[allow(dead_code)]
    fn pv_moves_as_string(&self) -> String {
        self.pv.iter().join(",")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepetitionKey {
    pub zobrist_hash: u64,
    pub half_move_clock: usize,
}

impl RepetitionKey {
    pub fn new(position: &Position) -> Self {
        Self { zobrist_hash: position.hash_code(), half_move_clock: position.half_move_clock() }
    }
}

pub fn increment_node_counter() -> NodeCountStats {
    let node_counter = NODE_COUNTER.read().unwrap();
    node_counter.increment();
    node_counter_stats()
}

pub fn iterative_deepening(
    position: &mut Position,
    search_params: &SearchParams,
    stop_flag: Arc<AtomicBool>,
    repetition_keys: &[RepetitionKey],
) -> SearchResults {
    reset_node_counter();
    let mut transposition_table = TranspositionTable::new_using_config();
    let mut search_results: Option<SearchResults> = None;
    for iteration_max_depth in 1..=search_params.max_depth {
        let mut search_context = SearchContext::new(
            &mut transposition_table,
            search_params,
            stop_flag.clone(),
            Vec::from(repetition_keys),
            MoveOrderer::new(),
            iteration_max_depth,
        );
        let mut pv: ArrayVec<Move, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
        let score = negamax(
            position,
            &mut ArrayVec::new(),
            &mut pv,
            iteration_max_depth,
            &mut search_context,
            -MAXIMUM_SCORE,
            MAXIMUM_SCORE,
        );
        if !search_context.stop_search_requested() {
            let iteration_search_results =
                create_search_results(position, score, iteration_max_depth, &pv, &search_context);
            search_results = Some(iteration_search_results.clone());
            debug!("Search results for depth {iteration_max_depth}: {iteration_search_results}");
            uci::send_to_gui(format_uci_info(position, &iteration_search_results, &node_counter_stats()).as_str());
            if is_terminal_score(score) {
                info!(
                    "Found terminal position with score {} and game status {:?} at depth {} - stopping search",
                    iteration_search_results.score, iteration_search_results.game_status, iteration_max_depth
                );
                break;
            }
        } else if iteration_max_depth > 1 {
            break;
        }
    }
    search_results.unwrap()
}

fn negamax(
    position: &mut Position,
    current_line: &mut ArrayVec<Move, MAXIMUM_SEARCH_DEPTH>,
    pv: &mut ArrayVec<Move, MAXIMUM_SEARCH_DEPTH>,
    depth: u8,
    search_context: &mut SearchContext,
    mut alpha: i32,
    mut beta: i32,
) -> i32 {
    increment_node_counter();
    let ply = search_context.max_depth - depth;
    let alpha_original = alpha;
    let beta_original = beta;
    if used_allocated_move_time(search_context.search_params) {
        search_context.request_stop_search();
        return 0;
    }
    if evaluation::is_drawn_by_agreement(position, &search_context.repetition_key_stack) {
        insert_into_t_table(search_context, position, depth, alpha_original, beta_original, DRAW_SCORE, None);
        return DRAW_SCORE;
    }
    let t_table_entry = search_context.transposition_table.probe(position.hash_code());
    if let Some(entry) = t_table_entry {
        if entry.depth >= depth {
            match entry.bound_type {
                BoundType::Exact => {
                    return entry.score;
                }
                BoundType::LowerBound => {
                    alpha = alpha.max(entry.score);
                }
                BoundType::UpperBound => {
                    beta = beta.min(entry.score);
                }
            }
            if alpha >= beta {
                return entry.score;
            }
        }
    }
    if depth == 0 {
        let mut eval = evaluation::evaluate(position, ply, search_context.repetition_key_stack.as_ref());
        if !is_terminal_score(eval) {
            eval = quiescence::quiescence_search(position, ply + 1, search_context, alpha, beta);
        }
        insert_into_t_table(search_context, position, 0, alpha_original, beta_original, eval, None);
        eval
    } else {
        let mut moves = move_gen::generate_moves(position);
        let hash_move = t_table_entry.and_then(|entry| entry.best_move);
        let last_move = &current_line.last().cloned();
        move_ordering::order_moves(position, &mut moves, &search_context.move_orderer, ply, hash_move, last_move);
        let mut best_score = -MAXIMUM_SCORE;
        let mut best_move = None;
        for mv in moves {
            if let Some(undo_move_info) = position.make_move(&mv) {
                let mut child_pv: ArrayVec<Move, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
                current_line.push(mv);
                search_context.repetition_key_stack.push(RepetitionKey::new(position));
                let next_score =
                    -negamax(position, current_line, &mut child_pv, depth - 1, search_context, -beta, -alpha);
                search_context.repetition_key_stack.pop();
                current_line.pop();
                position.unmake_move(&undo_move_info);
                if next_score > best_score || best_move.is_none() {
                    best_score = next_score;
                    best_move = Some(mv);
                    pv.clear();
                    pv.push(mv);
                    pv.extend(child_pv);
                }
                alpha = alpha.max(next_score);
                if alpha >= beta {
                    search_context.move_orderer.add_killer_move(mv, ply);
                    break; // beta cutoff
                }
                if depth >= 2 && search_context.stop_search_requested() {
                    break;
                }
            }
        }
        if best_move.is_none() {
            best_score = evaluation::evaluate(position, ply, search_context.repetition_key_stack.as_ref());
        }
        insert_into_t_table(search_context, position, depth, alpha_original, beta_original, best_score, best_move);
        best_score
    }
}

fn insert_into_t_table(
    search_context: &mut SearchContext,
    position: &Position,
    depth: u8,
    alpha: i32,
    beta: i32,
    score: i32,
    mov: Option<Move>,
) {
    if !search_context.stop_search_requested() {
        search_context.transposition_table.insert(position, depth, alpha, beta, score, mov);
    }
}

fn create_search_results(
    position: &Position,
    score: i32,
    max_depth: u8,
    pv: &[Move],
    search_context: &SearchContext,
) -> SearchResults {
    let get_game_status = |last_position, repetition_keys: &Vec<RepetitionKey>| -> GameStatus {
        let game_status = evaluation::get_game_status(last_position, repetition_keys);
        match game_status {
            GameStatus::InProgress if score == 0 => GameStatus::Draw,
            _ => game_status,
        }
    };

    let pv_with_positions: Vec<(Position, Move)> = util::replay_moves(position, pv).unwrap();
    let final_pv: Vec<(Position, Move)> = if pv.len() < max_depth as usize {
        extend_principal_variation(search_context.transposition_table, position, &pv_with_positions, max_depth)
    } else {
        pv_with_positions
    };
    let last_position = final_pv.last().map_or(position, |(p, _)| p);
    let pv_repetition_keys: Vec<RepetitionKey> = final_pv.iter().map(|(p, _)| RepetitionKey::new(p)).collect();
    let repetition_keys = [search_context.repetition_key_stack.clone(), pv_repetition_keys].concat();
    let game_status = get_game_status(last_position, &repetition_keys);
    let (_, moves): (Vec<Position>, Vec<Move>) = final_pv.into_iter().unzip();
    SearchResults { position: *position, score, depth: max_depth, pv: moves, game_status }
}

fn extend_principal_variation(
    transposition_table: &TranspositionTable,
    position: &Position,
    current_pv: &[(Position, Move)],
    max_depth: u8,
) -> Vec<(Position, Move)> {
    let mut result_pv = current_pv.to_owned();
    let last_position = current_pv.last().map_or(position, |(p, _)| p);
    let mut current_position = *last_position;

    let mut visited_positions = HashSet::new();
    let mut num_missing_moves = max_depth - current_pv.len() as u8;

    while let Some(entry) = transposition_table.probe(current_position.hash_code()) {
        if num_missing_moves == 0 || entry.depth < num_missing_moves || entry.bound_type != BoundType::Exact {
            break;
        }

        if let Some(best_mv) = entry.best_move {
            if current_position.make_move(&best_mv).is_some() {
                if visited_positions.contains(&current_position.hash_code()) {
                    break;
                }
                result_pv.push((*last_position, best_mv));
                num_missing_moves -= 1;
                visited_positions.insert(current_position.hash_code());
            } else {
                break;
            }
        } else {
            break;
        }
    }
    debug!("PV extended from length {} to length {}", current_pv.len(), result_pv.len());
    result_pv
}

fn format_uci_info(position: &Position, search_results: &SearchResults, node_counter_stats: &NodeCountStats) -> String {
    let moves_string = search_results
        .pv
        .iter()
        .map(|pos| r#move::convert_move_to_raw(pos).to_string())
        .collect::<Vec<String>>()
        .join(" ");

    let moves = util::replay_move_string(position, moves_string.clone());
    if moves.is_none() {
        error!(
            "Invalid moves for position [{}] being sent to host as UCI info: [{}]",
            fen::write(position),
            moves_string
        );
    }

    format!(
        "info depth {} score cp {} time {} nodes {} nps {} pv {}",
        search_results.depth,
        search_results.score,
        node_counter_stats.elapsed_time.as_millis(),
        node_counter_stats.node_count,
        node_counter_stats.nodes_per_second,
        moves_string
    )
}

fn is_mating_score(score: i32) -> bool {
    score.abs() >= MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as i32
}

fn is_drawing_score(score: i32) -> bool {
    score == 0
}

fn is_terminal_score(score: i32) -> bool {
    is_mating_score(score) || is_drawing_score(score)
}

fn used_allocated_move_time(search_params: &SearchParams) -> bool {
    let stats = node_counter_stats();
    stats.elapsed_time.as_millis() > search_params.allocated_time_millis.try_into().unwrap()
}

fn reset_node_counter() {
    NODE_COUNTER.write().unwrap().reset();
}

fn node_counter_stats() -> NodeCountStats {
    NODE_COUNTER.read().unwrap().stats()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config;

    fn setup() {
        config::tests::initialize_test_config();
    }

    fn test_eq(search_results: &SearchResults, expected: &SearchResults) {
        assert_eq!(search_results.score, expected.score);
        assert_eq!(search_results.depth, expected.depth);
        assert_eq!(search_results.game_status, expected.game_status);
    }

    fn long_format_moves(position: &Position, search_results: &SearchResults) -> String {
        move_formatter::LONG_FORMATTER.format_move_list(position, &search_results.pv).unwrap().join(",")
    }

    #[test]
    fn test_piece_captured() {
        setup();
        let fen = "4k3/8/1P1Q4/R7/2n5/4N3/1B6/4K3 b - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams { allocated_time_millis: usize::MAX, max_depth: 1, max_nodes: usize::MAX },
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(search_results.score, -1020);
        let pv = move_formatter::LONG_FORMATTER.format_move_list(&mut position, &search_results.pv).unwrap().join(", ");
        assert_eq!(pv, "♞c4xd6");
    }

    #[test]
    fn test_already_checkmated() {
        setup();
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(
            search_results,
            SearchResults { position, score: -100_000, depth: 1, pv: vec![], game_status: GameStatus::Checkmate }
        );
    }

    #[test]
    fn test_already_stalemated() {
        setup();
        let fen = "8/6n1/5k1K/6n1/8/8/8/8 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(
            search_results,
            SearchResults { position, score: 0, depth: 1, pv: vec![], game_status: GameStatus::Stalemate }
        );
    }

    #[test]
    fn test_mate_in_one() {
        setup();
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    #[test]
    fn test_mate_in_one_using_high_depth() {
        setup();
        let fen = "r1bqkbnr/p2p1ppp/1pn5/2p1p3/2B1P3/2N2Q2/PPPP1PPP/R1B1K1NR w KQkq - 2 5";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(3),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    #[test]
    fn test_mate_in_two() {
        setup();
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(3),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♕f5-g6+,h7xg6,♗c2xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 3,
                depth: 3,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    #[test]
    fn test_mate_in_three() {
        setup();
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(5),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(
            move_formatter::format_move_list(&position, &search_results),
            "♖f6-a6+,f7-f6,♗e5xf6+,♜g8-g7,♖a6xa8#"
        );
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    /// Test case for verifying if the search algorithm can correctly identify and handle a checkmate scenario in four moves.
    ///
    /// This test is based on a FEN string representing a chess position from which a "mate-in-four" can occur.
    /// The function runs an iterative deepening search algorithm with a specified depth and validates the resulting move sequence,
    /// search results, and the correctness of the algorithm's evaluation against the expected values.
    ///
    /// # Steps Performed:
    /// - The given FEN string (`fen`) is used to initialize a `Position` object.
    /// - The `iterative_deepening` function is invoked to search the position up to a depth of 7, simulating a scenario
    ///   where the algorithm should identify a "mate-in-four".
    /// - The sequence of moves leading to the checkmate is compared with the expected output using `assert_eq!`.
    /// - The full search results, including position, score, depth, principal variation (`pv`), and game status
    ///   (`Checkmate`), are tested for correctness using `test_eq`.
    ///
    /// # Test Parameters:
    /// - FEN string: `"4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1"`
    ///     - This represents a position where white has a clear forced checkmate sequence in four moves.
    ///     - "w" indicates that white is to move.
    /// - Search Depth: `7`
    ///     - The test checks the algorithm's capabilities to compute up to seven ply to retain accuracy.
    ///
    /// # Validations:
    /// - The move sequence (`long_format_moves`) leading to the checkmate is expected to be:
    ///     `"♕g3-g6+,f7xg6,♗d5-g8+,♚h7-h8,♗g8-f7+,♚h8-h7,♗f7xg6#"`
    /// - The search results, encapsulated in the `SearchResults` structure, are expected to match:
    ///     - Position: Original position object derived from FEN.
    ///     - Score: `MAXIMUM_SCORE - 7`
    ///         - Indicates the estimated score for the winning move, adjusted for the number of moves remaining.
    ///     - Depth: `7`
    ///         - Confirms the depth to which the position was searched in the test.
    ///     - Principal Variation (`pv`): Expected to be empty (`vec![]`) in this case.
    ///     - Game Status: `Checkmate`
    ///
    /// # Importance:
    /// This test ensures that the chess engine's search algorithm can correctly evaluate and play out forced checkmate sequences,
    /// a critical functionality for any chess evaluation engine. It also confirms robustness across deeper search depths and complex scenarios.
    #[test]
    fn test_mate_in_four() {
        setup();
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(7),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(long_format_moves(&position, &search_results), "♕g3-g6+,f7xg6,♗d5-g8+,♚h7-h8,♗g8-f7+,♚h8-h7,f5xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 7,
                depth: 7,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    #[test]
    fn test_mate_in_three_fischer() {
        setup();
        let fen = "8/8/8/8/4k3/8/8/2BQKB2 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(5),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(
            move_formatter::format_move_list(&position, &search_results),
            "♗f1-c4,♚e4-e5,♕d1-d5+,♚e5-f6,♕d5-g5#"
        );
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            },
        );
    }

    #[test]
    fn test_hiarcs_game_engine_would_not_get_out_of_check() {
        setup();
        let fen = "N7/pp6/8/1k6/2QR4/8/PPP4P/R1B1K3 b Q - 2 32";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(2),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }

    #[test]
    fn test_hiarcs_blunder() {
        setup();
        let fen = "r3k2r/4n1pp/pqpQ1p2/8/1P2b1P1/2P2N1P/P4P2/R1B2RK1 w kq - 0 17";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(5),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert!(search_results.pv_moves_as_string().starts_with("f1-e1"));
    }

    #[test]
    fn test_rooks_on_seventh_rank_preferred() {
        setup();
        let fen = "2q2rk1/B3ppbp/6p1/1Q2P3/8/PP2PN2/6r1/3RK2R b K - 0 19";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♛c8-c2");
    }

    #[test]
    fn test_50_move_rule_is_recognised() {
        setup();
        let fen = "4k3/8/R7/7n/7r/8/8/4K3 b - - 98 76";
        let original_position: Position = Position::from(fen);

        let mut in_progress_position: Position = original_position.clone();
        let in_progress_search_results = iterative_deepening(
            &mut in_progress_position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(in_progress_search_results.pv_moves_as_string(), "h5-f4".to_string());
        test_eq(
            &in_progress_search_results,
            &SearchResults {
                position: in_progress_position,
                score: 316,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::InProgress,
            },
        );

        let mut drawn_position: Position = original_position.clone();
        drawn_position.make_raw_move(&r#move::RawMove::new(sq!("h5"), sq!("f4"), None)).unwrap();
        let drawn_position_search_results = iterative_deepening(
            &mut drawn_position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(drawn_position_search_results.pv_moves_as_string(), "e1-d1".to_string());
        test_eq(
            &drawn_position_search_results,
            &SearchResults {
                position: drawn_position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::DrawnByFiftyMoveRule,
            },
        );
    }

    #[test]
    fn test_losing_side_plays_for_draw() {
        setup();
        let go_for_draw_uci_position_str = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_for_win_uci_position_str = "position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_options_str = "depth 1";
        let drawn_search_results = uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
        assert_eq!(drawn_search_results.pv_moves_as_string(), "f6-g8");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                position: drawn_search_results.position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::DrawnByThreefoldRepetition,
            },
        );

        let win_search_results = uci::run_uci_position(go_for_win_uci_position_str, go_options_str);
        assert_eq!(win_search_results.pv_moves_as_string(), "e7-e6".to_string());
        test_eq(
            &win_search_results,
            &SearchResults {
                position: win_search_results.position,
                score: 976,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::InProgress,
            },
        );
    }

    // https://lichess.org/YKQcIIfi/black#97
    #[test]
    fn test_li_chess_draws_problem() {
        // setup();
        // let fen = "6k1/5p1p/1Q4p1/q1P1P3/3P4/4Pb2/2K5/8 b - - 0 45";
        // let uci_initial_position_str = format!("position fen {}", fen);
        //
        // let go_options_str = "depth 5";
        // let search_results_1 = uci::run_uci_position(&uci_initial_position_str, go_options_str);
        // let _pv_moves_1 = search_results_1.pv_moves_as_string();
        // assert_eq!(search_results_1.pv_moves_as_string(), "f3-e4,c2-d1,a5-c3,d1-e2,c3-c2");
        //
        // let search_results_2 =
        //     uci::run_uci_position(&format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3"), go_options_str);
        // let _pv_moves_2 = search_results_2.pv_moves_as_string();
        // assert_eq!(search_results_2.pv_moves_as_string(), "e4-d5,b3-c2,d5-e4,c2-d1,a5-c3");
        //
        // let search_results_3 = uci::run_uci_position(
        //     &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2"),
        //     go_options_str,
        // );
        // let pv_moves_3 = search_results_3.pv_moves_as_string();
        // assert_eq!(pv_moves_3, "d5-e4,c2-d1,a5-c3,d1-e2,c3-c2");
        //
        // let search_results_4 = uci::run_uci_position(
        //     &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2 d5e4 c2b3"),
        //     go_options_str,
        // );
        // let pv_moves_4 = search_results_4.pv_moves_as_string();
        // assert_eq!(pv_moves_4, "e4-d5,b3-c2,d5-e4,c2-d1,a5-c3");
        //
        // //TRANSPOSITION_TABLE.clear();
        // let search_results_5 = uci::run_uci_position(
        //     &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2 d5e4 c2b3 e4d5 b3c2"),
        //     go_options_str,
        // );
        // let pv_moves_5 = search_results_5.pv_moves_as_string();
        // assert_eq!(pv_moves_5, "d5-e4,c2-d1,a5-c3,d1-e2,c3-c2");
        //
        //
        //
        //
        // let search_results_3 = uci::run_uci_position(&format!("{} {}", uci_initial_position_str, "f3e4"), go_options_str);
        // let search_results_4 = uci::run_uci_position(&format!("{} {}", uci_initial_position_str, "f3e4"), go_options_str);
        // let search_results_5 = uci::run_uci_position(&format!("{} {}", uci_initial_position_str, "f3e4"), go_options_str);

        // assert_eq!(drawn_search_results.pv_moves_as_string(), "f6-g8");
        // test_eq(
        //     &drawn_search_results,
        //     &SearchResults {
        //         position: drawn_search_results.position,
        //         score: 0,
        //         depth: 1,
        //         pv: vec![],
        //         game_status: GameStatus::Draw,
        //     }
        // );
        //
        // TRANSPOSITION_TABLE.clear();
    }

    // #[test]
    // fn test_black_avoids_draw_using_contempt() {
    //     setup();
    //     let go_for_draw_uci_position_str = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
    //     let go_options_str = "depth 1";
    //     let drawn_search_results = uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
    //     assert_eq!(drawn_search_results.pv_moves_as_string(), "f6-g8");
    //     test_eq(
    //         &drawn_search_results,
    //         &SearchResults {
    //             position: drawn_search_results.position,
    //             score: 0,
    //             depth: 1,
    //             pv: vec![],
    //             game_status: GameStatus::DrawnByThreefoldRepetition,
    //         },
    //     );
    //
    //     TRANSPOSITION_TABLE.clear();
    //
    //     config::set_contempt(1000);
    //     let drawn_search_results = uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
    //     assert_eq!(drawn_search_results.pv_moves_as_string(), "e7-e6");
    //     test_eq(
    //         &drawn_search_results,
    //         &SearchResults {
    //             position: drawn_search_results.position,
    //             score: -826,
    //             depth: 1,
    //             pv: vec![],
    //             game_status: GameStatus::InProgress,
    //         },
    //     );
    //     config::set_contempt(0);
    // }
    #[test]
    fn test_li_chess_game() {
        setup();
        // https://lichess.org/RZTYaEbP#87
        let uci_position_str = "position fen 4kb1Q/p4p2/2pp4/5Q2/P4PK1/4P3/3q4/4n3 b - - 10 40 moves d2g2 g4h5 g2h2 h5g4 h2g2 g4h5 g2h2 h5g4 h2h8";
        let drawn_search_results = uci::run_uci_position(uci_position_str, "depth 2");
        assert_eq!(drawn_search_results.pv_moves_as_string(), "f5-c8,e8-e7");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                position: drawn_search_results.position,
                score: -581,
                depth: 2,
                pv: vec![],
                game_status: GameStatus::InProgress,
            },
        );
    }

    #[test]
    fn test_perpetual_check() {
        setup();
        let go_for_draw_uci_position_str =
            "position fen r1b5/ppp2Bpk/3p2Np/4p3/4P2q/3P1n1P/PPP2bP1/R1B4K w - - 0 1 moves g6f8 h7h8 f8g6 h8h7";
        let search_results = uci::run_uci_position(go_for_draw_uci_position_str, "depth 4");
        assert_eq!(search_results.pv_moves_as_string(), "g6-f8,h7-h8,f8-g6,h8-h7".to_string());
        test_eq(
            &search_results,
            &SearchResults {
                position: search_results.position,
                score: 0,
                depth: 4,
                pv: vec![],
                game_status: GameStatus::DrawnByThreefoldRepetition,
            },
        );
    }

    #[test]
    fn test_is_mating_score() {
        setup();
        let score = MAXIMUM_SCORE;
        assert!(is_mating_score(score));

        let score = -MAXIMUM_SCORE;
        assert!(is_mating_score(score));

        let score = MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as i32;
        assert!(is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as i32);
        assert!(is_mating_score(score));

        let score = (MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as i32) - 1;
        assert!(!is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as i32) + 1;
        assert!(!is_mating_score(score));
    }

    #[test]
    fn test_is_drawing_score() {
        setup();
        let score = -1;
        assert!(!is_drawing_score(score));

        let score = 1;
        assert!(!is_drawing_score(score));

        let score = 0;
        assert!(is_drawing_score(score));
    }

    #[test]
    fn test_quiescence_search() {
        setup();
        let fen = "3k4/5pq1/5ppP/5b2/4R3/8/4K3/8 b - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results = iterative_deepening(
            &mut position,
            &SearchParams::new_by_depth(1),
            Arc::new(AtomicBool::new(false)),
            &vec![],
        );
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♛g7xh6");
    }
}

// #[test]
// fn test_draw_avoidance_1() {
//     // position after moves - 8/6Q1/8/3q2KP/8/6P1/8/1k6 w - - 13 115
//     let uci_str = "position startpos moves e2e4 e7e5 g1f3 b8c6 f1b5 a7a6 b5c6 d7c6 e1g1 c8g4 h2h3 g4f3 d1f3 g8f6 d2d3 f8d6 b1d2 e8g8 d2c4 f8e8 c1g5 d6c5 a1e1 h7h6 g5h4 b7b5 c4e3 c5e3 f2e3 e8e6 f3g3 g7g5 f1f5 g8h8 h4g5 h6g5 f5g5 d8e7 e1f1 f6g8 f1f5 g8h6 f5e5 a8c8 e5e6 e7e6 g1h2 h8h7 b2b3 e6f6 e4e5 f6e6 g3h4 a6a5 h4e4 h7h8 e4h4 c8e8 d3d4 h8h7 h4e4 h7h8 e4h4 h8h7 a2a4 b5a4 b3a4 e8b8 h4e4 h7h8 e4h4 h8h7 h4e4 h7h8 e4h4 b8g8 g5g8 h8g8 h4d8 g8g7 d8c7 h6f5 c7a5 f5e3 a5c3 e6h6 h2g1 h6f4 c3c6 f4f1 g1h2 f1f4 h2g1 f4f1 g1h2 e3c2 c6c2 f1f4 h2h1 f4d4 c2c7 d4h4 a4a5 h4e1 h1h2 e1e3 c7e7 e3f4 h2g1 f4e3 g1h2 e3f4 h2g1 f4c1 g1f2 c1f4 f2e2 f4e4 e2f2 e4f4 f2e2 f4e4 e2f1 e4f4 f1e1 f4g3 e1d1 g3d3 d1e1 d3c3 e1f2 c3a5 e7f6 g7f8 f6h8 f8e7 h8f6 e7e8 f2g1 a5e1 g1h2 e1e4 f6h8 e8d7 h8f6 d7e8 f6g5 e8d7 h2g1 e4g6 g5d2 d7c6 d2d4 g6e6 g1h2 c6b5 d4c3 b5a4 c3d4 a4b3 d4d3 b3b4 d3d4 b4b5 d4c3 e6f5 h2g1 f5e4 c3b3 b5c5 b3a3 c5d4 a3a1 d4d3 a1f1 d3d4 f1f7 e4e5 f7f1 d4c5 f1f2 c5b5 g1h1 b5c4 g2g3 e5d5 h1h2 d5e5 f2f1 c4c5 h3h4 e5d5 f1f4 d5e6 h2g2 e6d5 g2h3 d5h1 h3g4 h1d1 f4f3 d1d4 g4h3 d4d7 f3g4 d7c6 g4g5 c5b4 g5f4 b4b3 f4f7 b3b2 f7g7 b2b1 h4h5 c6h1 h3g4 h1d1 g4h4 d1h1 h4g5 h1d5 g5g4 d5d1 g4h4 d1h1 h4g5 h1d5";
//     let search_results = uci::run_uci_position(uci_str, "depth 5");
//     assert_eq!(search_results.pv_moves_as_string(), "g5-h6,d5-d2,h6-h7,d2-d3,h7-h6".to_string());
//     assert_ne!(search_results.score, 0);
// }

// #[test]
// fn test_draw_avoidance_2() {
//     // position after moves - 8/5b2/3k4/R4Q2/3q4/1P6/5PKP/8 w - - 2 106
//     let uci_str = "position startpos moves d2d4 g8f6 c2c4 e7e6 b1c3 f8b4 e2e3 e8g8 g1e2 d7d5 c4d5 e6d5 g2g3 c7c6 f1g2 f8e8 e1g1 b8d7 c1d2 d7b6 b2b3 c8f5 e2f4 a8c8 f1e1 c6c5 c3e2 b4d2 d1d2 f6e4 d2a5 g7g5 f4d3 c5d4 e2d4 f5g6 g2e4 d5e4 d3c5 d8e7 a1c1 c8a8 e1d1 b6d5 a5b5 d5b6 a2a4 a8b8 a4a5 b6d5 c5d7 b8d8 b5d5 e7d7 d5g5 d7d6 g5g4 h7h5 g4e2 d6e7 a5a6 b7a6 c1c6 a6a5 c6a6 d8d5 e2d2 e7d7 a6a5 d5a5 d2a5 h5h4 d1a1 e8c8 a5g5 h4h3 g5h4 c8c3 g3g4 a7a5 a1f1 d7d5 h4f6 c3c5 f6h4 c5c3 f1d1 c3d3 d1e1 d5d7 e1f1 d3c3 f1a1 c3d3 a1f1 d3c3 f1b1 c3d3 b1a1 g8g7 h4g3 d7d5 g3c7 g7g8 a1e1 d5g5 c7b8 g8h7 b8g3 h7g8 g3h3 f7f5 e1a1 f5g4 h3g3 g5h5 g3b8 g8h7 d4e6 h5f5 b8c7 g6f7 e6d4 f5h5 c7e7 h5d5 a1c1 d5h5 e7e4 f7g6 c1c7 h7g8 c7c8 g8h7 c8c7 h7g8 e4a8 g6e8 c7c1 g8h7 a8e4 e8g6 e4e6 d3d2 e6d7 g6f7 c1a1 d2d3 d7c8 h5g6 c8b7 g6h5 b7c7 h5g6 a1c1 g6h5 g1f1 d3d2 c1c5 h5g6 c7e7 d2d4 e3d4 g6d3 f1e1 d3b1 e1e2 b1b2 e2f1 b2b1 f1g2 b1g6 e7h4 g6h6 h4g4 h6b6 g4h3 h7g6 h3h5 g6g7 h5g4 g7h7 g4h3 h7g6 h3h5 g6g7 h5e5 g7h7 e5f5 h7g7 f5g5 g7h7 g5h4 h7g7 h4g3 g7f8 c5c8 f8e7 c8c7 e7f8 c7c8 f8e7 g3g5 e7d7 c8c5 b6d6 c5a5 d6d4 g5f5 d7d6";
//     let search_results = uci::run_uci_position(uci_str, "depth 10");
//     assert_eq!(search_results.pv_moves_as_string(), "a5-a6,d6-e7,f5-g5,e7-d7,g5-b5,d7-e7,b5-g5,e7-d7,g5-f5,d7-e8".to_string());
//     assert_ne!(search_results.score, 0);
//
// }
