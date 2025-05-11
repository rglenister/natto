extern crate core;

use std::io::{self};
pub mod fen;
pub mod chessboard;
pub mod r#move;
pub mod position;
pub mod uci;
pub mod util;
pub mod move_generator;
pub mod game;
pub mod move_formatter;
pub mod eval;

pub mod engine;

pub mod config;


use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::info;
use log::error;
use crate::eval::ttable::TRANSPOSITION_TABLE;
use crate::config::CONFIG;

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
        .chain(io::stderr())        
        .chain(fern::log_file(CONFIG.log_file.clone())?)
        .apply()?;
    Ok(())
}