use crate::opening_book::lichess_book::ErrorKind;
use crate::position::Position;
use crate::r#move::RawMove;

pub trait OpeningBook {
    fn get_opening_move(&self, position: &Position) -> Result<RawMove, ErrorKind>;
}