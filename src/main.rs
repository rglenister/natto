use std::io::{self, BufRead};
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, thread};
use std::time::Duration;
use crate::bit_board::BitBoard;
pub mod fen;
pub mod node_counter;
pub mod board;
pub mod chess_move;
pub mod bit_board;
pub mod position;
pub mod util;
pub mod uci;
pub mod move_generator;
pub mod search;
pub mod game;
pub mod move_formatter;
pub mod piece_score_tables;

use fern::Dispatch;
use log::{info, debug, warn, error};
use chrono::Local;
use dirs::home_dir;
use dotenv::dotenv;
use crate::position::Position;
use crate::uci::UciGoOptions;

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

fn main() {
    setup_logging().expect("Failed to initialize logging");
    info!("Chess engine started");

    let (tx, rx) = mpsc::channel(); // Channel for UCI commands
    let stop_flag = Arc::new(AtomicBool::new(false)); // Shared stop flag
    let quit_flag = Arc::new(AtomicBool::new(false)); // Flag to exit main loop

    let mut position: Option<Position> = None;

    // Spawn input-handling thread
    let input_thread = {
        let tx = tx.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if tx.send(line).is_err() {
                    break; // Stop if main thread is gone
                }
            }
        })
    };

    let mut search_handle: Option<thread::JoinHandle<()>> = None; // Track search thread

    while !quit_flag.load(Ordering::Relaxed) {
        if let Ok(input) = rx.recv() {
            let command = UciCommand::from_input(&input);
            match command {
                UciCommand::Stop => {
                    println!("Stopping search...");
                    stop_flag.store(true, Ordering::Relaxed);
                    if let Some(handle) = search_handle.take() {
                        handle.join().unwrap(); // Ensure search thread stops
                    }

                }

                UciCommand::Quit => {
                    println!("Shutting down...");
                    stop_flag.store(true, Ordering::Relaxed);
                    quit_flag.store(true, Ordering::Relaxed);
                }

                UciCommand::Uci => {
                    let response = "id name natto\nid author Richard Glenister\nuciok";
                    println!("{}", response);
                    info!("{}", response);
                }

                UciCommand::IsReady => {
                    let response = "readyok";
                    println!("{}", response);
                    info!("{}", response);
                }

                UciCommand::UciNewGame => {
                    position = None;
                }

                UciCommand::Position(position_str) => {
                    position = uci::parse_position(&input);
                    if let Some(ref pos) = position {
                        info!("uci set position to: {}", fen::write(&pos.clone()));
                    } else {
                        error!("failed to parse position: {}", &input)
                    }
                }

                UciCommand::Go(go_options_string) => {
                    if position.is_some() {
                        let uci_go_options: UciGoOptions = uci::parse_uci_go_options(Some(input.clone()));
                        debug!("info string Setting up go - option = {:?}", uci_go_options);

                        let search_params = uci::create_search_params(&uci_go_options, position.clone().unwrap().side_to_move());
                        debug!("Starting search...");
                        stop_flag.store(false, Ordering::Relaxed); // Reset stop flag

                        let stop_flag = Arc::clone(&stop_flag);
                        search_handle = Some(thread::spawn(move || {
                            perform_search(stop_flag);
                        }));
                    } else {
                        error!("cannot search because the position has not been set");
                    }
                }

                UciCommand::None => {
                    error!("invalid UCI command");
                }
            }
        }
    }
    if let Some(handle) = search_handle {
        handle.join().unwrap(); // Ensure search finishes cleanly
    }


    debug!("Engine exited cleanly.");
    input_thread.join().unwrap(); // Ensure input thread finishes
}

/// Simulated search function that checks for stop signals.
fn perform_search(stop_flag: Arc<AtomicBool>) {
    for depth in 1..=10 {
        if stop_flag.load(Ordering::Relaxed) {
            println!("Search terminated early at depth {}", depth);
            return;
        }
        println!("Searching at depth {}", depth);
        thread::sleep(Duration::from_secs(1)); // Simulate deep search
    }
    println!("Best move: e2e4"); // Placeholder best move
}

fn setup_logging() -> Result<(), fern::InitError> {
    dotenv().ok();
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)  // Set the default log level
        .chain(std::io::stdout())        // Log to the console
        .chain(fern::log_file(env::var("LOGFILE").unwrap_or_else(|_| "engine.log".to_string()))?) // Log to a file
        .apply()?;
    Ok(())
}