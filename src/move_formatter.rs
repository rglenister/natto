use itertools::Itertools;
use crate::position::Position;
use crate::chess_move::{BaseMove, ChessMove};
use phf::phf_map;
use crate::board::{Board, Piece, PieceType};
use crate::board::BoardSide::KingSide;
use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};
use crate::util;

include!("util/generated_macro.rs");


pub enum MoveFormat {
    Algebraic,
    LongAlgebraic,
}

static PIECE_CHAR_TO_UNICODE: phf::Map<char, char> = phf_map! {
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

pub fn format_moves(position: &Position, moves: Vec<ChessMove>, move_format: MoveFormat) -> String {
    let legal_moves: Vec<_> = moves.iter().filter_map(|m| position.make_move(m)).collect();
    let mut output = String::new();
    "".to_string()
}

pub fn format_move(position: &Position, chess_move: &ChessMove) -> String {
    match chess_move {
        CastlingMove { base_move: _ , board_side } => {
            if *board_side == KingSide {"0-0".to_string()} else {"0-0-0".to_string()}
        }
        _ => basic_format(position, chess_move)
    }
}
fn basic_format(position: &Position, chess_move: &ChessMove) -> String {
    format!("{}{}{}{}{}{}{}",
        get_piece(position, chess_move),
        get_from_square(position, chess_move),
        get_from_to_separator(position, chess_move),
        get_to_square(position, chess_move),
        get_promotion_piece(position, chess_move),
        get_en_passant_indicator(position, chess_move),
        get_result(position, chess_move)
    )
}
fn get_piece(position: &Position, chess_move: &ChessMove) -> String {
    let piece = get_moved_piece(position, chess_move);
    if piece.piece_type != PieceType::Pawn {
        PIECE_CHAR_TO_UNICODE[&piece.to_char()].to_string()
    } else {
        "".to_string()
    }
    }
fn get_from_square(position: &Position, chess_move: &ChessMove) -> String {
    util::write_square(chess_move.get_base_move().from)
}
fn get_from_to_separator(position: &Position, chess_move: &ChessMove) -> String {
    if chess_move.get_base_move().capture {'x'.to_string()} else {"-".to_string()}
}

fn get_to_square(position: &Position, chess_move: &ChessMove) -> String {
    util::write_square(chess_move.get_base_move().to)
}

fn get_promotion_piece(position: &Position, chess_move: &ChessMove) -> String {
    match chess_move {
        PromotionMove { base_move: _ , promote_to } => {
            PIECE_CHAR_TO_UNICODE[&Piece { piece_color: position.side_to_move(), piece_type: *promote_to }.to_char()].to_string()
        }
        _ => "".to_string()
    }
}

fn get_en_passant_indicator(position: &Position, chess_move: &ChessMove) -> String {
    match chess_move {
        EnPassantMove {base_move: _ , capture_square: _} => {
            "e.p".to_string()
        }
        _ => "".to_string()
    }
}

fn get_result(position: &Position, chess_move: &ChessMove) -> String {
    "".to_string()
}

fn get_moved_piece(position: &Position, chess_move: &ChessMove) -> Piece {
    position.board_unmut().get_piece(chess_move.get_base_move().from).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::position::NEW_GAME_FEN;
    use super::*;

    #[test]
    fn test_opening_move() {
        let position1 = Position::from(NEW_GAME_FEN);
        let position2 =  position1.make_raw_move(sq!("e2"), sq!("e4"), None).unwrap();
        let s = format_move(&position1, &position2.1);
        assert_eq!(s, "e2-e4");
    }

    #[test]
    fn test_knight_move() {
        let position1 = Position::from(NEW_GAME_FEN);
        let position2 =  position1.make_raw_move(sq!("g1"), sq!("f3"), None).unwrap();
        let s = format_move(&position1, &position2.1);
        println!("{}", s);
//        assert_eq!(s, "Ng1-f3");
        let position3 =  position2.0.make_raw_move(sq!("g8"), sq!("f6"), None).unwrap();
        let s = format_move(&position2.0, &position3.1);
        println!("{}", s);
//        assert_eq!(s, "ng8-f6");
        let position4 =  position3.0.make_raw_move(sq!("g8"), sq!("f6"), None).unwrap();
        let s = format_move(&position3.0, &position4.1);
        println!("{}", s);
//        assert_eq!(s, "ng8-f6");
    }
}
