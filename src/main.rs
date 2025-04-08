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

pub mod config;


use crate::r#move::{convert_chess_move_to_raw, RawMove}; 
use crate::game::GameStatus::{Checkmate, Stalemate};
use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::{debug, error, info, trace, warn, LevelFilter};
use crate::eval::ttable::TRANSPOSITION_TABLE;
use crate::config::{Config, CONFIG};

fn main() {
    dotenv().ok();
    eprintln!("Configuration: {:?}", *CONFIG);
    setup_logging().or_else(|err| {
        error!("Failed to initialize logging: {:?}", err);
        Err(err)
    }).ok();
    info!("Configuration: {:?}", *CONFIG);
    let _ = *TRANSPOSITION_TABLE;
    info!("Starting engine");
    engine::run();
    info!("Engine exited cleanly");
}

fn setup_logging() -> Result<(), fern::InitError> {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(CONFIG.log_level)
        .chain(std::io::stderr())        
        .chain(fern::log_file(CONFIG.log_file.clone())?)
        .apply()?;
    Ok(())
}


