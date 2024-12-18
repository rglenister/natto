extern crate core;

use crate::bit_board::BitBoard;

mod fen;
mod board;
mod map_board;
mod moves;
mod bit_board;
mod position;
mod util;
mod engine;
mod uci;
mod move_generator;

fn main() {
    uci::process_input::<BitBoard>();
}
