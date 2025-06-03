use crate::chessboard::piece::PieceColor::{Black, White};
use crate::r#move::{Move, RawMove};
use crate::position::Position;
use crate::search::negamax::{SearchParams, SearchResults, MAXIMUM_SEARCH_DEPTH};
use log::{error, info};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::chess_util::util;
use crate::chess_util::util::create_repeat_position_counts;
use crate::search;

include!("chess_util/generated_macro.rs");

static UCI_POSITION_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^position\s+(startpos|fen\s+([^\s]+(?:\s+[^\s]+){5}))(?:\s+moves\s+([\s\w]+))?$").unwrap());

#[derive(Clone, Debug)]
pub struct UciPosition {
    pub given_position: Position,
    pub end_position: Position,
    pub position_move_pairs: Option<Vec<(Position, Move)>>,
}

impl UciPosition {
    pub fn all_game_positions(&self) -> Vec<Position> {
        let game_positions: Vec<_> = self.position_move_pairs
            .iter()
            .flat_map(|pairs| pairs.iter().map(|pm| pm.0))
            .collect();

        [vec!(self.given_position).as_slice(), game_positions.as_slice()].concat()
    }
}

#[derive(Clone)]
#[derive(Default)]
#[derive(Debug)]
pub struct UciGoOptions {
    pub time: [Option<usize>; 2],
    pub inc: [Option<usize>; 2],
    pub moves_to_go: Option<usize>,
    pub depth: Option<usize>,
    pub nodes: Option<usize>,
    pub mate: Option<usize>,
    pub move_time: Option<usize>,
    pub ponder: bool,
    pub infinite: bool,
    pub search_moves: Option<Vec<RawMove>>,
}

pub(crate) fn parse_uci_go_options(options_string: Option<String>) -> UciGoOptions {
    let mut uci_go_options = UciGoOptions::default();
    if let Some(options_string) = options_string {
        let re_numeric_options = Regex::new(r"(wtime|btime|winc|binc|movestogo|depth|nodes|mate|movetime) (\d+)").unwrap();
        let mut params = HashMap::new();

        for cap in re_numeric_options.captures_iter(&options_string) {
            params.insert(cap[1].to_string(), cap[2].parse::<usize>().unwrap());
        }
        uci_go_options.time[White as usize] = params.get("wtime").copied();
        uci_go_options.time[Black as usize] = params.get("btime").copied();
        uci_go_options.inc[White as usize] = params.get("winc").copied();
        uci_go_options.inc[Black as usize] = params.get("binc").copied();
        uci_go_options.moves_to_go = params.get("movestogo").copied();
        uci_go_options.depth = params.get("depth").copied();
        uci_go_options.nodes = params.get("nodes").copied();
        uci_go_options.mate = params.get("mate").copied();
        uci_go_options.move_time = params.get("movetime").copied();

        let re_flag_options = Regex::new(r"(infinite|ponder)").unwrap();
        for cap in re_flag_options.captures_iter(&options_string) {
            match &cap[1] {
                "infinite" => uci_go_options.infinite = true,
                "ponder" => uci_go_options.ponder = true,
                _ => ()
            }
        }

        let re_search_moves_option = Regex::new(r"(searchmoves (.*))+$").unwrap();
        if let Some(captures) = re_search_moves_option.captures(&options_string) {
            uci_go_options.search_moves = util::moves_string_to_raw_moves(captures[2].to_string());
        }
    }
    uci_go_options
}

pub(crate) fn parse_position(input: &str) -> Option<UciPosition> {
    fn create_uci_position(position: &Position, captures: &Captures) -> Option<UciPosition> {
        captures.get(3)
            .map_or(Some(vec!()), |m| { util::replay_moves(position, m.as_str().to_string()) })
            .map(|moves| UciPosition {
                given_position: *position,
                end_position: if !moves.is_empty() { moves.last().unwrap().0 } else {*position},
                position_move_pairs: Some(moves)
            })
    }

    if let Some(captures) = UCI_POSITION_REGEX.captures(input) {
        if &captures[1] == "startpos" {
            let new_game_position = Position::new_game();
            create_uci_position(&new_game_position, &captures)
        } else if let Some(fen) = captures.get(2) {
            let fen_position = Position::from(fen.as_str());
            create_uci_position(&fen_position, &captures)
        } else {
            None
        }
    } else {
        error!("UCI unable to parse position: {}", input);
        None
    }
}

pub fn create_search_params(uci_go_options: &UciGoOptions, uci_position: &UciPosition) -> SearchParams {
    let allocate_move_time_millis = || -> Option<usize> {
        if uci_go_options.move_time.is_some() {
            uci_go_options.move_time
        } else {
            let side_to_move = uci_position.end_position.side_to_move();
            let remaining_time_millis: usize = uci_go_options.time[side_to_move as usize]?;
            let inc_per_move_millis: usize = uci_go_options.inc[side_to_move as usize].map_or(0, |inc| inc);
            let remaining_number_of_moves_to_go: usize = uci_go_options.moves_to_go.map_or(30, |moves_to_go| moves_to_go);

            let base_time = remaining_time_millis / remaining_number_of_moves_to_go;
            // Add a portion of the increment (50% here)
            let inc_bonus = inc_per_move_millis / 2;

            // Cap at a maximum thinking time (e.g., ⅓ of total remaining time)
            let max_time = remaining_time_millis / 3;

            // Final time calculation
            Some((base_time + inc_bonus).min(max_time))
        }
    };

    let allocate_max_depth = || -> usize {
        let depth = uci_go_options.depth.max(uci_go_options.mate);
        MAXIMUM_SEARCH_DEPTH.min(depth.map_or(usize::MAX, |d| d).try_into().unwrap_or(usize::MAX))
    };

    let allocate_max_nodes = || -> usize {
        uci_go_options.nodes.map_or(usize::MAX, |nodes| nodes)
    };

    SearchParams {
        allocated_time_millis: allocate_move_time_millis().map_or(usize::MAX, |mtm| mtm),
        max_depth: allocate_max_depth() as usize,
        max_nodes: allocate_max_nodes(),
    }
}

pub fn send_to_gui(data: &str) {
    println!("{}", data);
    info!("UCI Protocol: sending to GUI: {}", data);
}

pub fn run_uci_position(uci_position_str: &str, go_options_str: &str) -> SearchResults {
    let uci_position = parse_position(uci_position_str).unwrap();
    let uci_go_options = parse_uci_go_options(Some(go_options_str.to_string()));
    let search_params = create_search_params(&uci_go_options, &uci_position);
    let repeat_position_counts = Some(create_repeat_position_counts(uci_position.all_game_positions()));
    search::negamax::iterative_deepening(&uci_position.end_position, &search_params, Arc::new(AtomicBool::new(false)), repeat_position_counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chessboard::piece::PieceColor;
    use crate::chessboard::piece::PieceColor::{Black, White};
    fn create_uci_position(side_to_move: PieceColor) -> UciPosition {
        let white_to_move = Position::new_game();
        let black_to_move = white_to_move.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None));
        let position = if side_to_move == White {white_to_move} else {black_to_move.unwrap().0};
        UciPosition { given_position: position, end_position: position, position_move_pairs: None }
    }

    #[test]
    fn test_parse_position() {
        assert!(parse_position("position startpos").is_some());
        assert!(parse_position("position fen r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0").is_some());
        assert!(parse_position("position startpos moves e2e4 e7e5").is_some());
        assert!(parse_position("position fen 8/8/8/8/4k3/8/8/2BQKB2 w - - 0 1 moves f1c4 e4e5").is_some());
        assert!(parse_position("position startpos moves e2e4 e7e4").is_none());
        assert!(parse_position("position startpos moves e2e3 e7e5 b1c3 d7d5 a2a4 f8a3 b2a3 b8c6 f1b5 d8h4 c3d5 h4f2 e1f2    c8g1").is_none());
    }

    #[test]
    fn test_parse_uci_go_options() {
        let command = "go wtime 10 btime 11 winc 2 binc 4 movestogo 23 depth 30 nodes 1001 mate 3 movetime 1234 ponder infinite searchmoves e2e4 e7e5".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        assert_eq!(uci_go_options.time[White as usize], Some(10));
        assert_eq!(uci_go_options.time[Black as usize], Some(11));
        assert_eq!(uci_go_options.inc[White as usize], Some(2));
        assert_eq!(uci_go_options.inc[Black as usize], Some(4));
        assert_eq!(uci_go_options.moves_to_go, Some(23));
        assert_eq!(uci_go_options.depth, Some(30));
        assert_eq!(uci_go_options.nodes, Some(1001));
        assert_eq!(uci_go_options.mate, Some(3));
        assert_eq!(uci_go_options.move_time, Some(1234));
        assert_eq!(uci_go_options.ponder, true);
        assert_eq!(uci_go_options.infinite, true);
        assert_eq!(uci_go_options.search_moves, Some(vec!(RawMove::new(sq!("e2"), sq!("e4"), None), RawMove::new(sq!("e7"), sq!("e5"), None))));
    }

    #[test]
    fn test_create_search_params_with_no_go_params() {
        let command = "go".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, usize::MAX);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }

    #[test]
    fn test_create_search_params_time_white() {
        let command = "go wtime 1000 btime 1100 winc 200 binc 400".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, 133);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_black() {
        let command = "go wtime 1000 btime 1100 winc 200 binc 400".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(Black));
        assert_eq!(search_params.allocated_time_millis, 236);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_white_with_moves_to_go() {
        let command = "go wtime 10000 btime 1100 winc 200 binc 400 movestogo 10".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis,1100);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_white_with_move_time() {
        let command = "go wtime 10000 btime 1100 winc 200 binc 400 movetime 1234".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, 1234);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_depth() {
        let command = "go depth 3".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, usize::MAX);
        assert_eq!(search_params.max_depth, 3);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_depth_with_mate() {
        let command = "go depth 3 mate 5".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, usize::MAX);
        assert_eq!(search_params.max_depth, 5);
        assert_eq!(search_params.max_nodes, usize::MAX);

        let command = "go depth 10 mate 5".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, usize::MAX);
        assert_eq!(search_params.max_depth, 10);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_nodes() {
        let command = "go nodes 1001".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, &create_uci_position(White));
        assert_eq!(search_params.allocated_time_millis, usize::MAX);
        assert_eq!(search_params.max_depth, MAXIMUM_SEARCH_DEPTH);
        assert_eq!(search_params.max_nodes, 1001);
    }
}
