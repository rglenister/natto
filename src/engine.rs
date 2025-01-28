use crate::{engine, fen, search};
use std::thread;
use crate::position::Position;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use crate::chess_move::convert_chess_move_to_raw;
use crate::search::search;

pub(crate) struct Engine {

}

impl Engine {

    pub fn new() -> Self {
        Self {
            //position: None,
        }
    }

    pub fn go(&mut self, position: Position, stop_flag: Arc<AtomicBool>) -> Arc<AtomicBool> {
        let (_command_sender, command_receiver): (Sender<String>, Receiver<String>) = unbounded();
        let stop_flag_clone = Arc::clone(&stop_flag);

        // Spawn the search thread
        thread::spawn(move || {
            let results = search(&position, 0, 3);
            if let Some(chess_move) = results.best_line.first() {

                println!("bestmove {}", convert_chess_move_to_raw(chess_move))
            }
        });
        stop_flag
    }
}

