use itertools::Itertools;
use crate::position::Position;
use crate::chess_move::{BaseMove, ChessMove};
use phf::phf_map;
use MoveFormat::{LongAlgebraic, ShortAlgebraic};
use crate::board::{Board, Piece, PieceType};
use crate::board::BoardSide::KingSide;
use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};
use crate::game::Game;
use crate::util;

include!("util/generated_macro.rs");

pub const SHORT_FORMATTER: MoveFormatter = MoveFormatter::new(ShortAlgebraic);
pub const LONG_FORMATTER: MoveFormatter = MoveFormatter::new(LongAlgebraic);

pub trait FormatMove {
    fn format_move_list(&self, position: &Position, chess_moves: Vec<ChessMove>) -> Option<Vec<String>>;
}

enum MoveFormat {
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

struct MoveFormatter {
    move_format: MoveFormat,
}

struct GameMove {
    game: Game,
    position: Position,
    chess_move: ChessMove,
}

impl FormatMove for MoveFormatter {
    fn format_move_list(&self, position: &Position, chess_moves: Vec<ChessMove>) -> Option<Vec<String>> {
        let game_moves: Option<Vec<GameMove>> = chess_moves.iter().try_fold(Vec::new(), |mut acc: Vec<GameMove>, &cm| {
            let pos: &Position = if !acc.is_empty() { &acc.last().unwrap().position.clone()} else { position };
            let next_pos: Option<(Position, ChessMove)> = pos.make_raw_move(cm.get_base_move().from, cm.get_base_move().to, get_promote_to(cm));
            if next_pos.is_some() {
                next_pos.map(|np| acc.push(GameMove::new(Game::new(&np.0), pos, &cm)));
                Some(acc)
            } else {
                return None;
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
        PromotionMove {base_move, promote_to} => Some(promote_to),
        _ => None
    }
}

impl GameMove {
    fn new(game: Game, position: &Position, chess_move: &ChessMove) -> Self {
        GameMove { game, position: position.clone(), chess_move: *chess_move }
    }
}

impl MoveFormatter {
    pub const fn new(move_format: MoveFormat) -> MoveFormatter {
        MoveFormatter { move_format }
    }

    fn format_move_internal(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            CastlingMove { base_move: _, board_side } => {
                if board_side == KingSide { "0-0".to_string() } else { "0-0-0".to_string() }
            }
            _ => self.basic_format(game_move)
        }
    }
    fn basic_format(&self, game_move: &GameMove) -> String {
        format!("{}{}{}{}{}{}{}",
                self.get_piece(&game_move),
                self.get_from_square(&game_move),
                self.get_from_to_separator(&game_move),
                self.get_to_square(&game_move),
                self.get_promotion_piece(&game_move),
                self.get_en_passant_indicator(&game_move),
                self.get_result(&game_move)
        )
    }
    fn get_piece(&self, game_move: &GameMove) -> String {
        let piece = self.get_moved_piece(game_move);
        if piece.piece_type != PieceType::Pawn {
            PIECE_CHAR_TO_UNICODE[&piece.to_char()].to_string()
        } else {
            "".to_string()
        }
    }
    fn get_from_square(&self, game_move: &GameMove) -> String {
        util::write_square(game_move.chess_move.get_base_move().from)
    }
    fn get_from_to_separator(&self, game_move: &GameMove) -> String {
        if game_move.chess_move.get_base_move().capture {'x'.to_string()} else {"-".to_string()}
    }

    fn get_to_square(&self, game_move: &GameMove) -> String {
        util::write_square(game_move.chess_move.get_base_move().to)
    }

    fn get_promotion_piece(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            PromotionMove { base_move: _ , promote_to } => {
                PIECE_CHAR_TO_UNICODE[&Piece { piece_color: game_move.position.side_to_move(), piece_type: promote_to }.to_char()].to_string()
            }
            _ => "".to_string()
        }
    }

    fn get_en_passant_indicator(&self, game_move: &GameMove) -> String {
        match game_move.chess_move {
            EnPassantMove {base_move: _ , capture_square: _} => {
                "e.p".to_string()
            }
            _ => "".to_string()
        }
    }

    fn get_result(&self, game_move: &GameMove) -> String {
        "".to_string()
    }

    fn get_moved_piece(&self, game_move: &GameMove) -> Piece {
        game_move.position.board_unmut().get_piece(game_move.chess_move.get_base_move().from).unwrap()
    }

}

// pub fn format_moves(position: &Position, moves: Vec<ChessMove>, move_format: MoveFormat) -> String {
//     let legal_moves: Vec<_> = moves.iter().filter_map(|m| position.make_move(m)).collect();
//     let mut output = String::new();
//     "".to_string()
// }


#[cfg(test)]
mod tests {
    use crate::board::PieceType::Bishop;
    use crate::move_formatter::MoveFormat::{LongAlgebraic, ShortAlgebraic};
    use crate::position::NEW_GAME_FEN;
    use super::*;

    #[test]
    fn test_opening_move() {
        let position1 = Position::from(NEW_GAME_FEN);
        let position2 =  position1.make_raw_move(sq!("e2"), sq!("e4"), None).unwrap();
        let s = LONG_FORMATTER.format_move_list(&position1, vec!(position2.1, BasicMove { base_move: { BaseMove {from: sq!("e8"), to: sq!("e5"), capture: false}}}));
        let result = s.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(0), Some("e2-e4".to_string()).as_ref());
        assert_eq!(result.get(1), Some("e8-e5".to_string()).as_ref());
    }

    #[test]
    fn test_knight_move() {
        let position1 = Position::from(NEW_GAME_FEN);
        let position2 =  position1.make_raw_move(sq!("g1"), sq!("f3"), None).unwrap();
        let s = LONG_FORMATTER.format_move_list(&position1, vec!(position2.1));
//        println!("{}", s);
        assert_eq!(s.unwrap()[0], "\u{2658}g1-f3");
        let position3 =  position2.0.make_raw_move(sq!("g8"), sq!("f6"), None).unwrap();
        let s = LONG_FORMATTER.format_move_list(&position2.0, vec!(position3.1));
//        println!("{}", s);
        assert_eq!(s.unwrap()[0], "\u{265E}g8-f6");
        let position4 =  position3.0.make_raw_move(sq!("f3"), sq!("e5"), None).unwrap();
        let s = LONG_FORMATTER.format_move_list(&position3.0, vec!(position4.1));
//        println!("{}", s);
        assert_eq!(s.unwrap()[0], "\u{2658}f3-e5");
    }

    #[test]
    fn test_get_promote_to() {
        let chess_move: ChessMove = PromotionMove {base_move: BaseMove{from: 1, to: 2, capture: false}, promote_to: Bishop};
        let promote_to = get_promote_to(chess_move);
        assert_eq!(promote_to, Some(Bishop));
    }
}
