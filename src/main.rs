use crate::bit_board::BitBoard;
pub mod fen;
pub mod node_counter;
pub mod board;
pub mod chess_move;
pub mod bit_board;
pub mod position;
pub mod util;
pub mod engine;
pub mod uci;
pub mod move_generator;
pub mod search;
pub mod game;
pub mod move_formatter;
pub mod piece_score_tables;

fn main() {
    uci::process_input::<BitBoard>();
}
