use crate::chess_move::ChessMove;
use crate::move_generator;
use crate::position::Position;

include!("util/generated_macro.rs");

#[derive(Copy, Clone, Debug)]
#[derive(Eq, Hash, PartialEq)]
pub enum GameStatus {
    InProgress,
    DrawnByFiftyMoveRule,
    DrawnByThreefoldRepetition,
    Stalemate,
    TimeControl,
    Resignation,
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
        let game = Self {
            position: position.clone(),
            has_legal_move: move_generator::has_legal_move(&position),//.into_iter().find(|cm| position.make_move(&cm).is_some()).is_some(),
            check_count: move_generator::king_attacks_finder(position, position.side_to_move()).count_ones() as usize,
        };
        game
    }
    pub fn get_game_status(&self) -> GameStatus {
        match (!self.has_legal_move, self.check_count > 0) {
            (true, true) => GameStatus::Checkmate,
            (true, false) => GameStatus::Stalemate,
            _ => GameStatus::InProgress
        }
    }
    pub fn is_check(&self) -> bool {
        self.check_count >= 1
    }

    pub fn check_count(&self) -> usize {
        self.check_count
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::BaseMove;
    use crate::chess_move::ChessMove::BasicMove;

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
}
