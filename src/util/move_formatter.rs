use crate::core::board::Board;
use crate::core::board::BoardSide::KingSide;
use crate::core::piece::PieceType::Pawn;
use crate::core::{piece::Piece, piece::PieceType, r#move::Move};
use crate::util::move_formatter::MoveFormat::{LongAlgebraic, ShortAlgebraic};
use crate::search::negamax::SearchResults;
use phf::phf_map;
use crate::util::util;
use crate::core::position::Position;
use crate::core::move_gen::generate_moves;
use crate::eval::evaluation;
use crate::eval::evaluation::GameStatus;

include!("generated_macro.rs");

pub const SHORT_FORMATTER: MoveFormatter = MoveFormatter::new(ShortAlgebraic);
pub const LONG_FORMATTER: MoveFormatter = MoveFormatter::new(LongAlgebraic);

pub trait FormatMove {
    fn format_move_list(&self, position: &Position, chess_moves: &[Move]) -> Option<Vec<String>>;
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

pub fn format_move_list(position: &Position, search_results: &SearchResults) -> String {
    LONG_FORMATTER.format_move_list(position, &search_results.pv).unwrap().join(",")
}

impl FormatMove for MoveFormatter {
    fn format_move_list(&self, position: &Position, chess_moves: &[Move]) -> Option<Vec<String>> {
        let mut result = Vec::new();
        let mut current_position: Position = position.clone();
        for mv in chess_moves.iter() {
            let mut next_position = current_position.clone();
            next_position.make_move(mv).unwrap();
            result.push(self.format_move_internal(&current_position, &mv, &next_position));
            current_position.make_move(mv).unwrap();
        }
        Some(result)
    }
}

impl MoveFormatter {
    pub const fn new(move_format: MoveFormat) -> MoveFormatter {
        MoveFormatter { move_format }
    }

    fn format_move_internal(&self, position: &Position, mov: &Move, next_position: &Position) -> String {
        match mov {
            Move::Castling { base_move: _, board_side } => {
                if *board_side == KingSide { "0-0".to_string() } else { "0-0-0".to_string() }
            }
            _ => self.basic_format(position, &mov, &next_position)
        }
    }
    
    fn basic_format(&self, position: &Position, mov: &Move, next_position: &Position) -> String {
        format!("{}{}{}{}{}{}{}",
                self.get_piece(position, mov),
                self.get_from_square(position, mov),
                self.get_from_to_separator(mov),
                self.get_to_square(mov),
                self.get_promotion_piece(position, mov),
                self.get_en_passant_indicator(mov),
                self.get_result(next_position)
        )
    }
    fn get_piece(&self, position: &Position, mov: &Move) -> String {
        let piece = self.get_moved_piece(position, &mov);
        if piece.piece_type != PieceType::Pawn {
            PIECE_CHAR_TO_UNICODE[&piece.to_char()].to_string()
        } else {
            "".to_string()
        }
    }
    fn get_from_square(&self, position: &Position, mov: &Move) -> String {
        if self.move_format ==  ShortAlgebraic {
            let piece = self.get_moved_piece(position, mov);
            if piece.piece_type == Pawn {
                if mov.get_base_move().capture {
                    util::format_square(mov.get_base_move().from as usize).chars().nth(0).unwrap().to_string()
                } else {
                    "".to_string()
                }
            } else {
                self.get_short_algebraic_from_square_for_piece(position, mov)
            }
        } else {
            util::format_square(mov.get_base_move().from as usize)
        }
    }
    fn get_from_to_separator(&self, mov: &Move) -> String {
        if mov.get_base_move().capture {
            'x'.to_string()
        } else if self.move_format == ShortAlgebraic {
            "".to_string()
        } else {
            "-".to_string()
        }
    }

    fn get_to_square(&self, mov: &Move) -> String {
        util::format_square(mov.get_base_move().to as usize)
    }

    fn get_promotion_piece(&self, position: &Position, mov: &Move) -> String {
        match mov {
            Move::Promotion { base_move: _, promote_to } => {
                PIECE_CHAR_TO_UNICODE[&Piece { piece_color: position.side_to_move(), piece_type: *promote_to }.to_char()].to_string()
            }
            _ => String::new()
        }
    }

    fn get_en_passant_indicator(&self, mov: &Move) -> String {
        match mov {
            Move::EnPassant { base_move: _, capture_square: _ } => {
                "ep".to_string()
            }
            _ => String::new()
        }
    }

    fn get_result(&self, next_position: &Position) -> String {
        match evaluation::get_game_status(next_position, &vec!()) {
            GameStatus::Checkmate => { "#".to_string() }
            _ => "+".repeat(evaluation::check_count(next_position)).to_string()
        }
    }

    fn get_short_algebraic_from_square_for_piece(&self, position: &Position, mov: &Move) -> String {
        let moves: Vec<Move> = generate_moves(position);
        let other_moves_to_the_same_square: Vec<_> = moves
            .iter().filter(|m|
                    m.get_base_move().to == mov.get_base_move().to
                    && *m != mov
                    && self.get_moved_piece(position, mov) == self.get_moved_piece(position, m))
            .collect();

        if other_moves_to_the_same_square.is_empty() {
            String::new()
        } else {
            let algebraic = util::format_square(mov.get_base_move().from as usize);
            if other_moves_to_the_same_square.iter().filter(|m| Board::column(m.get_base_move().from as usize) == Board::column(mov.get_base_move().from as usize)).collect::<Vec<_>>().is_empty() {
                algebraic.chars().nth(0).unwrap().to_string()
            } else if other_moves_to_the_same_square.iter().filter(|m| Board::row(m.get_base_move().from as usize) == Board::row(mov.get_base_move().from as usize)).collect::<Vec<_>>().is_empty() {
                algebraic.chars().nth(1).unwrap().to_string()
            } else {
                algebraic
            }
        }
    }

    fn get_moved_piece(&self, position: &Position, chess_move: &Move) -> Piece {
        position.board().get_piece(chess_move.get_base_move().from as usize).unwrap()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opening_move() {
        let position = Position::new_game();
        let raw_moves_string = "e2e4 e7e5".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "e2-e4,e7-e5");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "e4,e5");
    }

    #[test]
    fn test_promotion_move_white() {
        let position = Position::from("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let raw_moves_string = "a7a8n".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "a7-a8\u{2658}");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "a8\u{2658}");
    }

    #[test]
    fn test_promotion_move_black() {
        let position = Position::from("4k3/P7/8/8/8/8/p7/4K3 b - - 0 1");
        let raw_moves_string = "a2a1q".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "a2-a1\u{265B}+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "a1\u{265B}+");
    }

    #[test]
    fn test_capture_move() {
        let position = Position::from("b3k3/8/8/8/8/8/8/4K2R b K - 0 1");
        let raw_moves_string = "a8h1".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265D}a8xh1");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265D}xh1");
    }

    #[test]
    fn test_en_passant_capture_move() {
        let position = Position::from("4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1");
        let raw_moves_string = "e4d3".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "e4xd3ep");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "exd3ep");
    }

    #[test]
    fn test_checking_move() {
        let position = Position::from("4k2q/8/8/8/3Pp3/8/8/4K3 b - - 0 1");
        let raw_moves_string = "h8h4".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265B}h8-h4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265B}h4+");
    }

    #[test]
    fn test_double_checking_move() {
        let position = Position::from("4k2q/8/8/3P4/4p3/2n5/1K6/8 b - - 0 1");
        let raw_moves_string = "c3a4".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265E}c3-a4++");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{265E}a4++");
    }

    #[test]
    fn test_mating_move() {
        let position = Position::from("8/8/8/8/8/4K3/6R1/4k3 w - - 0 1");
        let raw_moves_string = "g2g1".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}g2-g1#");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}g1#");
    }

    #[test]
    fn test_ambiguous_move_needs_col() {
        let position = Position::from("4k3/8/8/8/R6R/8/8/4K3 w - - 0 1");
        let raw_moves_string = "a4e4".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}a4-e4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}ae4+");
    }

    #[test]
    fn test_ambiguous_move_needs_row() {
        let position = Position::from("8/8/8/6R1/k7/6R1/8/4K3 w - - 0 1");
        let raw_moves_string = "g5g4".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}g5-g4+");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2656}5g4+");
    }

    #[test]
    fn test_ambiguous_move_needs_col_and_row() {
        let position = Position::from("8/8/8/1k6/4Q2Q/8/8/1K5Q w - - 0 1");
        let raw_moves_string = "h4e1".to_string();
        let moves = util::create_move_list(&position, raw_moves_string).unwrap();
        assert_eq!(LONG_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2655}h4-e1");
        assert_eq!(SHORT_FORMATTER.format_move_list(&position, &moves).unwrap().join(","), "\u{2655}h4e1");
    }
}
