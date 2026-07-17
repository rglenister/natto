use crate::uci::config;
use chrono::Local;
use std::fs::File;
use std::io;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub static LOG_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn configure_logging() -> Result<LoggerController, fern::InitError> {
    let logger_controller = LoggerController::new();
    setup_logging(&logger_controller)
        .map_err(|err| {
            eprintln!("Failed to initialize logging: {err:?}");
            err
        })
        .ok();
    Ok(logger_controller)
}

fn setup_logging(logger_controller: &LoggerController) -> Result<(), fern::InitError> {
    let base = fern::Dispatch::new()
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
        .filter(|_| LOG_ENABLED.load(Ordering::Relaxed)); // runtime switch

    logger_controller.chain_debug_file(base).apply()?;
    Ok(())
}
#[derive(Clone)]
pub struct LoggerController {
    debug_file: Arc<Mutex<Option<File>>>,
}

impl LoggerController {
    pub fn new() -> Self {
        Self { debug_file: Arc::new(Mutex::new(None)) }
    }

    pub fn set_debug_file(&self, path: &str) {
        let mut slot = self.debug_file.lock().unwrap();
        if path.is_empty() {
            *slot = None;
        } else if let Ok(file) = File::create(path) {
            *slot = Some(file);
        }
    }

    pub(crate) fn chain_debug_file(&self, dispatch: fern::Dispatch) -> fern::Dispatch {
        let slot = self.debug_file.clone();
        dispatch.chain(fern::Output::call(move |record| {
            if let Some(ref mut file) = *slot.lock().unwrap() {
                let _ = writeln!(file, "{}", record.args());
            }
        }))
    }
}
