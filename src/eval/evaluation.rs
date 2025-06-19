use std::collections::HashMap;
use strum::IntoEnumIterator;
use crate::core::piece::{Piece, PieceColor, PieceType};
use crate::core::piece::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::core::move_gen;
use crate::core::piece::PieceColor::{Black, White};
use crate::core::position::Position;
use crate::engine::config::get_contempt;
use crate::eval::kings::score_kings;
use crate::eval::pawns::score_pawns;
use crate::eval::psq::score_board_psq_values;
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

pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 10000];


const PHASE_TOTAL: isize = 24;

const PHASE_WEIGHTS: [isize; 6] = [
    0, // pawn
    1, // knight
    1, // bishop
    2, // rook
    4, // queen
    0, // king
];

fn calculate_game_phase(piece_counts: [[usize; 6]; 2]) -> isize {
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
    let has_legal_move = move_gen::has_legal_move(position);
    let check_count = move_gen::king_attacks_finder(position, position.side_to_move()).count_ones() as usize;
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
    move_gen::check_count(position)
}

fn score_bishops(position: &Position) -> isize {
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




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_pieces() {
        let position: Position = Position::new_game();
        assert_eq!(score_position(&position), 0);

        let missing_white_pawn: Position = Position::from("rnbqkbnr/pppppppp/8/8/8/8/1PPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_white_pawn), -65);

        let missing_black_pawn: Position = Position::from("rnbqkbnr/1ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_position(&missing_black_pawn), 65);


        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_position(&all_black_no_white), 4730);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_position(&black_pawn_on_seventh_rank), -485);
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
        assert_eq!(score_position(&position), 106);
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
        use crate::core::move_gen::has_legal_move;
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