use std::io;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use regex::Regex;
use crate::{board, engine};
use crate::engine::Engine;
use crate::position::Position;

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
                println!("id name EasyChess");
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
            UciCommand::Position(fen) => {
                println!("info string Setting up position");
                engine.position(Position::from(fen.as_str()));
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
                eprintln!("info string No input receivedf");
            }
        }
    }
}

// fn process_position(input: String) -> Position {
//     Regex::new(r"position (startpos|(fen (?:\b\w+\b(?:\s+|$)){6})) (moves (a-h1-8)+)").unwrap();
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PieceColor::White;
    use crate::chess_move::ChessMove::BasicMove;
    use crate::position::NEW_GAME_FEN;

    // position [fen <fenstring> | startpos ]  moves <move1> .... <movei>
    // position startpos moves e2e4 e7e5
    //         Regex::new(r"(?P<board>[pnbrqkPNBRQK12345678/]+) (?P<side_to_move>[wb]) (?P<castling_rights>K?Q?k?q?-?) (?P<en_passant_target_square>[a-h][1-8]|-) (?P<halfmove_clock>\d+) (?P<fullmove_number>\d+)").unwrap();
    //Regex::new(r"position");
    #[test]
    fn test_regex() {
        // process_position("position startpos moves e2e4 e7e5");
        // process_position("position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 moves e2e4 e7e5");
    }
}