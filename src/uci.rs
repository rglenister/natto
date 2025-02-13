use std::{io, thread};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::exit;
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::{debug, error, info};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use crate::{board, fen, util};
use crate::chess_move::{convert_chess_move_to_raw, RawChessMove};
use crate::position::{Position, NEW_GAME_FEN};
use crate::board::{Board, PieceColor};
use crate::board::PieceColor::{Black, White};
use crate::search::{search, SearchParams};

use std::time::Duration;

include!("util/generated_macro.rs");

static UCI_POSITION_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^position (startpos|fen ([^*]+))[ ]*(moves (.*))?$").unwrap());
static RAW_MOVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

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
    pub search_moves: Option<Vec<RawChessMove>>,
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
            uci_go_options.search_moves = moves_string_to_raw_moves(captures[2].to_string());
        }
    }
    uci_go_options
}

pub(crate) fn parse_position(input: &str) -> Option<Position> {
    fn load_moves(position: &Position, captures: &Captures) -> Option<Position> {
        captures.get(4).map_or(Some(*position), |mvs| {
            update_position(*position, mvs.as_str().to_string())
        })
    }

    if let Some(captures) = UCI_POSITION_REGEX.captures(input) {
        if &captures[1] == "startpos" {
            let new_game_position = Position::from(NEW_GAME_FEN);
            load_moves(&new_game_position, &captures)
        } else if let Some(fen) = captures.get(2) {
            let fen_position = Position::from(fen.as_str());
            load_moves(&fen_position, &captures)
        } else {
            None
        }
    } else {
        error!("Unable to parse position: {}", input);
        None
    }
}


fn update_position(position: Position, moves: String) -> Option<Position> {
    let raw_chess_moves = moves_string_to_raw_moves(moves)?;
    let positions = replay_moves(&position, raw_chess_moves)?;
    let last_position: Position = positions.last()?.clone();
    Some(last_position)
}

fn replay_moves(position: &Position, raw_moves: Vec<RawChessMove>) -> Option<Vec<Position>> {
    let result: Option<Vec<Position>> = raw_moves.iter().try_fold(Vec::new(), |mut acc: Vec<Position>, rm: &RawChessMove| {
        let current_position = if !acc.is_empty() { &acc.last().unwrap().clone()} else { position };
        let next_position = current_position.make_raw_move(rm);
        match next_position {
            Some((next_position, _cm)) => {
                acc.push(next_position);
                Some(acc)
            },
            None  => None
        }
    });
    result
}

fn parse_initial_moves(raw_move_strings: Vec<String>) -> Option<Vec<RawChessMove>> {
    let result: Option<Vec<RawChessMove>> = raw_move_strings.iter().try_fold(Vec::new(), |mut acc: Vec<RawChessMove>, rms: &String| {
        match parse_move(rms.clone()) {
            Some(raw_chess_move) => {
                acc.push(raw_chess_move);
                Some(acc)
            },
            None => None
        }
    });
    result
}

fn moves_string_to_raw_moves(moves: String) -> Option<Vec<RawChessMove>> {
    let moves_vec: Vec<String> = moves.split_whitespace().map(String::from).collect();
    let raw_chess_moves = parse_initial_moves(moves_vec)?;
    Some(raw_chess_moves)
}

fn parse_move(raw_move_string: String) -> Option<RawChessMove> {
    let captures = RAW_MOVE_REGEX.captures(&raw_move_string);
    captures.map(|captures| {
        let promote_to = captures.name("promote_to").map(|m| board::PieceType::from_char(m.as_str().to_string().chars().nth(0).unwrap()));
        return RawChessMove::new(
            util::parse_square(captures.name("from").unwrap().as_str()).unwrap(),
            util::parse_square(captures.name("to").unwrap().as_str()).unwrap(),
            if promote_to.is_some() { Some(promote_to.unwrap().expect("REASON")) } else { None }
        );
    })
}

pub fn create_search_params(uci_go_options: &UciGoOptions, side_to_move: PieceColor) -> SearchParams{
    let allocate_move_time_millis = || -> Option<usize> {
        if uci_go_options.move_time.is_some() {
            uci_go_options.move_time
        } else {
            let remaining_time_millis: usize = uci_go_options.time[side_to_move as usize]?;
            let inc_per_move_millis: usize = uci_go_options.inc[side_to_move as usize].map_or(0, |inc| inc);
            let remaining_number_of_moves_to_go: usize = uci_go_options.moves_to_go.map_or(1, |moves_to_go| moves_to_go);

            let base_time = remaining_time_millis / remaining_number_of_moves_to_go;
            // Add a portion of the increment (50% here)
            let inc_bonus = inc_per_move_millis / 2;

            // Cap at a maximum thinking time (e.g., ⅓ of total remaining time)
            let max_time = remaining_time_millis / 3;

            // Final time calculation
            Some((base_time + inc_bonus).min(max_time))
        }
    };

    let allocate_max_depth = || -> isize {
        let depth = uci_go_options.depth.max(uci_go_options.mate);
        depth.map_or(usize::MAX, |d| d).try_into().unwrap_or(isize::MAX)
    };

    let allocate_max_nodes = || -> usize {
        uci_go_options.nodes.map_or(usize::MAX, |nodes| nodes)
    };

    SearchParams {
        allocated_time_millis: allocate_move_time_millis().map_or(SearchParams::DEFAULT_MOVE_TIME_MILLIS, |mtm| mtm),
        max_depth: allocate_max_depth(),
        max_nodes: allocate_max_nodes()
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{Board, Piece};
    use super::*;
    use crate::board::PieceColor::{Black, White};
    use crate::board::PieceType::{Bishop, Knight, Pawn, Queen, Rook};
    use crate::position::NEW_GAME_FEN;

    #[test]
    fn test_parse_position() {
        assert!(parse_position("position startpos").is_some());
        assert!(parse_position("position fen r2qk2r/pb4pp/1n2Pb2/2B2Q2/p1p5/2P5/2B2PPP/RN2R1K1 w - - 1 0").is_some());
        assert!(parse_position("position startpos moves e2e4 e7e5").is_some());
        assert!(parse_position("position startpos moves e2e4 e7e4").is_none());
    }
    #[test]
    fn test_update_position() {
        let position = Position::from(NEW_GAME_FEN);
        let last_position = update_position(position, "e2e4 e7e5".to_string()).unwrap();
        let board = last_position.board();
        assert_eq!(board.get_piece(sq!("e2")), None);
        assert_eq!(board.get_piece(sq!("e4")), Some(Piece {piece_type: Pawn, piece_color: White}));
        assert_eq!(board.get_piece(sq!("e7")), None);
        assert_eq!(board.get_piece(sq!("e5")),Some(Piece {piece_type: Pawn, piece_color: Black}));
    }


    #[test]
    fn test_update_position_with_illegal_move() {
        let position = Position::from(NEW_GAME_FEN);
        let last_position = update_position(position, "e2e4 e8e5".to_string());
        assert!(last_position.is_none());
    }

    #[test]
    fn test_parse_initial_moves() {
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string())),
            Some(vec!(RawChessMove {from: sq!("e2"), to: sq!("e4"), promote_to: None})));
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string(), "e7e5".to_string())),
            Some(vec!(RawChessMove {from: sq!("e2"), to: sq!("e4"), promote_to: None}, RawChessMove {from: sq!("e7"), to: sq!("e5"), promote_to: None})));

        assert_eq!(
            parse_initial_moves(vec!("i2e4".to_string(), "e7e5".to_string())),
            None);
    }

    #[test]
    fn test_parse_move() {
        assert_eq!(parse_move("a1b1".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: None});
        assert_eq!(parse_move("h8a1n".to_string()).unwrap(), RawChessMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Knight)});
        assert_eq!(parse_move("h8a1b".to_string()).unwrap(), RawChessMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Bishop)});
        assert_eq!(parse_move("a1b1r".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Rook)});
        assert_eq!(parse_move("a1b1q".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Queen)});

        assert_eq!(parse_move("a1b1k".to_string()), None);
        assert_eq!(parse_move("".to_string()), None);
        assert_eq!(parse_move("i8h8".to_string()), None);
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
        assert_eq!(uci_go_options.search_moves, Some(vec!(RawChessMove::new(sq!("e2"), sq!("e4"), None), RawChessMove::new(sq!("e7"), sq!("e5"), None))));
    }

    #[test]
    fn test_create_search_params_time_white() {
        let command = "go wtime 1000 btime 1100 winc 200 binc 400".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, 333);
        assert_eq!(search_params.max_depth, isize::MAX);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_black() {
        let command = "go wtime 1000 btime 1100 winc 200 binc 400".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::Black);
        assert_eq!(search_params.allocated_time_millis, 366);
        assert_eq!(search_params.max_depth, isize::MAX);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_white_with_moves_to_go() {
        let command = "go wtime 10000 btime 1100 winc 200 binc 400 movestogo 10".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis,1100);
        assert_eq!(search_params.max_depth, isize::MAX);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_time_white_with_move_time() {
        let command = "go wtime 10000 btime 1100 winc 200 binc 400 movetime 1234".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, 1234);
        assert_eq!(search_params.max_depth, isize::MAX);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_depth() {
        let command = "go depth 3".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, SearchParams::DEFAULT_MOVE_TIME_MILLIS);
        assert_eq!(search_params.max_depth, 3);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_depth_with_mate() {
        let command = "go depth 3 mate 5".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, SearchParams::DEFAULT_MOVE_TIME_MILLIS);
        assert_eq!(search_params.max_depth, 5);
        assert_eq!(search_params.max_nodes, usize::MAX);

        let command = "go depth 10 mate 5".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, SearchParams::DEFAULT_MOVE_TIME_MILLIS);
        assert_eq!(search_params.max_depth, 10);
        assert_eq!(search_params.max_nodes, usize::MAX);
    }
    #[test]
    fn test_create_search_params_nodes() {
        let command = "go nodes 1001".to_string();
        let uci_go_options = parse_uci_go_options(Some(command));
        let search_params = create_search_params(&uci_go_options, PieceColor::White);
        assert_eq!(search_params.allocated_time_millis, SearchParams::DEFAULT_MOVE_TIME_MILLIS);
        assert_eq!(search_params.max_depth, isize::MAX);
        assert_eq!(search_params.max_nodes, 1001);
    }
}