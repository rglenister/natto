use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::core::{move_gen, r#move};
use crate::engine::uci;
use crate::eval::evaluation;
use crate::eval::evaluation::{has_three_fold_repetition, GameStatus};
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
use std::sync::Arc;

include!("../utils/generated_macro.rs");

pub const MAXIMUM_SEARCH_DEPTH: usize = 63;

pub const MAXIMUM_SCORE: isize = 100000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResults {
    pub position: Position,
    pub score: isize,
    pub depth: usize,
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
            move_formatter::LONG_FORMATTER
                .format_move_list(&self.position, &self.pv)
                .unwrap()
                .join(", "),
            self.game_status
        )
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
pub struct Search<'a> {
    pub(crate) position: &'a mut Position,
    pub node_counter: NodeCounter,
    pub(crate) transposition_table: &'a TranspositionTable,
    pub(crate) search_params: SearchParams,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) repetition_key_stack: Vec<RepetitionKey>,
    move_orderer: MoveOrderer,
    max_depth: usize,
}

impl<'a> Search<'a> {
    pub fn new(
        position: &'a mut Position,
        transposition_table: &'a TranspositionTable,
        search_params: SearchParams,
        stop_flag: Arc<AtomicBool>,
        repetition_keys: Vec<RepetitionKey>,
        move_orderer: MoveOrderer,
        max_depth: usize,
    ) -> Search<'a> {
        Self {
            position,
            transposition_table,
            search_params,
            stop_flag,
            repetition_key_stack: repetition_keys,
            node_counter: NodeCounter::new(),
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

    fn used_allocated_move_time(&self) -> bool {
        self.node_counter.stats().elapsed_time.as_millis()
            > self.search_params.allocated_time_millis as u128
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

impl Search<'_> {
    pub fn iterative_deepening(&mut self) -> SearchResults {
        let mut search_results: Option<SearchResults> = None;
        for iteration_max_depth in 1..=self.search_params.max_depth {
            self.move_orderer._clear();
            self.max_depth = iteration_max_depth;
            let mut pv: ArrayVec<Move, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
            let score = self.negamax(
                &mut ArrayVec::new(),
                &mut pv,
                iteration_max_depth,
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            if !self.stop_search_requested() {
                let iteration_search_results =
                    self.create_search_results(self.position, score, iteration_max_depth, &pv);
                search_results = Some(iteration_search_results.clone());
                debug!(
                    "Search results for depth {}: {}",
                    iteration_max_depth,
                    iteration_search_results.clone()
                );
                uci::send_to_gui(
                    Search::format_uci_info(
                        self.position,
                        &iteration_search_results,
                        &self.node_counter.stats(),
                    )
                    .as_str(),
                );
                let is_checkmate = iteration_search_results.game_status == GameStatus::Checkmate
                    || Search::is_mating_score(iteration_search_results.score);
                if is_checkmate {
                    info!("Found mate at depth {iteration_max_depth} - stopping search");
                    self.transposition_table.clear();
                    break;
                }
            } else if search_results.is_some() {
                break;
            }
        }
        search_results.unwrap()
    }

    fn negamax(
        &mut self,
        current_line: &mut ArrayVec<Move, MAXIMUM_SEARCH_DEPTH>,
        pv: &mut ArrayVec<Move, MAXIMUM_SEARCH_DEPTH>,
        depth: usize,
        mut alpha: isize,
        mut beta: isize,
    ) -> isize {
        self.node_counter.increment();
        let ply = self.max_depth - depth;
        let alpha_original = alpha;
        let beta_original = beta;

        if self.used_allocated_move_time() {
            self.request_stop_search();
            return 0;
        }
        let ttable_entry = self.transposition_table.probe(self.position.hash_code());
        if let Some(entry) = ttable_entry {
            if entry.depth >= depth {
                match entry.bound_type {
                    BoundType::Exact => {
                        if let Some(best_move) = entry.best_move {
                            if let Some(undo_move_info) = self.position.make_move(&best_move) {
                                let is_drawn = {
                                    self.position.is_drawn_by_fifty_moves_rule() || {
                                        self.repetition_key_stack
                                            .push(RepetitionKey::new(self.position));
                                        let drawn_by_threefold_repetition =
                                            has_three_fold_repetition(&self.repetition_key_stack);
                                        self.repetition_key_stack.pop();
                                        drawn_by_threefold_repetition
                                    }
                                };

                                if !is_drawn {
                                    pv.clear();
                                    pv.push(best_move);
                                    self.position.unmake_move(&undo_move_info);
                                    return entry.score;
                                }
                                self.position.unmake_move(&undo_move_info);
                            }
                        }
                    }
                    BoundType::LowerBound => {
                        if entry.score > alpha {
                            alpha = entry.score
                        }
                    }
                    BoundType::UpperBound => {
                        if entry.score < beta {
                            beta = entry.score;
                        }
                    }
                }
                if alpha >= beta {
                    return entry.score;
                }
            }
        }
        if depth > 0 {
            let mut moves = move_gen::generate_moves(self.position);
            let hash_move = ttable_entry.and_then(|entry| entry.best_move);
            let last_move = &current_line.last().cloned();
            move_ordering::order_moves(
                self.position,
                &mut moves,
                &self.move_orderer,
                ply,
                hash_move,
                last_move,
            );
            let mut best_score = -MAXIMUM_SCORE;
            let mut best_move = None;
            for mv in moves {
                if let Some(undo_move_info) = self.position.make_move(&mv) {
                    // there isn't a checkmate or a stalemate
                    let mut child_pv: ArrayVec<Move, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
                    current_line.push(mv);
                    self.repetition_key_stack.push(RepetitionKey::new(self.position));
                    let next_score = if self.position.is_drawn_by_fifty_moves_rule()
                        || has_three_fold_repetition(&self.repetition_key_stack)
                    {
                        // Apply contempt to repetition-based draws
                        evaluation::apply_contempt(0)
                    } else {
                        -self.negamax(current_line, &mut child_pv, depth - 1, -beta, -alpha)
                    };
                    current_line.pop();
                    self.repetition_key_stack.pop();
                    if next_score > best_score || best_move.is_none() {
                        best_score = next_score;
                        best_move = Some(mv);
                        pv.clear();
                        pv.push(mv);
                        pv.extend(child_pv);
                    }
                    alpha = alpha.max(next_score);
                    self.position.unmake_move(&undo_move_info);
                    if alpha >= beta {
                        self.move_orderer.add_killer_move(mv, ply);
                        break;
                    }
                    if depth >= 2 && self.stop_search_requested() {
                        break;
                    }
                }
            }
            if best_move.is_some() {
                if !self.stop_search_requested() {
                    self.transposition_table.insert(
                        self.position,
                        depth,
                        alpha_original,
                        beta_original,
                        best_score,
                        best_move,
                    );
                }
            } else {
                best_score =
                    evaluation::evaluate(self.position, ply, self.repetition_key_stack.as_ref());
                if !self.stop_search_requested() {
                    self.transposition_table.insert(
                        self.position,
                        depth,
                        alpha_original,
                        beta_original,
                        best_score,
                        None,
                    );
                }
            }
            best_score
        } else {
            let mut score =
                evaluation::evaluate(self.position, ply, self.repetition_key_stack.as_ref());
            if !Search::is_terminal_score(score) {
                score = quiescence::quiescence_search((ply + 1) as isize, self, alpha, beta);
            }
            if !self.stop_search_requested() {
                self.transposition_table.insert(
                    self.position,
                    0,
                    alpha_original,
                    beta_original,
                    score,
                    None,
                );
            }
            score
        }
    }

    fn create_search_results(
        &self,
        position: &Position,
        score: isize,
        max_depth: usize,
        pv: &[Move],
    ) -> SearchResults {
        let get_game_status = |last_position, repetition_keys: &Vec<RepetitionKey>| -> GameStatus {
            let game_status = evaluation::get_game_status(last_position, repetition_keys);
            match game_status {
                GameStatus::InProgress if score == 0 => GameStatus::Draw,
                _ => game_status,
            }
        };

        let pv_with_positions: Vec<(Position, Move)> = util::replay_moves(position, pv).unwrap();
        let final_pv: Vec<(Position, Move)> = if pv.len() < max_depth {
            Search::extend_principal_variation(
                self.transposition_table,
                position,
                &pv_with_positions,
                max_depth,
            )
        } else {
            pv_with_positions
        };
        let last_position = final_pv.last().map_or(position, |(p, _)| p);
        let pv_repetition_keys: Vec<RepetitionKey> =
            final_pv.iter().map(|(p, _)| RepetitionKey::new(p)).collect();
        let repetition_keys = [self.repetition_key_stack.clone(), pv_repetition_keys].concat();
        let game_status = get_game_status(last_position, &repetition_keys);
        let (_, moves): (Vec<Position>, Vec<Move>) = final_pv.into_iter().unzip();
        SearchResults { position: *position, score, depth: max_depth, pv: moves, game_status }
    }

    fn extend_principal_variation(
        transposition_table: &TranspositionTable,
        position: &Position,
        current_pv: &[(Position, Move)],
        max_depth: usize,
    ) -> Vec<(Position, Move)> {
        let mut result_pv = current_pv.to_owned();
        let last_position = current_pv.last().map_or(position, |(p, _)| p);
        let mut current_position = *last_position;

        let mut visited_positions = HashSet::new();
        let mut num_missing_moves = max_depth - current_pv.len();

        while let Some(entry) = transposition_table.probe(current_position.hash_code()) {
            if num_missing_moves == 0
                || entry.depth < num_missing_moves
                || entry.bound_type != BoundType::Exact
            {
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

    pub fn get_repeat_position_count(repetition_key_stack: &[RepetitionKey]) -> usize {
        let mut repetition_count = 0;
        let length = repetition_key_stack.len();
        if length >= 5 {
            let current_key = repetition_key_stack.last().unwrap();
            if current_key.half_move_clock >= 3 {
                for i in (0..=length - 5).rev().step_by(2) {
                    let key = repetition_key_stack.get(i).unwrap();
                    if key.zobrist_hash == current_key.zobrist_hash {
                        repetition_count += 1;
                    }
                    if key.half_move_clock <= 1 {
                        break;
                    }
                }
            }
        }
        // #[cfg(debug_assertions)]
        // {
        //     let last_position_instance_count: usize = repetition_key_stack.last().map_or(0,|last_key| {
        //         repetition_key_stack.iter().filter(|key| key.zobrist_hash == last_key.zobrist_hash).count() - 1
        //     });
        //     assert_eq!(repetition_count, last_position_instance_count);
        // }
        repetition_count
    }

    fn format_uci_info(
        position: &Position,
        search_results: &SearchResults,
        node_counter_stats: &NodeCountStats,
    ) -> String {
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

    fn is_mating_score(score: isize) -> bool {
        score.abs() >= MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize
    }

    fn is_drawing_score(score: isize) -> bool {
        score == 0
    }

    fn is_terminal_score(score: isize) -> bool {
        Search::is_mating_score(score) || Search::is_drawing_score(score)
    }
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
        move_formatter::LONG_FORMATTER
            .format_move_list(position, &search_results.pv)
            .unwrap()
            .join(",")
    }

    fn create_search<'a>(
        position: &'a mut Position,
        transposition_table: &'a TranspositionTable,
        depth: u8,
    ) -> Search<'a> {
        Search::new(
            position,
            transposition_table,
            SearchParams::new_by_depth(depth as isize),
            Arc::new(AtomicBool::new(false)),
            vec![],
            MoveOrderer::new(),
            0,
        )
    }

    #[test]
    fn test_piece_captured() {
        setup();
        let fen = "4k3/8/1P1Q4/R7/2n5/4N3/1B6/4K3 b - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
        assert_eq!(search_results.score, -1020);
        let pv = move_formatter::LONG_FORMATTER
            .format_move_list(&mut position, &search_results.pv)
            .unwrap()
            .join(", ");
        assert_eq!(pv, "♞c4xd6");
    }

    #[test]
    fn test_already_checkmated() {
        setup();
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
        assert_eq!(
            search_results,
            SearchResults {
                position,
                score: -100_000,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::Checkmate,
            }
        );
    }

    #[test]
    fn test_already_stalemated() {
        setup();
        let fen = "8/6n1/5k1K/6n1/8/8/8/8 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
        assert_eq!(
            search_results,
            SearchResults {
                position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::Stalemate,
            }
        );
    }

    #[test]
    fn test_mate_in_one() {
        setup();
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
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
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 3).iterative_deepening();
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
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 3).iterative_deepening();
        assert_eq!(
            move_formatter::format_move_list(&position, &search_results),
            "♕f5-g6+,h7xg6,♗c2xg6#"
        );
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
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 5).iterative_deepening();
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

    #[test]
    fn test_mate_in_four() {
        setup();
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 7).iterative_deepening();
        assert_eq!(
            long_format_moves(&position, &search_results),
            "♕g3-g6+,f7xg6,♗d5-g8+,♚h7-h8,♗g8-f7+,♚h8-h7,f5xg6#"
        );
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
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 5).iterative_deepening();
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
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 2).iterative_deepening();
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }

    #[test]
    fn test_hiarcs_blunder() {
        setup();
        let fen = "r3k2r/4n1pp/pqpQ1p2/8/1P2b1P1/2P2N1P/P4P2/R1B2RK1 w kq - 0 17";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 5).iterative_deepening();
        assert!(search_results.pv_moves_as_string().starts_with("f1-e1"));
    }

    #[test]
    fn test_rooks_on_seventh_rank_preferred() {
        setup();
        let fen = "2q2rk1/B3ppbp/6p1/1Q2P3/8/PP2PN2/6r1/3RK2R b K - 0 19";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♛c8-c2");
    }

    #[test]
    fn test_50_move_rule_is_recognised() {
        setup();
        let fen = "4k3/8/R7/7n/7r/8/8/4K3 b - - 98 76";
        let original_position: Position = Position::from(fen);

        let mut in_progress_position: Position = original_position.clone();
        let in_progress_search_results =
            create_search(&mut in_progress_position, &TranspositionTable::new(1), 1)
                .iterative_deepening();
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
        let drawn_position_search_results =
            create_search(&mut drawn_position, &TranspositionTable::new(1), 1)
                .iterative_deepening();
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
        let drawn_search_results =
            uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
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
        setup();
        let fen = "6k1/5p1p/1Q4p1/q1P1P3/3P4/4Pb2/2K5/8 b - - 0 45";
        let uci_initial_position_str = format!("position fen {}", fen);

        let go_options_str = "depth 5";
        let search_results_1 = uci::run_uci_position(&uci_initial_position_str, go_options_str);
        let _pv_moves_1 = search_results_1.pv_moves_as_string();
        assert_eq!(search_results_1.pv_moves_as_string(), "f3-e4,c2-d1,a5-c3,d1-e2,c3-c2");

        let search_results_2 = uci::run_uci_position(
            &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3"),
            go_options_str,
        );
        let _pv_moves_2 = search_results_2.pv_moves_as_string();
        assert_eq!(search_results_2.pv_moves_as_string(), "a5-e1,b6-d8,g8-g7,d8-f6,g7-g8");

        let search_results_3 = uci::run_uci_position(
            &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2"),
            go_options_str,
        );
        let pv_moves_3 = search_results_3.pv_moves_as_string();
        assert_eq!(pv_moves_3, "d5-e4,c2-d1,e4-f3,d1-c2,a5-e1");

        let search_results_4 = uci::run_uci_position(
            &format!("{} {}", uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2 d5e4 c2b3"),
            go_options_str,
        );
        let pv_moves_4 = search_results_4.pv_moves_as_string();
        assert_eq!(pv_moves_4, "a5-e1,b6-d8,g8-g7,d8-f6,g7-g8");

        //TRANSPOSITION_TABLE.clear();
        let search_results_5 = uci::run_uci_position(
            &format!(
                "{} {}",
                uci_initial_position_str, " moves f3e4 c2b3 e4d5 b3c2 d5e4 c2b3 e4d5 b3c2"
            ),
            go_options_str,
        );
        let pv_moves_5 = search_results_5.pv_moves_as_string();
        assert_eq!(pv_moves_5, "a5-a2,c2-d3,a2-c4,d3-d2,g8-g7");

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

    #[test]
    fn test_black_avoids_draw_using_contempt() {
        setup();
        let go_for_draw_uci_position_str = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_options_str = "depth 1";
        let drawn_search_results =
            uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
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

        config::set_contempt(1000);
        let drawn_search_results =
            uci::run_uci_position(go_for_draw_uci_position_str, go_options_str);
        assert_eq!(drawn_search_results.pv_moves_as_string(), "e7-e6");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                position: drawn_search_results.position,
                score: -826,
                depth: 1,
                pv: vec![],
                game_status: GameStatus::InProgress,
            },
        );
        config::set_contempt(0);
    }
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
        let go_for_draw_uci_position_str = "position fen r1b5/ppp2Bpk/3p2Np/4p3/4P2q/3P1n1P/PPP2bP1/R1B4K w - - 0 1 moves g6f8 h7h8 f8g6 h8h7";
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
        assert!(Search::is_mating_score(score));

        let score = -MAXIMUM_SCORE;
        assert!(Search::is_mating_score(score));

        let score = MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize;
        assert!(Search::is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize);
        assert!(Search::is_mating_score(score));

        let score = (MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize) - 1;
        assert!(!Search::is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize) + 1;
        assert!(!Search::is_mating_score(score));
    }

    #[test]
    fn test_is_drawing_score() {
        setup();
        let score = -1;
        assert!(!Search::is_drawing_score(score));

        let score = 1;
        assert!(!Search::is_drawing_score(score));

        let score = 0;
        assert!(Search::is_drawing_score(score));
    }

    #[test]
    fn test_quiescence_search() {
        setup();
        let fen = "3k4/5pq1/5ppP/5b2/4R3/8/4K3/8 b - - 0 1";
        let mut position: Position = Position::from(fen);
        let search_results =
            create_search(&mut position, &TranspositionTable::new(1), 1).iterative_deepening();
        assert_eq!(move_formatter::format_move_list(&position, &search_results), "♛g7xh6");
    }

    #[test]
    fn test_get_repetition_count() {
        assert_eq!(Search::get_repeat_position_count(&vec!()), 0);

        let k1 = || RepetitionKey { zobrist_hash: 1, half_move_clock: 100 };
        let k2 = || RepetitionKey { zobrist_hash: 2, half_move_clock: 100 };
        assert_eq!(Search::get_repeat_position_count(&vec![k1()]), 0);
        assert_eq!(Search::get_repeat_position_count(&vec![k2(), k1()]), 0);
        assert_eq!(Search::get_repeat_position_count(&vec![k2(), k2(), k1()]), 0);
        assert_eq!(Search::get_repeat_position_count(&vec![k2(), k2(), k2(), k1()]), 0);
        assert_eq!(Search::get_repeat_position_count(&vec![k1(), k2(), k2(), k2(), k1()]), 1);
        assert_eq!(Search::get_repeat_position_count(&vec![k2(), k1(), k2(), k2(), k2(), k1()]), 1);
        assert_eq!(
            Search::get_repeat_position_count(&vec![k1(), k2(), k1(), k2(), k2(), k2(), k1()]),
            2
        );
    }
}
