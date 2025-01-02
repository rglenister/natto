use crate::board::{Board, PieceColor};
use crate::board::PieceColor::White;
use crate::board::PieceColor::Black;

pub fn create_color(initial: &str) -> Option<PieceColor> {
    if initial == "w" { Some(White) } else if initial == "b" { Some(Black) } else { None }
}

pub fn parse_square(square: &str) -> Option<usize> {
    if square == "-" {
        return None
    } else {
        let row = square.chars().nth(1).expect("Invalid square").to_digit(10).expect("Invalid square");
        let col_char = square.chars().nth(0).expect("Invalid square");
        let col = col_char as u32 - 'a' as u32;
        Some(((row - 1) * 8 + col).try_into().unwrap())
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

#[cfg(test)]
mod tests {
    use std::result;
    use crate::bit_board::BitBoard;
    use crate::board::{Piece, PieceType};
    use super::*;

    #[test]
    fn test_bit_indexes() {
        let result = bit_indexes(1 << 0 | 1 << 1 | 1 << 32 | 1 << 63);
        assert_eq!(result.len(), 4);
        assert_eq!(result, vec![0, 1, 32, 63]);
    }

    fn test_create_color() {
        assert_eq!(None, create_color("a"));
        assert_eq!(Some(Black), create_color("b"));
        assert_eq!(Some(White), create_color("w"));
    }

    #[test]
    fn test_parse_square() {
        assert_eq!(parse_square("a1").unwrap(), 0);
        assert_eq!(parse_square("a2").unwrap(), 8);
        assert_eq!(parse_square("h7").unwrap(), 55);
        assert_eq!(parse_square("h8").unwrap(), 63);
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
        println!("{}", string);
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
}

