use crate::fen;
use std::thread;
use crate::position::Position;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::sleep;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::Duration;
use crate::board::Board;

pub(crate) struct Engine {
    position: Option<Position>,
}

impl Engine {

    pub fn new() -> Self {
        Self {
            position: None,
        }
    }

    pub fn position(&mut self, position: Position) {
        self.position = Some(position.clone());
        println!("{}", fen::write(&position));
        println!("{}", position.board_unmut().to_string());
    }

    pub fn go(&mut self) -> Arc<AtomicBool> {
        match &self.position {
            Some(_position) => {
                let (_command_sender, command_receiver): (Sender<String>, Receiver<String>) = unbounded();
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
    fn search_loop(_command_receiver: Receiver<String>, stop_flag: Arc<AtomicBool>) {
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

