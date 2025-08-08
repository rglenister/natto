use std::io::{self};
pub mod core;
pub mod eval;
pub mod utils;

pub mod engine;

mod book;
mod search;
use crate::eval::perf_t;
use chrono::Local;
use dotenv::dotenv;
use fern::Dispatch;
use log::error;
use log::info;

use crate::engine::config;

fn main() {
    //    println!("Debug assertions are {}", if cfg!(debug_assertions) { "enabled" } else { "disabled" });
    dotenv().ok();
    eprintln!("{}", config::get_config_as_string());
    setup_logging()
        .map_err(|err| {
            eprintln!("Failed to initialize logging: {err:?}");
            error!("Failed to initialize logging: {err:?}");
            err
        })
        .ok();
    info!("{}", config::get_config_as_string());
    //let _ = *TRANSPOSITION_TABLE;
    if config::get_perft() {
        println!("Running perft test");
        perf_t::perf_t();
    } else {
        info!("Starting engine");
        engine::uci_interface::run(&config::get_uci_commands());
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
