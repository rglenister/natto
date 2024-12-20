use crate::board::PieceType;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use crate::util;

static MOVE_TABLE: Lazy<HashMap<PieceType, [Vec<i32>; 64]>> = Lazy::new(|| {
    let knight_increments = [10, 17, 15, 6, -10, -17, -15, -6];
    let king_increments = [9, 7, -9, -7, 1, 8, -1, -8];

    let mut move_table: HashMap<PieceType, [Vec<i32>; 64]> = HashMap::new();

    generate_move_table(&mut move_table, PieceType::Knight, &knight_increments);
    generate_move_table(&mut move_table, PieceType::King, &king_increments);
    fn generate_move_table(
        move_table: &mut HashMap<PieceType, [Vec<i32>; 64]>,
        piece_type: PieceType,
        increments: &[i32],
    ) {
        let mut squares: [Vec<i32>; 64] = core::array::from_fn(|_i| vec![]);
        for square_index in 0..64 {
            let mut move_squares: Vec<i32> = vec![];
            for square_increment in increments.iter() {
                get_moves_for_increment(&mut move_squares, square_index, *square_increment);
            }
            squares[square_index as usize] = move_squares;
        }
        move_table.insert(piece_type.clone(), squares);
    }
    return move_table;

    fn get_moves_for_increment(move_squares: &mut Vec<i32>, source_square_index: i32, increment: i32) {
        let destination_square = source_square_index + increment;
        if (0..64).contains(&destination_square) && util::distance(source_square_index, destination_square) <= 2 {
            move_squares.push(destination_square);
        }
    }
});



#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_knight_lookup_table() {
        let table = &MOVE_TABLE;
        let moves_for_knight = table.get(&PieceType::Knight).unwrap();
        assert_eq!(moves_for_knight[0], vec![10, 17]);
        assert_eq!(moves_for_knight[36], vec![46, 53, 51, 42, 26, 19, 21, 30]);
        assert_eq!(moves_for_knight[63], vec![53, 46]);
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
