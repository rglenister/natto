use thiserror::Error;
use crate::chessboard::position::Position;
use crate::r#move::RawMove;


#[derive(Debug, Error)]
#[derive(PartialEq)]
pub enum ErrorKind {
    #[error("No opening moves found")]
    NoOpeningMovesFound,
    #[error("Communications failed: {message}")]
    CommunicationsFailed { message: String },
    #[error("Invalid move string: {move_string}")]
    InvalidMoveString { move_string: String },
    #[error("Illegal move: {raw_chess_move}")]
    IllegalMove { raw_chess_move: RawMove },
    #[error("Out of book")]
    OutOfBook,
}
pub trait OpeningBook {
    fn get_opening_move(&self, position: &Position) -> Result<RawMove, ErrorKind>;
}