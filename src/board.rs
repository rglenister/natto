use std::fmt::Write;
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};
use crate::board::PieceColor::{Black, White};
use crate::board::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};

pub(crate) static NUMBER_OF_SQUARES: usize = 64;

#[derive(Debug, EnumCountMacro, EnumIter)]
#[derive(Clone)]
#[derive(Copy)]
#[repr(u8)]
#[derive(Eq, Hash, PartialEq)]
pub enum PieceColor {
    White = 0,
    Black = 1,
}

#[derive(Clone, Debug, Copy)]
#[derive(strum_macros::Display)]
#[derive(EnumCountMacro, EnumIter)]
#[derive(Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King
}

#[derive(Copy, Clone, Debug)]
#[derive(strum_macros::Display)]
#[derive(EnumCountMacro, EnumIter)]
#[derive(Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum BoardSide {
    KingSide = 0,
    QueenSide = 1,
}

#[derive(Debug, PartialEq)]
#[derive(Clone)]
pub struct Piece {
    pub(crate) piece_color: PieceColor,
    pub(crate) piece_type: PieceType
}

impl Piece {
    pub fn to_char(&self) -> char {
        let first_letter: char =
            if self.piece_type != Knight {
                self.piece_type.to_string().chars().next().unwrap()
            } else {
                'N'
            };
        if self.piece_color == White { first_letter } else { first_letter.to_ascii_lowercase() }
    }

    pub fn from_char(piece: char) -> Result<Piece, String>{
        let piece_color = if piece.is_ascii_uppercase() { White } else { Black };
        let piece_type: PieceType = PieceType::from_char(piece)?;
        Ok(Piece {piece_type, piece_color})
    }
}

impl PieceType {
    pub fn from_char(piece: char) -> Result<PieceType, String>{
        let piece_type: PieceType = match piece.to_ascii_lowercase() {
            'p' => Pawn,
            'n' => Knight,
            'b' => Bishop,
            'r' => Rook,
            'q' => Queen,
            'k' => King,
            _ => return Err(format!("Invalid piece: {}", piece)),
        };
        Ok(piece_type)
    }
}

pub trait Board {

    fn new() -> Self where Self: Sized;

    fn get_piece(&self, square_index: usize) -> Option<Piece>;

    fn put_piece(&mut self, square_index: usize, piece: Piece);

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece>;

    fn clear(&mut self);

    fn to_string(&self) -> String {
        let mut s = String::new();
        for row in (0..8).rev() {
            for col in 0..8 {
                let square_index = row * 8 + col;
                let piece = &self.get_piece(square_index);
                match piece {
                    Some(piece) => {
                        write!(s, "{}", format_args!("{}  ", piece.to_char())).expect("");
                    }
                    None => {
                        let _ = write!(s, "-  ");
                    }
                }
            }
            s.write_char('\n').unwrap()
        }
        return s;
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{Piece, PieceType::*};
    use crate::board::PieceColor::*;

    #[test]
    fn test_piece_to_char() {
        assert_eq!(Piece {piece_color: White, piece_type: Pawn}.to_char(), 'P');
        assert_eq!(Piece {piece_color: White, piece_type: Knight}.to_char(), 'N');
        assert_eq!(Piece {piece_color: White, piece_type: Bishop}.to_char(), 'B');
        assert_eq!(Piece {piece_color: White, piece_type: Rook}.to_char(), 'R');
        assert_eq!(Piece {piece_color: White, piece_type: Queen}.to_char(), 'Q');
        assert_eq!(Piece {piece_color: White, piece_type: King}.to_char(), 'K');

        assert_eq!(Piece {piece_color: Black, piece_type: Pawn}.to_char(), 'p');
        assert_eq!(Piece {piece_color: Black, piece_type: Knight}.to_char(), 'n');
        assert_eq!(Piece {piece_color: Black, piece_type: Bishop}.to_char(), 'b');
        assert_eq!(Piece {piece_color: Black, piece_type: Rook}.to_char(), 'r');
        assert_eq!(Piece {piece_color: Black, piece_type: Queen}.to_char(), 'q');
        assert_eq!(Piece {piece_color: Black, piece_type: King}.to_char(), 'k');
    }

    #[test]
    fn test_from_char() {
        assert_eq!(Piece::from_char('K'), Result::Ok(Piece {piece_color: White, piece_type: King}));
        assert_eq!(Piece::from_char('Q'), Result::Ok(Piece {piece_color: White, piece_type: Queen}));
        assert_eq!(Piece::from_char('R'), Result::Ok(Piece {piece_color: White, piece_type: Rook}));
        assert_eq!(Piece::from_char('B'), Result::Ok(Piece {piece_color: White, piece_type: Bishop}));
        assert_eq!(Piece::from_char('N'), Result::Ok(Piece {piece_color: White, piece_type: Knight}));
        assert_eq!(Piece::from_char('P'), Result::Ok(Piece {piece_color: White, piece_type: Pawn}));

        assert_eq!(Piece::from_char('k'), Result::Ok(Piece {piece_color: Black, piece_type: King}));
        assert_eq!(Piece::from_char('q'), Result::Ok(Piece {piece_color: Black, piece_type: Queen}));
        assert_eq!(Piece::from_char('r'), Result::Ok(Piece {piece_color: Black, piece_type: Rook}));
        assert_eq!(Piece::from_char('b'), Result::Ok(Piece {piece_color: Black, piece_type: Bishop}));
        assert_eq!(Piece::from_char('n'), Result::Ok(Piece {piece_color: Black, piece_type: Knight}));
        assert_eq!(Piece::from_char('p'), Result::Ok(Piece {piece_color: Black, piece_type: Pawn}));

        assert_eq!(Piece::from_char('x'), Result::Err("Invalid piece: x".to_string()));

    }

}