use crate::board::PieceType;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static MOVE_TABLE: Lazy<HashMap<PieceType, [Vec<i32>; 64]>> = Lazy::new(|| {
    let increments = HashMap::from([
        (PieceType::Knight, [10, 17, 15, 6, -10, -17, -15, -6]),
        (PieceType::King, [1, 7, 8, 9, -1, -7, -8, -9])]);

    let mut move_table: HashMap<PieceType, [Vec<i32>; 64]> = HashMap::new();

    for piece_type in [PieceType::Knight, PieceType::King].iter() {
        let mut squares: [Vec<i32>; 64] = core::array::from_fn(|i| vec![]);
        for square_index in 0..64 {
            let mut move_squares: Vec<i32> = vec![];
            for square_increment in increments[&piece_type].iter() {
                let destination_square = square_index + square_increment;
                if (0..64).contains(&destination_square)
                    && distance(square_index, destination_square) <= 2
                {
                    move_squares.push(destination_square);
                }
            }
            squares[square_index as usize] = move_squares;
        }
        move_table.insert(piece_type.clone(), squares);
    }

    return move_table;
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
        let mut table = &MOVE_TABLE;
        let moves_for_knight = table.get(&PieceType::Knight).unwrap();
        assert_eq!(moves_for_knight[0], vec![10, 17]);
        assert_eq!(moves_for_knight[36], vec![46, 53, 51, 42, 26, 19, 21, 30]);
        assert_eq!(moves_for_knight[63], vec![53, 46]);
    }
    #[test]
    fn test_king_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_king = table.get(&PieceType::King).unwrap();
        assert_eq!(moves_for_king[0], vec![1, 8, 9]);
        assert_eq!(moves_for_king[7], vec![14, 15, 6]);
        assert_eq!(moves_for_king[1], vec![2, 8, 9, 10, 0]);
        assert_eq!(moves_for_king[8], vec![9, 16, 17, 1, 0]);
        assert_eq!(moves_for_king[2], vec![3, 9, 10, 11, 1]);
        assert_eq!(moves_for_king[9], vec![10, 16, 17, 18, 8, 2, 1, 0]);
    }

}
