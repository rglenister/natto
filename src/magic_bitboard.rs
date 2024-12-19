use crate::board::PieceType;
use crate::util;
use crate::util::{distance, on_board};

fn generate_attack_masks() {
    let bishop_table = generate_attack_masks_for_piece_type([9, 7, -9, -7]);
    let rook_table = generate_attack_masks_for_piece_type([1, 8, -1, -8]);

    let queen_table: Vec<u64> =
        bishop_table.iter().zip(rook_table.iter()).map(|(&b, &r)| b | r).collect();

    for i in 0..64 {
        println!("i is {}", i);
        util::print_bitboard(*queen_table.get(i).unwrap());
    }
}

fn generate_attack_masks_for_piece_type(increments: [i32; 4]) -> Vec<u64> {
    let mut result: Vec<u64> = Vec::new();
    for square in (0..64) {
        let mut result_bitboard: u64 = 0;
        for increment in increments {
            result_bitboard |= generate_attack_mask_for_increment(0, square, increment);
        }
        result.push(result_bitboard);
    }
    result
}

fn generate_attack_mask_for_increment(bitboard: u64, source_square: i32, increment: i32) -> u64 {
    let destination_square: i32 = source_square + increment;
    if on_board(source_square, destination_square)
        && on_board(destination_square, destination_square + increment) {
        generate_attack_mask_for_increment(bitboard | 1 << destination_square, destination_square, increment)
    } else {
        bitboard
    }
}

#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_attest_attack_maskstack_masks() {
        generate_attack_masks();
    }
}