use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

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
