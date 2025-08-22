use crate::core::board::Board;
use crate::core::move_gen;
use crate::core::piece::{PieceColor, PieceType};
use crate::core::position::Position;
use crate::uci::config::get_contempt;
use crate::eval::kings::score_kings;
use crate::eval::pawns::score_pawns;
use crate::eval::psq::score_board_psq_values;
use crate::search::negamax::{RepetitionKey, Search, MAXIMUM_SCORE};
use crate::utils::bitboard_iterator::BitboardIterator;
use crate::utils::util;
use crate::utils::util::row_bitboard;
use strum::IntoEnumIterator;

include!("../utils/generated_macro.rs");

#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum GameStatus {
    #[default]
    InProgress,
    DrawnByFiftyMoveRule,
    DrawnByThreefoldRepetition,
    DrawnByInsufficientMaterial,
    Stalemate,
    Draw, // Fallback for unknown draws
    Checkmate,
}

pub const PIECE_SCORES: [i32; 6] = [100, 300, 300, 500, 900, 10000];

const PHASE_TOTAL: i32 = 24;

const PHASE_WEIGHTS: [i32; 6] = [
    0, // pawn
    1, // knight
    1, // bishop
    2, // rook
    4, // queen
    0, // king
];

const BISHOP_PAIR_BONUS: i32 = 50;
const ROOK_ON_OPEN_FILE_BONUS: i32 = 30;
const DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS: i32 = 75;

fn calculate_game_phase(piece_counts: [[usize; 6]; 2]) -> i32 {
    let mut phase = PHASE_TOTAL;

    for piece_type in [PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
        let count = piece_counts[PieceColor::White as usize][piece_type as usize]
            + piece_counts[PieceColor::Black as usize][piece_type as usize];
        phase -= PHASE_WEIGHTS[piece_type as usize] * count as i32;
    }

    phase.clamp(0, PHASE_TOTAL)
}

pub fn apply_contempt(score: i32) -> i32 {
    if score == 0 {
        -get_contempt()
    } else {
        score
    }
}

pub fn score_position(position: &Position) -> i32 {
    let board = position.board();
    let piece_counts = board.get_piece_counts();
    let phase = calculate_game_phase(piece_counts);
    let piece_material_balance = calculate_material_balance(piece_counts);
    let material_score = piece_material_balance
        .iter()
        .enumerate()
        .map(|(idx, &balance)| balance as i32 * PIECE_SCORES[idx])
        .sum::<i32>();

    let (psq_mg, psq_eg) = score_board_psq_values(board);
    let (king_mg, king_eg) = score_kings(position);
    let (pawn_mg, pawn_eg) = score_pawns(position);

    let (score_mg, score_eg) = (psq_mg + king_mg + pawn_mg, psq_eg + king_eg + pawn_eg);
    let blended_score = (score_mg * (PHASE_TOTAL - phase) + score_eg * phase) / PHASE_TOTAL;

    let mut score =
        blended_score + material_score + score_bishops(position) + score_rooks(position);

    if score == 0 {
        score = -1;
    }
    if position.side_to_move() == PieceColor::White {
        score
    } else {
        -score
    }
}

pub fn evaluate(position: &Position, depth: u8, repetition_key_stack: &[RepetitionKey]) -> i32 {
    let game_status = get_game_status(position, repetition_key_stack);
    match game_status {
        GameStatus::InProgress => score_position(position),
        GameStatus::Checkmate => depth as i32 - MAXIMUM_SCORE,
        _ => apply_contempt(0),
    }
}
pub fn has_insufficient_material(position: &Position) -> bool {
    let board = position.board();
    let all_bitboards = &board.all_bitboards();
    for piece_color in PieceColor::iter() {
        for piece_type in [PieceType::Pawn, PieceType::Rook, PieceType::Queen] {
            if all_bitboards[piece_color as usize][piece_type as usize] != 0 {
                return false;
            }
        }
    }
    let whites_bishop_count = board.get_piece_count(PieceColor::White, PieceType::Bishop);
    let blacks_bishop_count = board.get_piece_count(PieceColor::Black, PieceType::Bishop);
    let whites_knight_count = board.get_piece_count(PieceColor::White, PieceType::Knight);
    let blacks_knight_count = board.get_piece_count(PieceColor::Black, PieceType::Knight);
    let whites_minor_piece_count = whites_bishop_count + whites_knight_count;
    let blacks_minor_piece_count = blacks_bishop_count + blacks_knight_count;

    if (whites_minor_piece_count <= 1) && (blacks_minor_piece_count <= 1) {
        return true;
    }

    let has_insufficient_minor_pieces =
        |piece_color: PieceColor, knight_count: usize, bishop_count: usize| -> bool {
            knight_count == 2
                || (bishop_count == 2 && board.has_bishops_on_same_color_squares(piece_color))
        };

    if blacks_minor_piece_count == 0
        && whites_minor_piece_count == 2
        && has_insufficient_minor_pieces(
            PieceColor::White,
            whites_knight_count,
            whites_bishop_count,
        )
    {
        return true;
    }
    if whites_minor_piece_count == 0
        && blacks_minor_piece_count == 2
        && has_insufficient_minor_pieces(
            PieceColor::Black,
            blacks_knight_count,
            blacks_bishop_count,
        )
    {
        return true;
    }
    false
}

pub fn get_game_status(position: &Position, repetition_key_stack: &[RepetitionKey]) -> GameStatus {
    let has_legal_move = move_gen::has_legal_move(position);
    let check_count = move_gen::check_count(position);
    match (!has_legal_move, check_count > 0) {
        (true, true) => GameStatus::Checkmate,
        (true, false) => GameStatus::Stalemate,
        _ => {
            if position.half_move_clock() >= 100 {
                GameStatus::DrawnByFiftyMoveRule
            } else if Search::position_occurrence_count(repetition_key_stack) >= 3 {
                GameStatus::DrawnByThreefoldRepetition
            } else if has_insufficient_material(position) {
                GameStatus::DrawnByInsufficientMaterial
            } else {
                GameStatus::InProgress
            }
        }
    }
}
pub fn is_check(position: &Position) -> bool {
    check_count(position) >= 1
}

pub fn check_count(position: &Position) -> usize {
    move_gen::check_count(position)
}

fn score_bishops(position: &Position) -> i32 {
    let board = position.board();
    (board.has_bishop_pair(PieceColor::White) as i32
        - board.has_bishop_pair(PieceColor::Black) as i32)
        * BISHOP_PAIR_BONUS
}

fn score_rooks(position: &Position) -> i32 {
    fn score_rooks_for_color(board: &Board, piece_color: PieceColor) -> i32 {
        let my_bitboards = board.bitboards_for_color(piece_color);
        let pawns = my_bitboards[PieceType::Pawn as usize];
        let rooks = my_bitboards[PieceType::Rook as usize];
        let queens = my_bitboards[PieceType::Queen as usize];
        let row = if piece_color == PieceColor::White { 6 } else { 1 };
        let seventh_rank_bonus = ((((rooks | queens) & row_bitboard(row)).count_ones()) >= 2)
            as i32
            * DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS;
        let mut on_open_file_count = 0;
        let rook_iterator = BitboardIterator::new(rooks);
        for rook_index in rook_iterator {
            if util::column_bitboard(rook_index % 8) & (pawns) == 0 {
                on_open_file_count += 1;
            }
        }
        seventh_rank_bonus + on_open_file_count * ROOK_ON_OPEN_FILE_BONUS
    }
    let board = position.board();
    score_rooks_for_color(board, PieceColor::White)
        - score_rooks_for_color(board, PieceColor::Black)
}

fn calculate_material_balance(piece_counts: [[usize; 6]; 2]) -> [isize; 6] {
    let mut result = [0; 6];
    for piece_type in PieceType::iter() {
        result[piece_type as usize] = piece_counts[PieceColor::White as usize][piece_type as usize]
            as isize
            - piece_counts[PieceColor::Black as usize][piece_type as usize] as isize;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::negamax::Search;

    #[test]
    fn test_score_pieces() {
        let position: Position = Position::new_game();
        assert_eq!(score_position(&position), -1);

        let missing_white_pawn: Position =
            Position::from("rnbqkbnr/pppppppp/8/8/8/8/1PPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_white_pawn), -35);

        let missing_black_pawn: Position =
            Position::from("rnbqkbnr/1ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_black_pawn), 35);

        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_position(&all_black_no_white), 4060);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_position(&black_pawn_on_seventh_rank), -310);
    }

    #[test]
    fn test_get_repetition_count() {
        assert_eq!(Search::position_occurrence_count(&vec!()), 0);

        let k1 = || RepetitionKey { zobrist_hash: 1, half_move_clock: 100 };
        let k2 = || RepetitionKey { zobrist_hash: 2, half_move_clock: 100 };
        let k3 = || RepetitionKey { zobrist_hash: 2, half_move_clock: 0 };
        assert_eq!(Search::position_occurrence_count(&vec![]), 0);
        assert_eq!(Search::position_occurrence_count(&vec![k1()]), 1);
        assert_eq!(Search::position_occurrence_count(&vec![k2(), k1()]), 1);
        assert_eq!(Search::position_occurrence_count(&vec![k1(), k2(), k1()]), 2);
        assert_eq!(Search::position_occurrence_count(&vec![k2(), k3(), k1(), k2()]), 2);
        assert_eq!(Search::position_occurrence_count(&vec![k2(), k3(), k1(), k3()]), 1);
        assert_eq!(Search::position_occurrence_count(&vec![k2(), k2(), k2(), k2(), k2()]), 5);
        assert_eq!(Search::position_occurrence_count(&vec![k2(), k3(), k2(), k2(), k2()]), 4);
    }

    #[test]
    fn test_calculate_new_game_phase() {
        let position: Position = Position::new_game();
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 0);
    }

    #[test]
    fn test_calculate_no_queens_game_phase() {
        let position: Position =
            Position::from("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1");
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 8);
    }

    #[test]
    fn test_calculate_empty_board_game_phase() {
        let position: Position = Position::from("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(calculate_game_phase(position.board().get_piece_counts()), 24);
    }

    #[test]
    fn test_calculate_material_balance() {
        let position: Position =
            Position::from("r1bqkbn1/1ppppppp/8/8/8/8/PPPPP3/RN2KBN1 w Qq - 0 1");
        assert_eq!(
            calculate_material_balance(position.board().get_piece_counts()),
            [-2, 1, -1, 0, -1, 0]
        );
    }

    #[test]
    fn test_pawn_scores() {
        let position: Position = Position::from("4k3/P7/8/8/8/6p1/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 96);
    }

    #[test]
    fn test_knight_scores() {
        let position: Position = Position::from("N3k3/8/8/4n3/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), -84);
    }

    #[test]
    fn test_bishop_scores() {
        let position: Position = Position::from("b3k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 43);
    }

    #[test]
    fn test_rook_scores() {
        let position: Position = Position::from("4k1r1/8/R7/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 7);
    }

    #[test]
    fn test_queen_scores() {
        let position: Position = Position::from("4k1q1/8/QQ6/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_position(&position), 903);
    }

    #[test]
    fn test_king_scores() {
        let position: Position = Position::from("8/7k/8/8/8/2K5/8/8 w - - 0 1");
        assert_eq!(score_position(&position), 28);
    }

    mod bishops {
        use super::*;
        #[test]
        fn test_score_bishop_pairs() {
            let position: Position = Position::from(
                "r2qk1nr/pppb1ppp/2n1b3/3pp3/3PP3/3B1N2/PPPB1PPP/RN1QK2R w KQkq - 0 1",
            );
            assert_eq!(score_bishops(&position), BISHOP_PAIR_BONUS);
        }
    }

    mod rooks {
        use super::*;
        #[test]
        fn test_score_doubled_rooks_on_seventh_rank() {
            let position: Position = Position::from("4k3/1R5R/8/8/8/8/7P/4K3 w - - 0 1");
            assert_eq!(
                score_rooks(&position),
                DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS + ROOK_ON_OPEN_FILE_BONUS
            );

            let position: Position = Position::from("4k3/p6p/8/8/8/8/r6r/4K3 w - - 0 1");
            assert_eq!(score_rooks(&position), -DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS);
        }

        #[test]
        fn test_score_rook_and_queen_on_seventh_rank() {
            let position: Position = Position::from("4k3/6QR/8/8/8/8/7P/4K3 w - - 0 1");
            assert_eq!(score_rooks(&position), DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS);

            let position: Position = Position::from("4k3/7p/8/8/8/8/6qr/4K3 w - - 0 1");
            assert_eq!(score_rooks(&position), -DOUBLED_ROOKS_ON_SEVENTH_RANK_BONUS);
        }

        #[test]
        fn test_rook_on_open_file() {
            let position: Position = Position::from("4k3/8/8/8/8/8/5P1P/4KRRR w K - 0 1");
            assert_eq!(score_rooks(&position), ROOK_ON_OPEN_FILE_BONUS);

            let position: Position = Position::from("2rrk2r/8/3p4/8/8/8/8/4K3 w k - 0 1");
            assert_eq!(score_rooks(&position), -(ROOK_ON_OPEN_FILE_BONUS * 2));
        }
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
        use super::*;
        use crate::core::move_gen::has_legal_move;
        #[test]
        fn test_double_check() {
            let fen = "2r2q1k/5pp1/4p1N1/8/1bp5/5P1R/6P1/2R4K b - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), true);
            assert_eq!(check_count(&position), 2);
            assert_eq!(get_game_status(&position, &vec!()), GameStatus::InProgress);
            assert_eq!(has_legal_move(&position), true);
        }

        #[test]
        fn test_checkmate() {
            let fen = "8/8/8/5k1K/8/8/8/7r w - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), true);
            assert_eq!(check_count(&position), 1);
            assert_eq!(get_game_status(&position, &vec!()), GameStatus::Checkmate);
            assert_eq!(has_legal_move(&position), false);
        }

        #[test]
        fn test_stalemate() {
            let fen = "7K/5k2/5n2/8/8/8/8/8 w - - 0 1";
            let position = Position::from(fen);
            assert_eq!(is_check(&position), false);
            assert_eq!(check_count(&position), 0);
            assert_eq!(get_game_status(&position, &vec!()), GameStatus::Stalemate);
            assert_eq!(has_legal_move(&position), false);
        }
    }
}
