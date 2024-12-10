//use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

#[derive(Debug, EnumCountMacro, EnumIter, PartialEq)]
#[derive(Clone)]
#[derive(Copy)]
#[repr(u8)]
pub enum PieceColor {
    White,
    Black
}

#[derive(Debug, PartialEq)]
#[derive(Clone)]
#[derive(EnumCountMacro, EnumIter)]
#[repr(u8)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King
}

#[derive(Debug, PartialEq)]
#[derive(Clone)]
pub struct Piece {
    pub(crate) piece_color: PieceColor,
    pub(crate) piece_type: PieceType
}
pub trait Board {

    fn new() -> Self;

    fn get_piece(&mut self, square_index: usize) -> Option<Piece>;

    fn put_piece(&mut self, square_index: usize, piece: Piece);

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece>;

    fn clear(&mut self);
}
