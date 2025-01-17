use crate::board::{BoardSide, PieceType};

#[derive(Debug, PartialEq, Eq)]
#[derive(Clone, Copy)]
pub enum ChessMove {
//    #[default]
    BasicMove {
        from: usize,
        to: usize,
        capture: bool,
    },
    EnPassantMove {
        from: usize,
        to: usize,
        capture: bool,
        capture_square: usize
    },
    PromotionMove{
        from: usize,
        to: usize,
        capture: bool,
        promote_to: PieceType,
    },
    CastlingMove {
        from: usize,
        to: usize,
        capture: bool,
        board_side: BoardSide,
    }
}

pub struct RawChessMove {
    pub from: usize,
    pub to: usize,
    pub promote_to: PieceType,
}


#[cfg(test)]
mod tests {
    use crate::board::{BoardSide, PieceType};
    use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};

    #[test]
    fn test_basic_move() {
        let basic_move = BasicMove { from: 1, to: 2, capture: false };
        match basic_move {
            BasicMove { from, to, capture } => {
                assert_eq!(from, 1);
                assert_eq!(to, 2);
                assert_eq!(capture, false);

            }
            _ => {}
        }
    }

    #[test]
    fn test_en_passant_move() {
        let en_passant_move = EnPassantMove { from: 1, to: 2, capture: true, capture_square: 3 };
        match en_passant_move {
            EnPassantMove { from, to, capture, capture_square } => {
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
        let promotion_move = PromotionMove { from: 8, to: 0, capture: true, promote_to: PieceType::Rook };
        match promotion_move {
            PromotionMove { from, to, capture, promote_to } => {
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
        let castling_move = CastlingMove { from: 4, to: 6, capture: false, board_side: BoardSide::KingSide };
        match castling_move {
            CastlingMove { from, to, capture, board_side } => {
                assert_eq!(from, 4);
                assert_eq!(to, 6);
                assert_eq!(capture, false);
                assert_eq!(board_side, BoardSide::KingSide);
            }
            _ => {}
        }
    }
}