use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::bit_board::BitBoard;
use crate::board::{PieceColor, PieceType};
use crate::board::PieceType::{King, Knight, Pawn, Queen};
use crate::eval::search::MAXIMUM_SCORE;
use crate::game::Game;
use crate::position::Position;
use crate::{game, util};
use crate::util::filter_bits;

static COLUMN_SQUARE_INDEXES: Lazy<[u64; 8]> = Lazy::new(|| {
    let mut result = [0; 8];
    let b: u64 = !0;
    for column_index in 0..8 {
        result[column_index] = filter_bits(!0, |square_index| square_index % 8 == column_index as u64);
    }
    result
});


pub const PIECE_SCORES: [isize; 6] = [100, 300, 300, 500, 900, 0];


pub const PIECE_SCORE_ADJUSTMENT_TABLE: [[isize; 64]; 6] = [
    [ // pawns get their own table
        0; 64
    ],
    [ // knight
        -60, -50, -40, -40, -40, -40, -50, -60,
        -50, -40, -20, -20, -20, -20, -40, -50,
        -40, -20, -00, -00, -00, -00, -20, -40,
        -40, -20, -00, -00, -00, -00, -20, -40,
        -40, -20, -00, -00, -00, -00, -20, -40,
        -40, -20, -00, -00, -00, -00, -20, -40,
        -50, -40, -20, -20, -20, -20, -40, -50,
        -60, -50, -40, -40, -40, -40, -50, -60,
    ],
    [ // bishop
        -10, -10, -10, -10, -10, -10, -10, -10,
        -10, -00, -00, -00, -00, -00, -00, -10,
        -10, -00, -20, -20, -20, -20, -00, -10,
        -10, -00, -20, -40, -40, -20, -00, -10,
        -10, -00, -20, -40, -40, -20, -00, -10,
        -10, -00, -20, -20, -20, -20, -00, -10,
        -10, -00, -00, -00, -00, -00, -00, -10,
        -10, -10, -10, -10, -10, -10, -10, -10,
    ],
    [ // rook
        0; 64
    ],
    [ // queen
        0; 64
    ],
    [ // kings get their own table
        0; 64
    ],
];

pub const PAWN_SCORE_ADJUSTMENT_TABLE: [[isize; 64]; 2] = [
    [ // white
        00, 00, 00, 00, 00, 00, 00, 00,
        00, 00, 00, 00, 00, 00, 00, 00,
        10, 10, 10, 10, 10, 10, 10, 10,
        20, 20, 20, 20, 20, 20, 20, 20,
        40, 40, 40, 40, 40, 40, 40, 40,
        80, 80, 80, 80, 80, 80, 80, 80,
        160, 160, 160, 160, 160, 160, 160, 160,
        00, 00, 00, 00, 00, 00, 00, 00,
    ],
    [ // black
        00, 00, 00, 00, 00, 00, 00, 00,
        160, 160, 160, 160, 160, 160, 160, 160,
        80, 80, 80, 80, 80, 80, 80, 80,
        40, 40, 40, 40, 40, 40, 40, 40,
        20, 20, 20, 20, 20, 20, 20, 20,
        10, 10, 10, 10, 10, 10, 10, 10,
        00, 00, 00, 00, 00, 00, 00, 00,
        00, 00, 00, 00, 00, 00, 00, 00,
    ]
];

pub const KING_SCORE_ADJUSTMENT_TABLE: [[isize; 64]; 2] = [
    [ // white
        20, 30, 10, 00, 00, 10, 30, 20,
        20, 20, 00, 00, 00, 00, 20, 20,
        -20, -20, -20, -20, -20, -20, -20, -80,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
    ],
    [ // black
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -30, -40, -40, -50, -50, -40, -40, -30,
        -20, -20, -20, -20, -20, -20, -20, -20,
        20, 20, 00, 00, 00, 00, 20, 20,
        20, 30, 10, 00, 00, 10, 30, 20,
    ]
];


pub fn evaluate(position: &Position, depth: usize, historic_repeat_position_counts: Option<&HashMap<u64, (Position, usize)>>) -> isize {
    let game = Game::new(&position, historic_repeat_position_counts);
    let game_status = game.get_game_status();
    match game_status {
        game::GameStatus::InProgress => {
            let score = score_pieces(position);
            if score == 0 { -1 } else { score }
        },
        game::GameStatus::Checkmate => depth as isize - MAXIMUM_SCORE,
        _ => 0,
    }
}

pub fn score_pieces(position: &Position) -> isize {
    fn score_board_for_color(board: &BitBoard, color: PieceColor) -> isize {
        let bitboards = board.bitboards_for_color(color);
        let mut score: isize = 0;
        util::process_bits(bitboards[Pawn as usize], |square_index| {
            score += PIECE_SCORES[Pawn as usize] + PAWN_SCORE_ADJUSTMENT_TABLE[color as usize][square_index as usize];
        });
        for piece_type in Knight as usize ..=Queen as usize {
            util::process_bits(bitboards[piece_type], |square_index| {
                score += PIECE_SCORES[piece_type] + PIECE_SCORE_ADJUSTMENT_TABLE[piece_type][square_index as usize];
            });
        }
        util::process_bits(bitboards[King as usize], |square_index| {
            score += PIECE_SCORES[King as usize] + KING_SCORE_ADJUSTMENT_TABLE[color as usize][square_index as usize];
        });
        score
    }

    score_board_for_color(position.board(), position.side_to_move())
        - score_board_for_color(position.board(), position.opposing_side())
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
        assert_eq!(score_pieces(&missing_black_pawn), 100);


        let fen = "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 b kq - 0 1";
        let all_black_no_white: Position = Position::from(fen);
        assert_eq!(score_pieces(&all_black_no_white), 3780);

        let fen = "3k4/8/8/8/8/8/2p5/4K3 w - - 0 1";
        let black_pawn_on_seventh_rank: Position = Position::from(fen);
        assert_eq!(score_pieces(&black_pawn_on_seventh_rank), -260);
    }

    #[test]
    fn test_pawn_scores() {
        let position: Position = Position::from("4k3/P7/8/8/8/6p1/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 80);
    }

    #[test]
    fn test_knight_scores() {
        let position: Position = Position::from("N3k3/8/8/4n3/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -60);
    }

    #[test]
    fn test_bishop_scores() {
        let position: Position = Position::from("b3k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), -30);
    }

    #[test]
    fn test_rook_scores() {
        let position: Position = Position::from("4k1r1/8/R7/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 0);
    }

    #[test]
    fn test_queen_scores() {
        let position: Position = Position::from("4k1q1/8/QQ6/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(score_pieces(&position), 900);
    }

    #[test]
    fn test_king_scores() {
        let position: Position = Position::from("8/7k/8/8/8/2K5/8/8 w - - 0 1");
        assert_eq!(score_pieces(&position), -40);
    }

    #[test]
    fn test_score_board_for_color() {
        assert_eq!(COLUMN_SQUARE_INDEXES[0], 1 << 0 | 1 << 8 | 1 << 16 | 1 << 24 | 1 << 32 | 1 << 40 | 1 << 48 | 1 << 56);
        assert_eq!(COLUMN_SQUARE_INDEXES[7], 1 << 7 | 1 << 15 | 1 << 23 | 1 << 31 | 1 << 39 | 1 << 47 | 1 << 55 | 1 << 63);
    }
}