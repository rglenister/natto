extern crate core;

use std::io::{self};
pub mod fen;
pub mod chessboard;
pub mod r#move;
pub mod position;
pub mod uci;
pub mod chess_util;
pub mod move_generator;
pub mod game;
pub mod move_formatter;
pub mod eval;

pub mod engine;

pub mod config;

mod opening_book;
mod search;
use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::info;
use log::error;
use chess_engine::eval::node_counter::count_nodes;
use chess_engine::position::Position;
use crate::search::transposition_table::TRANSPOSITION_TABLE;
use crate::config::CONFIG;
use crate::eval::node_counter;

fn main() {
    dotenv().ok();
    eprintln!("Configuration: {:?}", *CONFIG);
    setup_logging().or_else(|err| {
        error!("Failed to initialize logging: {:?}", err);
        Err(err)
    }).ok();
    info!("Configuration: {:?}", *CONFIG);
    let _ = *TRANSPOSITION_TABLE;
    if CONFIG.perft {
        node_counter::perf_t();
    } else {
        info!("Starting engine");
        engine::run();
        info!("Engine exited cleanly");
    }
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
        .chain(io::stderr())        
        .chain(fern::log_file(CONFIG.log_file.clone())?)
        .apply()?;
    Ok(())
}