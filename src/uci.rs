
use std::io::{self, BufRead};

enum UciCommand {
    Uci,
    IsReady,
    UciNewGame,
    Position(String),
    Go(Option<String>),
    Stop,
    Quit,
    Unknown(String),
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
            Some(cmd) => UciCommand::Unknown(cmd.to_string()),
            _ => UciCommand::None
        }

    }

}

pub fn process_input() -> () {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let input = line.expect("Failed to read line").trim().to_string();
        let command = UciCommand::from_input(&input);

        match command {
            UciCommand::Uci => {
                println!("id name ThreeBitChess");
                println!("id author Richard Glenister");
                println!("uciok");
            }
            UciCommand::IsReady => {
                println!("readyok");
            }
            UciCommand::UciNewGame => {
                println!("info string Setting up new game");
            }
            UciCommand::Position(p) => {
                println!("info string Setting up position");
            }
            UciCommand::Go(go) => {
                println!("info string Setting up go");
            }
            UciCommand::Stop => {
                println!("info string Stopping");
            }
            UciCommand::Quit => {
                println!("info string Quitting");
            }
            UciCommand::Unknown(_) => {
                eprintln!("unknown command");
            }
            UciCommand::None => {
                eprintln!("info string No input receivedf");
            }
        }
    }
}
