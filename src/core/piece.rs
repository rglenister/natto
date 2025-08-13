use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use std::ops::Not;
use strum_macros::{EnumCount as EnumCountMacro, EnumIter};

#[derive(Debug, EnumCountMacro, EnumIter, Default, Clone, Copy)]
#[repr(u8)]
#[derive(Eq, Hash, PartialEq)]
pub enum PieceColor {
    #[default]
    White = 0,
    Black = 1,
}

impl Not for PieceColor {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.opposite()
    }
}

#[derive(Clone, Debug, Copy, Ord, PartialOrd, strum_macros::Display, EnumCountMacro, EnumIter, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Piece {
    pub(crate) piece_color: PieceColor,
    pub(crate) piece_type: PieceType,
}

impl Piece {
    pub fn to_char(&self) -> char {
        let first_letter: char = if self.piece_type != PieceType::Knight {
            self.piece_type.to_string().chars().next().unwrap()
        } else {
            'N'
        };
        if self.piece_color == PieceColor::White {
            first_letter
        } else {
            first_letter.to_ascii_lowercase()
        }
    }

    pub fn from_char(piece: char) -> Result<Piece, String> {
        let piece_color = if piece.is_ascii_uppercase() { PieceColor::White } else { PieceColor::Black };
        let piece_type: PieceType = PieceType::from_char(piece)?;
        Ok(Piece { piece_type, piece_color })
    }
}

impl PieceType {
    pub fn from_char(piece: char) -> Result<PieceType, String> {
        let piece_type: PieceType = match piece.to_ascii_lowercase() {
            'p' => Pawn,
            'n' => Knight,
            'b' => Bishop,
            'r' => Rook,
            'q' => Queen,
            'k' => King,
            _ => return Err(format!("Invalid piece: {piece}")),
        };
        Ok(piece_type)
    }

    pub fn first_letter(&self) -> char {
        if *self != Knight {
            self.to_string().chars().next().unwrap()
        } else {
            'N'
        }
    }
}

impl PieceColor {
    pub fn opposite(&self) -> PieceColor {
        match self {
            White => Black,
            Black => White,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::piece::PieceColor::*;
    use crate::core::piece::{Piece, PieceType::*};

    #[test]
    fn test_piece_to_char() {
        assert_eq!(Piece { piece_color: White, piece_type: Pawn }.to_char(), 'P');
        assert_eq!(Piece { piece_color: White, piece_type: Knight }.to_char(), 'N');
        assert_eq!(Piece { piece_color: White, piece_type: Bishop }.to_char(), 'B');
        assert_eq!(Piece { piece_color: White, piece_type: Rook }.to_char(), 'R');
        assert_eq!(Piece { piece_color: White, piece_type: Queen }.to_char(), 'Q');
        assert_eq!(Piece { piece_color: White, piece_type: King }.to_char(), 'K');

        assert_eq!(Piece { piece_color: Black, piece_type: Pawn }.to_char(), 'p');
        assert_eq!(Piece { piece_color: Black, piece_type: Knight }.to_char(), 'n');
        assert_eq!(Piece { piece_color: Black, piece_type: Bishop }.to_char(), 'b');
        assert_eq!(Piece { piece_color: Black, piece_type: Rook }.to_char(), 'r');
        assert_eq!(Piece { piece_color: Black, piece_type: Queen }.to_char(), 'q');
        assert_eq!(Piece { piece_color: Black, piece_type: King }.to_char(), 'k');
    }

    #[test]
    fn test_from_char() {
        assert_eq!(Piece::from_char('K'), Result::Ok(Piece { piece_color: White, piece_type: King }));
        assert_eq!(Piece::from_char('Q'), Result::Ok(Piece { piece_color: White, piece_type: Queen }));
        assert_eq!(Piece::from_char('R'), Result::Ok(Piece { piece_color: White, piece_type: Rook }));
        assert_eq!(Piece::from_char('B'), Result::Ok(Piece { piece_color: White, piece_type: Bishop }));
        assert_eq!(Piece::from_char('N'), Result::Ok(Piece { piece_color: White, piece_type: Knight }));
        assert_eq!(Piece::from_char('P'), Result::Ok(Piece { piece_color: White, piece_type: Pawn }));

        assert_eq!(Piece::from_char('k'), Result::Ok(Piece { piece_color: Black, piece_type: King }));
        assert_eq!(Piece::from_char('q'), Result::Ok(Piece { piece_color: Black, piece_type: Queen }));
        assert_eq!(Piece::from_char('r'), Result::Ok(Piece { piece_color: Black, piece_type: Rook }));
        assert_eq!(Piece::from_char('b'), Result::Ok(Piece { piece_color: Black, piece_type: Bishop }));
        assert_eq!(Piece::from_char('n'), Result::Ok(Piece { piece_color: Black, piece_type: Knight }));
        assert_eq!(Piece::from_char('p'), Result::Ok(Piece { piece_color: Black, piece_type: Pawn }));

        assert_eq!(Piece::from_char('x'), Result::Err("Invalid piece: x".to_string()));
    }
}
