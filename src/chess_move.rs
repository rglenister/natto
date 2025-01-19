use std::fmt;
use itertools::Itertools;
use json5::to_string;
use crate::board::{BoardSide, PieceType};
use crate::board::BoardSide::KingSide;
use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};
use crate::util::write_square;


#[derive(Debug, PartialEq, Eq)]
#[derive(Clone, Copy)]
pub struct BaseMove {
    pub from: usize,
    pub to: usize,
    pub capture: bool,
}

impl BaseMove {
    pub(crate) fn new(from: usize, to: usize, capture: bool) -> BaseMove {
        BaseMove { from, to, capture }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[derive(Clone, Copy)]
pub enum ChessMove {
//    #[default]
    BasicMove {
        base_move: BaseMove,
    },
    EnPassantMove {
        base_move: BaseMove,
        capture_square: usize
    },
    PromotionMove{
        base_move: BaseMove,
        promote_to: PieceType,
    },
    CastlingMove {
        base_move: BaseMove,
        board_side: BoardSide,
    }
}

impl ChessMove {
    pub fn get_base_move(&self) -> &BaseMove {
        match self {
            BasicMove { base_move }
                | EnPassantMove { base_move, .. }
                | PromotionMove { base_move, ..}
                | CastlingMove { base_move, ..} => &base_move,
        }
    }
}

pub fn format_moves(moves: Vec<ChessMove>) -> String {
    moves.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ")
}

impl fmt::Display for ChessMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BasicMove {base_move} => write!(f, "{}", write_default(base_move.from, base_move.to, base_move.capture)),
            EnPassantMove {base_move, capture_square} => write!(f, "{}{}", write_default(base_move.from, base_move.to, base_move.capture), "e.p"),
            PromotionMove  {base_move,promote_to} => write!(f, "{}{}", write_default(base_move.from, base_move.to, base_move.capture), promote_to),
            CastlingMove {base_move, board_side} => write!(f, "{}", if *board_side == KingSide {"0-0"} else {"0-0-0"}),
        }

    }
}
fn write_default(from: usize, to: usize, capture: bool) -> String {
    format!("{}{}{}", write_square(from), if capture { 'x' } else { '-' }, write_square(to),)
}

pub struct RawChessMove {
    pub from: usize,
    pub to: usize,
    pub promote_to: PieceType,
}


#[cfg(test)]
mod tests {
    use crate::board::{BoardSide, PieceType};
    use crate::chess_move::BaseMove;
    use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};

    #[test]
    fn test_basic_move() {
        let basic_move = BasicMove { base_move: { BaseMove::new( 1, 2, false)} };
        match basic_move {
            BasicMove {
                base_move: BaseMove { from, to, capture },
            } => {
                assert_eq!(from, 1);
                assert_eq!(to, 2);
                assert_eq!(capture, false);
            }
            _ => {}
        }
    }

    #[test]
    fn test_en_passant_move() {
        let en_passant_move = EnPassantMove { base_move: { BaseMove::new(1, 2, true)}, capture_square: 3 };
        match en_passant_move {
            EnPassantMove {
                base_move: BaseMove { from, to, capture}, capture_square } => {
                assert_eq!(from, 1);
                assert_eq!(to, 2);
                assert_eq!(capture, true);
                assert_eq!(capture_square, 3);
            }
            _ => {}
        }
    }

    #[test]
    fn test_promotion_move() {
        let promotion_move = PromotionMove { base_move: {BaseMove::new(8, 0,  true)}, promote_to: PieceType::Rook };
        match promotion_move {
            PromotionMove {
                base_move: BaseMove { from, to, capture}, promote_to } => {
                assert_eq!(from, 8);
                assert_eq!(to, 0);
                assert_eq!(capture, true);
                assert_eq!(promote_to, PieceType::Rook);
            }
            _ => {}
        }
    }

    #[test]
    fn test_castling_move() {
        let castling_move = CastlingMove { base_move: {BaseMove::new(4, 6, false)}, board_side: BoardSide::KingSide };
        match castling_move {
            CastlingMove {
                base_move: BaseMove { from, to, capture}, board_side } => {
                assert_eq!(from, 4);
                assert_eq!(to, 6);
                assert_eq!(capture, false);
                assert_eq!(board_side, BoardSide::KingSide);
            }
            _ => {}
        }
    }
}