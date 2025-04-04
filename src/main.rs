extern crate core;

use crate::eval::opening_book;
use crate::eval::search;
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::{env, thread};
pub mod fen;
pub mod board;
pub mod r#move;
pub mod bit_board;
pub mod position;
pub mod uci;
pub mod util;
pub mod move_generator;
pub mod game;
pub mod move_formatter;
pub mod eval;

pub mod engine;


use crate::r#move::{convert_chess_move_to_raw, RawMove}; 
use crate::game::GameStatus::{Checkmate, Stalemate};
use crate::uci::UciGoOptions;
use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::{debug, error, info, trace, warn, LevelFilter};
use eval::opening_book::ErrorKind;
use uci::UciPosition;
use crate::engine::Engine;
use crate::eval::opening_book::{LiChessOpeningBook, OpeningBook};
use crate::eval::search::iterative_deepening_search;
use crate::eval::ttable::TRANSPOSITION_TABLE;
use crate::move_generator::generate_moves;
use crate::position::Position;
use crate::util::find_generated_move;

fn main() {
    setup_logging().expect("Failed to initialize logging");
    //    log_test_messages();
    let _ = *TRANSPOSITION_TABLE;

    info!("Chess engine started");
    
    let mut engine = Engine::new();
    engine.run();

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


