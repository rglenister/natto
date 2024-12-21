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
mod magic_bitboard;
mod dpec;

fn main() {
    uci::process_input::<BitBoard>();
}
