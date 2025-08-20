use crate::core::move_gen;
use crate::core::piece::PieceColor;
use crate::core::piece::PieceType;
use crate::core::position::Position;
use crate::eval::pawns;
use crate::utils::bitboard_iterator::BitboardIterator;
use crate::utils::util;

include!("../utils/generated_macro.rs");

const ENEMY_PIECES_NEAR_KING_RADIUS: usize = 2;
const ENEMY_PIECE_NEAR_KING_PENALTIES: [isize; 6] = [
    2,  // pawn
    5,  // knight
    5,  // bishop
    10, // rook
    15, // queen
    0,  // king
];

pub fn score_kings(position: &Position) -> (i32, i32) {
    let score_mg =
        score_king_mg(position, PieceColor::White) - score_king_mg(position, PieceColor::Black);
    let score_eg =
        score_king_eg(position, PieceColor::White) - score_king_eg(position, PieceColor::Black);
    (score_mg, score_eg)
}

fn score_king_mg(position: &Position, piece_color: PieceColor) -> i32 {
    let mut score = 0i32;

    let king_square = position.board().king_square(piece_color);
    let king_file = king_square % 8;

    let pawns_near_king = count_pawns_near_king(position, piece_color, king_square);
    if pawns_near_king == 0 {
        score -= 200; // No pawn shield: major penalty
    } else if pawns_near_king == 1 {
        score -= 100; // Weak pawn shield
    }

    if is_open_file(position, king_file) {
        score -= 50;
    }

    if position.has_castled(piece_color) {
        score += 30;
    }

    let attackers_near_king = count_attackers(position, piece_color);
    score -= 20 * attackers_near_king as i32;

    score -= score_enemy_pieces_near_king(position, piece_color, king_square) as i32;

    score
}

// End game king safety evaluation
fn score_king_eg(position: &Position, piece_color: PieceColor) -> i32 {
    let mut score = 0i32;
    let king_square = position.board().king_square(piece_color);
    score += king_near_passed_pawns(position, piece_color, king_square) as i32 * 50;
    score
}

fn count_pawns_near_king(
    position: &Position,
    piece_color: PieceColor,
    king_square: usize,
) -> usize {
    let pawns = position.board().bitboard_by_color_and_piece_type(piece_color, PieceType::Pawn);
    (pawns & square_proximity_mask_of_radius(king_square, 1)).count_ones() as usize
}

fn is_open_file(position: &Position, file: usize) -> bool {
    let file_mask = 0x0101010101010101 << file;
    let all_pawns =
        position.board().bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn)
            | position.board().bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);
    all_pawns & file_mask == 0
}

fn count_attackers(position: &Position, king_color: PieceColor) -> usize {
    let attacking_squares = move_gen::king_attacks_finder_empty_board(position, king_color);
    BitboardIterator::new(attacking_squares)
        .filter(|square| !util::is_piece_pinned(position, *square as isize, king_color))
        .count()
}

fn score_enemy_pieces_near_king(
    position: &Position,
    piece_color: PieceColor,
    king_square: usize,
) -> isize {
    let enemy_piece_mask =
        square_proximity_mask_of_radius(king_square, ENEMY_PIECES_NEAR_KING_RADIUS);
    let enemy_piece_bitboards = position.board().bitboards_for_color(!piece_color);
    enemy_piece_bitboards.iter().enumerate().fold(0, |acc, (index, bitboard)| {
        acc + (bitboard & enemy_piece_mask).count_ones() as isize
            * ENEMY_PIECE_NEAR_KING_PENALTIES[index]
    })
}

fn king_near_passed_pawns(
    position: &Position,
    piece_color: PieceColor,
    king_square: usize,
) -> usize {
    let square_proximity_mask = square_proximity_mask_of_radius(king_square, 1);
    let our_nearby_pawns = square_proximity_mask
        & position.board().bitboard_by_color_and_piece_type(piece_color, PieceType::Pawn);
    if our_nearby_pawns != 0 {
        let their_pawns =
            position.board().bitboard_by_color_and_piece_type(!piece_color, PieceType::Pawn);
        return BitboardIterator::new(our_nearby_pawns)
            .filter(|square| pawns::is_passed_pawn(*square, piece_color, their_pawns))
            .count();
    }
    0
}

fn square_proximity_mask_of_radius(centre: usize, radius: usize) -> u64 {
    let centre_row = centre / 8;
    let row_range = centre_row.saturating_sub(radius)..(centre_row + radius + 1).min(8);
    let mut rows: u64 = 0;
    for row in row_range {
        rows |= util::row_bitboard(row);
    }
    let mut columns: u64 = 0;
    let centre_column = centre % 8;
    let column_range = (centre % 8).saturating_sub(radius)..(centre_column + radius + 1).min(8);
    for column in column_range {
        columns |= util::column_bitboard(column);
    }
    rows & columns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::piece::PieceType::{Bishop, Knight, Queen, Rook};

    #[test]
    fn test_square_proximity_mask_of_radius() {
        let mask = square_proximity_mask_of_radius(sq!("e1"), 1);
        assert_eq!(mask, 14392);

        let mask = square_proximity_mask_of_radius(sq!("e3"), 1);
        assert_eq!(mask, 943208448);

        let mask = square_proximity_mask_of_radius(sq!("h1"), 1);
        assert_eq!(mask, 49344);

        assert_eq!(square_proximity_mask_of_radius(sq!("e1"), 2), 0x7C7C7C);
        assert_eq!(square_proximity_mask_of_radius(sq!("e8"), 3), 0xFEFEFEFE00000000);
        assert_eq!(square_proximity_mask_of_radius(sq!("a1"), 3), 0xF0F0F0F);
        assert_eq!(square_proximity_mask_of_radius(sq!("a1"), 1), 0x303);
        assert_eq!(square_proximity_mask_of_radius(sq!("a1"), 0), 0x1);
    }

    #[test]
    fn test_count_friendly_pawns_near_king() {
        let position = Position::new_game();
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("e1")), 3);
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("e2")), 3);
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("e3")), 3);
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("e4")), 0);
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("e8")), 0);
        assert_eq!(count_pawns_near_king(&position, PieceColor::Black, sq!("e8")), 3);
        assert_eq!(count_pawns_near_king(&position, PieceColor::White, sq!("h1")), 2);
    }

    #[test]
    fn test_score_enemy_pieces_near_king() {
        let position: Position = Position::new_game();
        assert_eq!(score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")), 0);
        assert_eq!(score_enemy_pieces_near_king(&position, PieceColor::Black, sq!("e8")), 0);

        let position = Position::from("4k3/8/8/8/8/pppppppp/8/4K3 w - - 0 1");
        assert_eq!(
            score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")),
            5 * ENEMY_PIECE_NEAR_KING_PENALTIES[PieceType::Pawn as usize]
        );

        let position = Position::from("4k3/8/8/8/8/pppppppp/6q1/4K3 w - - 0 1");
        assert_eq!(
            score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")),
            5 * ENEMY_PIECE_NEAR_KING_PENALTIES[PieceType::Pawn as usize]
                + ENEMY_PIECE_NEAR_KING_PENALTIES[Queen as usize]
        );

        let position = Position::from("4k3/8/8/8/8/pppppppp/6r1/4K3 w - - 0 1");
        assert_eq!(
            score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")),
            5 * ENEMY_PIECE_NEAR_KING_PENALTIES[PieceType::Pawn as usize]
                + ENEMY_PIECE_NEAR_KING_PENALTIES[Rook as usize]
        );

        let position = Position::from("4k3/8/8/8/8/8/8/3bK3 w - - 0 1");
        assert_eq!(
            score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")),
            ENEMY_PIECE_NEAR_KING_PENALTIES[Bishop as usize]
        );

        let position = Position::from("4k3/8/8/8/8/8/8/3nK3 w - - 0 1");
        assert_eq!(
            score_enemy_pieces_near_king(&position, PieceColor::White, sq!("e1")),
            ENEMY_PIECE_NEAR_KING_PENALTIES[Knight as usize]
        );
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
        assert_eq!(count_attackers(&position, PieceColor::Black), 2);

        let position = Position::from("1r3rk1/5ppp/8/8/6R1/1B6/8/1K6 w - - 0 1");
        assert_eq!(count_attackers(&position, PieceColor::Black), 1);
    }

    #[test]
    fn test_king_safety_opening() {
        let position = Position::new_game();
        assert_eq!(score_kings(&position), (0, 0)); // Initial position should be balanced
    }

    #[test]
    fn test_king_near_passed_pawn() {
        let position = Position::from("4k3/8/4K3/4pPp1/4P3/8/8/8 w - - 0 1");
        let nearby_passed_pawn_count = king_near_passed_pawns(
            &position,
            PieceColor::White,
            position.board().king_square(PieceColor::White),
        );
        assert_eq!(nearby_passed_pawn_count, 1);

        let position = Position::from("4k3/8/4K3/3PpPp1/4P3/8/8/8 w - - 0 1");
        let nearby_passed_pawn_count = king_near_passed_pawns(
            &position,
            PieceColor::White,
            position.board().king_square(PieceColor::White),
        );
        assert_eq!(nearby_passed_pawn_count, 2);
    }
}
