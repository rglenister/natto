use crate::{engine, fen, search};
use std::thread;
use crate::position::Position;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::sleep;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::Duration;
use itertools::Itertools;
use crate::board::Board;
use crate::search::search;

pub(crate) struct Engine {
//    position: Option<Position>,
}

impl Engine {

    pub fn new() -> Self {
        Self {
            //position: None,
        }
    }

    pub fn position(&mut self, position: Option<Position>) {
        // self.position = position.clone();
        // if let Some(pos) = position {
        //     println!("{}", fen::write(&pos.clone()));
        //     println!("{}", pos.board().to_string());
        // }
    }

    pub fn go(&mut self, position: Position, stop_flag: Arc<AtomicBool>) -> Arc<AtomicBool> {
        let (_command_sender, command_receiver): (Sender<String>, Receiver<String>) = unbounded();
        let stop_flag_clone = Arc::clone(&stop_flag);

        // Spawn the search thread
        thread::spawn(move || {
            let results = search(&position, 0, 3);
            if let Some(chess_move) = results.best_line.first() {
                println!("Bestmove {}", chess_move)
            }
        });
        stop_flag
    }
}

