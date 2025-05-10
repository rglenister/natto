use crate::bit_board::BitBoard;
use crate::board::{Board, Piece, PieceColor, PieceType};
use crate::position::Position;

pub fn score_pawn_structure(position: &Position) -> isize {
    let board = position.board();
    let white_pawns = board.bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
    let black_pawns = board.bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);
    let mut score = 0;

    // Score white pawns
    score += evaluate_pawns(white_pawns, PieceColor::White);

    // Score black pawns (negative for white's perspective)
    score -= evaluate_pawns(black_pawns, PieceColor::Black);

    score
}

fn evaluate_pawns(pawns: u64, color: PieceColor) -> isize {
    let mut score = 0;

    let mut pawn_positions = pawns;
    while pawn_positions != 0 {
        let pawn_square = pawn_positions.trailing_zeros() as usize;
        pawn_positions &= pawn_positions - 1; // Remove the pawn from the pawn bitboard

        // Doubled pawns
        if is_doubled_pawn(pawn_square, pawns) {
            score -= 10; // Penalize doubled pawns
        }

        // Isolated pawns
        if is_isolated_pawn(pawn_square, pawns) {
            score -= 15; // Penalize isolated pawns
        }

        // Passed pawns
        if is_passed_pawn(pawn_square, pawns, color) {
            score += 20; // Reward passed pawns
        }
    }

    score
}

fn is_doubled_pawn(square: usize, pawns: u64) -> bool {
    let file_mask = 0x0101010101010101 << (square % 8);
    let pawns_on_file = pawns & file_mask;
    pawns_on_file.count_ones() > 1
}

fn is_isolated_pawn(square: usize, pawns: u64) -> bool {
    let file = square % 8;
    let left_file_mask = if file > 0 { 0x0101010101010101 << (file - 1) } else { 0 };
    let right_file_mask = if file < 7 { 0x0101010101010101 << (file + 1) } else { 0 };

    let neighbors = pawns & (left_file_mask | right_file_mask);
    neighbors == 0
}

fn is_passed_pawn(square: usize, pawns: u64, color: PieceColor) -> bool {
    let file = square % 8;
    let rank = square / 8;

    let mut blocking_files_mask = 0x0101010101010101 << file;
    if file > 0 {
        blocking_files_mask |= 0x0101010101010101 << (file - 1);
    }
    if file < 7 {
        blocking_files_mask |= 0x0101010101010101 << (file + 1);
    }

    if let PieceColor::White = color {
        for r in (rank + 1)..8 {
            if (pawns & (blocking_files_mask & (0xFF << (r * 8)))) != 0 {
                return false;
            }
        }
    } else {
        for r in (0..rank).rev() {
            if (pawns & (blocking_files_mask & (0xFF << (r * 8)))) != 0 {
                return false;
            }
        }
    }

    true
}
