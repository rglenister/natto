
pub enum PieceColor {
    White,
    Black
}

pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King
}

pub struct Piece {
    piece_color: PieceColor,
    piece_type: PieceType
}
pub trait Board {
    fn get_piece(&self, row: usize, col: usize) -> Piece;

    fn put_piece(&self, piece: Piece, row: usize, col: usize);

}
