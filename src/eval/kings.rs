use crate::core::move_gen;
use crate::core::move_gen::{king_attacks_finder, king_attacks_finder_empty_board};
use crate::core::piece::{PieceColor, PieceType};
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::Pawn;
use crate::core::position::Position;
use crate::util::bitboard_iterator::BitboardIterator;
use crate::util::util;

include!("../util/generated_macro.rs");


pub fn score_kings(position: &Position) -> (isize, isize) {
    let middle_game_score = evaluate_middle_game_king_safety(position, White) - evaluate_middle_game_king_safety(position, White);
    let end_game_score = evaluate_end_game_king_safety(position, White) - evaluate_end_game_king_safety(position, Black);
    (middle_game_score, end_game_score)
}

fn evaluate_middle_game_king_safety(position: &Position, piece_color: PieceColor) -> isize {
    let mut score = 0isize;

    let king_square = position.board().king_square(piece_color);
    let king_file = king_square % 8;

    // 1. Penalize exposed king (lack of pawn shield)
    let pawns_near_king = count_pawns_near_king(position, piece_color, king_square);
    if pawns_near_king == 0 {
        score -= 200; // No pawn shield: major penalty
    } else if pawns_near_king == 1 {
        score -= 100; // Weak pawn shield
    }

    // 2. Penalize being on open files
    if is_open_file(position, king_file) {
        score -= 50; // Position is on open file
    }

    // 3. Reward castling
    if position.has_castled(piece_color) {
        score += 150;
    }

    // 4. Penalize attacking pieces near king
    let attackers_near_king = count_attackers(position, piece_color);
    score -= 20 * attackers_near_king as isize; // Each attacking piece near the king = penalty

    score
}

// End game king safety evaluation
fn evaluate_end_game_king_safety(position: &Position, piece_color: PieceColor) -> isize {
    let mut score = 0;
    let king_square = position.board().king_square(piece_color);
    score += king_near_passed_pawns(position, piece_color, king_square) * 50;
    score as isize
}

fn count_pawns_near_king(position: &Position, piece_color: PieceColor, king_square: usize) -> usize {
    let pawns = position.board().bitboard_by_color_and_piece_type(piece_color, Pawn);
    (pawns & square_proximity_mask(king_square)).count_ones() as usize
}

fn is_open_file(position: &Position, file: usize) -> bool {
    let file_mask = 0x0101010101010101 << file;
    let all_pawns = position.board().bitboard_by_color_and_piece_type(White, Pawn) | position.board().bitboard_by_color_and_piece_type(Black, Pawn);
    all_pawns & file_mask == 0
}

fn count_attackers(position: &Position, king_color: PieceColor) -> usize {
    let attacking_squares = king_attacks_finder_empty_board(position, king_color);
    BitboardIterator::new(attacking_squares)
        .filter(|square| !util::is_piece_pinned(position, *square as isize, king_color))
        .count()
}

// Helper function: Count proximity to passed pawns
fn king_near_passed_pawns(position: &Position, piece_color: PieceColor, king_square: usize) -> usize {
    let passed_pawns = passed_pawn_bitboard(position, piece_color);
    (passed_pawns & square_proximity_mask(king_square)).count_ones() as usize
}

fn square_proximity_mask(square: usize) -> u64 {
    move_gen::non_sliding_piece_attacks_empty_board(PieceType::King, square) | 1 << square
}

pub fn passed_pawn_bitboard(position: &Position, color: PieceColor) -> u64 {
    let pawns = position.board().bitboard_by_color_and_piece_type(color, PieceType::Pawn);
    let opponent_pawns = position.board().bitboard_by_color_and_piece_type(!color, PieceType::Pawn);

    let (forward, left, right) = match color {
        White => (
            pawns << 8,                  // One rank up for White
            (pawns & 0x7F7F7F7F7F7F7F7F) << 7, // Diagonal left
            (pawns & 0xFEFEFEFEFEFEFEFE) << 9, // Diagonal right
        ),
        Black => (
            pawns >> 8,                  // One rank down for Black
            (pawns & 0xFEFEFEFEFEFEFEFE) >> 9, // Diagonal left
            (pawns & 0x7F7F7F7F7F7F7F7F) >> 7, // Diagonal right
        ),
    };

    let opponent_blockers = forward | left | right;
    let passed_pawns = !opponent_pawns & opponent_blockers;
    passed_pawns & pawns // Return passed pawns that correspond to the current player's pawns
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_square_proximity_mask() {
        let mask = square_proximity_mask(sq!("e1"));
        assert_eq!(mask, 14392);

        let mask = square_proximity_mask(sq!("e3"));
        assert_eq!(mask, 943208448);

        let mask = square_proximity_mask(sq!("h1"));
        assert_eq!(mask, 49344);
    }

    #[test]
    fn test_count_friendly_pawns_near_king() {
        let position = Position::new_game();
        assert_eq!(count_pawns_near_king(&position, White, sq!("e1")), 3);
        assert_eq!(count_pawns_near_king(&position, White, sq!("e2")), 3);
        assert_eq!(count_pawns_near_king(&position, White, sq!("e3")), 3);
        assert_eq!(count_pawns_near_king(&position, White, sq!("e4")), 0);
        assert_eq!(count_pawns_near_king(&position, White, sq!("e8")), 0);
        assert_eq!(count_pawns_near_king(&position, Black, sq!("e8")), 3);
        assert_eq!(count_pawns_near_king(&position, White, sq!("h1")), 2);
    }

    #[test]
    fn test_is_open_file() {
        let position = Position::from("2r4k/ppqb1p1Q/5Np1/3pPp2/8/P7/2P1RPPP/R5K1 b - - 0 30");
        for file in 0..8 {
            assert_eq!(is_open_file(&position, file), false);
        }

        let position = Position::from("2r4k/ppqb1p1Q/5Np1/3p1p2/8/P7/2P1RPPP/R5K1 b - - 0 30");
        assert_eq!(is_open_file(&position, 4), true);
        for file in 0..8 {
            let expected = file == 4;
            assert_eq!(is_open_file(&position, file), expected);
        }

    }

    #[test]
    fn test_count_attackers() {
        let position = Position::from("r4rk1/5ppp/8/8/6R1/1B6/8/1K6 w - - 0 1");
        assert_eq!(count_attackers(&position, Black), 2);

        let position = Position::from("1r3rk1/5ppp/8/8/6R1/1B6/8/1K6 w - - 0 1");
        assert_eq!(count_attackers(&position, Black), 1);
    }


    #[test]
    fn test_king_safety_opening() {
        let position = Position::new_game();
//        assert_eq!(score_kings(&position), 0); // Initial position should be balanced
    }

    #[test]
    fn test_passed_pawns() {
        let position = Position::from("4k3/8/8/4pPp1/4P3/8/8/4K3 w - - 0 1");
        let passed_pawns = passed_pawn_bitboard(&position, PieceColor::White);
        assert_eq!(passed_pawns.count_ones(), 1);
        assert_eq!(passed_pawns.trailing_zeros(), sq!("f5")); // Should detect passed pawn
    }

    #[test]
    fn test_king_near_passed_pawn() {
        let position = Position::from("4k3/8/4K3/4pPp1/4P3/8/8/8 w - - 0 1");
        let passed_pawns = passed_pawn_bitboard(&position, White);
        assert_eq!(passed_pawns.count_ones(), 1);
        assert_eq!(passed_pawns.trailing_zeros(), sq!("f5"));
        assert_eq!(king_near_passed_pawns(&position, White, sq!("e6")), 1);
        
        let position = Position::from("4k3/8/4K3/3PpPp1/4P3/8/8/8 w - - 0 1");
        let passed_pawns = passed_pawn_bitboard(&position, White);
        assert_eq!(passed_pawns.count_ones(), 2);
        assert_eq!(king_near_passed_pawns(&position, White, sq!("e6")), 2);
    }
}
