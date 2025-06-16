use crate::core::board::{Board, BoardSide};
use crate::core::board::BoardSide::{KingSide, QueenSide};
use crate::core::piece::{PieceColor, PieceType};
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::Pawn;
use crate::core::position::Position;
use crate::util::bitboard_iterator::BitboardIterator;
use crate::util::util::set_bitboard_column;

const BITBOARD_REGIONS: [u64; 2] = [
    set_bitboard_column(5) | set_bitboard_column(6) | set_bitboard_column(7), // kingside
    set_bitboard_column(0) | set_bitboard_column(1) | set_bitboard_column(2), // queenside
];

pub fn score_pawns(position: &Position) -> (isize, isize) {
    let middle_game_score = evaluate_pawn_structure_mg(position, White) - evaluate_pawn_structure_mg(position, Black);
    let end_game_score = evaluate_pawn_structure_eg(position, White) - evaluate_pawn_structure_eg(position, Black);
    (middle_game_score, end_game_score)
}

pub fn evaluate_pawn_structure_mg(position: &Position, piece_color: PieceColor) -> isize {
    let board: &Board = position.board();
    let pawns = board.bitboard_by_color_and_piece_type(piece_color, PieceType::Pawn);
    
    let mut score = 0;

    BitboardIterator::new(pawns).for_each(|pawn_square| {
        // Assess pawn chains
        if is_part_of_chain(position.board(), ((pawn_square / 8) as i8, (pawn_square % 8) as i8), piece_color) {
            score += 50; // Bonus for pawn chain
        }

        if is_doubled_pawn(pawn_square, pawns) {
            score -= 30;
        }

        if is_isolated_pawn(pawn_square, pawns) {
            score -= 50;
        }
    });
    score
}

fn evaluate_pawn_structure_eg(position: &Position, piece_color: PieceColor) -> isize {
    let board: &Board = position.board();
    let pawns = board.bitboard_by_color_and_piece_type(piece_color, PieceType::Pawn);

    let mut score = 0isize;

    BitboardIterator::new(pawns).for_each(|pawn_square| {
        if is_passed_pawn(pawn_square, pawns, piece_color) {
            score += 100 + 10 * Board::rank(pawn_square, piece_color) as isize;
        }

        if has_pawn_majority(board, piece_color, KingSide) {
            score += 50;
        }

        if has_pawn_majority(board, piece_color, QueenSide) {
            score += 50;
        }
    });
    score
}

pub fn is_passed_pawn(square: usize, pawns: u64, color: PieceColor) -> bool {
    let file = square % 8;
    let rank = square / 8;

    let mut blocking_files_mask = 0x0101010101010101 << file;
    if file > 0 {
        blocking_files_mask |= 0x0101010101010101 << (file - 1);
    }
    if file < 7 {
        blocking_files_mask |= 0x0101010101010101 << (file + 1);
    }

    if let White = color {
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

fn has_pawn_majority(board: &Board, piece_color: PieceColor, board_side: BoardSide) -> bool {
    let pawns = [board.bitboard_by_color_and_piece_type(White, Pawn), board.bitboard_by_color_and_piece_type(Black, Pawn)];
    let (our_pawns, their_pawns) = (
        (pawns[piece_color as usize] & BITBOARD_REGIONS[board_side as usize]).count_ones(),
        (pawns[!piece_color as usize] & BITBOARD_REGIONS[board_side as usize]).count_ones()
    );
    our_pawns > their_pawns
}


/// Checks if a given pawn is part of a pawn chain.
fn is_part_of_chain(board: &Board, position: (i8, i8), color: PieceColor) -> bool {
    // Direction offsets for backward diagonals (depends on pawn color)
    let (backward_rank_offset, backward_left_file_offset, backward_right_file_offset) = match color {
        PieceColor::White => (-1, -1, 1), // White pawns look backward towards lower ranks
        PieceColor::Black => (1, -1, 1),  // Black pawns look backward towards higher ranks
    };

    // Check if either of the backward diagonal squares contains a friendly pawn
    check_diagonal_for_chain(
        board,
        position,
        backward_rank_offset,
        backward_left_file_offset,
    ) || check_diagonal_for_chain(
        board,
        position,
        backward_rank_offset,
        backward_right_file_offset,
    )
}

/// Helper function to check one diagonal for a pawn chain
fn check_diagonal_for_chain(
    board: &Board,
    position: (i8, i8),
    rank_offset: i8,
    file_offset: i8,
) -> bool {
    let (file, rank) = position;
    let new_position = (file + file_offset, rank + rank_offset);

    // Ensure the square is valid and contains a friendly pawn
    // board.is_square_occupied_by(new_position, PieceColor::White, PieceType::Pawn)
    //     || board.is_square_occupied_by(new_position, PieceColor::Black, PieceType::Pawn)
    false
}

fn is_isolated_pawn(square: usize, pawns: u64) -> bool {
    let file = square % 8;
    let left_file_mask = if file > 0 { 0x0101010101010101 << (file - 1) } else { 0 };
    let right_file_mask = if file < 7 { 0x0101010101010101 << (file + 1) } else { 0 };

    let neighbors = pawns & (left_file_mask | right_file_mask);
    neighbors == 0
}


fn is_doubled_pawn(square: usize, pawns: u64) -> bool {
    let file_mask = 0x0101010101010101 << (square % 8);
    let pawns_on_file = pawns & file_mask;
    pawns_on_file.count_ones() > 1
}
mod tests {
    use crate::core::piece::PieceColor::Black;
    use crate::core::position::Position;
    use super::*;
    include!("../util/generated_macro.rs");

    #[test]
    fn test_is_doubled_pawn() {
        let fen = "4k3/8/6P1/P2P4/8/2P3P1/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let pawn_bitboard = position.board().bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
        assert_eq!(is_doubled_pawn(sq!("a5"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("c3"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("d5"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("g3"), pawn_bitboard), true);
        assert_eq!(is_doubled_pawn(sq!("g6"), pawn_bitboard), true);
    }
    #[test]
    fn test_is_isolated_pawn() {
        let fen = "4k3/8/6P1/P2P4/8/2P3P1/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let pawn_bitboard = position.board().bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
        assert_eq!(is_isolated_pawn(sq!("a5"), pawn_bitboard), true);
        assert_eq!(is_isolated_pawn(sq!("c3"), pawn_bitboard), false);
        assert_eq!(is_isolated_pawn(sq!("d5"), pawn_bitboard), false);
        assert_eq!(is_isolated_pawn(sq!("g3"), pawn_bitboard), true);
        assert_eq!(is_isolated_pawn(sq!("g6"), pawn_bitboard), true);
    }

    #[test]
    fn test_is_passed_pawn() {
        let fen = "4k3/8/6P1/P2P4/8/2P3P1/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let pawn_bitboard = position.board().bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
        assert_eq!(is_passed_pawn(sq!("a5"), pawn_bitboard, White), true);
        assert_eq!(is_passed_pawn(sq!("c3"), pawn_bitboard, White), false);
        assert_eq!(is_passed_pawn(sq!("d5"), pawn_bitboard, White), true);
        assert_eq!(is_passed_pawn(sq!("g3"), pawn_bitboard, White), false);
        assert_eq!(is_passed_pawn(sq!("g6"), pawn_bitboard, White), true);
    }

    #[test]
    fn test_simple_white_passed_pawn() {
        let fen = "8/8/8/4P3/8/8/8/5k1K w - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("e5"), pawns, White));
    }

    #[test]
    fn test_simple_black_passed_pawn() {
        let fen = "8/8/8/4p3/8/8/8/5k1K b - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(Black, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("e5"), pawns, Black));
    }

    #[test]
    fn test_protected_white_passed_pawn() {
        let fen = "8/8/8/3Pp3/8/8/8/k6K w - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("d5"), pawns, White));
    }

    #[test]
    fn test_not_a_passed_pawn_blocked() {
        let fen = "8/8/8/3Pp3/8/8/8/k6K w - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("e5"), pawns, White));
    }

    #[test]
    fn test_not_a_passed_pawn_adjacent_opponent() {
        let fen = "8/8/8/3pP3/8/8/8/k6K w - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("e5"), pawns, White));
    }

    #[test]
    fn test_connected_passed_pawns() { // ok
        let fen = "8/8/8/3PP3/8/8/8/k6K w - - 0 1";
        let position: Position = Position::from(fen);
        let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
        assert!(is_passed_pawn(sq!("d5"), pawns, White));
        assert!(is_passed_pawn(sq!("e5"), pawns, White));
    }
}
