use crate::board::{Board, Piece, PieceColor, PieceType};
use strum::{IntoEnumIterator};


pub struct BitBoard {
    bit_boards: [[u64; 6]; 2],
}

impl BitBoard {
    pub fn bitboards_for_color(&self, piece_color: PieceColor) -> [u64; 6] {
        return self.bit_boards[piece_color as usize];
    }
    pub fn bitboard_all_pieces(&self) -> u64 {
        BitBoard::bitboard_by_color(&self, PieceColor::White) | BitBoard::bitboard_by_color(&self, PieceColor::Black)
    }

    pub fn bitboard_by_color_and_piece_type(&self, piece_color: PieceColor, piece_type: PieceType) -> u64 {
        self.bit_boards[piece_color as usize][piece_type as usize]
    }

    pub fn bitboard_by_color(&self, piece_color: PieceColor) -> u64 {
        self.bit_boards[piece_color as usize].iter().fold(0, |acc, x| acc | *x as u64)
    }

    pub fn row(square_index: i32) -> i32 {
        square_index / 8
    }

    pub fn col(square_index: i32) -> i32 {
        square_index % 8
    }

    pub fn rank(square_index: i32, piece_color: PieceColor) -> i32 {
        if piece_color == PieceColor::White {
            BitBoard::row(square_index)
        } else {
            7 - BitBoard::row(square_index)
        }
    }

    pub fn is_along_side(square_1: i32, square_2: i32) -> bool {
        (square_2 - square_1).abs() == 1 && BitBoard::row(square_1) == BitBoard::row(square_2)
    }

}

impl Board for BitBoard {
    fn new() -> Self {
        Self {
            bit_boards: [[0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0]]
        }
    }

    fn get_piece(&self, square_index: usize) -> Option<Piece> {
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
        let bit_board: crate::bit_board::BitBoard = crate::bit_board::BitBoard::new();
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

    #[test]
    fn test_clear() {
        let mut bit_board: BitBoard = BitBoard::new();
        let square_index = 16;
        assert!(bit_board.get_piece(square_index).is_none());
        bit_board.put_piece(square_index, Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight});
        assert!(bit_board.get_piece(square_index).is_some());
        bit_board.clear();
        assert!(bit_board.get_piece(square_index).is_none());
    }

}
