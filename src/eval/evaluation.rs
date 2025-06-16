use std::collections::HashMap;
use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use crate::util::bitboard_iterator::BitboardIterator;
use crate::core::piece::{Piece, PieceColor, PieceType};
use crate::core::piece::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::util::util;
use crate::core::board::Board;
use crate::core::move_generator;
use crate::core::piece::PieceColor::{Black, White};
use crate::core::position::Position;
use crate::engine::config::get_contempt;
use crate::eval::kings::score_kings;
use crate::eval::pawns::score_pawns;
use crate::search;
use crate::search::negamax::MAXIMUM_SCORE;

include!("../util/generated_macro.rs");

#[derive(Copy, Clone, Debug)]
#[derive(Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum GameStatus {
    InProgress,
    DrawnByFiftyMoveRule,
    DrawnByThreefoldRepetition,
    DrawnByInsufficientMaterial,
    Stalemate,
    Checkmate,
}

static COLUMN_SQUARE_INDEXES: Lazy<[u64; 8]> = Lazy::new(|| {
    let mut result = [0; 8];
    for column_index in 0..8 {
        result[column_index] = util::filter_bits(!0, |square_index| square_index % 8 == column_index as u64);
    }
    result
});


pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 10000];


pub const MG_PST: [[isize; 64]; 6] = [
    [ // mg pawns
        0,   0,   0,   0,   0,   0,  0,   0,
        98, 134,  61,  95,  68, 126, 34, -11,
        -6,   7,  26,  31,  65,  56, 25, -20,
        -14,  13,   6,  21,  23,  12, 17, -23,
        -27,  -2,  -5,  12,  17,   6, 10, -25,
        -26,  -4,  -4, -10,   3,   3, 33, -12,
        -35,  -1, -20, -23, -15,  24, 38, -22,
        0,   0,   0,   0,   0,   0,  0,   0,
    ],
    [ // mg knights
        -167, -89, -34, -49,  61, -97, -15, -107,
        -73, -41,  72,  36,  23,  62,   7,  -17,
        -47,  60,  37,  65,  84, 129,  73,   44,
        -9,  17,  19,  53,  37,  69,  18,   22,
        -13,   4,  16,  13,  28,  19,  21,   -8,
        -23,  -9,  12,  10,  19,  17,  25,  -16,
        -29, -53, -12,  -3,  -1,  18, -14,  -19,
        -105, -21, -58, -33, -17, -28, -19,  -23,
    ],
    [ // mg bishops
        -29,   4, -82, -37, -25, -42,   7,  -8,
        -26,  16, -18, -13,  30,  59,  18, -47,
        -16,  37,  43,  40,  35,  50,  37,  -2,
        -4,   5,  19,  50,  37,  37,   7,  -2,
        -6,  13,  13,  26,  34,  12,  10,   4,
        0,  15,  15,  15,  14,  27,  18,  10,
        4,  15,  16,   0,   7,  21,  33,   1,
        -33,  -3, -14, -21, -13, -12, -39, -21,
    ],
    [// mg rooks
        32,  42,  32,  51, 63,  9,  31,  43,
        27,  32,  58,  62, 80, 67,  26,  44,
        -5,  19,  26,  36, 17, 45,  61,  16,
        -24, -11,   7,  26, 24, 35,  -8, -20,
        -36, -26, -12,  -1,  9, -7,   6, -23,
        -45, -25, -16, -17,  3,  0,  -5, -33,
        -44, -16, -20,  -9, -1, 11,  -6, -71,
        -19, -13,   1,  17, 16,  7, -37, -26,
    ],
    [// mg queens
        -28,   0,  29,  12,  59,  44,  43,  45,
        -24, -39,  -5,   1, -16,  57,  28,  54,
        -13, -17,   7,   8,  29,  56,  47,  57,
        -27, -27, -16, -16,  -1,  17,  -2,   1,
        -9, -26,  -9, -10,  -2,  -4,   3,  -3,
        -14,   2, -11,  -2,  -5,   2,  14,   5,
        -35,  -8,  11,   2,   8,  15,  -3,   1,
        -1, -18,  -9,  10, -15, -25, -31, -50,
    ],
    [// mg kings
        -65,  23,  16, -15, -56, -34,   2,  13,
        29,  -1, -20,  -7,  -8,  -4, -38, -29,
        -9,  24,   2, -16, -20,   6,  22, -22,
        -17, -20, -12, -27, -30, -25, -14, -36,
        -49,  -1, -27, -39, -46, -44, -33, -51,
        -14, -14, -22, -46, -44, -30, -15, -27,
        1,   7,  -8, -64, -43, -16,   9,   8,
        -15,  36,  12, -54,   8, -28,  24,  14,
    ],
];

pub const EG_PST: [[isize; 64]; 6] = [
    [ // eg pawns
        0,   0,   0,   0,   0,   0,   0,   0,
        178, 173, 158, 134, 147, 132, 165, 187,
        94, 100,  85,  67,  56,  53,  82,  84,
        32,  24,  13,   5,  -2,   4,  17,  17,
        13,   9,  -3,  -7,  -7,  -8,   3,  -1,
        4,   7,  -6,   1,   0,  -5,  -1,  -8,
        13,   8,   8,  10,  13,   0,   2,  -7,
        0,   0,   0,   0,   0,   0,   0,   0,
    ],
    [ // eg knights
        -58, -38, -13, -28, -31, -27, -63, -99,
        -25,  -8, -25,  -2,  -9, -25, -24, -52,
        -24, -20,  10,   9,  -1,  -9, -19, -41,
        -17,   3,  22,  22,  22,  11,   8, -18,
        -18,  -6,  16,  25,  16,  17,   4, -18,
        -23,  -3,  -1,  15,  10,  -3, -20, -22,
        -42, -20, -10,  -5,  -2, -20, -23, -44,
        -29, -51, -23, -15, -22, -18, -50, -64,
    ],
    [ // eg bishops
        -14, -21, -11,  -8, -7,  -9, -17, -24,
        -8,  -4,   7, -12, -3, -13,  -4, -14,
        2,  -8,   0,  -1, -2,   6,   0,   4,
        -3,   9,  12,   9, 14,  10,   3,   2,
        -6,   3,  13,  19,  7,  10,  -3,  -9,
        -12,  -3,   8,  10, 13,   3,  -7, -15,
        -14, -18,  -7,  -1,  4,  -9, -15, -27,
        -23,  -9, -23,  -5, -9, -16,  -5, -17,
    ],
    [// eg rooks
        13, 10, 18, 15, 12,  12,   8,   5,
        11, 13, 13, 11, -3,   3,   8,   3,
        7,  7,  7,  5,  4,  -3,  -5,  -3,
        4,  3, 13,  1,  2,   1,  -1,   2,
        3,  5,  8,  4, -5,  -6,  -8, -11,
        -4,  0, -5, -1, -7, -12,  -8, -16,
        -6, -6,  0,  2, -9,  -9, -11,  -3,
        -9,  2,  3, -1, -5, -13,   4, -20,
    ],
    [// eg queens
        -9,  22,  22,  27,  27,  19,  10,  20,
        -17,  20,  32,  41,  58,  25,  30,   0,
        -20,   6,   9,  49,  47,  35,  19,   9,
        3,  22,  24,  45,  57,  40,  57,  36,
        -18,  28,  19,  47,  31,  34,  39,  23,
        -16, -27,  15,   6,   9,  17,  10,   5,
        -22, -23, -30, -16, -16, -23, -36, -32,
        -33, -28, -22, -43,  -5, -32, -20, -41
    ],
    [// eg kings
        -74, -35, -18, -18, -11,  15,   4, -17,
        -12,  17,  14,  17,  17,  38,  23,  11,
        10,  17,  23,  15,  20,  45,  44,  13,
        -8,  22,  24,  27,  26,  33,  26,   3,
        -18,  -4,  21,  24,  27,  23,   9, -11,
        -19,  -3,  11,  21,  23,  16,   7,  -9,
        -27, -11,   4,  13,  14,   4,  -5, -17,
        -53, -34, -21, -11, -28, -14, -24, -43
    ],
];

pub const PHASE_TOTAL: isize = 24;

pub const PHASE_WEIGHTS: [isize; 6] = [
    0, // pawn
    1, // knight
    1, // bishop
    2, // rook
    4, // queen
    0, // king
];

pub fn calculate_game_phase(piece_counts: [[usize; 6]; 2]) -> isize {
    let mut phase = PHASE_TOTAL;

    for piece_type in [Knight, Bishop, Rook, Queen] {
        let count = piece_counts[White as usize][piece_type as usize] + piece_counts[Black as usize][piece_type as usize];
        phase -= PHASE_WEIGHTS[piece_type as usize] * count as isize;
    }

    phase.clamp(0, PHASE_TOTAL)
}

pub fn apply_contempt(score: isize) -> isize {
    let contempt = get_contempt();
    
    if score == 0 {
        -contempt
    } else {
        score
    }
}

pub fn score_bishops(position: &Position) -> isize {
    let board = position.board();
    board.has_bishop_pair(White) as isize * 50 - board.has_bishop_pair(Black) as isize * 50
}

fn calculate_material_balance(piece_counts: [[usize; 6]; 2]) -> [isize; 6] {
    let mut result = [0; 6];
    for piece_type in PieceType::iter() {
        result[piece_type as usize] =  
            piece_counts[White as usize][piece_type as usize] as isize - piece_counts[Black as usize][piece_type as usize] as isize;
    }
    result
}

fn score_board_psq_values(board: &Board) -> (isize, isize) {
    let (white_mg, white_eg) = score_board_psq_values_for_color(board, White);
    let (black_mg, black_eg) = score_board_psq_values_for_color(board, Black);
    (white_mg - black_mg, white_eg - black_eg)
}
fn score_board_psq_values_for_color(board: &Board, color: PieceColor) -> (isize, isize) {
    let mut mg_score = 0;
    let mut eg_score = 0;
    let bitboards = board.bitboards_for_color(color);
    let square_index_xor = if color == White { 56 } else { 0 };
    for piece_type in PieceType::iter() {
        for square_index in  BitboardIterator::new(bitboards[piece_type as usize]) {
            mg_score += MG_PST[piece_type as usize][square_index ^ square_index_xor];
            eg_score += EG_PST[piece_type as usize][square_index ^ square_index_xor];
        }
    }
    (mg_score, eg_score)
}

pub fn score_position(position: &Position) -> isize {
    let board = position.board();
    let piece_counts = board.get_piece_counts();
    let phase = calculate_game_phase(piece_counts);
    let piece_material_balance = calculate_material_balance(piece_counts);
    let material_score = piece_material_balance.iter().enumerate().map(|(idx, &balance)| {
        balance * PIECE_SCORES[idx]
    })   
        .sum::<isize>();

    let (psq_mg, psq_eg) = score_board_psq_values(board);
    let (king_mg, king_eg) = score_kings(position);
    let (pawn_mg, pawn_eg) = score_pawns(position);
    
    let (score_mg, score_eg) = (psq_mg + king_mg + pawn_mg, psq_eg + king_eg + pawn_eg);
    let blended_score = (score_mg * (PHASE_TOTAL - phase) + score_eg * phase) / PHASE_TOTAL;

    let score =  blended_score
        + material_score
        + score_bishops(position);

    if position.side_to_move() == White { score } else { -score }
}

pub fn evaluate(position: &Position, depth: usize, historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> isize {
    let game_status = get_game_status(position, historic_repeat_position_counts.cloned());
    match game_status {
        GameStatus::InProgress => {
            let score = score_position(position);
            if score != 0 { score } else { -1 }
        },
        GameStatus::Checkmate => depth as isize - MAXIMUM_SCORE,
        _ => apply_contempt(0), // Apply contempt to drawn positions
    }
}

pub fn has_insufficient_material(position: &Position) -> bool {
    let board = position.board();
    let all_bitboards = &board.all_bitboards();
    for piece_color in PieceColor::iter() {
        for piece_type in [Pawn, Rook, Queen] {
            if all_bitboards[piece_color as usize][piece_type as usize] != 0 {
                return false;
            }
        }
    }
    let whites_bishop_count = board.get_piece_count(White, Bishop);
    let blacks_bishop_count = board.get_piece_count(Black, Bishop);
    let whites_knight_count = board.get_piece_count(White, Knight);
    let blacks_knight_count = board.get_piece_count(Black, Knight);
    let whites_minor_piece_count = whites_bishop_count + whites_knight_count;
    let blacks_minor_piece_count = blacks_bishop_count + blacks_knight_count;

    if (whites_minor_piece_count <= 1) && (blacks_minor_piece_count <= 1) {
        return true;
    }

    if blacks_minor_piece_count == 0 && whites_minor_piece_count == 2 {
        if whites_knight_count == 2 || (whites_bishop_count == 2 && board.has_bishops_on_same_color_squares(White)) {
            return true;
        }
    } else if whites_minor_piece_count == 0 && blacks_minor_piece_count == 2 {
        if blacks_knight_count == 2 || (blacks_bishop_count == 2 && board.has_bishops_on_same_color_squares(Black)) {
            return true;
        }
    }
    false
}

pub fn get_game_status(position: &Position, historic_repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> GameStatus {
    let has_legal_move = move_generator::has_legal_move(position);
    let check_count = move_generator::king_attacks_finder(position, position.side_to_move()).count_ones() as usize;
    match (!has_legal_move, check_count > 0) {
        (true, true) => GameStatus::Checkmate,
        (true, false) => GameStatus::Stalemate,
        _ => {
            if position.half_move_clock() >= 100 {
                GameStatus::DrawnByFiftyMoveRule
            } else if has_three_fold_repetition(position, historic_repeat_position_counts) {
                GameStatus::DrawnByThreefoldRepetition
            } else if has_insufficient_material(&position) {
                GameStatus::DrawnByInsufficientMaterial
            } else {
                GameStatus::InProgress
            }
        }
    }
}
pub fn has_three_fold_repetition(position: &Position, historic_repeat_position_counts: Option<HashMap<u64, (Position, usize)>>) -> bool {
    search::negamax::get_repeat_position_count(position, &*vec!(), historic_repeat_position_counts.as_ref()) >= 3
}
pub fn is_check(position: &Position) -> bool {
    check_count(position) >= 1
}

pub fn check_count(position: &Position) -> usize { 
    move_generator::check_count(position)
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_pieces() {
        let position: Position = Position::new_game();
        assert_eq!(score_position(&position), 0);

        let missing_white_pawn: Position = Position::from("rnbqkbnr/pppppppp/8/8/8/8/PPP1PPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_white_pawn), -97);

        let missing_black_pawn: Position = Position::from("rnbqkbnr/1ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_black_pawn), 85);


        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_position(&all_black_no_white), 3950);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_position(&black_pawn_on_seventh_rank), -280);
    }

    #[test]
    fn test_score_board_material_balance() {
        let position = Position::new_game();
        let board = position.board();
        assert_eq!(score_board_psq_values_for_color(&board, White), (-147, -193));
        assert_eq!(score_board_psq_values_for_color(&board, Black), (-147, -193));
        
        let mut board = Board::new();
        assert_eq!(score_board_psq_values_for_color(&board, White), (0, 0));
        assert_eq!(score_board_psq_values_for_color(&board, Black), (0, 0));
        
        board.put_piece(sq!("a2"), Piece { piece_color: White, piece_type: Pawn});
        assert_eq!(score_board_psq_values_for_color(&board, White), (-35, 13));

        board.put_piece(sq!("b2"), Piece { piece_color: White, piece_type: Queen});
        board.put_piece(sq!("b3"), Piece { piece_color: White, piece_type: Queen});
        board.put_piece(sq!("b4"), Piece { piece_color: White, piece_type: Queen});
        board.put_piece(sq!("b5"), Piece { piece_color: Black, piece_type: Queen});
        board.put_piece(sq!("b6"), Piece { piece_color: Black, piece_type: Queen});
        board.put_piece(sq!("b7"), Piece { piece_color: Black, piece_type: Queen});
        assert_eq!(score_board_psq_values_for_color(&board, White), (-67, -9));
        board.remove_piece(sq!("b2"));
        assert_eq!(score_board_psq_values_for_color(&board, White), (-59, 14));
    }
    #[test]
    fn test_calculate_new_game_phase() {
        let position: Position = Position::new_game();
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 0);
    }

    #[test]
    fn test_calculate_no_queens_game_phase() {
        let position: Position = Position::from("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1");
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 8);
    }

    #[test]
    fn test_calculate_empty_board_game_phase() {
        let position: Position = Position::from("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 24);
    }

    #[test]
    fn test_calculate_material_balance() {
        let position: Position = Position::from("r1bqkbn1/1ppppppp/8/8/8/8/PPPPP3/RN2KBN1 w Qq - 0 1");
        assert_eq!(calculate_material_balance(position.board().get_piece_counts()), [-2, 1, -1, 0, -1, 0]);
    }

    #[test]
    fn test_pawn_scores() {
        let position: Position = Position::from("4k3/P7/8/8/8/6p1/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 94);
    }

    #[test]
    fn test_knight_scores() {
        let position: Position = Position::from("N3k3/8/8/4n3/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), -86);
    }

    #[test]
    fn test_bishop_scores() {
        let position: Position = Position::from("b3k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 41);
    }

    #[test]
    fn test_rook_scores() {
        let position: Position = Position::from("4k1r1/8/R7/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 5);
    }

    #[test]
    fn test_queen_scores() {
        let position: Position = Position::from("4k1q1/8/QQ6/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 901);
    }

    #[test]
    fn test_king_scores() {
        let position: Position = Position::from("8/7k/8/8/8/2K5/8/8 w - - 0 1");
        assert_eq!(score_position(&position), 30);
    }

    #[test]
    fn test_score_board_for_color() {
        assert_eq!(COLUMN_SQUARE_INDEXES[0], 1 << 0 | 1 << 8 | 1 << 16 | 1 << 24 | 1 << 32 | 1 << 40 | 1 << 48 | 1 << 56);
        assert_eq!(COLUMN_SQUARE_INDEXES[7], 1 << 7 | 1 << 15 | 1 << 23 | 1 << 31 | 1 << 39 | 1 << 47 | 1 << 55 | 1 << 63);
    }

    #[test]
    fn test_score_bishops() {
        let position: Position = Position::from("r2qk1nr/pppb1ppp/2n1b3/3pp3/3PP3/3B1N2/PPPB1PPP/RN1QK2R w KQkq - 0 1");
        assert_eq!(score_bishops(&position), 50);
    }

    mod insufficient_material {
        use super::*;

        #[test]
        fn test_new_game() {
            let position = Position::new_game();
            assert_eq!(has_insufficient_material(&position), false);
        }

        #[test]
        fn test_only_kings() {
            let fen = "4k3/8/8/8/8/8/8/3K4 b - - 1 1";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), true);
        }

        #[test]
        fn test_has_one_queen() {
            let fen = "4k3/8/8/8/8/8/4q3/1K6 b - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), false);
        }
        #[test]
        fn test_has_one_rook() {
            let fen = "4k3/8/8/8/8/8/4r3/1K6 b - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), false);
        }
        #[test]
        fn test_has_one_bishop() {
            let fen = "4k3/8/8/8/8/8/4b3/1K6 b - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), true);
        }
        #[test]
        fn test_has_one_knight() {
            let fen = "4k3/8/8/8/8/8/4n3/1K6 b - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), true);
        }
        #[test]
        fn test_has_two_knights() {
            let fen = "4k3/8/8/8/8/8/n3n3/1K6 b - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), true);
        }

        #[test]
        fn test_has_two_bishops_on_same_color_squares() {
            let fen = "4k3/1b6/8/8/6b1/8/8/1K6 w - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), true);
        }

        #[test]
        fn test_has_two_bishops_on_different_color_squares() {
            let fen = "4k3/1b6/8/8/5b2/8/8/1K6 w - - 5 3";
            let position = Position::from(fen);
            assert_eq!(has_insufficient_material(&position), false);
        }
    }

    #[cfg(test)]
    mod game_tests {
        use crate::core::move_generator::has_legal_move;
        use crate::eval::evaluation;
        use super::*;
        #[test]
        fn test_double_check() {
            let fen = "2r2q1k/5pp1/4p1N1/8/1bp5/5P1R/6P1/2R4K b - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), true);
            assert_eq!(check_count(&position), 2);
            assert_eq!(get_game_status(&position, None), GameStatus::InProgress);
            assert_eq!(has_legal_move(&position), true);
        }

        #[test]
        fn test_checkmate() {
            let fen = "8/8/8/5k1K/8/8/8/7r w - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), true);
            assert_eq!(check_count(&position), 1);
            assert_eq!(get_game_status(&position, None), GameStatus::Checkmate);
            assert_eq!(has_legal_move(&position), false);
        }

        #[test]
        fn test_stalemate() {
            let fen = "7K/5k2/5n2/8/8/8/8/8 w - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), false);
            assert_eq!(check_count(&position), 0);
            assert_eq!(get_game_status(&position, None), GameStatus::Stalemate);
            assert_eq!(has_legal_move(&position), false);
        }
    }
}