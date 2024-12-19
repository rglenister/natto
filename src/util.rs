use crate::board::PieceColor;
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
    square_to >= 0 && square_to < 64 && (square_to % 8 - square_from % 8).abs() <= 1
}

pub fn print_bitboard2(bitboard: u64) {
    println!("{:064b}", bitboard);
}
pub fn print_bitboard(bitboard: u64) {
    for rank in (0..8) {
        let row = (bitboard >> (rank * 8)) & 0xFF;
        println!("{}", format!("{:08b}", row).replace('0', "-")
            .chars().fold("".to_string(), |cur, nxt| cur + "  " + nxt.to_string().as_str()));
    }
    println!();
    print_bitboard2(bitboard);
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_bitboard_to_string() {
        let board: u64 = (1 as u64);
        print_bitboard(board);

        let board: u64 = (1 as u64) << 63;
        let board: u64 = (1 as u64) << 62;
        print_bitboard(board);
    }

}
