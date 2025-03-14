use strum::IntoEnumIterator;
use crate::board::{PieceColor, PieceType};
use crate::board::PieceColor::{Black, White};
use crate::board::PieceType::{Bishop, Knight, Pawn, Queen, Rook};
use crate::move_generator;
use crate::position::Position;

include!("util/generated_macro.rs");

#[derive(Copy, Clone, Debug)]
#[derive(Eq, Hash, PartialEq)]
pub enum GameStatus {
    InProgress,
    DrawnByFiftyMoveRule,
    DrawnByThreefoldRepetition,
    DrawnByInsufficientMaterial,
    Stalemate,
    Checkmate
}

pub struct Game {
    pub position: Position,
    has_legal_move: bool,
    check_count: usize,
}


impl Game {
    pub(crate) fn new(
        position: &Position,
    ) -> Self {
        Self {
            position: *position,
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
        false
    }
    pub fn is_check(&self) -> bool {
        self.check_count >= 1
    }

    pub fn check_count(&self) -> usize {
        self.check_count
    }

    pub fn has_insufficient_material(&self) -> bool {
        let board = self.position.board();
        for piece_color in PieceColor::iter() {
            let bitboards_for_color = board.bitboards_for_color(piece_color);
            for piece_type in [Pawn, Rook, Queen] {
                if bitboards_for_color[piece_type as usize] != 0 {
                    return false;
                }
            }
        }
        let all_bitboards = board.all_bitboards();
        let white_bishop_count = u64::count_ones(all_bitboards[White as usize][Bishop as usize]);
        let black_bishop_count = u64::count_ones(all_bitboards[Black as usize][Bishop as usize]);
        let white_knight_count = u64::count_ones(all_bitboards[White as usize][Knight as usize]);
        let black_knight_count = u64::count_ones(all_bitboards[Black as usize][Knight as usize]);
        let white_minor_piece_count = white_bishop_count + white_knight_count;
        let black_minor_piece_count = black_bishop_count + black_knight_count;

        if (white_minor_piece_count <= 1) && (black_minor_piece_count <= 1) {
            return true;
        }
        
        if (black_minor_piece_count == 0 && white_minor_piece_count == 2 && white_knight_count == 2) || 
            (white_minor_piece_count == 0 && black_minor_piece_count == 2 && black_knight_count == 2) {
            return true;
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
        let game = Game::new(&position);
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
        let game = Game::new(&position);
        assert_eq!(game.is_check(), true);
        assert_eq!(game.check_count(), 1);
        assert_eq!(game.get_game_status(), GameStatus::Checkmate);
        assert_eq!(game.has_legal_move, false);
    }

    #[test]
    fn test_stalemate() {
        let fen = "7K/5k2/5n2/8/8/8/8/8 w - - 0 1";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.is_check(), false);
        assert_eq!(game.check_count(), 0);
        assert_eq!(game.get_game_status(), GameStatus::Stalemate);
        assert_eq!(game.has_legal_move, false);
    }

    
    #[test]
    fn test_draw_by_insufficient_material_new_game() {
        let position = Position::new_game();
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), false);
    }

    #[test]
    fn test_draw_by_insufficient_material_only_kings() {
        let fen = "4k3/8/8/8/8/8/8/3K4 b - - 1 1";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), true);
    }

    #[test]
    fn test_draw_by_insufficient_material_has_queen() {
        let fen = "4k3/8/8/8/8/8/4q3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), false);
    }
    #[test]
    fn test_draw_by_insufficient_material_has_rook() {
        let fen = "4k3/8/8/8/8/8/4r3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), false);
    }
    #[test]
    fn test_draw_by_insufficient_material_has_bishop() {
        let fen = "4k3/8/8/8/8/8/4b3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), true);
    }
    #[test]
    fn test_draw_by_insufficient_material_has_knight() {
        let fen = "4k3/8/8/8/8/8/4n3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), true);
    }
    #[test]
    fn test_draw_by_insufficient_material_has_two_knights() {
        let fen = "4k3/8/8/8/8/8/n3n3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), true);
    }

    #[test]
    fn test_draw_by_insufficient_material_has_two_bishops() {
        let fen = "4k3/8/8/8/8/8/b3b3/1K6 b - - 5 3";
        let position = Position::from(fen);
        let game = Game::new(&position);
        assert_eq!(game.has_insufficient_material(), false);
    }
}
