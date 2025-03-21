use crate::bit_board::BitBoard;
use crate::board::BoardSide::KingSide;
use crate::board::PieceType::Pawn;
use crate::board::{Board, Piece, PieceType};
use crate::chess_move::ChessMove::{Castling, EnPassant, Promotion};
use crate::chess_move::{ChessMove, RawChessMove};
use crate::game::{Game, GameStatus};
use crate::move_formatter::MoveFormat::{LongAlgebraic, ShortAlgebraic};
use crate::move_generator::generate;
use crate::position::Position;
use crate::evaluation::search::SearchResults;
use crate::util;
use phf::phf_map;

include!("util/generated_macro.rs");

pub const SHORT_FORMATTER: MoveFormatter = MoveFormatter::new(ShortAlgebraic);
pub const LONG_FORMATTER: MoveFormatter = MoveFormatter::new(LongAlgebraic);

pub trait FormatMove {
    fn format_move_list(&self, position: &Position, chess_moves: &[(Position, ChessMove)]) -> Option<Vec<String>>;
}

#[derive(Eq, Hash, PartialEq)]
pub enum MoveFormat {
    ShortAlgebraic,
    LongAlgebraic,
}

const PIECE_CHAR_TO_UNICODE: phf::Map<char, char> = phf_map! {
    'P' => '\u{2659}',
    'N' => '\u{2658}',
    'B' => '\u{2657}',
    'R' => '\u{2656}',
    'Q' => '\u{2655}',
    'K' => '\u{2654}',
    'p' => '\u{265F}',
    'n' => '\u{265E}',
    'b' => '\u{265D}',
    'r' => '\u{265C}',
    'q' => '\u{265B}',
    'k' => '\u{265A}'
};

pub struct MoveFormatter {
    move_format: MoveFormat,
}

pub struct GameMove {
    game: Game,
    position: Position,
    chess_move: ChessMove,
}

pub fn format_move_list(position: &Position, search_results: &SearchResults) -> String {
    LONG_FORMATTER.format_move_list(position, &search_results.best_line).unwrap().join(",")
}

impl FormatMove for MoveFormatter {
    fn format_move_list(&self, position: &Position, chess_moves: &[(Position, ChessMove)]) -> Option<Vec<String>> {
        let game_moves: Option<Vec<GameMove>> = chess_moves.iter().try_fold(Vec::new(), |mut acc: Vec<GameMove>, &cm| {
            let pos: &Position = if !acc.is_empty() { &acc.last().unwrap().game.position.clone()} else { position };
            let next_pos: Option<(Position, ChessMove)> = pos.make_raw_move(&RawChessMove::new(cm.1.get_base_move().from, cm.1.get_base_move().to, get_promote_to(cm.1)));
            if next_pos.is_some() {
                if let Some(np) = next_pos { acc.push(GameMove::new(Game::new(&np.0, None), pos, &cm.1)) }
                Some(acc)
            } else {
                None
            }
        });
        game_moves.map(|gms| {
            gms.iter().map(|gm|
                self.format_move_internal(gm))
                    .collect::<Vec<String>>()
        })
    }
}

fn get_promote_to(chess_move: ChessMove) -> Option<PieceType> {
    match chess_move {
        Promotion { base_move: _ , promote_to} => Some(promote_to),
        _ => None
    }
}

impl GameMove {
    fn new(game: Game, position: &Position, chess_move: &ChessMove) -> Self {
        GameMove { game, position: *position, chess_move: *chess_move }
    }
}

impl MoveFormatter {
    pub const fn new(move_format: MoveFormat) -> MoveFormatter {
        MoveFormatter { move_format }
    }

    fn format_move_internal(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            Castling { base_move: _, board_side } => {
                if board_side == KingSide { "0-0".to_string() } else { "0-0-0".to_string() }
            }
            _ => self.basic_format(game_move)
        }
    }
    fn basic_format(&self, game_move: &GameMove) -> String {
        format!("{}{}{}{}{}{}{}",
                self.get_piece(game_move),
                self.get_from_square(game_move),
                self.get_from_to_separator(game_move),
                self.get_to_square(game_move),
                self.get_promotion_piece(game_move),
                self.get_en_passant_indicator(game_move),
                self.get_result(game_move)
        )
    }
    fn get_piece(&self, game_move: &GameMove) -> String {
        let piece = self.get_moved_piece(&game_move.position, &game_move.chess_move);
        if piece.piece_type != PieceType::Pawn {
            PIECE_CHAR_TO_UNICODE[&piece.to_char()].to_string()
        } else {
            "".to_string()
        }
    }
    fn get_from_square(&self, game_move: &GameMove) -> String {
        if self.move_format ==  ShortAlgebraic {
            let piece = self.get_moved_piece(&game_move.position, &game_move.chess_move);
            if piece.piece_type == Pawn {
                if game_move.chess_move.get_base_move().capture {
                    util::format_square(game_move.chess_move.get_base_move().from).chars().nth(0).unwrap().to_string()
                } else {
                    "".to_string()
                }
            } else {
                self.get_short_algebraic_from_square_for_piece(game_move)
            }
        } else {
            util::format_square(game_move.chess_move.get_base_move().from)
        }
    }
    fn get_from_to_separator(&self, game_move: &GameMove) -> String {
        if game_move.chess_move.get_base_move().capture {
            'x'.to_string()
        } else if self.move_format == ShortAlgebraic {
            "".to_string()
        } else {
            "-".to_string()
        }
    }

    fn get_to_square(&self, game_move: &GameMove) -> String {
        util::format_square(game_move.chess_move.get_base_move().to)
    }

    fn get_promotion_piece(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            Promotion { base_move: _ , promote_to } => {
                PIECE_CHAR_TO_UNICODE[&Piece { piece_color: game_move.position.side_to_move(), piece_type: promote_to }.to_char()].to_string()
            }
            _ => String::new()
        }
    }

    fn get_en_passant_indicator(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            EnPassant {base_move: _ , capture_square: _} => {
                " e.p".to_string()
            }
            _ => String::new()
        }
    }

    fn get_result(&self, game_move: &GameMove) -> String {
        match game_move.game.get_game_status() {
            GameStatus::Checkmate => { "#".to_string() }
            _ => "+".repeat(game_move.game.check_count()).to_string()
        }
    }

    fn get_short_algebraic_from_square_for_piece(&self, game_move: &GameMove) -> String {
        let cm = game_move.chess_move;
        let moves = generate(&game_move.position);
        let other_moves_to_the_same_square: Vec<_> = moves
            .iter().filter(|m|
                    m.get_base_move().to == cm.get_base_move().to
                    && **m != cm
                    && self.get_moved_piece(&game_move.position, &game_move.chess_move) == self.get_moved_piece(&game_move.position, m))
            .collect();

        if other_moves_to_the_same_square.is_empty() {
            String::new()
        } else {
            let algebraic = util::format_square(cm.get_base_move().from);
            if other_moves_to_the_same_square.iter().filter(|m| BitBoard::column(m.get_base_move().from) == BitBoard::column(cm.get_base_move().from)).collect::<Vec<_>>().is_empty() {
                algebraic.chars().nth(0).unwrap().to_string()
            } else if other_moves_to_the_same_square.iter().filter(|m| BitBoard::row(m.get_base_move().from) == BitBoard::row(cm.get_base_move().from)).collect::<Vec<_>>().is_empty() {
                algebraic.chars().nth(1).unwrap().to_string()
            } else {
                algebraic
            }
        }
    }

    fn get_moved_piece(&self, position: &Position, chess_move: &ChessMove) -> Piece {
        position.board().get_piece(chess_move.get_base_move().from).unwrap()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::NEW_GAME_FEN;

    #[test]
    fn test_opening_move() {
        let position = Position::from(NEW_GAME_FEN);
        let raw_moves_string = "e2e4 e7e5".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "e2-e4,e7-e5");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "e4,e5");
    }

    #[test]
    fn test_promotion_move_white() {
        let position = Position::from("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let raw_moves_string = "a7a8n".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "a7-a8\u{2658}");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "a8\u{2658}");
    }

    #[test]
    fn test_promotion_move_black() {
        let position = Position::from("4k3/P7/8/8/8/8/p7/4K3 b - - 0 1");
        let raw_moves_string = "a2a1q".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "a2-a1\u{265B}+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "a1\u{265B}+");
    }

    #[test]
    fn test_capture_move() {
        let position = Position::from("b3k3/8/8/8/8/8/8/4K2R b K - 0 1");
        let raw_moves_string = "a8h1".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265D}a8xh1");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265D}xh1");
    }

    #[test]
    fn test_en_passant_capture_move() {
        let position = Position::from("4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1");
        let raw_moves_string = "e4d3".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "e4xd3 e.p");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "exd3 e.p");
    }

    #[test]
    fn test_checking_move() {
        let position = Position::from("4k2q/8/8/8/3Pp3/8/8/4K3 b - - 0 1");
        let raw_moves_string = "h8h4".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265B}h8-h4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265B}h4+");
    }

    #[test]
    fn test_double_checking_move() {
        let position = Position::from("4k2q/8/8/3P4/4p3/2n5/1K6/8 b - - 0 1");
        let raw_moves_string = "c3a4".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265E}c3-a4++");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{265E}a4++");
    }

    #[test]
    fn test_mating_move() {
        let position = Position::from("8/8/8/8/8/4K3/6R1/4k3 w - - 0 1");
        let raw_moves_string = "g2g1".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}g2-g1#");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}g1#");
    }

    #[test]
    fn test_ambiguous_move_needs_col() {
        let position = Position::from("4k3/8/8/8/R6R/8/8/4K3 w - - 0 1");
        let raw_moves_string = "a4e4".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}a4-e4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}ae4+");
    }

    #[test]
    fn test_ambiguous_move_needs_row() {
        let position = Position::from("8/8/8/6R1/k7/6R1/8/4K3 w - - 0 1");
        let raw_moves_string = "g5g4".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}g5-g4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2656}5g4+");
    }

    #[test]
    fn test_ambiguous_move_needs_col_and_row() {
        let position = Position::from("8/8/8/1k6/4Q2Q/8/8/1K5Q w - - 0 1");
        let raw_moves_string = "h4e1".to_string();
        let position_move_pairs: Vec<(Position, ChessMove)> = util::replay_moves(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2655}h4-e1");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &position_move_pairs).unwrap().join(","), "\u{2655}h4e1");
    }
}
