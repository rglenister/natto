use crate::bit_board::BitBoard;
use crate::board::PieceColor;
use crate::position::Position;

pub fn score_king_safety(position: &Position) -> isize {
    let board = position.board();
    let white_king_square = board.king_square(PieceColor::White);
    let black_king_square = board.king_square(PieceColor::Black);

    let mut score = 0;

    // Evaluate white king safety
    score += evaluate_king_safety(PieceColor::White, white_king_square, board);

    // Evaluate black king safety (negative for white's perspective)
    score -= evaluate_king_safety(PieceColor::Black, black_king_square, board);

    score
}

fn evaluate_king_safety(color: PieceColor, king_square: usize, board: &BitBoard) -> isize {
    let mut score = 0;

    // Exposing the king is penalized
    if is_king_exposed(king_square, color, board) {
        score -= 50; // Example penalty for exposing the king
    }

    // Bonus for castling - safer positioning of the king
    if !board.can_castle(color, &crate::board::BoardSide::KingSide)
        && !board.can_castle(color, &crate::board::BoardSide::QueenSide)
    {
        score -= 30; // Penalty for no castling rights left
    }

    // Endgame king positioning (encourages centralization in the endgame)
    if is_endgame(board) {
        score += evaluate_king_centralization(king_square, color);
    }

    score
}

fn is_king_exposed(king_square: usize, color: PieceColor, board: &BitBoard) -> bool {
    // Check the squares around the king for pawn protection
    let king_attacks = king_attack_mask(king_square);
    match color {
        PieceColor::White => king_attacks & board.white_pawn_attacks() == 0,
        PieceColor::Black => king_attacks & board.black_pawn_attacks() == 0,
    }
}

fn king_attack_mask(square: usize) -> u64 {
    let mut mask = 0;
    let (row, col) = (square / 8, square % 8);

    // Generate mask for surrounding king moves (8 surrounding squares)
    for d_row in -1..=1 {
        for d_col in -1..=1 {
            if d_row == 0 && d_col == 0 {
                continue; // Skip king's current position
            }
            let new_row = row as i32 + d_row;
            let new_col = col as i32 + d_col;

            if new_row >= 0 && new_row < 8 && new_col >= 0 && new_col < 8 {
                mask |= 1 << (new_row * 8 + new_col);
            }
        }
    }

    mask
}

fn is_endgame(board: &BitBoard) -> bool {
    // Define endgame as both sides having only kings with one or no minor pieces
    let white_pieces = board.bitboard_by_color(PieceColor::White);
    let black_pieces = board.bitboard_by_color(PieceColor::Black);
    let total_pieces = u64::count_ones(white_pieces) + u64::count_ones(black_pieces);

    total_pieces <= 6 // Endgame threshold - can be adjusted
}

fn evaluate_king_centralization(king_square: usize, color: PieceColor) -> isize {
    // Encourage king centralization in the endgame
    let (row, col) = (king_square / 8, king_square % 8);

    // Manhattan distance from the center (e4, d4, e5, d5 squares are the center)
    let center_distance = |r, c| (4 - r as isize).abs() + (4 - c as isize).abs();
    let dist = center_distance(row as isize, col as isize);

    10 - dist * 2 // Bonus decreases as the king is further from the center
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    #[test]
    fn test_king_safety_opening() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        let score = score_king_safety(&position);
        assert_eq!(score, -2); // Both kings are safe with castling rights
    }

    #[test]
    fn test_king_exposed() {
        let fen = "4k3/8/8/8/8/8/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        let score = score_king_safety(&position);
        assert!(score < 0); // Both kings are exposed, so negative score
    }

    #[test]
    fn test_king_centralization_in_endgame() {
        let fen = "8/8/8/3K4/8/8/8/3k4 w - - 0 1";
        let position = Position::from(fen);
        let score = score_king_safety(&position);
        assert!(score > 0); // Kings are centralized in endgame
    }
}
