use std::collections::HashMap;
use std::ops::Add;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::board;
use crate::board::PieceColor;
use crate::board::PieceColor::{Black, White};
use crate::chess_move::{ChessMove, RawChessMove};
use crate::position::Position;

mod sq_macro_generator;
mod generated_macro;

include!("util/generated_macro.rs");

static RAW_MOVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

pub fn create_color(initial: &str) -> Option<PieceColor> {
    if initial == "w" { Some(White) } else if initial == "b" { Some(Black) } else { None }
}

pub fn parse_square(square: &str) -> Option<usize> {
    if square == "-" {
        None
    } else {
        let row = square.chars().nth(1).expect("Invalid square").to_digit(10).expect("Invalid square");
        let col_char = square.chars().nth(0).expect("Invalid square");
        let col = col_char as u32 - 'a' as u32;
        Some(((row - 1) * 8 + col).try_into().unwrap())
    }
}

pub fn format_square(square_index: usize) -> String {
    if square_index < board::NUMBER_OF_SQUARES {
        (('a' as u8 + (square_index % 8) as u8) as char).to_string().add(&(square_index / 8 + 1).to_string())
    } else {
        "Invalid square".to_string()
    }
}

pub(crate) fn distance(square_index_1: i32, square_index_2: i32) -> i32 {
    let square_1_row = square_index_1 / 8;
    let square_1_col = square_index_1 % 8;

    let square_2_row = square_index_2 / 8;
    let square_2_col = square_index_2 % 8;

    let row_difference = (square_2_row - square_1_row).abs();
    let col_difference = (square_2_col - square_1_col).abs();

    row_difference.max(col_difference)
}

pub fn on_board(square_from: i32, square_to: i32) -> bool {
    square_to >= 0 && square_to < 64 && (square_to % 8 - square_from % 8).abs() <= 2
}

pub fn print_bitboard(bitboard: u64) {
    for row in (0..8).rev() {
        for col in 0..8 {
            let square_index = row * 8 + col;
            let bit = (bitboard >> square_index) & 1;
            if bit == 1 {
                print!("1 ");
            } else {
                print!("- ");
            }
        }
        println!()
    }
    println!();
    println!("{:064b}", bitboard);
    println!();
}

pub fn process_bits<F>(mut bitmap: u64, mut func: F)
where F: FnMut(u64),
{
    while bitmap != 0 {
        func(bitmap.trailing_zeros() as u64);
        bitmap &= bitmap - 1;
    }
}

pub fn bit_indexes(bitmap: u64) -> Vec<u64> {
    let mut indexes: Vec<u64> = Vec::new();
    process_bits(bitmap, |index: u64| {
        indexes.push(index)
    });
    indexes
}

pub fn filter_moves_by_from_square(moves: Vec<ChessMove>, from_square: usize) -> Vec<ChessMove> {
    moves.into_iter().filter(|chess_move | {
        match chess_move {
            ChessMove::BasicMove { base_move, .. } => base_move.from == from_square,
            ChessMove::EnPassantMove { base_move, .. } => base_move.from == from_square,
            ChessMove::PromotionMove { base_move, .. } => base_move.from == from_square,
            ChessMove::CastlingMove { base_move, .. } => base_move.from == from_square,
        }
    }).collect::<Vec<ChessMove>>()
}

pub fn find_generated_move(moves: Vec<ChessMove>, raw_chess_move: &RawChessMove) -> Option<ChessMove> {
    let results = moves.into_iter().filter(|chess_move | {
        match chess_move {
            ChessMove::BasicMove { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
            ChessMove::EnPassantMove { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
            ChessMove::PromotionMove { base_move, promote_to, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to && Some(promote_to) == raw_chess_move.promote_to.as_ref() }
            ChessMove::CastlingMove { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
        }
    }).collect::<Vec<ChessMove>>();
    if results.len() > 1 { panic!("Duplicate moves found") }
    results.into_iter().next()
}

pub fn replay_moves(position: &Position, raw_moves_string: String) -> Option<Vec<(Position, ChessMove)>> {
    let raw_moves = moves_string_to_raw_moves(raw_moves_string)?;
    let result: Option<Vec<(Position, ChessMove)>> = raw_moves.iter().try_fold(Vec::new(), |mut acc: Vec<(Position, ChessMove)>, rm: &RawChessMove| {
        let current_position = if !acc.is_empty() { &acc.last().unwrap().0.clone()} else { position };
        if let Some(next_position) = current_position.make_raw_move(rm) {
            acc.push(next_position);
            return Some(acc);
        }
        None
    });
    result
}

pub fn parse_initial_moves(raw_move_strings: Vec<String>) -> Option<Vec<RawChessMove>> {
    let result: Option<Vec<RawChessMove>> = raw_move_strings.iter().try_fold(Vec::new(), |mut acc: Vec<RawChessMove>, rms: &String| {
        match parse_move(rms.clone()) {
            Some(raw_chess_move) => {
                acc.push(raw_chess_move);
                Some(acc)
            },
            None => None
        }
    });
    result
}

pub fn moves_string_to_raw_moves(moves: String) -> Option<Vec<RawChessMove>> {
    let moves_vec: Vec<String> = moves.split_whitespace().map(String::from).collect();
    let raw_chess_moves = parse_initial_moves(moves_vec)?;
    Some(raw_chess_moves)
}

pub fn parse_move(raw_move_string: String) -> Option<RawChessMove> {
    let captures = RAW_MOVE_REGEX.captures(&raw_move_string);
    captures.map(|captures| {
        let promote_to = captures.name("promote_to").map(|m| board::PieceType::from_char(m.as_str().to_string().chars().nth(0).unwrap()));
        return RawChessMove::new(
            parse_square(captures.name("from").unwrap().as_str()).unwrap(),
            parse_square(captures.name("to").unwrap().as_str()).unwrap(),
            if promote_to.is_some() { Some(promote_to.unwrap().expect("REASON")) } else { None }
        );
    })
}

pub fn create_repeat_position_counts(positions: Vec<Position>) -> HashMap<u64, (Position, usize)> {
    let mut repeat_position_counts: HashMap<u64, (Position, usize)> = HashMap::new();
    for position in positions {
        repeat_position_counts.entry(position.hash_code()).or_insert((position.clone(), 0)).1 += 1;
    }
    repeat_position_counts
}

#[cfg(test)]
mod tests {
    use crate::position::NEW_GAME_FEN;
    use crate::bit_board::BitBoard;
    use crate::board::{Board, Piece, PieceType};
    use crate::board::PieceType::{Bishop, Knight, Queen, Rook};
    use crate::chess_move::BaseMove;
    use crate::chess_move::ChessMove::{BasicMove, PromotionMove};
    use super::*;

    #[test]
    fn test_bit_indexes() {
        let result = bit_indexes(1 << 0 | 1 << 1 | 1 << 32 | 1 << 63);
        assert_eq!(result.len(), 4);
        assert_eq!(result, vec![0, 1, 32, 63]);
    }

    #[test]
    fn test_create_color() {
        assert_eq!(None, create_color("a"));
        assert_eq!(Some(Black), create_color("b"));
        assert_eq!(Some(White), create_color("w"));
    }

    #[test]
    fn test_parse_square() {
        assert_eq!(parse_square("a1").unwrap(), 0);
        assert_eq!(parse_square("a2").unwrap(), 8);
        assert_eq!(parse_square("e3").unwrap(), 20);
        assert_eq!(parse_square("h7").unwrap(), 55);
        assert_eq!(parse_square("h8").unwrap(), 63);
    }

    #[test]
    fn test_format_square() {
        assert_eq!(format_square(0), "a1");
        assert_eq!(format_square(8), "a2");
        assert_eq!(format_square(20), "e3");
        assert_eq!(format_square(62), "g8");
        assert_eq!(format_square(63), "h8");
    }

    #[test]
    fn test_distance() {
        assert_eq!(distance(0, 0), 0);
        assert_eq!(distance(0, 1), 1);
        assert_eq!(distance(6, 7), 1);
        assert_eq!(distance(7, 8), 7);
        assert_eq!(distance(60, 68), 1);
    }
    #[test]
    fn test_print_bitboard() {
        let board: u64 = 1 as u64;
        print_bitboard(board);

        let board: u64 = (1 as u64) << 63;
        print_bitboard(board);
    }

    #[test]
    fn test_print_board() {
        let mut board = BitBoard::new();
        board.put_piece(0, Piece { piece_color: White, piece_type: PieceType::Rook });
        board.put_piece(63, Piece { piece_color: Black, piece_type: PieceType::Rook });
        let string = board.to_string();
    }

    #[test]
    fn test_bit_indices() {
        let selector_mask: u64 = 0b10100101;
        let indices = bit_indexes(selector_mask);
        assert!(indices.len().eq(&4));
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(indices.contains(&5));
        assert!(indices.contains(&7));
    }

    #[test]
    fn test_count_bits() {
        let number: u64 = 0xff00fff;
        let count = number.count_ones();
        assert!(count.eq(&20));
    }

    #[test]
    fn test_find_generated_basic_move() {
        let mut moves: Vec<ChessMove> = vec![];
        moves.push(BasicMove {base_move: { BaseMove { from: 1, to: 2, capture: false, score: 0 } }});
        moves.push(BasicMove {base_move: { BaseMove {from: 3, to: 4, capture: false, score: 0 }}});
        let matched_move = find_generated_move(moves, &RawChessMove::new(1, 2, None));
        assert_eq!(matched_move.unwrap(), BasicMove {base_move: BaseMove {from: 1, to: 2, capture: false, score: 0}});
    }

    #[test]
    fn test_find_generated_promotion_move() {
        let mut moves: Vec<ChessMove> = vec![];
        moves.push(BasicMove {base_move: { BaseMove { from: 1, to: 2, capture: false, score: 0  }}});
        moves.push(BasicMove {base_move: { BaseMove { from: 3, to: 4, capture: false, score: 0 }}});
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Queen });
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Rook });
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Knight });
        let matched_move = find_generated_move(moves, &RawChessMove::new(3, 9, Some(Rook)));
        assert_eq!(matched_move.unwrap(), PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Rook });
    }

    #[test]
    fn test_parse_initial_moves() {
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string())),
            Some(vec!(RawChessMove {from: sq!("e2"), to: sq!("e4"), promote_to: None})));
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string(), "e7e5".to_string())),
            Some(vec!(RawChessMove {from: sq!("e2"), to: sq!("e4"), promote_to: None}, RawChessMove {from: sq!("e7"), to: sq!("e5"), promote_to: None})));

        assert_eq!(
            parse_initial_moves(vec!("i2e4".to_string(), "e7e5".to_string())),
            None);
    }

    #[test]
    fn test_parse_move() {
        assert_eq!(parse_move("a1b1".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: None});
        assert_eq!(parse_move("h8a1n".to_string()).unwrap(), RawChessMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Knight)});
        assert_eq!(parse_move("h8a1b".to_string()).unwrap(), RawChessMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Bishop)});
        assert_eq!(parse_move("a1b1r".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Rook)});
        assert_eq!(parse_move("a1b1q".to_string()).unwrap(), RawChessMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Queen)});

        assert_eq!(parse_move("a1b1k".to_string()), None);
        assert_eq!(parse_move("".to_string()), None);
        assert_eq!(parse_move("i8h8".to_string()), None);
    }

    #[test]
    fn test_repeat_position_counts() {
        let position_1 = Position::from(NEW_GAME_FEN);
        let position_2 = position_1.make_raw_move(&RawChessMove::new(sq!("e2"), sq!("e4"), None)).unwrap().0;
        let position_3 = Position::from(NEW_GAME_FEN);
        let repeat_position_counts = create_repeat_position_counts(vec!(position_1, position_2, position_3));

        assert_eq!(position_1, position_3);
        assert_eq!(position_1.hash_code(), position_3.hash_code());
        assert_eq!(repeat_position_counts.iter().count(), 2);
        assert_eq!(repeat_position_counts[&position_1.hash_code()], (position_1, 2));
        assert_eq!(repeat_position_counts[&position_2.hash_code()], (position_2, 1));

    }
}

