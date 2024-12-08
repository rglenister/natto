use crate::board::PieceType;

struct BasicMove {
    from: usize,
    to: usize,
    capture: bool,
}

struct EnPassantMove {
    basic_move: BasicMove,
    capture_square: usize,
}

struct PromotionMove {
    basic_move: BasicMove,
    promote_to: PieceType,
}
struct CastlingMove {
    basic_move: BasicMove,
}

trait Move {
    fn from(&self) -> usize;

    fn to(&self) -> usize;

    fn capture(&self) -> bool;

}

impl BasicMove {
    fn new(from: usize, to: usize, capture: bool) -> BasicMove {
        BasicMove { from, to, capture }
    }
}
impl Move for BasicMove {
    fn from(&self) -> usize { self.from }
    fn to(&self) -> usize { self.to }
    fn capture(&self) -> bool { self.capture }

}

impl EnPassantMove {
    fn new(from: usize, to: usize, capture_square: usize) -> EnPassantMove {
        EnPassantMove {
            basic_move: BasicMove { from, to, capture: false},
            capture_square
        }
    }

    fn capture_square(&self) -> usize { self.capture_square }
}

 impl Move for EnPassantMove {
     fn from(&self) -> usize { self.basic_move.from() }
     fn to(&self) -> usize { self.basic_move.to() }
     fn capture(&self) -> bool { true }
 }

impl PromotionMove {
    fn new(from: usize, to: usize, capture: bool, promote_to: PieceType) -> PromotionMove {
        PromotionMove {
            basic_move: BasicMove { from, to, capture},
            promote_to,
        }
    }

    fn promote_to(&self) -> PieceType { self.promote_to.clone() }
}
impl Move for PromotionMove {
    fn from(&self) -> usize { self.basic_move.from() }
    fn to(&self) -> usize { self.basic_move.to() }
    fn capture(&self) -> bool { self.basic_move.capture() }
}

impl CastlingMove {
    fn new(from: usize, to: usize) -> CastlingMove {
        CastlingMove {
            basic_move: BasicMove { from, to, capture: false},
        }
    }
}
impl Move for CastlingMove {
    fn from(&self) -> usize { self.basic_move.from() }
    fn to(&self) -> usize { self.basic_move.to() }
    fn capture(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
    use crate::Move::EnPassantMove;
    use super::*;
    #[test]
    fn test_basic_move() {
        let basic_move= BasicMove::new(1, 2, false);
        assert_eq!(basic_move.from(), 1);
        assert_eq!(basic_move.to(), 2);
        assert_eq!(basic_move.capture(), false);
    }

    #[test]
    fn test_en_passant_move() {
        let en_passant_move= EnPassantMove::new(1, 2, 3);
        assert_eq!(en_passant_move.from(), 1);
        assert_eq!(en_passant_move.to(), 2);
        assert_eq!(en_passant_move.capture(), true);
        assert_eq!(en_passant_move.capture_square(), 3);
    }

    #[test]
    fn test_promotion_move() {
        let promotion_move = PromotionMove::new(1, 2, true, PieceType::Rook);
        assert_eq!(promotion_move.from(), 1);
        assert_eq!(promotion_move.to(), 2);
        assert_eq!(promotion_move.capture(), true);
        assert_eq!(promotion_move.promote_to(), PieceType::Rook);
    }

    #[test]
    fn test_castling_move() {
        let castling_move = CastlingMove::new(1, 2);
        assert_eq!(castling_move.from(), 1);
        assert_eq!(castling_move.to(), 2);
        assert_eq!(castling_move.capture(), false);
    }
}