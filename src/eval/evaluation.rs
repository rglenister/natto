use std::collections::HashMap;
use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use crate::chessboard::board::Board;
use crate::chessboard::piece::{PieceColor};
use crate::chessboard::piece::PieceType::{King, Knight, Pawn, Queen};
use crate::search::negamax::MAXIMUM_SCORE;
use crate::game::Game;
use crate::chessboard::piece::{Piece, PieceColor, PieceType};
use crate::chessboard::piece::PieceType::{Bishop, King, Knight, Pawn, Queen};
use crate::position::Position;
use crate::game;
use crate::chess_util::util;
use crate::chessboard::piece::PieceColor::{Black, White};
use crate::eval::kings::score_king_safety;
use crate::eval::pawns::score_pawn_structure;

static COLUMN_SQUARE_INDEXES: Lazy<[u64; 8]> = Lazy::new(|| {
    let mut result = [0; 8];
    for column_index in 0..8 {
        result[column_index] = util::filter_bits(!0, |square_index| square_index % 8 == column_index as u64);
    }
    result
});


pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 10000];


pub const PIECE_SCORE_ADJUSTMENT_TABLE: [[isize; 64]; 6] = [
    [ // pawns
        0,  0,  0,  0,  0,  0,  0,  0,
        5, 10, 10,-20,-20, 10, 10,  5,
        5, -5,-10,  0,  0,-10, -5,  5,
        0,  0,  0, 20, 20,  0,  0,  0,
        5,  5, 10, 25, 25, 10,  5,  5,
        10, 10, 20, 30, 30, 20, 10, 10,
        50, 50, 50, 50, 50, 50, 50, 50,
        0,  0,  0,  0,  0,  0,  0,  0,
    ],
    [ // knights
        -50,-40,-30,-30,-30,-30,-40,-50,
        -40,-20,  0,  0,  0,  0,-20,-40,
        -30,  0, 10, 15, 15, 10,  0,-30,
        -30,  5, 15, 20, 20, 15,  5,-30,
        -30,  0, 15, 20, 20, 15,  0,-30,
        -30,  5, 10, 15, 15, 10,  5,-30,
        -40,-20,  0,  5,  5,  0,-20,-40,
        -50,-40,-30,-30,-30,-30,-40,-50,
    ],
    [ // bishops
        -20,-10,-10,-10,-10,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5, 10, 10,  5,  0,-10,
        -10,  5,  5, 10, 10,  5,  5,-10,
        -10,  0, 10, 10, 10, 10,  0,-10,
        -10, 10, 10, 10, 10, 10, 10,-10,
        -10,  5,  0,  0,  0,  0,  5,-10,
        -20,-10,-10,-10,-10,-10,-10,-20,
    ],
    [// rooks
        0,  0,  0,  5,  5,  0,  0,  0,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        5, 10, 10, 10, 10, 10, 10,  5,
        0,  0,  0,  0,  0,  0,  0,  0,
    ],
    [// queens
        -20,-10,-10, -5, -5,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5,  5,  5,  5,  0,-10,
        -5,  0,  5,  5,  5,  5,  0, -5,
        0,  0,  5,  5,  5,  5,  0, -5,
        -10,  5,  5,  5,  5,  5,  0,-10,
        -10,  0,  5,  0,  0,  0,  0,-10,
        -20,-10,-10, -5, -5,-10,-10,-20,
    ],
    [// kings
        -50,-40,-30,-20,-20,-30,-40,-50,
        -30,-20,-10,  0,  0,-10,-20,-30,
        -30,-10, 20, 30, 30, 20,-10,-30,
        -30,-10, 30, 40, 40, 30,-10,-30,
        -30,-10, 30, 40, 40, 30,-10,-30,
        -30,-10, 20, 30, 30, 20,-10,-30,
        -30,-30,  0,  0,  0,  0,-30,-30,
        -50,-30,-30,-30,-30,-30,-30,-50,
    ],
];

pub struct EvalWeights {
    pub mg_pst: [[i32; 64]; 6], // Midgame PSTs
    pub eg_pst: [[i32; 64]; 6], // Endgame PSTs
    // Optional:
    pub mg_values: [i32; 6],
    pub eg_values: [i32; 6],
}

pub const PHASE_TOTAL: i32 = 24;

pub const PHASE_WEIGHTS: [i32; 6] = [
    0, // Pawn
    1, // Knight
    1, // Bishop
    2, // Rook
    4, // Queen
    0, // King
];


pub fn game_phase(board: &Board) -> i32 {
    use PieceType::*;

    let mut phase = PHASE_TOTAL;

    for (piece, weight) in [
        (Knight, 1),
        (Bishop, 1),
        (Rook, 2),
        (Queen, 4),
    ] {
        let count = board.count_piece(piece, true) + board.count_piece(piece, false);
        phase -= weight * count;
    }

    phase.clamp(0, PHASE_TOTAL)
}

pub fn evaluate(board: &Board, weights: &EvalWeights) -> i32 {
    let mut mg_score = 0;
    let mut eg_score = 0;

    for piece in PieceType::iter() {
        let idx = piece as usize;

        for sq in board.piece_squares(piece, true) {
            mg_score += weights.mg_pst[idx][sq];
            eg_score += weights.eg_pst[idx][sq];
        }

        for sq in board.piece_squares(piece, false) {
            let sq_mirrored = sq ^ 56;
            mg_score -= weights.mg_pst[idx][sq_mirrored];
            eg_score -= weights.eg_pst[idx][sq_mirrored];
        }
    }

    let phase = game_phase(board);

    (mg_score * phase + eg_score * (PHASE_TOTAL - phase)) / PHASE_TOTAL
}

// pub fn evaluate(position: &Position, depth: usize, historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> isize {
//     let game = Game::new(&position, historic_repeat_position_counts);
//     let game_status = game.get_game_status();
//     match game_status {
//         game::GameStatus::InProgress => {
//             let score = score_pieces(position);
//             if score != 0 { score } else { -1 }
//         },
//         game::GameStatus::Checkmate => depth as isize - MAXIMUM_SCORE,
//         _ => 0,
//     }
// }

pub fn score_pieces(position: &Position) -> isize {
    fn score_board_for_color(board: &Board, color: PieceColor) -> isize {
        let bitboards = board.bitboards_for_color(color);
        let square_index_xor = if color == White { 0 } else { 56 };
        let mut score: isize = 0;
        for piece_type in PieceType::iter() {
            util::process_bits(bitboards[piece_type as usize], |square_index| {
                score += PIECE_SCORES[piece_type as usize] +
                    PIECE_SCORE_ADJUSTMENT_TABLE[piece_type as usize][square_index as usize ^ square_index_xor];
            });
        }
        score
    }
    
    let score = score_board_for_color(position.board(), White) - score_board_for_color(position.board(), Black)
        + score_pawn_structure(position)
        + score_king_safety(position)
        + score_bishops(position);
    
    if position.side_to_move() == White { score } else { -score }
}

pub fn score_bishops(position: &Position) -> isize {
    let board = position.board();
    board.has_bishop_pair(White) as isize * 50 - board.has_bishop_pair(Black) as isize * 50
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, NEW_GAME_FEN};

    #[test]
    fn test_score_pieces() {
        let position: Position = Position::from(NEW_GAME_FEN);
        assert_eq!(score_pieces(&position), 0);

        let missing_white_pawn: Position = Position::from("rnbqkbnr/pppppppp/8/8/8/8/PPP1PPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_pieces(&missing_white_pawn), -100);

        let missing_black_pawn: Position = Position::from("rnbqkbnr/1ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(score_pieces(&missing_black_pawn), 125);


        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_pieces(&all_black_no_white), 4015);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_pieces(&black_pawn_on_seventh_rank), -155);
    }

    #[test]
    fn test_pawn_scores() {
        let position: Position = Position::from("4k3/P7/8/8/8/6p1/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 38);
    }

    #[test]
    fn test_knight_scores() {
        let position: Position = Position::from("N3k3/8/8/4n3/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -72);
    }

    #[test]
    fn test_bishop_scores() {
        let position: Position = Position::from("b3k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 28);
    }

    #[test]
    fn test_rook_scores() {
        let position: Position = Position::from("4k1r1/8/R7/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -7);
    }

    #[test]
    fn test_queen_scores() {
        let position: Position = Position::from("4k1q1/8/QQ6/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 903);
    }

    #[test]
    fn test_king_scores() {
        let position: Position = Position::from("8/7k/8/8/8/2K5/8/8 w - - 0 1");
        assert_eq!(score_pieces(&position), 52);
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


}