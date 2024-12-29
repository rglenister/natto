extern crate core;

use crate::bit_board::BitBoard;

mod fen;
mod board;
mod chess_move;
mod bit_board;
mod position;
mod util;
mod engine;
mod uci;
mod moves_cache;
mod search;

fn main() {
    uci::process_input::<BitBoard>();
}
