use std::collections::HashMap;
use strum::IntoEnumIterator;
use crate::chessboard::piece::PieceColor;
use crate::chessboard::piece::PieceColor::{Black, White};
use crate::chessboard::piece::PieceType::{Bishop, Knight, Pawn, Queen, Rook};
use crate::{move_generator, search};
use crate::chessboard::position::Position;

include!("chess_util/generated_macro.rs");

#[derive(Copy, Clone, Debug)]
#[derive(Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum GameStatus {
    InProgress,
    DrawnByFiftyMoveRule,
    DrawnByThreefoldRepetition,
    DrawnByInsufficientMaterial,
    Stalemate,
    Checkmate,
}

pub struct Game {
    pub position: Position,
    has_legal_move: bool,
    check_count: usize,
    historic_repeat_position_counts: Option<HashMap<u64, (Position, usize)>>,
}


impl Game {
    pub(crate) fn new(
        position: &Position,
        historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>,
    ) -> Self {
        Self {
            position: *position,
            historic_repeat_position_counts: historic_repeat_position_counts.map(|x| x.clone()),
            has_legal_move: move_generator::has_legal_move(position),
            check_count: move_generator::king_attacks_finder(position, position.side_to_move()).count_ones() as usize,
        }
    }
    pub fn get_game_status(&self) -> GameStatus {
        match (!self.has_legal_move, self.check_count > 0) {
            (true, true) => GameStatus::Checkmate,
            (true, false) => GameStatus::Stalemate,
            _ => {
                if self.position.half_move_clock() >= 100 {
                    GameStatus::DrawnByFiftyMoveRule
                } else if self.has_three_fold_repetition() {
                    GameStatus::DrawnByThreefoldRepetition
                } else if self.has_insufficient_material() {
                    GameStatus::DrawnByInsufficientMaterial
                } else {
                    GameStatus::InProgress
                }
            }
        }
    }

    pub fn has_three_fold_repetition(&self) -> bool {
        search::negamax::get_repeat_position_count(&self.position, &*vec!(), self.historic_repeat_position_counts.as_ref()) >= 3
    }
    pub fn is_check(&self) -> bool {
        self.check_count >= 1
    }

    pub fn check_count(&self) -> usize {
        self.check_count
    }

    pub fn has_insufficient_material(&self) -> bool {
        let board = self.position.board();
        let all_bitboards = &board.all_bitboards();
        for piece_color in PieceColor::iter() {
            for piece_type in [Pawn, Rook, Queen] {
                if all_bitboards[piece_color as usize][piece_type as usize] != 0 {
                    return false;
                }
            }
        }
        let whites_bishop_count = board.get_piece_count(White, Bishop);
        let blacks_bishop_count = board.get_piece_count(Black, Bishop);
        let whites_knight_count = board.get_piece_count(White, Knight);
        let blacks_knight_count = board.get_piece_count(Black, Knight);
        let whites_minor_piece_count = whites_bishop_count + whites_knight_count;
        let blacks_minor_piece_count = blacks_bishop_count + blacks_knight_count;

        if (whites_minor_piece_count <= 1) && (blacks_minor_piece_count <= 1) {
            return true;
        }
        
        if blacks_minor_piece_count == 0 && whites_minor_piece_count == 2 {
            if whites_knight_count == 2 || (whites_bishop_count == 2 && board.has_bishops_on_same_color_squares(White)) {
                return true;
            }
        } else if whites_minor_piece_count == 0 && blacks_minor_piece_count == 2 {
            if blacks_knight_count == 2 || (blacks_bishop_count == 2 && board.has_bishops_on_same_color_squares(Black)) {
                return true;
            }   
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_double_check() {
        let fen = "2r2q1k/5pp1/4p1N1/8/1bp5/5P1R/6P1/2R4K b - - 0 1";
        let position = Position::from(fen);
        let game = Game::new(&position, None);
        assert_eq!(game.is_check(), true);
        assert_eq!(game.check_count(), 2);
        assert_eq!(game.get_game_status(), GameStatus::InProgress);
        assert_eq!(game.has_legal_move, true);
//        assert_eq!(game.legal_moves[0], BasicMove { base_move: {BaseMove::new(sq!("h8"), sq!("g8"), false)}})
    }

    #[test]
    fn test_checkmate() {
        let fen = "8/8/8/5k1K/8/8/8/7r w - - 0 1";
        let position = Position::from(fen);
        let game = Game::new(&position, None);
        assert_eq!(game.is_check(), true);
        assert_eq!(game.check_count(), 1);
        assert_eq!(game.get_game_status(), GameStatus::Checkmate);
        assert_eq!(game.has_legal_move, false);
    }

    #[test]
    fn test_stalemate() {
        let fen = "7K/5k2/5n2/8/8/8/8/8 w - - 0 1";
        let position = Position::from(fen);
        let game = Game::new(&position, None);
        assert_eq!(game.is_check(), false);
        assert_eq!(game.check_count(), 0);
        assert_eq!(game.get_game_status(), GameStatus::Stalemate);
        assert_eq!(game.has_legal_move, false);
    }


    mod insufficient_material {
        use super::*;

        #[test]
        fn test_new_game() {
            let position = Position::new_game();
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), false);
        }

        #[test]
        fn test_only_kings() {
            let fen = "4k3/8/8/8/8/8/8/3K4 b - - 1 1";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), true);
        }

        #[test]
        fn test_has_one_queen() {
            let fen = "4k3/8/8/8/8/8/4q3/1K6 b - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), false);
        }
        #[test]
        fn test_has_one_rook() {
            let fen = "4k3/8/8/8/8/8/4r3/1K6 b - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), false);
        }
        #[test]
        fn test_has_one_bishop() {
            let fen = "4k3/8/8/8/8/8/4b3/1K6 b - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), true);
        }
        #[test]
        fn test_has_one_knight() {
            let fen = "4k3/8/8/8/8/8/4n3/1K6 b - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), true);
        }
        #[test]
        fn test_has_two_knights() {
            let fen = "4k3/8/8/8/8/8/n3n3/1K6 b - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), true);
        }

        #[test]
        fn test_has_two_bishops_on_same_color_squares() {
            let fen = "4k3/1b6/8/8/6b1/8/8/1K6 w - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), true);
        }

        #[test]
        fn test_has_two_bishops_on_different_color_squares() {
            let fen = "4k3/1b6/8/8/5b2/8/8/1K6 w - - 5 3";
            let position = Position::from(fen);
            let game = Game::new(&position, None);
            assert_eq!(game.has_insufficient_material(), false);
        }
    }
}
