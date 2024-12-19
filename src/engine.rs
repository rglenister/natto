use std::thread;
use crate::board;
use crate::position::Position;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::sleep;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::Duration;

pub(crate) struct Engine<T: board::Board> {
    position: Option<Position<T>>,
}

impl<T: board::Board> Engine<T> {

    pub fn new() -> Self {
        Self {
            position: None,
        }
    }

    pub fn position(&mut self, position: Position<T>) {
        self.position = Some(position);
    }

    pub fn go(&mut self) -> Arc<AtomicBool> {
        match &self.position {
            Some(position) => {
                let (command_sender, command_receiver): (Sender<String>, Receiver<String>) = unbounded();
                let stop_flag = Arc::new(AtomicBool::new(false));
                let stop_flag_clone = Arc::clone(&stop_flag);

                // Spawn the search thread
                thread::spawn(move || {
                    Self::search_loop(command_receiver, stop_flag_clone);
                });
                stop_flag
            }
            None {

            } => todo!(),
        }
    }
    fn search_loop(command_receiver: Receiver<String>, stop_flag: Arc<AtomicBool>) {
        loop {
            println!("searching...");
            if stop_flag.load(Ordering::Relaxed) {
                println!("Search stopped!!");
                break;
            }
            sleep(Duration::from_millis(1000))
        }
    }
}

