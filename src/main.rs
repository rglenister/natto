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
use crate::search::transposition_table::TRANSPOSITION_TABLE;
use crate::eval::node_counter;

fn main() {
    dotenv().ok();
    eprintln!("{}", config::get_config_as_string());
    setup_logging().or_else(|err| {
        eprintln!("Failed to initialize logging: {:?}", err);
        error!("Failed to initialize logging: {:?}", err);
        Err(err)
    }).ok();
    info!("{}", config::get_config_as_string());
    let _ = *TRANSPOSITION_TABLE;
    if config::get_perft() {
        println!("Running perft test");
        node_counter::perf_t();
    } else {
        info!("Starting engine");
        engine::run(&config::get_uci_commands());
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
        .level(config::get_log_level())
        .chain(io::stderr())        
        .chain(fern::log_file(config::get_log_file().clone())?)
        .apply()?;
    Ok(())
}