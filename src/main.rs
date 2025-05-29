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

pub mod tablebase;
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
    init_tablebase();
    if CONFIG.perft {
        println!("Running perft test");
        node_counter::perf_t();
    } else {
        info!("Starting engine");
        engine::run(&CONFIG.uci_commands);
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

fn init_tablebase() {
    if CONFIG.use_tablebase {
        if !CONFIG.tablebase_dir.is_empty() {
            tablebase::syzygy::init_tablebases(&CONFIG.tablebase_dir)
                .or_else(|err| {
                    error!("Failed to initialize tablebases: {:?}", err);
                    Err(err)
                }).ok();
            info!("Tablebases initialized")
        } else {
            info!("No tablebases directory specified");
        }
    } else {
        info!("Tablebases disabled");
    }
}