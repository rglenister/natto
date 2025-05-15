use crate::chessboard::board::Board;
use crate::chessboard::piece::PieceColor;
use crate::chessboard::piece::PieceColor::White;
use crate::position::Position;
use crate::{move_generator, util};
use crate::chessboard::piece::PieceType::King;

pub fn score_king_safety(position: &Position) -> isize {
    let mut score = 0;
    score += evaluate_king_safety(position, PieceColor::White);
    score -= evaluate_king_safety(position, PieceColor::Black);
    score
}

fn evaluate_king_safety(position: &Position, color: PieceColor) -> isize {
    let board = position.board();
    let king_square = position.board().king_square(color);
    let mut score = 0;
    
    // Exposing the king is penalized
    if is_king_exposed(position, king_square, color) {
        score -= 50; // Example penalty for exposing the king
    }

    // Bonus for castling - safer positioning of the king
    if !position.can_castle(color, &crate::chessboard::board::BoardSide::KingSide)
        && !position.can_castle(color, &crate::chessboard::board::BoardSide::QueenSide)
    {
        score -= 30; // Penalty for no castling rights left
    }

    // Endgame king positioning (encourages centralization in the endgame)
    if is_endgame(board) {
        score += evaluate_king_centralization(king_square, color);
    }

    score
}

fn is_king_exposed(position: &Position, king_square: usize, color: PieceColor) -> bool {
    // Check the squares around the king for pawn protection
    let board = position.board();
    let king_attacks = move_generator::non_sliding_piece_attacks_empty_board(King, king_square);
    match color {
        PieceColor::White => king_attacks & board.white_pawn_attacks() == 0,
        PieceColor::Black => king_attacks & board.black_pawn_attacks() == 0,
    }
}

fn is_endgame(board: &Board) -> bool {
    let total_pieces = u64::count_ones(board.bitboard_all_pieces());
    total_pieces <= 6
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
