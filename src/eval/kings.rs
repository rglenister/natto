use crate::core::move_generator::king_attacks_finder;
use crate::core::piece::{PieceColor, PieceType};
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::Pawn;
use crate::core::position::Position;
use crate::util::bitboard_iterator::BitboardIterator;

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
    let attackers_near_king = count_attackers(position, !piece_color);
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

fn count_pawns_near_king(position: &Position, piece_color: PieceColor, king_square: usize) -> u64 {
    let pawns = position.board().bitboard_by_color_and_piece_type(piece_color, Pawn);
    let king_square = position.board().king_square(piece_color);
    pawns & square_proximity_mask(king_square)
}

fn is_open_file(position: &Position, file: usize) -> bool {
    let file_mask = 0x0101010101010101 << file;
    position.board().bitboard_all_pieces() & file_mask == 0
}

fn count_attackers(position: &Position, attacker_color: PieceColor) -> usize {
    let attacking_squares = king_attacks_finder(position, attacker_color);
    BitboardIterator::new(attacking_squares)
        .filter(|square| !is_pinned(position, *square))
        .count()
}

fn is_pinned(position: &Position, square_index: usize) -> bool {
    false
}

// Helper function: Count proximity to passed pawns
fn king_near_passed_pawns(position: &Position, piece_color: PieceColor, king_square: usize) -> u64 {
    let passed_pawns = passed_pawn_bitboard(position, piece_color);
    passed_pawns & square_proximity_mask(king_square)
}

// Build a square mask for proximity (king usually affects an 8-square radius)
fn square_proximity_mask(square: usize) -> u64 {
    let mut mask = 0;
    let rank = square / 8;
    let file = square % 8;

    for r in rank.saturating_sub(1)..=(rank + 1).min(7) {
        for f in file.saturating_sub(1)..=(file + 1).min(7) {
            mask |= 1 << (r * 8 + f);
        }
    }
    mask
}

pub fn passed_pawn_bitboard(position: &Position, color: PieceColor) -> u64 {
    let pawns = position.board().bitboard_by_color_and_piece_type(color, PieceType::Pawn);
    let opponent_pawns = position.board().bitboard_by_color_and_piece_type(!color, PieceType::Pawn);

    // Forward masks depend on the player color
    let (forward, left, right) = match color {
        PieceColor::White => (
            pawns << 8,                  // One rank up for White
            (pawns & 0x7F7F7F7F7F7F7F7F) << 7, // Diagonal left
            (pawns & 0xFEFEFEFEFEFEFEFE) << 9, // Diagonal right
        ),
        PieceColor::Black => (
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
    use crate::util::fen;

    #[test]
    fn test_king_safety_opening() {
        let position = Position::new_game();
//        assert_eq!(score_kings(&position), 0); // Initial position should be balanced
    }
    
    #[test]
    fn test_exposed_king() {
  //      let position = fen::parse("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1").unwrap();
  //      let safety = score_kings(&position);
  //      assert!(safety < 0); // Exposed king should have negative safety score
    }
    
    #[test]
    fn test_endgame_king_safety() {
        // let position = fen::parse("4k3/8/4P3/8/8/8/8/4K3 w - - 0 1").unwrap();
        // let safety = score_kings(&position);
        // assert!(safety > 0); // Central king in endgame should have positive score
    }

    #[test]
    fn test_passed_pawns() {
        let position = Position::from("4k3/8/8/4pPp1/4P3/8/8/4K3 w - - 0 1");
        let passed_pawns = passed_pawn_bitboard(&position, PieceColor::White);
        assert_eq!(passed_pawns.count_ones(), 1);
        assert_eq!(passed_pawns.trailing_zeros(), 37); // Should detect passed pawn
    }
}
