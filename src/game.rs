use crate::bit_board::BitBoard;
use crate::board::PieceColor;
use crate::chess_move::ChessMove;
use crate::move_generator;
use crate::position::Position;


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
    legal_moves: Vec<ChessMove>,
    check_count: isize,
}


impl Game {

    pub(crate) fn new(
        position: &Position,
    ) -> Self {
        let game = Self {
            legal_moves: move_generator::generate(&position).into_iter().filter(|cm| position.make_move(&cm).is_some()).collect::<Vec<_>>(),
            check_count: move_generator::king_attacks_finder(position, position.side_to_move()).count_ones() as isize,
        };
        game
    }
    pub fn get_game_status(legal_moves: Vec<ChessMove>, check_count: isize) -> GameStatus {
        match (legal_moves.is_empty(), check_count > 0) {
            (true, true) => GameStatus::Checkmate,
            (true, false) => GameStatus::Stalemate,
            _ => GameStatus::InProgress
        }
    }
    pub fn is_check(&self) -> bool {
        self.check_count >= 1
    }
    pub fn check_count(&self) -> isize {
        self.check_count
    }

    fn get_legal_moves(position: Position) -> Vec<ChessMove>{
        let moves = move_generator::generate(&position);
        let moves2= moves.into_iter().filter(|cm| position.make_move(cm).is_some()).collect();
        moves2
    }
}