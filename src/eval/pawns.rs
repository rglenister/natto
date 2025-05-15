use crate::chessboard::piece::{PieceColor, PieceType};
use crate::chessboard::piece::PieceColor::White;
use crate::position::Position;

pub fn score_pawn_structure(position: &Position) -> isize {
    let board = position.board();
    let white_pawns = board.bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
    let black_pawns = board.bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);
    let mut score = 0;

    score += evaluate_pawns(white_pawns, PieceColor::White);
    score -= evaluate_pawns(black_pawns, PieceColor::Black);
    score
}

fn evaluate_pawns(pawns: u64, color: PieceColor) -> isize {
    let mut score = 0;
    let mut pawn_positions = pawns;
    while pawn_positions != 0 {
        let pawn_square = pawn_positions.trailing_zeros() as usize;
        pawn_positions &= pawn_positions - 1; // Remove the pawn from the pawn bitboard

        if is_doubled_pawn(pawn_square, pawns) {
            score -= 10;
        }

        if is_isolated_pawn(pawn_square, pawns) {
            score -= 15;
        }

        if is_passed_pawn(pawn_square, pawns, color) {
            score += 20;
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

mod tests {
    use crate::chessboard::piece::{PieceColor, PieceType};
    use crate::chessboard::piece::PieceColor::{White, Black};
    use crate::eval::pawns::{is_doubled_pawn, is_isolated_pawn, is_passed_pawn};
    use crate::position::Position;

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