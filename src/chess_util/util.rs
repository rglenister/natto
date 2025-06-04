use crate::chessboard::{board, piece};
use crate::chessboard::piece::PieceColor;
use crate::chessboard::piece::PieceColor::{Black, White};
use crate::r#move::{Move, RawMove};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::ops::Add;
use crate::chessboard::position::Position;

include!("generated_macro.rs");

static RAW_MOVE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

pub fn create_color(initial: &str) -> Option<PieceColor> {
    if initial == "w" { Some(White) } else if initial == "b" { Some(Black) } else { None }
}

pub fn parse_square(square: &str) -> Option<usize> {
    if square == "-" {
        None
    } else {
        let row = square.chars().nth(1).expect("Invalid square").to_digit(10).expect("Invalid square");
        let col_char = square.chars().next().expect("Invalid square");
        let col = col_char as u32 - 'a' as u32;
        Some(((row - 1) * 8 + col).try_into().unwrap())
    }
}

pub fn format_square(square_index: usize) -> String {
    if square_index < board::NUMBER_OF_SQUARES {
        ((b'a' + (square_index % 8) as u8) as char).to_string().add(&(square_index / 8 + 1).to_string())
    } else {
        "Invalid square".to_string()
    }
}

pub(crate) fn distance(square_index_1: isize, square_index_2: isize) -> isize {
    let square_1_row = square_index_1 / 8;
    let square_1_col = square_index_1 % 8;

    let square_2_row = square_index_2 / 8;
    let square_2_col = square_index_2 % 8;

    let row_difference = (square_2_row - square_1_row).abs();
    let col_difference = (square_2_col - square_1_col).abs();

    row_difference.max(col_difference)
}

pub fn on_board(square_from: isize, square_to: isize) -> bool {
    (0..64).contains(&square_to) && (square_to % 8 - square_from % 8).abs() <= 2
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
    println!("{:0x}", bitboard);
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

pub fn filter_bits<F>(bitmap: u64, filter_fn: F) -> u64 
where
    F: Fn(u64) -> bool,
{
    let mut result: u64 = 0;
    process_bits(bitmap, |index: u64| {
        if filter_fn(index) {
            result |= 1 << index;
        }
    });
    result
}

pub fn filter_moves_by_from_square(moves: Vec<Move>, from_square: usize) -> Vec<Move> {
    moves.into_iter().filter(|mov| {
        match mov {
            Move::Basic { base_move, .. } => base_move.from == from_square,
            Move::EnPassant { base_move, .. } => base_move.from == from_square,
            Move::Promotion { base_move, .. } => base_move.from == from_square,
            Move::Castling { base_move, .. } => base_move.from == from_square,
        }
    }).collect::<Vec<Move>>()
}

pub fn find_generated_move(moves: Vec<Move>, raw_chess_move: &RawMove) -> Option<Move> {
    let results = moves.into_iter().filter(|chess_move | {
        match chess_move {
            Move::Basic { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
            Move::EnPassant { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
            Move::Promotion { base_move, promote_to, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to && Some(promote_to) == raw_chess_move.promote_to.as_ref() }
            Move::Castling { base_move, .. } => { base_move.from == raw_chess_move.from && base_move.to == raw_chess_move.to }
        }
    }).collect::<Vec<Move>>();
    if results.len() > 1 { panic!("Duplicate moves found") }
    results.into_iter().next()
}

pub fn replay_moves(position: &Position, raw_moves_string: String) -> Option<Vec<(Position, Move)>> {
    let raw_moves = moves_string_to_raw_moves(raw_moves_string)?;
    let result: Option<Vec<(Position, Move)>> = raw_moves.iter().try_fold(Vec::new(), |mut acc: Vec<(Position, Move)>, rm: &RawMove| {
        let current_position = if !acc.is_empty() { &acc.last().unwrap().0.clone()} else { position };
        if let Some(next_position) = current_position.make_raw_move(rm) {
            acc.push(next_position);
            return Some(acc);
        }
        None
    });
    result
}

pub fn parse_initial_moves(raw_move_strings: Vec<String>) -> Option<Vec<RawMove>> {
    let result: Option<Vec<RawMove>> = raw_move_strings.iter().try_fold(Vec::new(), |mut acc: Vec<RawMove>, rms: &String| {
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

pub fn moves_string_to_raw_moves(moves: String) -> Option<Vec<RawMove>> {
    let moves_vec: Vec<String> = moves.split_whitespace().map(String::from).collect();
    let raw_chess_moves = parse_initial_moves(moves_vec)?;
    Some(raw_chess_moves)
}

pub fn parse_move(raw_move_string: String) -> Option<RawMove> {
    let captures = RAW_MOVE_REGEX.captures(&raw_move_string);
    captures.map(|captures| {
        let promote_to = captures.name("promote_to").map(|m| piece::PieceType::from_char(m.as_str().to_string().chars().next().unwrap()));
        RawMove::new(
            parse_square(captures.name("from").unwrap().as_str()).unwrap(),
            parse_square(captures.name("to").unwrap().as_str()).unwrap(),
            if promote_to.is_some() { Some(promote_to.unwrap().expect("REASON")) } else { None }
        )
    })
}

pub fn create_repeat_position_counts(positions: Vec<Position>) -> HashMap<u64, (Position, usize)> {
    let mut repeat_position_counts: HashMap<u64, (Position, usize)> = HashMap::new();
    for position in positions {
        repeat_position_counts.entry(position.hash_code()).or_insert((position, 0)).1 += 1;
    }
    repeat_position_counts
}

#[cfg(test)]
mod tests {
    use crate::chess_util::fen;
    use crate::chessboard::board::Board;
    use crate::chessboard::piece::{Piece, PieceType};
    use crate::chessboard::piece::PieceType::{Bishop, Knight, Queen, Rook};
    use crate::chessboard::position::NEW_GAME_FEN;
    use crate::r#move::BaseMove;
    use crate::r#move::Move::{Basic, Promotion};
    use super::*;

    #[test]
    fn test_bit_indexes() {
        let result = bit_indexes(1 << 0 | 1 << 1 | 1 << 32 | 1 << 63);
        assert_eq!(result.len(), 4);
        assert_eq!(result, vec![0, 1, 32, 63]);
    }

    #[test]
    fn test_filter_bits() {
        assert_eq!(filter_bits(!0, |index| index / 8 == 0), 1 << 0 | 1 << 1 | 1 << 2 | 1 << 3 | 1 << 4 | 1 << 5 | 1 << 6 | 1 << 7);
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
        let mut board = Board::new();
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
        let mut moves: Vec<Move> = vec![];
        moves.push(Basic {base_move: { BaseMove { from: 1, to: 2, capture: false } }});
        moves.push(Basic {base_move: { BaseMove {from: 3, to: 4, capture: false }}});
        let matched_move = find_generated_move(moves, &RawMove::new(1, 2, None));
        assert_eq!(matched_move.unwrap(), Basic {base_move: BaseMove {from: 1, to: 2, capture: false}});
    }

    #[test]
    fn test_find_generated_promotion_move() {
        let mut moves: Vec<Move> = vec![];
        moves.push(Basic {base_move: { BaseMove { from: 1, to: 2, capture: false }}});
        moves.push(Basic {base_move: { BaseMove { from: 3, to: 4, capture: false }}});
        moves.push(Promotion {base_move: { BaseMove{ from: 3, to: 9, capture: false }}, promote_to: Queen });
        moves.push(Promotion {base_move: { BaseMove{ from: 3, to: 9, capture: false }}, promote_to: Rook });
        moves.push(Promotion {base_move: { BaseMove{ from: 3, to: 9, capture: false }}, promote_to: Knight });
        let matched_move = find_generated_move(moves, &RawMove::new(3, 9, Some(Rook)));
        assert_eq!(matched_move.unwrap(), Promotion {base_move: { BaseMove{ from: 3, to: 9, capture: false }}, promote_to: Rook });
    }

    #[test]
    fn test_parse_initial_moves() {
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string())),
            Some(vec!(RawMove {from: sq!("e2"), to: sq!("e4"), promote_to: None})));
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string(), "e7e5".to_string())),
            Some(vec!(RawMove {from: sq!("e2"), to: sq!("e4"), promote_to: None}, RawMove {from: sq!("e7"), to: sq!("e5"), promote_to: None})));

        assert_eq!(
            parse_initial_moves(vec!("i2e4".to_string(), "e7e5".to_string())),
            None);
    }

    #[test]
    fn test_parse_move() {
        assert_eq!(parse_move("a1b1".to_string()).unwrap(), RawMove {from: sq!("a1"), to: sq!("b1"), promote_to: None});
        assert_eq!(parse_move("h8a1n".to_string()).unwrap(), RawMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Knight)});
        assert_eq!(parse_move("h8a1b".to_string()).unwrap(), RawMove {from: sq!("h8"), to: sq!("a1"), promote_to: Some(Bishop)});
        assert_eq!(parse_move("a1b1r".to_string()).unwrap(), RawMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Rook)});
        assert_eq!(parse_move("a1b1q".to_string()).unwrap(), RawMove {from: sq!("a1"), to: sq!("b1"), promote_to: Some(Queen)});

        assert_eq!(parse_move("a1b1k".to_string()), None);
        assert_eq!(parse_move("".to_string()), None);
        assert_eq!(parse_move("i8h8".to_string()), None);
    }

    #[test]
    fn test_repeat_position_counts() {
        let position_1 = Position::from(NEW_GAME_FEN);
        let position_2 = position_1.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap().0;
        let position_3 = Position::from(NEW_GAME_FEN);
        let repeat_position_counts = create_repeat_position_counts(vec!(position_1, position_2, position_3));

        assert_eq!(position_1, position_3);
        assert_eq!(position_1.hash_code(), position_3.hash_code());
        assert_eq!(repeat_position_counts.iter().count(), 2);
        assert_eq!(repeat_position_counts[&position_1.hash_code()], (position_1, 2));
        assert_eq!(repeat_position_counts[&position_2.hash_code()], (position_2, 1));

    }

    #[test]
    fn test_replay_moves() {
        let position = Position::from(NEW_GAME_FEN);
        let moves = "e2e4 e7e5".to_string();
        let result = replay_moves(&position, moves);
        assert!(result.is_some());
        let moves = result.unwrap();
        assert_eq!(moves.len(), 2);
        let last_position = moves.last().unwrap().0;
        let fen = fen::write(&last_position);
        assert_eq!(fen, "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2".to_string());

        let position = Position::from(NEW_GAME_FEN);
        let moves = "e2e4 e6e5".to_string();
        let result = replay_moves(&position, moves);
        assert!(result.is_none());
    }
}

