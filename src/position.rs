use crate::board::{Board, PieceColor};
use crate::fen;

pub(crate) const NEW_GAME_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub(crate) struct Position<T: Board> {
    board: T,
    side_to_move: PieceColor,
    castling_rights: String,
    en_passant_target: Option<usize>,
    half_move_clock: usize,
    full_move_number: usize,
}

impl<T: Board> From<&str> for Position<T> {
    fn from(fen: &str) -> Self {
        fen::parse(fen.to_string())
    }
}

impl<T: Board> Position<T> {
    pub(crate) fn new(
        board: T,
        side_to_move: PieceColor,
        castling_rights: String,
        en_passant_target: Option<usize>,
        half_move_clock: usize,
        full_move_number: usize,
    ) -> Self {
        Self {
            board,
            side_to_move,
            castling_rights,
            en_passant_target,
            half_move_clock,
            full_move_number,
        }
    }

    pub fn new_game() -> Position<T> {
        Position::from(NEW_GAME_FEN)
    }

    pub(crate) fn board(&mut self) -> &T {
        &self.board
    }

    pub fn side_to_move(&self) -> PieceColor {
        self.side_to_move
    }

    pub fn castling_rights(&self) -> String {
        self.castling_rights.clone()
    }

    pub fn en_passant_target(&self) -> Option<usize> {
        self.en_passant_target.clone()
    }

    pub fn half_move_clock(&self) -> usize {
        self.half_move_clock
    }

    pub fn full_move_number(&self) -> usize {
        self.full_move_number
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PieceColor;
    use crate::map_board::MapBoard;

    #[test]
    fn test_general_usability() {
        let mut position: Position<MapBoard> =
            Position::new(
                MapBoard::new(),
                PieceColor::Black,
                "KQkq".to_string(),
                Some(31),
                99,
                50);

        assert!(position.board.get_piece(3).is_none());
        assert_eq!(position.side_to_move(), PieceColor::Black);
        assert_eq!(position.castling_rights(), "KQkq".to_string());
        assert_eq!(position.en_passant_target(), Some(31));
        assert_eq!(position.half_move_clock(), 99);
        assert_eq!(position.full_move_number(), 50);
    }
}
