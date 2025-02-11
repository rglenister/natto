use std::ops::Add;
use crate::board;
use crate::board::{PieceColor, PieceType};
use crate::board::PieceColor::{Black, White};
use crate::chess_move::ChessMove;

mod sq_macro_generator;
mod generated_macro;

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

pub fn process_bits<F>(mut bitmap: u64, mut func: F) -> ()
where F: FnMut(u64) -> (),
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

pub fn find_generated_move(moves: Vec<ChessMove>, from_square: usize, to_square: usize, promote_to_option: Option<PieceType>) -> Vec<ChessMove> {
    moves.into_iter().filter(|chess_move | {
        match chess_move {
            ChessMove::BasicMove { base_move, .. } => { base_move.from == from_square && base_move.to == to_square }
            ChessMove::EnPassantMove { base_move, .. } => { base_move.from == from_square && base_move.to == to_square }
            ChessMove::PromotionMove { base_move, promote_to, .. } => { let _ = base_move.from == from_square && base_move.to == to_square; Some(promote_to) == promote_to_option.as_ref() }
            ChessMove::CastlingMove { base_move, .. } => { base_move.from == from_square && base_move.to == to_square }
        }
    }).collect::<Vec<ChessMove>>()
}

#[cfg(test)]
mod tests {
    use crate::bit_board::BitBoard;
    use crate::board::{Board, Piece, PieceType};
    use crate::board::PieceType::{Knight, Queen, Rook};
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
        let matched_moves = find_generated_move(moves, 1, 2, None);
        assert_eq!(matched_moves.len(), 1);
        assert_eq!(*matched_moves.get(0).unwrap(), BasicMove {base_move: BaseMove {from: 1, to: 2, capture: false, score: 0}});
    }

    #[test]
    fn test_find_generated_promotion_move() {
        let mut moves: Vec<ChessMove> = vec![];
        moves.push(BasicMove {base_move: { BaseMove { from: 1, to: 2, capture: false, score: 0  }}});
        moves.push(BasicMove {base_move: { BaseMove { from: 3, to: 4, capture: false, score: 0 }}});
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Queen });
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Rook });
        moves.push(PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Knight });
        let matched_moves = find_generated_move(moves, 3, 9, Some(Rook));
        assert_eq!(matched_moves.len(), 1);
        assert_eq!(*matched_moves.get(0).unwrap(), PromotionMove {base_move: { BaseMove{ from: 3, to: 9, capture: false, score: 0 }}, promote_to: Rook });
    }
}

