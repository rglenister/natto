use std::{io, thread};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::exit;
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::{debug, error, info};
use once_cell::sync::Lazy;
use regex::Regex;
use crate::{board, fen, util};
use crate::chess_move::{convert_chess_move_to_raw, RawChessMove};
use crate::position::{Position, NEW_GAME_FEN};
use crate::board::Board;
use crate::board::PieceColor::{Black, White};
use crate::search::search;

use std::time::Duration;

include!("util/generated_macro.rs");

static UCI_POSITION_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^position (startpos|fen ([^*]+))[ ]*(moves (.*))?$").unwrap());
static RAW_MOVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

#[derive(Default)]
pub struct UciGoOptions {
    pub time: [Option<usize>; 2],
    pub inc: [Option<usize>; 2],
    pub moves_to_go: Option<usize>,
    pub depth: Option<usize>,
    pub nodes: Option<usize>,
    pub mate: Option<usize>,
    pub move_time: Option<usize>,
    pub infinite: bool,
}

pub(crate) fn parse_uci_go_options(options_string: Option<String>) -> UciGoOptions {
    let mut uci_go_options = UciGoOptions::default();
    if let Some(options_string) = options_string {
        let re = Regex::new(r"(wtime|btime|winc|binc|movestogo|depth|nodes|mate|movetime) (\d+)").unwrap();
        let mut params = HashMap::new();

        for cap in re.captures_iter(&options_string) {
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
    }
    uci_go_options
}

pub(crate) fn parse_position(input: &str) -> Option<Position> {
    if let Some(captures) = UCI_POSITION_REGEX.captures(input) {
        if &captures[1] == "startpos" {
            let new_game_position = Position::from(NEW_GAME_FEN);
            if let Some(moves) = captures.get(4) {
                let end_position = update_position(new_game_position, moves.as_str().to_string());
                eprintln!("Startpos with moves: {:?}", moves.as_str());
                end_position
            } else {
                eprintln!("Startpos with no moves");
                Some(new_game_position)
            }
        } else if let Some(fen) = captures.get(2) {
            let fen_position = Position::from(fen.as_str());
            if let Some(moves) = captures.get(4) {
                eprintln!("FEN: {}\nMoves: {:?}", fen.as_str(), moves);
                update_position(fen_position, moves.as_str().to_string())
            } else {
                eprintln!("FEN: {}\nNo moves provided", fen.as_str());
                Some(fen_position)
            }
        } else {
            None
        }
    } else {
        eprintln!("Unable to parse position: {}", input);
        None
    }
}

fn update_position(position: Position, moves: String) -> Option<Position> {
    let moves_vec: Vec<String> = moves.split_whitespace().map(String::from).collect();
    let raw_chess_moves = parse_initial_moves(moves_vec)?;
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

#[cfg(test)]
mod tests {
    use crate::board::{Board, Piece};
    use super::*;
    use crate::board::PieceColor::{Black, White};
    use crate::board::PieceType::{Bishop, Knight, Pawn, Queen, Rook};
    use crate::position::NEW_GAME_FEN;

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
        let command = "go wtime 10 btime 11 winc 2 binc 4 movestogo 23 depth 30 nodes 1001 mate 3 movetime 1234".to_string();
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
    }
}