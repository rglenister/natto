use crate::board::BoardSide::KingSide;
use crate::board::{BoardSide, PieceType};
use crate::chess_move::ChessMove::{Basic, Castling, EnPassant, Promotion};
use crate::util::format_square;
use std::fmt;

include!("util/generated_macro.rs");

#[derive(Debug, PartialEq, Eq)]
#[derive(Clone, Copy, Ord, PartialOrd)]
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
#[derive(Clone, Copy, Ord, PartialOrd)]
pub enum ChessMove {
    Basic {
        base_move: BaseMove,
    },
    EnPassant {
        base_move: BaseMove,
        capture_square: usize
    },
    Promotion {
        base_move: BaseMove,
        promote_to: PieceType,
    },
    Castling {
        base_move: BaseMove,
        board_side: BoardSide,
    }
}

impl ChessMove {
    pub(crate) fn default() -> ChessMove {
        Basic { base_move: BaseMove { from: 0, to: 0, capture: false } }   
    }
}

impl ChessMove {
    pub fn get_base_move(&self) -> &BaseMove {
        match self {
            Basic { base_move }
                | EnPassant { base_move, .. }
                | Promotion { base_move, ..}
                | Castling { base_move, ..} => base_move,
        }
    }
}

impl fmt::Display for ChessMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Basic {base_move} => write!(f, "{}", write_default(base_move.from, base_move.to, base_move.capture)),
            EnPassant {base_move, capture_square: _ } => write!(f, "{}e.p", write_default(base_move.from, base_move.to, base_move.capture)),
            Promotion {base_move, promote_to} => write!(f, "{}{}", write_default(base_move.from, base_move.to, base_move.capture), promote_to),
            Castling {base_move: _ , board_side} => write!(f, "{}", if *board_side == KingSide {"0-0"} else {"0-0-0"}),
        }

    }
}
fn write_default(from: usize, to: usize, capture: bool) -> String {
    format!("{}{}{}", format_square(from), if capture { 'x' } else { '-' }, format_square(to))
}

#[derive(Debug, PartialEq, Eq)]
#[derive(Clone, Copy)]
pub struct RawChessMove {
    pub from: usize,
    pub to: usize,
    pub promote_to: Option<PieceType>,
}

impl RawChessMove {
    pub(crate) fn new(from: usize, to: usize, promote_to: Option<PieceType>) -> RawChessMove {
        RawChessMove {from, to, promote_to}
    }
}
impl fmt::Display for RawChessMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let promote_to: String = self.promote_to.map_or(String::new(), |piece_type| piece_type.first_letter().to_lowercase().to_string());
        write!(f, "{}{}{}", format_square(self.from), format_square(self.to), promote_to)
    }
}

pub fn convert_chess_moves_to_raw(moves: Vec<ChessMove>) -> Vec<RawChessMove> {
    moves.into_iter().map(|m| {
        convert_chess_move_to_raw(&m)
    }).collect()
}

pub fn convert_chess_move_to_raw(chess_move: &ChessMove) -> RawChessMove {
    let promote_to: Option<PieceType>  = match chess_move {
        Promotion { base_move: _base_move, promote_to } => { Some(*promote_to) },
        _ => None
    };
    RawChessMove::new(chess_move.get_base_move().from, chess_move.get_base_move().to, promote_to)
}

#[cfg(test)]
mod tests {
    use crate::board::PieceType::Rook;
    use crate::board::{BoardSide, PieceType};
    use crate::chess_move::ChessMove::{Basic, Castling, EnPassant, Promotion};
    use crate::chess_move::{convert_chess_moves_to_raw, BaseMove, ChessMove, RawChessMove};

    #[test]
    fn test_basic_move() {
        let basic_move = Basic { base_move: { BaseMove::new(1, 2, false)} };
        match basic_move {
            Basic {
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
        let en_passant_move = EnPassant { base_move: { BaseMove::new(1, 2, true)}, capture_square: 3 };
        match en_passant_move {
            EnPassant {
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
        let promotion_move = Promotion { base_move: {BaseMove::new(8, 0, true)}, promote_to: PieceType::Rook };
        match promotion_move {
            Promotion {
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
        let castling_move = Castling { base_move: {BaseMove::new(4, 6, false)}, board_side: BoardSide::KingSide };
        match castling_move {
            Castling {
                base_move: BaseMove { from, to, capture}, board_side } => {
                assert_eq!(from, 4);
                assert_eq!(to, 6);
                assert_eq!(capture, false);
                assert_eq!(board_side, BoardSide::KingSide);
            }
            _ => {}
        }
    }

    #[test]
    fn test_raw_chess_move() {
        let raw_raw_move = RawChessMove::new(sq!{"b1"}, sq!("c1"), None);
        assert_eq!(raw_raw_move.to_string(), "b1c1");
        let raw_raw_move = RawChessMove::new(sq!{"e7"}, sq!("e8"), Some(PieceType::Knight));
        assert_eq!(raw_raw_move.to_string(), "e7e8n");
        let raw_raw_move = RawChessMove::new(sq!{"e7"}, sq!("e8"), Some(PieceType::Bishop));
        assert_eq!(raw_raw_move.to_string(), "e7e8b");
        let raw_raw_move = RawChessMove::new(sq!{"e7"}, sq!("e8"), Some(PieceType::Rook));
        assert_eq!(raw_raw_move.to_string(), "e7e8r");
        let raw_raw_move = RawChessMove::new(sq!{"e7"}, sq!("e8"), Some(PieceType::Queen));
        assert_eq!(raw_raw_move.to_string(), "e7e8q");
    }

    #[test]
    fn test_convert_chess_moves_to_raw() {
        let moves: Vec<ChessMove> = vec![
            Basic { base_move: BaseMove { from: 1, to: 2, capture: false }},
            EnPassant { base_move: BaseMove { from: 3, to: 4, capture: false}, capture_square: 33 },
            Promotion { base_move: BaseMove { from: 5, to: 6, capture: false}, promote_to: PieceType::Rook },
            Castling { base_move: BaseMove { from: 7, to: 8, capture: false}, board_side: BoardSide::KingSide },
        ];
        let raw_moves = convert_chess_moves_to_raw(moves);
        assert_eq!(raw_moves.len(), 4);
        assert_eq!(raw_moves[0], RawChessMove::new(1, 2, None));
        assert_eq!(raw_moves[1], RawChessMove::new(3, 4, None));
        assert_eq!(raw_moves[2], RawChessMove::new(5, 6, Some(Rook)));
        assert_eq!(raw_moves[3], RawChessMove::new(7, 8, None));
    }
}