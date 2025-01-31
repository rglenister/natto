extern crate core;

use crate::bit_board::BitBoard;

pub mod chess_engine;
pub mod fen;

pub mod node_counter;

mod board;
pub mod chess_move;
mod bit_board;
pub mod position;
mod util;
mod engine;
mod uci;
pub mod move_generator;
pub mod search;
mod game;


pub mod move_formatter;
//mod node_count_tests;
pub mod piece_score_tables;

fn main() {
    uci::process_input::<BitBoard>();
}
