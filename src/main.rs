extern crate core;

use crate::evaluation::opening_book;
use crate::evaluation::search;
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::{env, thread};
pub mod fen;
pub mod node_counter;
pub mod board;
pub mod chess_move;
pub mod bit_board;
pub mod position;
pub mod uci;
pub mod util;
pub mod move_generator;
pub mod game;
pub mod move_formatter;
pub mod evaluation;


use crate::chess_move::{convert_chess_move_to_raw, RawChessMove};
use crate::game::GameStatus::{Checkmate, Stalemate};
use crate::uci::UciGoOptions;
use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::{debug, error, info, trace, warn, LevelFilter};
use evaluation::search::search;
use uci::UciPosition;

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
//    log_test_messages();

    info!("Chess engine started");

    let (tx, rx) = mpsc::channel(); // Channel for UCI commands
    let search_stop_flag = Arc::new(AtomicBool::new(false)); // Shared stop flag
    let main_loop_quit_flag = Arc::new(AtomicBool::new(false)); // Flag to exit main loop

    let mut uci_position: Option<UciPosition> = None;

    // Spawn input-handling thread
    let _input_thread = {
        let tx = tx.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                info!("Received from stdin: {}", line);
                if tx.send(line).is_err() {
                    break; // Stop if main thread is gone
                }
            }
        })
    };

    let mut search_handle: Option<thread::JoinHandle<()>> = None; // Track search thread

    while !main_loop_quit_flag.load(Ordering::Relaxed) {
        if let Ok(input) = rx.recv() {
            let command = UciCommand::from_input(&input);
            match command {
                UciCommand::Stop => {
                    if let Some(handle) = search_handle.take() {
                        info!("Stopping search...");
                        search_stop_flag.store(true, Ordering::Relaxed);
                        handle.join().unwrap(); // Ensure search thread stops
                        info!("Search stopped");
                    } else {
                        info!("Not currently searching");
                    }
                }

                UciCommand::Quit => {
                    info!("UCI Quit command received. Shutting down...");
                    search_stop_flag.store(true, Ordering::Relaxed);
                    main_loop_quit_flag.store(true, Ordering::Relaxed);
                }

                UciCommand::Uci => {
                    uci::send_to_gui("id name natto\nid author Richard Glenister\nuciok".to_string());
                }

                UciCommand::IsReady => {
                    uci::send_to_gui("readyok".to_string());
                }

                UciCommand::UciNewGame => {
                    uci_position = None;
                }

                UciCommand::Position(_position_str) => {
                    uci_position = uci::parse_position(&input);
                    if let Some(ref uci_pos) = uci_position {
                        info!("uci set position to [{}] from input [{}]", fen::write(&uci_pos.given_position), &input);
                    } else {
                        error!("failed to parse position from input [{}]", &input)
                    }
                }

                UciCommand::Go(_go_options_string) => {
                    if let Some(uci_pos) = uci_position.clone() {
                        if let Ok(best_move) = opening_book::get_opening_move(&fen::write(&uci_pos.given_position), 1) {
                            uci::send_to_gui(format!("bestmove {}", best_move));
                        } else {
                            let uci_go_options: UciGoOptions = uci::parse_uci_go_options(Some(input.clone()));
                            debug!("go options = {:?}", uci_go_options);

                            let search_params = uci::create_search_params(&uci_go_options, &uci_pos);
                            let repeat_position_counts = Some(util::create_repeat_position_counts(uci_pos.all_game_positions()));

                            debug!("search params = {}", search_params);
                            debug!("Starting search...");
                            search_stop_flag.store(false, Ordering::Relaxed); // Reset stop flag

                            let stop_flag = Arc::clone(&search_stop_flag);
                            search_handle = Some(thread::spawn(move || {
                                let search_results = search(&uci_pos.given_position, &search_params, stop_flag, repeat_position_counts);
                                let best_move = search_results.best_line
                                    .first()
                                    .map(|cm| convert_chess_move_to_raw(&cm.1));
                                if let Some(best_move) = best_move {
                                    uci::send_to_gui(format!("bestmove {}", best_move));
                                } else if search_results.depth == 0 {
                                    match search_results.game_status {
                                        Checkmate => { uci::send_to_gui("info score mate 0".to_string()); }
                                        Stalemate => { uci::send_to_gui("info score 0".to_string()); }
                                        _ => ()
                                    }
                                }
                            }));
                        }
                    } else {
                        error!("Cannot initiate search because the position has not been set");
                    }
                }

                UciCommand::None => {
                    error!("invalid UCI command: {:?}", input);
                }
            }
        }
    }
    if let Some(handle) = search_handle {
        handle.join().unwrap();
        info!("Search thread has stopped");
    } else {
        info!("Search thread is not running");
    }

    info!("Engine exited cleanly.");
}

fn setup_logging() -> Result<(), fern::InitError> {
    dotenv().ok();

    let default_log_level = LevelFilter::Error;
    let log_level = env::var("LOGLEVEL").unwrap_or_else(|_| default_log_level.to_string());
    let log_level = match log_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => default_log_level,
    };

    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log_level)  // Set the default log level
        .chain(std::io::stderr())        // Log to the console
        .chain(fern::log_file(env::var("LOGFILE").unwrap_or_else(|_| "natto.log".to_string()))?) // Log to a file
        .apply()?;
    Ok(())
}

fn log_test_messages() {
    info!("Logging test messages from trace to error");
    trace!("trace message");
    debug!("debug message");
    info!("info message");
    warn!("warn message");
    error!("error message");
}
