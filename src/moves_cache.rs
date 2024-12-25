use crate::board::PieceType;
use crate::util;
use crate::util::on_board;
use crate::util::print_bitboard;
use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use std::collections::HashMap;

static PIECE_INCREMENTS_TABLE: Lazy<HashMap<&'static PieceType, Vec<i32>>> = Lazy::new(|| {
    let mut table = HashMap::new();
    table.insert(&PieceType::Knight, vec![10, 17, 15, 6, -10, -17, -15, -6]);
    table.insert(&PieceType::Bishop, vec![9, 7, -9, -7]);
    table.insert(&PieceType::Rook, vec![1, 8, -1, -8]);
    table.insert(&PieceType::Queen, vec![9, 7, -9, -7, 1, 8, -1, -8]);
    table.insert(&PieceType::King, vec![9, 7, -9, -7, 1, 8, -1, -8]);
    table
});

static NON_SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, [u64; 64]>> = Lazy::new(|| {
    let mut move_table: HashMap<PieceType, [u64; 64]> = HashMap::new();

    for piece_type in [PieceType::Knight, PieceType::King] {
        move_table.insert(piece_type.clone(), generate_move_table(piece_type));
    }
    fn generate_move_table(piece_type: PieceType) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        let increments = PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap();
        for square_index in 0..64 {
            let mut move_squares: u64 = 0;
            for increment in increments.iter() {
                let destination_square = square_index + increment;
                if (0..64).contains(&destination_square)
                    && util::distance(square_index, destination_square) <= 2
                {
                    move_squares |= 1 << destination_square;
                }
            }
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    return move_table;
});

struct TableEntry {
    blocking_squares_bitboard: u64,
    moves_bitmap: Vec<u64>,
}

static SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, Vec<TableEntry>>> = Lazy::new(|| {
    let mut move_table: HashMap<PieceType, Vec<TableEntry>> = HashMap::new();

    for piece_type in [PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
        move_table.insert(piece_type.clone(), generate_move_table(piece_type));
    }

    fn generate_move_table(piece_type: PieceType) -> Vec<TableEntry> {
        let mut squares: Vec<TableEntry> = Vec::new();
        for square_index in 0..1 {
            let blocking_squares_bitboard: u64 =
                generate_blocking_squares_bitboard(piece_type.clone(), square_index);
            let n_ones = blocking_squares_bitboard.count_ones() as u64;
            let table_size: u64 = 2_i32.pow((n_ones as i32).try_into().unwrap()) as u64;
            let mut moves_bitmap: Vec<u64> = Vec::new();
            for table_index in 0..table_size {
                let blocking_pieces_bitmap: u64 = table_index.pdep(blocking_squares_bitboard);
//                println!("blocking_pieces_bitmap:");
//                print_bitboard(blocking_pieces_bitmap);
                let sliding_move_bitboard = generate_sliding_piece_move_bitboard(
                    piece_type.clone(),
                    square_index,
                    blocking_pieces_bitmap,
                );
//                println!("sliding_move_bitmap:");
//                print_bitboard(sliding_move_bitboard);
                moves_bitmap.push(sliding_move_bitboard);
            }
            let table_entry: TableEntry = TableEntry {
                blocking_squares_bitboard,
                moves_bitmap,
            };
            squares.push(table_entry);
        }
        squares
    }

    fn generate_blocking_squares_bitboard(piece_type: PieceType, square: i32) -> u64 {
        let increments = PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap();
        let mut result_bitboard: u64 = 0;
        for increment in increments {
            result_bitboard |= generate_blocking_squares_bitboard_for_increment(0, square, *increment);
        }
        return result_bitboard;
    }

    fn generate_blocking_squares_bitboard_for_increment(
        bitboard: u64,
        source_square: i32,
        increment: i32,
    ) -> u64 {
        let destination_square: i32 = source_square + increment;
        if on_board(source_square, destination_square)
            && on_board(destination_square, destination_square + increment)
        {
            generate_blocking_squares_bitboard_for_increment(
                bitboard | 1 << destination_square,
                destination_square,
                increment,
            )
        } else {
            bitboard
        }
    }

    fn generate_sliding_piece_move_bitboard(
        piece_type: PieceType,
        square: i32,
        blocking_pieces_bitboard: u64,
    ) -> u64 {
        let increments = PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap();
        let mut result_bitboard: u64 = 0;
        for increment in increments {
            result_bitboard |= generate_sliding_piece_move_bitboard_for_increment(
                0,
                square,
                blocking_pieces_bitboard,
                *increment,
            );
        }
        return result_bitboard;
    }

    fn generate_sliding_piece_move_bitboard_for_increment(
        bitboard: u64,
        source_square: i32,
        blocking_pieces_bitmap: u64,
        increment: i32,
    ) -> u64 {
        let destination_square: i32 = source_square + increment;
        if on_board(source_square, destination_square) {
            if blocking_pieces_bitmap & 1 << destination_square == 0 {
                generate_sliding_piece_move_bitboard_for_increment(
                    bitboard | 1 << destination_square,
                    destination_square,
                    blocking_pieces_bitmap,
                    increment,
                )
            } else {
                bitboard | 1 << destination_square
            }
        } else {
            bitboard
        }
    }
    return move_table;
});

pub fn get_moves_by_piece_type(piece_type: PieceType, square_index: i32) -> u64 {
    return *NON_SLIDING_PIECE_MOVE_TABLE
        .get(&piece_type)
        .unwrap()
        .get(square_index as usize)
        .unwrap();
}

pub fn get_sliding_moves_by_piece_type(
    piece_type: PieceType,
    square_index: i32,
    occupied_squares: u64,
) -> u64 {
    let table_entry = SLIDING_PIECE_MOVE_TABLE
        .get(&piece_type)
        .unwrap()
        .get(square_index as usize)
        .unwrap();

    let occupied_blocking_squares_bitboard = occupied_squares & table_entry.blocking_squares_bitboard;
    let table_entry_bitmap_index = occupied_blocking_squares_bitboard.pext(table_entry.blocking_squares_bitboard);
    return *table_entry.moves_bitmap.get(table_entry_bitmap_index as usize).unwrap();
}

#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_knight_lookup_table() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 0),
            1 << 10 | 1 << 17
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 36),
            1 << 46 | 1 << 53 | 1 << 51 | 1 << 42 | 1 << 26 | 1 << 19 | 1 << 21 | 1 << 30
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 63),
            1 << 53 | 1 << 46
        );
    }
    #[test]
    fn test_king_lookup_table() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 0),
            1 << 9 | 1 << 1 | 1 << 8
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 7),
            1 << 14 | 1 << 15 | 1 << 6
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 1),
            1 << 10 | 1 << 8 | 1 << 2 | 1 << 9 | 1 << 0
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 2),
            1 << 11 | 1 << 9 | 1 << 3 | 1 << 10 | 1 << 1
        );
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 9),
            1 << 18 | 1 << 16 | 1 << 0 | 1 << 2 | 1 << 10 | 1 << 17 | 1 << 8 | 1 << 1
        );
    }

    #[test]
    fn test_bishop_lookup_table1() {
        let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0);
        print_bitboard(a);
        assert_eq!(
            get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0),
            1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54 | 1 << 63
        );
    }
    #[test]
    fn test_bishop_lookup_table2() {
        let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0);
        print_bitboard(a);
        assert_eq!(
            get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0),
            1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54 | 1 << 63
        );
    }
    #[test]
    fn test_bishop_lookup_table3() {
        let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 1 << 54);
        print_bitboard(a);
        assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }

    #[test]
    fn test_queen_lookup_table1() {
        let a = get_sliding_moves_by_piece_type(PieceType::Queen, 0, 1 << 3 | 1 << 8 | 1 << 18);
        print_bitboard(a);
        assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }






    //
    // #[test]
    // fn test_rook_lookup_table() {}
    //
    // #[test]
    // fn test_queen_lookup_table() {}
    //
    // #[test]
    // fn test_pdep() {
    //     println!("{:b}", 0b011u64.pdep(0b010000100001u64));
    // }
    // #[test]
    // fn test_pdep2() {
    //     // println!("{:b}", 0b0u64.pext(0b010000100001u64));
    //     // println!("{:b}", 0b1u64.pext(0b010000100001u64));
    //     // println!("{:b}", 0b11u64.pext(0b010000100001u64));
    //
    //     println!();
    //     for i in 0..20 {
    //         println!("{:b}", i.pdep(0b1000010000100001000001000000100u64));
    //     }
    // }
}
