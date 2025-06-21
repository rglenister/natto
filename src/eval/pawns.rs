use crate::core::board::{Board, BoardSide};
use crate::core::board::BoardSide::{KingSide, QueenSide};
use crate::core::move_gen;
use crate::core::piece::{Piece, PieceColor, PieceType};
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::Pawn;
use crate::core::position::Position;
use crate::util::bitboard_iterator::BitboardIterator;
use crate::util::util::column_bitboard;

const BITBOARD_REGIONS: [u64; 2] = [
    column_bitboard(5) | column_bitboard(6) | column_bitboard(7), // kingside
    column_bitboard(0) | column_bitboard(1) | column_bitboard(2), // queenside
];

const PASSED_PAWN_COLUMNS: [u64; 8] = [
                             column_bitboard(0) | column_bitboard(1),
    column_bitboard(0) | column_bitboard(1) | column_bitboard(2),
    column_bitboard(1) | column_bitboard(2) | column_bitboard(3),
    column_bitboard(2) | column_bitboard(3) | column_bitboard(4),
    column_bitboard(3) | column_bitboard(4) | column_bitboard(5),
    column_bitboard(4) | column_bitboard(5) | column_bitboard(6),
    column_bitboard(5) | column_bitboard(6) | column_bitboard(7),
    column_bitboard(6) | column_bitboard(7)
];

const PASSED_PAWNS_RANKS: [[u64; 8]; 2] = [
    [
        0xffffffffffffffff,
        0xffffffffffffff00,
        0xffffffffffff0000,
        0xffffffffff000000,
        0xffffffff00000000,
        0xffffff0000000000,
        0xffff000000000000,
        0xff00000000000000,
    ],
    [
        0xff00000000000000,
        0xffff000000000000,
        0xffffff0000000000,
        0xffffffff00000000,
        0xffffffffff000000,
        0xffffffffffff0000,
        0xffffffffffffff00,
        0xffffffffffffffff
    ]
];

pub fn score_pawns(position: &Position) -> (isize, isize) {
    let score_mg = score_pawn_structure_mg(position, White) - score_pawn_structure_mg(position, Black);
    let score_eg = score_pawn_structure_eg(position, White) - score_pawn_structure_eg(position, Black);
    (score_mg, score_eg)
}

pub fn score_pawn_structure_mg(position: &Position, piece_color: PieceColor) -> isize {
    let board = position.board();
    let pawns = board.bitboard_by_color_and_piece_type(piece_color, Pawn);
    
    let mut score = 0isize;

    BitboardIterator::new(pawns).for_each(|pawn_square| {
        if is_part_of_chain(piece_color, pawn_square, pawns) {
            score += 15; // Bonus for pawn chain
        }

        if is_doubled_pawn(pawn_square, pawns) {
            score -= 10;
        }

        if is_isolated_pawn(pawn_square, pawns) {
            score -= 15;
        }
    });
    score
}

pub fn is_passed_pawn(square: usize, piece_color: PieceColor, their_pawns: u64) -> bool {
    let file = square % 8;
    let rank = square as isize / 8 + if piece_color == White { 1 } else { -1 };
    (PASSED_PAWN_COLUMNS[file] & PASSED_PAWNS_RANKS[piece_color as usize][rank as usize] & their_pawns) == 0
}

fn score_pawn_structure_eg(position: &Position, piece_color: PieceColor) -> isize {
    let board: &Board = position.board();
    let our_pawns = board.bitboard_by_color_and_piece_type(piece_color, Pawn);
    let their_pawns = board.bitboard_by_color_and_piece_type(!piece_color, Pawn);

    let mut score = 0isize;
    score += score_passed_pawns(position, piece_color, our_pawns, their_pawns);

    if has_pawn_majority(board, piece_color, KingSide) {
        score += 15;
    }

    if has_pawn_majority(board, piece_color, QueenSide) {
        score += 15;
    }
    score
}

fn score_passed_pawns(position: &Position, piece_color: PieceColor, our_pawns: u64, their_pawns: u64) -> isize {
    let mut score = 0isize;
    for pawn_square in BitboardIterator::new(our_pawns) {
        if is_passed_pawn(pawn_square, piece_color, their_pawns) {
            score += 20;
        }
    }
    score
}

fn has_pawn_majority(board: &Board, piece_color: PieceColor, board_side: BoardSide) -> bool {
    let pawns = [board.bitboard_by_color_and_piece_type(White, Pawn), board.bitboard_by_color_and_piece_type(Black, Pawn)];
    let (our_pawns, their_pawns) = (
        (pawns[piece_color as usize] & BITBOARD_REGIONS[board_side as usize]).count_ones(),
        (pawns[!piece_color as usize] & BITBOARD_REGIONS[board_side as usize]).count_ones()
    );
    our_pawns > their_pawns
}

fn is_part_of_chain(piece_color: PieceColor, pawn_square_index: usize, pawns: u64) -> bool {
    move_gen::squares_attacked_by_pawn(piece_color, pawn_square_index) & pawns != 0
}

fn is_isolated_pawn(square: usize, pawns: u64) -> bool {
    let file = square % 8;
    let mask = adjacent_file_mask(file);
    let neighbors = pawns & mask;
    neighbors == 0
}

fn is_doubled_pawn(square: usize, pawns: u64) -> bool {
    let file = square % 8;
    let file_mask = column_bitboard(file);
    let pawns_on_file = pawns & file_mask;
    pawns_on_file.count_ones() > 1
}

fn adjacent_file_mask(file: usize) -> u64 {
    PASSED_PAWN_COLUMNS[file] & !column_bitboard(file)
}

mod tests {
    use super::*;

    include!("../util/generated_macro.rs");

    #[test]
    fn test_is_doubled_pawn() {
        let fen = "4k3/8/6p1/p2p4/8/2p3p1/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let pawn_bitboard = position.board().bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);
        assert_eq!(is_doubled_pawn(sq!("a5"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("c3"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("d5"), pawn_bitboard), false);
        assert_eq!(is_doubled_pawn(sq!("g3"), pawn_bitboard), true);
        assert_eq!(is_doubled_pawn(sq!("g6"), pawn_bitboard), true);
    }
    #[test]
    fn test_is_isolated_pawn() {
        let fen = "4k3/8/6p1/p2p4/8/2p3p1/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let pawn_bitboard = position.board().bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);
        assert_eq!(is_isolated_pawn(sq!("a5"), pawn_bitboard), true);
        assert_eq!(is_isolated_pawn(sq!("c3"), pawn_bitboard), false);
        assert_eq!(is_isolated_pawn(sq!("d5"), pawn_bitboard), false);
        assert_eq!(is_isolated_pawn(sq!("g3"), pawn_bitboard), true);
        assert_eq!(is_isolated_pawn(sq!("g6"), pawn_bitboard), true);
    }

    #[test]
    fn test_has_pawn_majority() {
        let position: Position = Position::new_game();
        assert_eq!(has_pawn_majority(position.board(), White, KingSide), false);
        assert_eq!(has_pawn_majority(position.board(), White, QueenSide), false);
        assert_eq!(has_pawn_majority(position.board(), Black, KingSide), false);
        assert_eq!(has_pawn_majority(position.board(), Black, QueenSide), false);

        let fen = "5rk1/5p1p/2P5/8/2b5/8/8/4K1R1 b - - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(has_pawn_majority(position.board(), White, KingSide), false);
        assert_eq!(has_pawn_majority(position.board(), White, QueenSide), true);
        assert_eq!(has_pawn_majority(position.board(), Black, KingSide), true);
        assert_eq!(has_pawn_majority(position.board(), Black, QueenSide), false);
        
        let fen = "4k3/8/3p4/8/8/4P3/8/4K3 b - - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(has_pawn_majority(position.board(), White, KingSide), false);
        assert_eq!(has_pawn_majority(position.board(), White, QueenSide), false);
        assert_eq!(has_pawn_majority(position.board(), Black, KingSide), false);
        assert_eq!(has_pawn_majority(position.board(), Black, QueenSide), false);
    }

    #[test]
    fn test_is_part_of_chain() {
        let fen = "4k3/1p6/p1p3P1/3P3P/8/8/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let white_pawns = position.board().bitboard_by_color_and_piece_type(PieceColor::White, PieceType::Pawn);
        let black_pawns = position.board().bitboard_by_color_and_piece_type(PieceColor::Black, PieceType::Pawn);

        assert_eq!(is_part_of_chain(PieceColor::White, sq!("d5"), white_pawns), false);
        assert_eq!(is_part_of_chain(PieceColor::White, sq!("h5"), white_pawns), true);
        assert_eq!(is_part_of_chain(PieceColor::White, sq!("g6"), white_pawns), false);

        assert_eq!(is_part_of_chain(PieceColor::Black, sq!("b7"), black_pawns), true);
        assert_eq!(is_part_of_chain(PieceColor::Black, sq!("a6"), black_pawns), false);
        assert_eq!(is_part_of_chain(PieceColor::Black, sq!("c6"), black_pawns), false);
    }

    #[test]
    fn test_adjacent_file_mask() {
        assert_eq!(adjacent_file_mask(0), column_bitboard(1));
        assert_eq!(adjacent_file_mask(1), column_bitboard(0) | column_bitboard(2));
        assert_eq!(adjacent_file_mask(6), column_bitboard(5) | column_bitboard(7));
        assert_eq!(adjacent_file_mask(7), column_bitboard(6));
    }

    mod passed_pawns {
        use super::*;

        #[test]
        fn test_is_passed_pawn() {
            let fen = "4k3/p6p/6P1/P2P4/8/2P3P1/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            let white_pawns = position.board().bitboard_by_color_and_piece_type(White, Pawn);
            let black_pawns = position.board().bitboard_by_color_and_piece_type(Black, Pawn);
            assert_eq!(is_passed_pawn(sq!("a5"), White, black_pawns), false);
            assert_eq!(is_passed_pawn(sq!("c3"), White, black_pawns), true);
            assert_eq!(is_passed_pawn(sq!("d5"), White, black_pawns), true);
            assert_eq!(is_passed_pawn(sq!("g3"), White, black_pawns), false);
            assert_eq!(is_passed_pawn(sq!("g6"), White, black_pawns), false);

            assert_eq!(is_passed_pawn(sq!("a7"), Black, white_pawns), false);
            assert_eq!(is_passed_pawn(sq!("h7"), Black, white_pawns), false);
        }

        #[test]
        fn test_is_passed_pawn_from_wikipedia() {
            let fen = "4K3/8/7p/1P2Pp1P/2Pp1PP1/8/8/4k3 w - - 0 1";
            let position: Position = Position::from(fen);
            let white_pawns = position.board().bitboard_by_color_and_piece_type(White, Pawn);
            let black_pawns = position.board().bitboard_by_color_and_piece_type(Black, Pawn);

            assert_eq!(is_passed_pawn(sq!("b5"), White, black_pawns), true);
            assert_eq!(is_passed_pawn(sq!("c4"), White, black_pawns), true);
            assert_eq!(is_passed_pawn(sq!("e5"), White, black_pawns), true);

            assert_eq!(is_passed_pawn(sq!("f4"), White, black_pawns), false);
            assert_eq!(is_passed_pawn(sq!("g4"), White, black_pawns), false);
            assert_eq!(is_passed_pawn(sq!("h5"), White, black_pawns), false);

            assert_eq!(is_passed_pawn(sq!("d4"), Black, white_pawns), true);
            assert_eq!(is_passed_pawn(sq!("f5"), Black, white_pawns), false);
            assert_eq!(is_passed_pawn(sq!("h6"), Black, white_pawns), false);
        }
        #[test]
        fn test_simple_white_passed_pawn() {
            let fen = "8/8/8/4P3/8/8/8/5k1K w - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(Black, Pawn);
            assert_eq!(is_passed_pawn(sq!("e5"), White, pawns), true);
        }

        #[test]
        fn test_simple_black_passed_pawn() {
            let fen = "8/8/8/4p3/8/8/8/5k1K b - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(White, Pawn);
            assert_eq!(is_passed_pawn(sq!("e5"), Black, pawns), true);
        }

        #[test]
        fn test_protected_white_passed_pawn() {
            let fen = "8/8/8/3Pp3/8/8/8/k6K w - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
            assert!(is_passed_pawn(sq!("d5"), White, pawns));
        }

        #[test]
        fn test_not_a_passed_pawn_blocked() {
            let fen = "8/8/8/3Pp3/8/8/8/k6K w - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
            assert!(is_passed_pawn(sq!("e5"), White, pawns));
        }

        #[test]
        fn test_not_a_passed_pawn_adjacent_opponent() {
            let fen = "8/8/8/3pP3/8/8/8/k6K w - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
            assert!(is_passed_pawn(sq!("e5"), White, pawns));
        }

        #[test]
        fn test_connected_passed_pawns() {
            let fen = "8/8/8/3PP3/8/8/8/k6K w - - 0 1";
            let position: Position = Position::from(fen);
            let pawns = position.board().bitboard_by_color_and_piece_type(White, PieceType::Pawn);
            assert!(is_passed_pawn(sq!("d5"), White, pawns));
            assert!(is_passed_pawn(sq!("e5"), White, pawns));
        }


    }

}
