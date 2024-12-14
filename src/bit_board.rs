use crate::board::{Board, Piece, PieceColor, PieceType};
use strum::{IntoEnumIterator};


pub struct BitBoard {
    bit_boards: [[u64; 6]; 2],
}

impl Board for BitBoard {
    fn new() -> Self {
        Self {
            bit_boards: [[0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0]]
        }
    }

    fn get_piece(&mut self, square_index: usize) -> Option<Piece> {
        let mask: u64 = 1 << square_index;
        for piece_color in PieceColor::iter() {
            for piece_type in PieceType::iter() {
                if self.bit_boards[piece_color.clone() as usize][piece_type.clone() as usize] & mask != 0 {
                    return Some(Piece { piece_color, piece_type });
                }
            }
        }
        None
    }

    fn put_piece(&mut self, square_index: usize, piece: Piece) {
        self.remove_piece(square_index);
        self.bit_boards[piece.piece_color.clone() as usize][piece.piece_type.clone() as usize] |= 1 << square_index;
    }

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece> {
        let mask: u64 = 1 << square_index;
        for piece_color in PieceColor::iter() {
            for piece_type in PieceType::iter() {
                if self.bit_boards[piece_color.clone() as usize][piece_type.clone() as usize] & mask != 0 {
                    self.bit_boards[piece_color.clone() as usize][piece_type.clone() as usize] &= !mask;
                    return Some(Piece { piece_color, piece_type })
                }
            }
        }
        None
    }

    fn clear(&mut self) {
        self.bit_boards = [[0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0]]
    }
}

#[cfg(test)]
mod tests {
//    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_get_from_empty_square() {
        let mut bit_board: crate::bit_board::BitBoard = crate::bit_board::BitBoard::new();
        assert!(bit_board.get_piece(0).is_none());
    }

    #[test]
    fn test_get() {
        let mut bit_board: crate::bit_board::BitBoard = crate::bit_board::BitBoard::new();
        let square_index = 63;
        let piece: Piece = Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight};
        bit_board.put_piece(square_index, piece);
        assert!(bit_board.get_piece(square_index).is_some());
        let retrieved_piece = bit_board.get_piece(square_index).expect("whatever");
        assert_eq!(retrieved_piece.piece_color, PieceColor::White);
        assert_eq!(retrieved_piece.piece_type, PieceType::Knight);
    }

    #[test]
    fn test_remove() {
        let mut bit_board: BitBoard = BitBoard::new();
        let square_index = 1;
        assert!(bit_board.remove_piece(square_index).is_none());
        let piece: Piece = Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight };
        bit_board.put_piece(square_index, piece.clone());
        let piece2: Piece = bit_board.remove_piece(square_index).expect("Whatever");
        assert_eq!(piece, piece2);
    }
}
