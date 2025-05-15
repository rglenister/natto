use crate::game::GameStatus::{Checkmate, InProgress};
use crate::game::{Game, GameStatus};
use crate::move_formatter::{FormatMove, LONG_FORMATTER};
use crate::move_generator::generate_moves;
use crate::position::Position;
use crate::search::sorted_move_list::SortedMoveList;
use crate::{fen, move_generator, r#move, uci};
use itertools::Itertools;
use log::{debug, info, error};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use arrayvec::ArrayVec;
use crate::eval::evaluation;
use crate::r#move::Move;
use crate::eval::node_counter::{NodeCountStats, NodeCounter};
use crate::search::quiescence;
use crate::search::transposition_table::{BoundType, TRANSPOSITION_TABLE};
use crate::util::replay_moves;

include!("../util/generated_macro.rs");

static NODE_COUNTER: LazyLock<RwLock<NodeCounter>> = LazyLock::new(|| {
    let node_counter = NodeCounter::new();
    RwLock::new(node_counter)
});

pub const MAXIMUM_SEARCH_DEPTH: usize = 63;

pub const MAXIMUM_SCORE: isize = 100000;


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResults {
    pub position: Position,
    pub score: isize,
    pub depth: usize,
    pub pv: Vec<(Position, Move)>,
    pub game_status: GameStatus,
}

impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "score: {} depth: {} bestline: {} game_status: {:?}",
               self.score,
               self.depth,
               LONG_FORMATTER.format_move_list(&self.position, &*self.pv).unwrap().join(", "),
               self.game_status)
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

    pub fn new(allocated_time_millis: usize, max_depth: isize, max_nodes: usize) -> SearchParams {
        SearchParams { allocated_time_millis, max_depth: max_depth.try_into().unwrap(), max_nodes }
    }

    pub fn new_by_depth(max_depth: isize) -> SearchParams {
        SearchParams::new(usize::MAX, max_depth, usize::MAX)
    }
}
pub struct SearchContext<'a> {
    search_params: &'a SearchParams,
    stop_flag: Arc<AtomicBool>,
    sorted_root_moves: RefCell<SortedMoveList>,
    pub repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
}

impl SearchContext<'_> {
    pub fn new(
        search_params: &SearchParams,
        stop_flag: Arc<AtomicBool>,
        repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
        moves: Vec<Move>,
    ) -> SearchContext {
        SearchContext {
            search_params,
            stop_flag,
            sorted_root_moves: RefCell::new(SortedMoveList::new(&moves)),
            repeat_position_counts,
        }
    }
}

impl SearchResults {
    fn pv_moves(&self) -> Vec<Move> {
        self.pv.clone().into_iter().map(|pm| pm.1).collect()
    }
    fn pv_moves_as_string(&self) -> String {
        self.pv_moves().iter().join(",")
    }
}

pub fn iterative_deepening(position: &Position, search_params: &SearchParams, stop_flag: Arc<AtomicBool>, repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> SearchResults {
    reset_node_counter();
    let mut search_results_stack = vec!();
    for iteration_max_depth in 1..=search_params.max_depth {
        let mut search_context = SearchContext::new(search_params, stop_flag.clone(), repeat_position_counts.clone(), generate_moves(position));
        let search_results = negamax(position, iteration_max_depth, &mut search_context);
        if !search_context.stop_flag.load(Ordering::Relaxed) {
            debug!("Search results for depth {}: {}", iteration_max_depth, search_results);
            uci::send_to_gui(format_uci_info(position, &search_results, &node_counter_stats()));
            let is_checkmate = search_results.game_status == Checkmate;
            search_results_stack.push(search_results);
            if is_checkmate {
                info!("Found mate at depth {} - stopping search", iteration_max_depth);
                TRANSPOSITION_TABLE.clear();
                break;
            }
        } else {
            break;
        }
    }
    search_results_stack.pop().unwrap()
}

fn negamax(position: &Position, max_depth: usize, search_context: &mut SearchContext) -> SearchResults {
    let mut pv: ArrayVec<(Position, Move), MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
    let score = negamax_search(position, &mut ArrayVec::new(), &mut pv, max_depth, max_depth, search_context, -MAXIMUM_SCORE, MAXIMUM_SCORE);
    create_search_results(position, score, max_depth, pv.to_vec(), search_context)
}

fn negamax_search(
    position: &Position,
    current_line: &mut ArrayVec<(Position, Move), MAXIMUM_SEARCH_DEPTH>,
    pv: &mut ArrayVec<(Position, Move), MAXIMUM_SEARCH_DEPTH>,
    depth: usize,
    max_depth: usize,
    search_context: &mut SearchContext,
    mut alpha: isize,
    mut beta: isize,
) -> isize {
    increment_node_counter();
    let ply = max_depth - depth;
    let alpha_original = alpha;
    let beta_original = beta;
    if used_allocated_move_time(search_context.search_params) {
        search_context.stop_flag.store(true, Ordering::Relaxed);
        return 0;
    }
    if let Some(entry) = TRANSPOSITION_TABLE.probe(position.hash_code()) {
        if entry.depth >= depth {
            match entry.bound_type {
                BoundType::Exact => {
                    pv.clear();
                    pv.extend(retrieve_principal_variation(*position, entry.best_move.clone()));
                    return entry.score
                },
                BoundType::LowerBound => if entry.score > alpha {
                    alpha = entry.score
                },
                // BoundType::UpperBound => if entry.score < beta {
                //     beta = entry.score
                // },
                BoundType::UpperBound => {  
                    // do nothing
                }
            }
            if alpha >= beta {
                pv.clear();
                pv.extend(retrieve_principal_variation(*position, entry.best_move.clone()));
                return entry.score;
            }
        }
    }
    if depth > 0 {
        let moves = if depth == max_depth { search_context.sorted_root_moves.get_mut().get_all_moves() } else { generate_moves(position) };
        let mut best_score = -MAXIMUM_SCORE;
        let mut best_move = None;
        for mv in moves {
            if let Some(next_position) = position.make_move(&mv) {
                // there isn't a checkmate or a stalemate
                let mut child_pv: ArrayVec<(Position, Move), MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
                current_line.push(next_position);
                let next_score = if get_repeat_position_count(&next_position.0, current_line, search_context.repeat_position_counts.as_ref()) >= 2 {
                    0
                } else {
                    -negamax_search(&next_position.0, current_line, &mut child_pv, depth - 1, max_depth, search_context, -beta, -alpha)
                };
                current_line.pop();
                if depth == max_depth { search_context.sorted_root_moves.borrow_mut().update_score(&mv, next_score) };
                if next_score > best_score || best_move.is_none() {
                    best_score = next_score;
                    best_move = Some(mv);
                    pv.clear();
                    pv.push(next_position);
                    pv.extend(child_pv);
                }
                alpha = alpha.max(next_score);
                if alpha >= beta || (depth >= 2 && search_context.stop_flag.load(Ordering::Relaxed)) {
                    break;
                }
            }
        };
        if best_move.is_some() {
            if !search_context.stop_flag.load(Ordering::Relaxed) {
                TRANSPOSITION_TABLE.insert(position, depth, alpha_original, beta_original, best_score, best_move);
            }
        } else {
            best_score = evaluation::evaluate(position, depth - 1, search_context.repeat_position_counts.as_ref());
        }     
        best_score
    } else {
        let mut score = evaluation::evaluate(position, ply, search_context.repeat_position_counts.as_ref());
        if !is_terminal_score(score) {
            score = quiescence::quiescence_search(position, (ply + 1) as isize, alpha, beta);
        }
        if !search_context.stop_flag.load(Ordering::Relaxed) {
            TRANSPOSITION_TABLE.insert(position, 0, alpha_original, beta_original, score, None);
        }
        score
    }
}

fn create_search_results(position: &Position, score: isize, depth: usize, pv: Vec<(Position, Move)>, search_context: &SearchContext) -> SearchResults {
    let last_position = pv.last().map_or(position, |m| &m.0);
    let game_status = get_game_status(last_position, search_context.repeat_position_counts.as_ref());
    SearchResults {
        position: *position,
        score,
        depth,
        pv: pv,
        game_status,
    }
}

fn retrieve_principal_variation(position: Position, mov: Option<Move>) -> Vec<(Position, Move)> {
    let mut pv = Vec::new();
    let mut current_position = position;
    
    if let Some(mv) = mov {
        if let Some(next_position) = current_position.make_move(&mv) {
            current_position = next_position.0;
            pv.push(next_position);
        }
    }

    while let Some(entry) = TRANSPOSITION_TABLE.probe(current_position.hash_code()) {
        if entry.depth == 0 || entry.best_move.is_none() || pv.len() >= MAXIMUM_SEARCH_DEPTH / 2 {
            break;
        }
        let next_pos = current_position.make_move(&entry.best_move.unwrap()).unwrap();
        pv.push(next_pos);
        current_position = next_pos.0;
    }
    pv
}

fn get_game_status(position: &Position, repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> GameStatus {
    let game = Game::new(position, repeat_position_counts);
    game.get_game_status()
}

pub fn get_repeat_position_count(current_position: &Position, current_line: &[(Position, Move)], historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> usize {
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

fn format_uci_info(position: &Position, search_results: &SearchResults, node_counter_stats: &NodeCountStats) -> String {
    let moves_string =             search_results.pv.iter()
        .map(|pos| r#move::convert_chess_move_to_raw(&pos.1).to_string())
        .collect::<Vec<String>>()
        .join(" ");
    
    let moves = replay_moves(position, moves_string.clone());
    if moves.is_none() {
        error!("Invalid moves for position [{}] being sent to host as UCI info: [{}]", fen::write(position), moves_string);
    }

    format!("info depth {} score cp {} time {} nodes {} nps {} pv {}",
            search_results.depth,
            search_results.score,
            node_counter_stats.elapsed_time.as_millis(),
            node_counter_stats.node_count,
            node_counter_stats.nodes_per_second,
            moves_string)
}

fn is_mating_score(score: isize) -> bool {
    score.abs() >= MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize
}

fn is_drawing_score(score: isize) -> bool {
    score == 0
}

fn is_terminal_score(score: isize) -> bool {
    is_mating_score(score) || is_drawing_score(score)
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
    use super::*;
    use crate::r#move::RawMove;
    use crate::game::GameStatus::{DrawnByFiftyMoveRule, DrawnByThreefoldRepetition, Stalemate};
    use crate::move_formatter::{format_move_list, FormatMove};
    use crate::search::negamax::{iterative_deepening, MAXIMUM_SCORE};
    use crate::{move_formatter, uci, util};

    fn test_uci_position(uci_position_str: &str, go_options_str: &str) -> SearchResults {
        let uci_position = uci::parse_position(uci_position_str).unwrap();
        let uci_go_options = uci::parse_uci_go_options(Some(go_options_str.to_string()));
        let search_params = uci::create_search_params(&uci_go_options, &uci_position);
        let repeat_position_counts = Some(util::create_repeat_position_counts(uci_position.all_game_positions()));
        iterative_deepening(&uci_position.end_position, &search_params, Arc::new(AtomicBool::new(false)), repeat_position_counts)
    }
    
    fn test_eq(search_results: &SearchResults, expected: &SearchResults) {
        assert_eq!(search_results.score, expected.score);
        assert_eq!(search_results.depth, expected.depth);
        assert_eq!(search_results.game_status, expected.game_status);
    }

    fn long_format_moves(position: &Position, search_results: &SearchResults) -> String {
        LONG_FORMATTER.format_move_list(position, &search_results.pv).unwrap().join(",")
    }
    
    #[test]
    fn test_piece_captured() {
        let fen = "4k3/8/1P1Q4/R7/2n5/4N3/1B6/4K3 b - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams { allocated_time_millis: usize::MAX, max_depth: 1, max_nodes: usize::MAX }, Arc::new(AtomicBool::new(false)), None);
        assert_eq!(search_results.score, -985);
        let pv = move_formatter::LONG_FORMATTER.format_move_list(&position, &search_results.pv).unwrap().join(", ");
        assert_eq!(pv, "♞c4xd6");
    }

    #[test]
    fn test_already_checkmated() {
        let fen = "7K/5k2/8/7r/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(
            search_results,
            SearchResults {
                position,
                score: -100_000,
                depth: 1,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_already_stalemated() {
        let fen = "8/6n1/5k1K/6n1/8/8/8/8 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(
            search_results,
            SearchResults {
                position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: Stalemate,
            }
        );
    }

    #[test]
    fn test_mate_in_one() {
        let fen = "rnbqkbnr/p2p1ppp/1p6/2p1p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 0 4";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_one_using_high_depth() {
        let fen = "r1bqkbnr/p2p1ppp/1pn5/2p1p3/2B1P3/2N2Q2/PPPP1PPP/R1B1K1NR w KQkq - 2 5";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f3xf7#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 1,
                depth: 1,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_two() {
        let fen = "r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(3), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♕f5-g6+,h7xg6,♗c2xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 3,
                depth: 3,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_three() {
        let fen = "r5rk/5p1p/5R2/4B3/8/8/7P/7K w - - 1 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♖f6-a6+,f7-f6,♗e5xf6+,♜g8-g7,♖a6xa8#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_four() {
        let fen = "4R3/5ppk/7p/3BpP2/3b4/1P4QP/r5PK/3q4 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(7), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(long_format_moves(&position, &search_results), "♕g3-g6+,f7xg6,♗d5-g8+,♚h7-h8,♗g8-f7+,♚h8-h7,f5xg6#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 7,
                depth: 7,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_mate_in_three_fischer() {
        let fen = "8/8/8/8/4k3/8/8/2BQKB2 w - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♗f1-c4,♚e4-e5,♕d1-d5+,♚e5-f6,♕d5-g5#");
        test_eq(
            &search_results,
            &SearchResults {
                position,
                score: MAXIMUM_SCORE - 5,
                depth: 5,
                pv: vec![],
                game_status: Checkmate,
            }
        );
    }

    #[test]
    fn test_hiarcs_game_engine_would_not_get_out_of_check() {
        let fen = "N7/pp6/8/1k6/2QR4/8/PPP4P/R1B1K3 b Q - 2 32";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(2), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(search_results.score, -MAXIMUM_SCORE + 2);
    }
    
    #[test]
    fn test_hiarcs_blunder() {
        let fen = "r3k2r/4n1pp/pqpQ1p2/8/1P2b1P1/2P2N1P/P4P2/R1B2RK1 w kq - 0 17";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(5), Arc::new(AtomicBool::new(false)), None);
        assert!(search_results.pv_moves_as_string().starts_with("f1-e1"));
    }

    #[test]
    fn test_50_move_rule_is_recognised() {
        let fen = "4k3/8/R7/7n/7r/8/8/4K3 b - - 98 76";
        let in_progress_position: Position = Position::from(fen);
        let in_progress_search_results = iterative_deepening(&in_progress_position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(in_progress_search_results.pv_moves_as_string(), "h5-f4".to_string());
        test_eq(
            &in_progress_search_results,
            &SearchResults {
                position: in_progress_position,
                score: 302,
                depth: 1,
                pv: vec![],
                game_status: InProgress,
            }
        );

        let drawn_position = in_progress_position.make_raw_move(&RawMove::new(sq!("h5"), sq!("f4"), None)).unwrap().0;
        let drawn_position_search_results = iterative_deepening(&drawn_position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(drawn_position_search_results.pv_moves_as_string(), "e1-d1".to_string());
        test_eq(
            &drawn_position_search_results,
            &SearchResults {
                position: drawn_position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: DrawnByFiftyMoveRule,
            }
        );
    }

    #[test]
    fn test_losing_side_plays_for_draw() {
        let go_for_draw_uci_position_str = "position fen rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_for_win_uci_position_str = "position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1 moves g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1";
        let go_options_str = "depth 1";
        let drawn_search_results = test_uci_position(go_for_draw_uci_position_str, go_options_str);
        assert_eq!(drawn_search_results.pv_moves_as_string(), "f6-g8");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                position: drawn_search_results.position,
                score: 0,
                depth: 1,
                pv: vec![],
                game_status: InProgress,
            }
        );

        let win_search_results = test_uci_position(go_for_win_uci_position_str, go_options_str);
        assert_eq!(win_search_results.pv_moves_as_string(), "b8-c6".to_string());
        test_eq(
            &win_search_results,
            &SearchResults {
                position: win_search_results.position,
                score: 1000,
                depth: 1,
                pv: vec![],
                game_status: InProgress,
            }
        );
    }
    
    #[test]
    fn test_li_chess_game() {
        // https://lichess.org/RZTYaEbP#87
        let uci_position_str = "position fen 4kb1Q/p4p2/2pp4/5Q2/P4PK1/4P3/3q4/4n3 b - - 10 40 moves d2g2 g4h5 g2h2 h5g4 h2g2 g4h5 g2h2 h5g4 h2h8";
        let drawn_search_results = test_uci_position(uci_position_str, "depth 2");
        assert_eq!(drawn_search_results.pv_moves_as_string(), "f5-c8,e8-e7");
        test_eq(
            &drawn_search_results,
            &SearchResults {
                position: drawn_search_results.position,
                score: -560,
                depth: 2,
                pv: vec![],
                game_status: InProgress,
            }
        );
    }

    #[test]
    fn test_perpetual_check() {
        let go_for_draw_uci_position_str = "position fen r1b5/ppp2Bpk/3p2Np/4p3/4P2q/3P1n1P/PPP2bP1/R1B4K w - - 0 1 moves g6f8 h7h8 f8g6 h8h7";
        let search_results = test_uci_position(go_for_draw_uci_position_str, "depth 5");
        assert_eq!(search_results.pv_moves_as_string(), "g6-f8".to_string());
        //assert_eq!(search_results.pv_moves_as_string(), "g6-f8,h7-h8,f8-g6,h8-h7".to_string());
        test_eq(
            &search_results,
            &SearchResults {
                position: search_results.position,
                score: 0,
                depth: 5,
                pv: vec![],
                // todo it'd be good to actually get the three fold repetition status
                game_status: InProgress,
            }
        );
    }

    #[test]
    fn test_is_mating_score() {
        let score = MAXIMUM_SCORE;
        assert!(is_mating_score(score));
        
        let score = -MAXIMUM_SCORE;
        assert!(is_mating_score(score));
        
        let score = MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize;
        assert!(is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize);
        assert!(is_mating_score(score));

        let score = (MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize) - 1;
        assert!(!is_mating_score(score));

        let score = -(MAXIMUM_SCORE - MAXIMUM_SEARCH_DEPTH as isize) + 1;
        assert!(!is_mating_score(score));

    }

    #[test]
    fn test_is_drawing_score() {
        let score = -1;
        assert!(!is_drawing_score(score));

        let score = 1;
        assert!(!is_drawing_score(score));

        let score = 0;
        assert!(is_drawing_score(score));
    }

    #[test]
    fn test_quiescence_search() {
        let fen = "3k4/5pq1/5ppP/5b2/4R3/8/4K3/8 b - - 0 1";
        let position: Position = Position::from(fen);
        let search_results = iterative_deepening(&position, &SearchParams::new_by_depth(1), Arc::new(AtomicBool::new(false)), None);
        assert_eq!(format_move_list(&position, &search_results), "♛g7xh6");
    }
}