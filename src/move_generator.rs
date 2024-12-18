use crate::board::PieceType;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static MOVE_TABLE: Lazy<HashMap<PieceType, [Vec<i32>; 64]>> = Lazy::new(|| {
    let knight_increments = [10, 17, 15, 6, -10, -17, -15, -6];
    let bishop_increments = [9, 7, -9, -7];
    let rook_increments = [1, 8, -1, -8];
    let queen_and_king_increments = [9, 7, -9, -7, 1, 8, -1, -8];

    let mut move_table: HashMap<PieceType, [Vec<i32>; 64]> = HashMap::new();

    generate_move_table(&mut move_table, PieceType::Knight, &knight_increments, false);
    generate_move_table(&mut move_table, PieceType::Bishop, &bishop_increments, true);
    generate_move_table(&mut move_table, PieceType::Rook, &rook_increments, true);
    generate_move_table(&mut move_table, PieceType::Queen, &queen_and_king_increments, true);
    generate_move_table(&mut move_table, PieceType::King, &queen_and_king_increments, false);
    fn generate_move_table(
        move_table: &mut HashMap<PieceType, [Vec<i32>; 64]>,
        piece_type: PieceType,
        increments: &[i32],
        sliding: bool,
    ) {
        let mut squares: [Vec<i32>; 64] = core::array::from_fn(|i| vec![]);
        for square_index in 0..64 {
            let mut move_squares: Vec<i32> = vec![];
            for square_increment in increments.iter() {
                get_moves_for_increment(&mut move_squares, square_index, *square_increment, sliding);
            }
            squares[square_index as usize] = move_squares;
        }
        move_table.insert(piece_type.clone(), squares);
    }
    return move_table;

    fn get_moves_for_increment(move_squares: &mut Vec<i32>, source_square_index: i32, increment: i32, sliding: bool) {
        let destination_square = source_square_index + increment;
        if (0..64).contains(&destination_square) && distance(source_square_index, destination_square) <= 2 {
            move_squares.push(destination_square);
            if sliding {
                get_moves_for_increment(&mut *move_squares, destination_square, increment, sliding);
            }
        }
    }
});

fn distance(square_index_1: i32, square_index_2: i32) -> i32 {
    let square_1_row = square_index_1 / 8;
    let square_1_col = square_index_1 % 8;

    let square_2_row = square_index_2 / 8;
    let square_2_col = square_index_2 % 8;

    let row_difference = (square_2_row - square_1_row).abs();
    let col_difference = (square_2_col - square_1_col).abs();

    row_difference.max(col_difference)
}

#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_distance() {
        assert_eq!(distance(0, 0), 0);
        assert_eq!(distance(0, 1), 1);
        assert_eq!(distance(6, 7), 1);
        assert_eq!(distance(7, 8), 7);
        assert_eq!(distance(60, 68), 1);
    }

    #[test]
    fn test_knight_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_knight = table.get(&PieceType::Knight).unwrap();
        assert_eq!(moves_for_knight[0], vec![10, 17]);
        assert_eq!(moves_for_knight[36], vec![46, 53, 51, 42, 26, 19, 21, 30]);
        assert_eq!(moves_for_knight[63], vec![53, 46]);
    }
    #[test]
    fn test_bishop_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_bishop = table.get(&PieceType::Bishop).unwrap();
        assert_eq!(moves_for_bishop[0], vec![9, 18, 27, 36, 45, 54, 63]);
        assert_eq!(moves_for_bishop[36], vec![45, 54, 63, 43, 50, 57, 27, 18, 9, 0, 29, 22, 15]);
        assert_eq!(moves_for_bishop[63], vec![54, 45, 36, 27, 18, 9, 0]);
    }
    #[test]
    fn test_rook_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_rook = table.get(&PieceType::Rook).unwrap();
        assert_eq!(moves_for_rook[0], vec![1, 2, 3, 4, 5, 6, 7, 8, 16, 24, 32, 40, 48, 56]);
        assert_eq!(moves_for_rook[63], vec![62, 61, 60, 59, 58, 57, 56, 55, 47, 39, 31, 23, 15, 7]);
    }
    #[test]
    fn test_queen_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_queen = table.get(&PieceType::Queen).unwrap();
        assert_eq!(moves_for_queen[0], vec![9, 18, 27, 36, 45, 54, 63, 1, 2, 3, 4, 5, 6, 7, 8, 16, 24, 32, 40, 48, 56]);
        assert_eq!(moves_for_queen[7], vec![14, 21, 28, 35, 42, 49, 56, 15, 23, 31, 39, 47, 55, 63, 6, 5, 4, 3, 2, 1, 0]);
        assert_eq!(moves_for_queen[1], vec![10, 19, 28, 37, 46, 55, 8, 2, 3, 4, 5, 6, 7, 9, 17, 25, 33, 41, 49, 57, 0]);
        assert_eq!(moves_for_queen[8], vec![17, 26, 35, 44, 53, 62, 1, 9, 10, 11, 12, 13, 14, 15, 16, 24, 32, 40, 48, 56, 0]);
        assert_eq!(moves_for_queen[2], vec![11, 20, 29, 38, 47, 9, 16, 3, 4, 5, 6, 7, 10, 18, 26, 34, 42, 50, 58, 1, 0]);
        assert_eq!(moves_for_queen[9], vec![18, 27, 36, 45, 54, 63, 16, 0, 2, 10, 11, 12, 13, 14, 15, 17, 25, 33, 41, 49, 57, 8, 1]);
    }
    #[test]
    fn test_king_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_king = table.get(&PieceType::King).unwrap();
        assert_eq!(moves_for_king[0], vec![9, 1, 8]);
        assert_eq!(moves_for_king[7], vec![14, 15, 6]);
        assert_eq!(moves_for_king[1], vec![10, 8, 2, 9, 0]);
        assert_eq!(moves_for_king[8], vec![17, 1, 9, 16, 0]);
        assert_eq!(moves_for_king[2], vec![11, 9, 3, 10, 1]);
        assert_eq!(moves_for_king[9], vec![18, 16, 0, 2, 10, 17, 8, 1]);
    }

}
