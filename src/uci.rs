use std::option::Option;
use std::io;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::{board, engine, util};
use crate::chess_move::{ChessMove, RawChessMove};
use crate::engine::Engine;
use crate::position::{Position, NEW_GAME_FEN};

include!("util/generated_macro.rs");

static UCI_POSITION_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^position (startpos|fen ([^*]+))\s?(moves (.*))?$").unwrap());
static RAW_MOVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

enum UciCommand {
    Uci,
    IsReady,
    UciNewGame,
    Position(String),
    Go(Option<String>),
    Stop,
    Quit,
    None
}

impl UciCommand {
    fn from_input(input: &str) -> Self {
        let mut parts = input.split_whitespace();
        match parts.next() {
            Some("uci") => UciCommand::Uci,
            Some("isready") => UciCommand::IsReady,
            Some("ucinewgame") => UciCommand::UciNewGame,
            Some("position") => UciCommand::Position(parts.next().unwrap().to_string()),
            Some("go") => UciCommand::Go(parts.next().map(|s| s.to_string())),
            Some("stop") => UciCommand::Stop,
            Some("quit") => UciCommand::Quit,
            _ => UciCommand::None
        }

    }

}

pub fn process_input<T: board::Board>() -> () {
    let mut engine: Engine = engine::Engine::new();
    let stdin = io::stdin();
    let mut stop_flag = Arc::new(AtomicBool::new(false));
    for line in stdin.lock().lines() {
        let input = line.expect("Failed to read line").trim().to_string();
        let command = UciCommand::from_input(&input);

        match command {
            UciCommand::Uci => {
                println!("id name Natto");
                println!("id author Richard Glenister");
                println!("uciok");
            }
            UciCommand::IsReady => {
                println!("readyok");
            }
            UciCommand::UciNewGame => {
                println!("info string Setting up new game");
                engine.position(Position::new_game());

            }
            UciCommand::Position(position_str) => {
                println!("info string Setting up position {}", position_str);
                let position = parse_position(&input);
                engine.position(position.unwrap());
            }
            UciCommand::Go(go) => {
                println!("info string Setting up go - option = {:?}", go);
                stop_flag = engine.go();
            }
            UciCommand::Stop => {
                println!("info string Stopping");
                stop_flag.store(true, Ordering::Relaxed);
            }
            UciCommand::Quit => {
                println!("info string Quitting");
            }
            UciCommand::None => {
                eprintln!("info string No input received");
            }
        }
    }
}

fn parse_position(input: &str) -> Option<Position> {
    if let Some(captures) = UCI_POSITION_REGEX.captures(input) {
        if &captures[1] == "startpos" {
            let new_game_position = Position::from(NEW_GAME_FEN);
            if let Some(moves) = captures.get(4) {
                let end_position = update_position(new_game_position, moves.as_str().to_string());
                println!("Startpos with moves: {:?}", moves);
                end_position
            } else {
                println!("Startpos with no moves");
                Some(new_game_position)
            }
        } else if let Some(fen) = captures.get(2) {
            let fen_position = Position::from(fen.as_str());
            if let Some(moves) = captures.get(4) {
                println!("FEN: {}\nMoves: {:?}", fen.as_str(), moves);
                update_position(fen_position, moves.as_str().to_string())
            } else {
                println!("FEN: {}\nNo moves provided", fen.as_str());
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
        let a = current_position.make_raw_move(rm);
        match a {
            Some((next_position, cm)) => {
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


}