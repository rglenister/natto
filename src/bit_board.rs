use crate::board::PieceType::{King, Pawn};
use crate::board::{Board, BoardSide, Piece, PieceColor, PieceType};
use crate::util::process_bits;
use std::fmt;
use std::fmt::Write;
use strum::IntoEnumIterator;

include!("util/generated_macro.rs");

#[derive(Clone, Debug, Copy, Default)]
#[derive(PartialEq, Eq)]
pub struct BitBoard {
    bit_boards: [[u64; 6]; 2],
}

pub struct CastlingMetadata {
    pub(crate) board_side: BoardSide,
    pub(crate) king_from_square: usize,
    pub(crate) king_through_square: usize,
    pub(crate) king_to_square: usize,
    pub(crate) rook_from_square: usize,
    pub(crate) rook_to_square: usize,
}

pub const CASTLING_METADATA: [[CastlingMetadata; 2]; 2] =
    [
        [
            CastlingMetadata {
                board_side: BoardSide::KingSide, king_from_square: sq!("e1"), king_through_square: sq!("f1"), king_to_square: sq!("g1"), rook_from_square: sq!("h1"), rook_to_square: sq!("f1")
            },
            CastlingMetadata {
                board_side: BoardSide::QueenSide, king_from_square: sq!("e1"), king_through_square: sq!("d1"), king_to_square: sq!("c1"), rook_from_square: sq!("a1"), rook_to_square: sq!("d1")
            }
        ],
        [
            CastlingMetadata {
                board_side: BoardSide::KingSide, king_from_square: sq!("e8"), king_through_square: sq!("f8"), king_to_square: sq!("g8"), rook_from_square: sq!("h8"), rook_to_square: sq!("f8")
            },
            CastlingMetadata {
                board_side: BoardSide::QueenSide, king_from_square: sq!("e8"), king_through_square: sq!("d8"), king_to_square: sq!("c8"), rook_from_square: sq!("a8"), rook_to_square: sq!("d8")
            }
        ]
    ];

pub const KING_HOME_SQUARE: [usize; 2] =
    [sq!("e1"), sq!("e8")];

const KING_HOME_SQUARE_MASKS: [u64; 2] =
    [1 << sq!("e1"), 1 << sq!("e8")];

const ROOK_HOME_SQUARE_MASKS: [[u64; 2]; 2] =
    [
        [1 << sq!("h1"), 1 << sq!("a1")],
        [1 << sq!("h8"), 1 << sq!("a8")]
    ];

const CASTLING_EMPTY_SQUARE_MASKS: [[u64; 2]; 2] =
    [
        [
            (1 << sq!("f1")) | (1 << sq!("g1")),
            (1 << sq!("b1")) | (1 << sq!("c1")) | (1 << sq!("d1"))
        ],
        [
            (1 << sq!("f8")) | (1 << sq!("g8")),
            (1 << sq!("b8")) | (1 << sq!("c8")) | (1 << sq!("d8"))
        ]
    ];

impl BitBoard {
    pub fn all_bitboards(&self) -> [[u64; 6]; 2] {
        self.bit_boards
    }
    pub fn bitboards_for_color(&self, piece_color: PieceColor) -> [u64; 6] {
        self.bit_boards[piece_color as usize]
    }
    pub fn bitboard_all_pieces(&self) -> u64 {
        BitBoard::bitboard_by_color(self, PieceColor::White) | BitBoard::bitboard_by_color(self, PieceColor::Black)
    }

    pub fn bitboard_by_color_and_piece_type(&self, piece_color: PieceColor, piece_type: PieceType) -> u64 {
        self.bit_boards[piece_color as usize][piece_type as usize]
    }

    pub fn bitboard_by_color(&self, piece_color: PieceColor) -> u64 {
        self.bit_boards[piece_color as usize].iter().fold(0, |acc, x| acc | *x)
    }

    pub fn king_square(&self, piece_color: PieceColor) -> i32 {
        self.bitboard_by_color_and_piece_type(piece_color, King).trailing_zeros() as i32
    }

    pub fn white_pawn_attacks(&self) -> u64 {
        let bitboard: u64 = self.bitboard_by_color_and_piece_type(PieceColor::White, Pawn);
        let left_attacks = (bitboard & !0x0101010101010101) << 7;
        let right_attacks = (bitboard & !0x8080808080808080) << 9;
        left_attacks | right_attacks
    }

    pub fn black_pawn_attacks(&self) -> u64 {
        let bitboard: u64 = self.bitboard_by_color_and_piece_type(PieceColor::Black, Pawn);
        let left_attacks = (bitboard & !0x0101010101010101) >> 9;
        let right_attacks = (bitboard & !0x8080808080808080) >> 7;
        left_attacks | right_attacks
    }

    pub fn can_castle(&self, side_to_move: PieceColor, board_side: &BoardSide) -> bool {
        let king_home_square_mask = KING_HOME_SQUARE_MASKS[side_to_move as usize];
        let king_bitboard: u64 = self.bitboard_by_color_and_piece_type(side_to_move, King);
        if (king_bitboard & king_home_square_mask) != 0 {
            let rook_home_square_mask = ROOK_HOME_SQUARE_MASKS[side_to_move as usize][*board_side as usize];
            let rook_bitboard: u64 = self.bitboard_by_color_and_piece_type(side_to_move, PieceType::Rook);


            if rook_bitboard & rook_home_square_mask != 0 && CASTLING_EMPTY_SQUARE_MASKS[side_to_move as usize][*board_side as usize] & self.bitboard_all_pieces() == 0 {
                return true; 
            } 
        }
        false
    }

    pub fn process_pieces<F>(&self, mut func: F)
    where F: FnMut(PieceColor, PieceType, usize), {
        for piece_color in PieceColor::iter() {
            for piece_type in PieceType::iter() {
                let bitboard = self.bitboard_by_color_and_piece_type(piece_color, piece_type);
                process_bits(bitboard, |square_index| {
                    func(piece_color, piece_type, square_index.try_into().unwrap());
                });
            }
        }
    }
    
    pub fn get_piece_counts(&self) -> [[usize; 6]; 2] {
        let mut counts: [[usize; 6]; 2] = [[0; 6]; 2];
        self.process_pieces(|piece_color, piece_type, square_index| {
            counts[piece_color as usize][piece_type as usize] += 1;
        });
        counts
    }

    pub fn row(square_index: usize) -> usize {
        square_index / 8
    }

    pub fn column(square_index: usize) -> usize {
        square_index % 8
    }
    
    pub fn color(square_index: usize) -> PieceColor {
        if BitBoard::row(square_index) & 1 != BitBoard::column(square_index) & 1 {
            PieceColor::White
        } else {
            PieceColor::Black
        }
    }

    pub fn rank(square_index: usize, piece_color: PieceColor) -> usize {
        if piece_color == PieceColor::White {
            BitBoard::row(square_index)
        } else {
            7 - BitBoard::row(square_index)
        }
    }

    pub fn is_along_side(square_1: usize, square_2: usize) -> bool {
        (square_2 as i32 - square_1 as i32).abs() == 1 && BitBoard::row(square_1) == BitBoard::row(square_2)
    }
}

impl Board for BitBoard {
    fn new() -> Self {
        Self {
            bit_boards: [[0; 6]; 2]
        }
    }

    fn get_piece(&self, square_index: usize) -> Option<Piece> {
        let mask: u64 = 1 << square_index;
        for piece_color in PieceColor::iter() {
            for piece_type in PieceType::iter() {
                if self.bit_boards[piece_color as usize][piece_type.clone() as usize] & mask != 0 {
                    return Some(Piece { piece_color, piece_type });
                }
            }
        }
        None
    }

    fn put_piece(&mut self, square_index: usize, piece: Piece) {
        self.remove_piece(square_index);
        self.bit_boards[piece.piece_color as usize][piece.piece_type as usize] |= 1 << square_index;
    }

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece> {
        let mask: u64 = 1 << square_index;
        for piece_color in PieceColor::iter() {
            for piece_type in PieceType::iter() {
                if self.bit_boards[piece_color as usize][piece_type as usize] & mask != 0 {
                    self.bit_boards[piece_color as usize][piece_type as usize] &= !mask;
                    return Some(Piece { piece_color, piece_type })
                }
            }
        }
        None
    }

    fn clear(&mut self) {
        self.bit_boards = [[0; 6]; 2]
    }
}

impl fmt::Display for BitBoard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for row in (0..8).rev() {
            for col in 0..8 {
                let square_index = row * 8 + col;
                let piece = &self.get_piece(square_index);
                match piece {
                    Some(piece) => {
                        write!(f, "{}", format_args!("{}  ", piece.to_char())).expect("");
                    }
                    None => {
                        let _ = write!(f, "-  ");
                    }
                }
            }
            f.write_char('\n').unwrap()
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;
    use crate::board::PieceType::{Bishop, Knight, Queen, Rook};

    #[test]
    fn test_get_from_empty_square() {
        let bit_board: crate::bit_board::BitBoard = BitBoard::new();
        assert!(bit_board.get_piece(0).is_none());
    }

    #[test]
    fn test_get() {
        let mut bit_board: crate::bit_board::BitBoard = BitBoard::new();
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

    #[test]
    fn test_king_side_white_castling() {
        let mut bit_board: BitBoard = BitBoard::new();
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), false);
        bit_board.put_piece(4, Piece { piece_color: PieceColor::White, piece_type: PieceType::King});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), false);
        bit_board.put_piece(7, Piece { piece_color: PieceColor::White, piece_type: PieceType::Rook});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), true);

        bit_board.put_piece(6, Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), false);
        bit_board.remove_piece(6);
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), true);

        bit_board.put_piece(5, Piece { piece_color: PieceColor::White, piece_type: PieceType::Bishop});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), false);
        bit_board.remove_piece(5);
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::KingSide), true);
    }
    #[test]
    fn test_queen_side_white_castling() {
        let mut bit_board: BitBoard = BitBoard::new();
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), false);
        bit_board.put_piece(4, Piece { piece_color: PieceColor::White, piece_type: PieceType::King});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), false);
        bit_board.put_piece(0, Piece { piece_color: PieceColor::White, piece_type: PieceType::Rook});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), true);

        bit_board.put_piece(1, Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), false);
        bit_board.remove_piece(1);
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), true);

        bit_board.put_piece(2, Piece { piece_color: PieceColor::White, piece_type: PieceType::Bishop});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), false);
        bit_board.remove_piece(2);
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), true);

        bit_board.put_piece(3, Piece { piece_color: PieceColor::White, piece_type: PieceType::Queen});
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), false);
        bit_board.remove_piece(3);
        assert_eq!(bit_board.can_castle(PieceColor::White, &BoardSide::QueenSide), true);
    }

    #[test]
    fn test_king_side_black_castling() {
        let mut bit_board: BitBoard = BitBoard::new();
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), false);
        bit_board.put_piece(60, Piece { piece_color: PieceColor::Black, piece_type: PieceType::King});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), false);
        bit_board.put_piece(63, Piece { piece_color: PieceColor::Black, piece_type: PieceType::Rook});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), true);

        bit_board.put_piece(62, Piece { piece_color: PieceColor::Black, piece_type: PieceType::Knight});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), false);
        bit_board.remove_piece(62);
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), true);

        bit_board.put_piece(61, Piece { piece_color: PieceColor::Black, piece_type: PieceType::Bishop});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), false);
        bit_board.remove_piece(61);
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::KingSide), true);
    }

    #[test]
    fn test_queen_side_black_castling() {
        let mut bit_board: BitBoard = BitBoard::new();
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), false);
        bit_board.put_piece(60, Piece { piece_color: PieceColor::Black, piece_type: King});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), false);
        bit_board.put_piece(56, Piece { piece_color: PieceColor::Black, piece_type: Rook});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), true);

        bit_board.put_piece(57, Piece { piece_color: PieceColor::Black, piece_type: Knight});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), false);
        bit_board.remove_piece(57);
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), true);

        bit_board.put_piece(58, Piece { piece_color: PieceColor::Black, piece_type: Bishop});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), false);
        bit_board.remove_piece(58);
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), true);

        bit_board.put_piece(59, Piece { piece_color: PieceColor::Black, piece_type: Queen});
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), false);
        bit_board.remove_piece(59);
        assert_eq!(bit_board.can_castle(PieceColor::Black, &BoardSide::QueenSide), true);
    }

    #[test]
    fn test_equals() {
        let mut bit_board1: BitBoard = BitBoard::new();
        let mut bit_board2: BitBoard = BitBoard::new();
        assert_eq!(bit_board1, bit_board2);

        bit_board1.put_piece(57, Piece { piece_color: PieceColor::Black, piece_type: Knight});
        assert_ne!(bit_board1, bit_board2);

        bit_board2.put_piece(57, Piece { piece_color: PieceColor::Black, piece_type: Knight});
        assert_eq!(bit_board1, bit_board2);
    }

    #[test]
    fn test_color() {
        assert_eq!(BitBoard::color(0), PieceColor::Black);
        assert_eq!(BitBoard::color(1), PieceColor::White);
        assert_eq!(BitBoard::color(8), PieceColor::White);
        assert_eq!(BitBoard::color(9), PieceColor::Black);
        assert_eq!(BitBoard::color(17), PieceColor::White);
        assert_eq!(BitBoard::color(63), PieceColor::Black);
    }
}
