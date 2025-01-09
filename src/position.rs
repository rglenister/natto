use crate::bit_board::{BitBoard, CASTLING_METADATA};
use crate::board::{Board, BoardSide, Piece, PieceColor, PieceType};
use crate::chess_move::ChessMove;
use crate::{fen, move_generator, position, util};
use crate::board::PieceColor::{Black, White};
use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};
use crate::move_generator::{king_attacks_finder, square_attacks_finder};

include!("util/generated_macro.rs");

pub(crate) const NEW_GAME_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
#[derive(Clone)]
pub(crate) struct Position {
    board: BitBoard,
    side_to_move: PieceColor,
    castling_rights: [[bool; 2]; 2],
    en_passant_capture_square: Option<usize>,
    half_move_clock: usize,
    full_move_number: usize,
}

impl From<&str> for Position {
    fn from(fen: &str) -> Self {
        fen::parse(fen.to_string())
    }
}

impl From<&Position> for Position {
    fn from(position: &Position) -> Self {
        position.into()
    }
}


impl Position {
    pub(crate) fn new(
        board: BitBoard,
        side_to_move: PieceColor,
        fen_castling_rights: String,
        en_passant_capture_square: Option<usize>,
        half_move_clock: usize,
        full_move_number: usize,
    ) -> Self {
        Self {
            board,
            side_to_move,
            castling_rights: Position::create_castling_rights(fen_castling_rights),
            en_passant_capture_square,
            half_move_clock,
            full_move_number,
        }
    }

    pub fn to_string(&self) -> String {
        format!("{} {:?} {:?} {} {} {}", self.board.to_string(), self.side_to_move, self.castling_rights, self. en_passant_capture_square.unwrap_or(0), self.half_move_clock, self.full_move_number)
    }
    pub fn new_game() -> Position {
        Position::from(NEW_GAME_FEN)
    }

    pub fn board_unmut(&self) -> &BitBoard {
        &self.board
    }

    pub fn board(&mut self) -> &mut BitBoard {
        &mut self.board
    }

    pub fn side_to_move(&self) -> PieceColor {
        self.side_to_move
    }

    pub fn opposing_side(&self) -> PieceColor {
        if self.side_to_move == PieceColor::White {PieceColor::Black} else {PieceColor::White}
    }

    pub fn castling_rights(&self) -> [[bool; 2]; 2] {
        self.castling_rights.clone()
    }

    pub fn en_passant_capture_square(&self) -> Option<usize> {
        self.en_passant_capture_square.clone()
    }

    pub fn half_move_clock(&self) -> usize {
        self.half_move_clock
    }

    pub fn full_move_number(&self) -> usize {
        self.full_move_number
    }

    pub fn make_raw_move(&self, square_from: usize, square_to: usize, promote_to: Option<PieceType>) -> Option<Self> {
        let moves = util::find_generated_move(move_generator::generate(self), square_from, square_to, promote_to);
        if moves.len() == 1 {
            self.make_move(&moves[0])
        } else {
            None
        }
    }

    pub fn make_move(&self, chess_move: &ChessMove) -> Option<Self> {
        let mut new_position = self.clone();

        match chess_move {
            ChessMove::BasicMove { from, to , capture} => {
                do_basic_move(&mut new_position.board, *from, *to);
            }
            ChessMove::EnPassantMove { from, to, capture, capture_square } => {
                do_basic_move(&mut new_position.board, *from, *to);
                let forward_pawn_increment: i32 = if self.side_to_move == PieceColor::White {-8} else {8};
                new_position.board.remove_piece((self.en_passant_capture_square.unwrap() as i32 + forward_pawn_increment)as usize);
            }
            ChessMove::CastlingMove { from, to, capture, board_side } => {
                let castling_metadata = &CASTLING_METADATA[self.side_to_move as usize][*board_side as usize];
                if king_attacks_finder(&mut new_position, self.side_to_move) == 0 &&
                            square_attacks_finder(&mut new_position, self.opposing_side(), castling_metadata.king_through_square as i32) == 0 {
                    do_basic_move(&mut new_position.board, *from, *to);
                    let castling_meta_data = &CASTLING_METADATA[self.side_to_move as usize][*board_side as usize];
                    let rook = new_position.board.remove_piece(castling_meta_data.rook_from_square).unwrap();
                    new_position.board.put_piece(castling_meta_data.rook_to_square, rook);
                } else {
                    return None;
                }
            }
            ChessMove::PromotionMove { from, to, capture, promote_to } => {
                new_position.board.remove_piece(*from);
                new_position.board.put_piece(*to, Piece { piece_color: self.side_to_move(), piece_type: *promote_to });
            }
        }
        fn do_basic_move(board: &mut BitBoard, from: usize, to: usize) {
            let piece = board.remove_piece(from).unwrap();
            let captured_piece = board.remove_piece(to);
            board.put_piece(to, piece)
        }

        let is_valid_move = king_attacks_finder(&mut new_position, self.side_to_move()) == 0;
        new_position.side_to_move = if self.side_to_move == White {Black} else {White};
        is_valid_move.then(|| Self::from(new_position))
    }

    pub fn can_castle(&self, piece_color: PieceColor, board_side: &BoardSide) -> bool {
        self.castling_rights[piece_color as usize][*board_side as usize]
            && self.board.can_castle(piece_color, board_side)
    }

    fn create_castling_rights(castling_rights: String) -> [[bool; 2]; 2] {
        let mut flags = [[false; 2]; 2];
        if !castling_rights.contains('-') {
            flags[0][0] = castling_rights.contains('K');
            flags[0][1] = castling_rights.contains('Q');
            flags[1][0] = castling_rights.contains('k');
            flags[1][1] = castling_rights.contains('q');
        }
        flags
    }
}
#[cfg(test)]
mod tests {
    use crate::bit_board::BitBoard;
    use super::*;
    use crate::board::{BoardSide, PieceColor};
    use crate::chess_move::ChessMove::CastlingMove;
    use crate::move_generator::generate;

    #[test]
    fn test_general_usability() {
        let position: Position =
            Position::new(
                BitBoard::new(),
                PieceColor::Black,
                "KQkq".to_string(),
                Some(31),
                99,
                50);

        assert!(position.board.get_piece(3).is_none());
        assert_eq!(position.side_to_move(), PieceColor::Black);
        assert_eq!(position.castling_rights(), [[true; 2]; 2]);
        assert_eq!(position.en_passant_capture_square(), Some(31));
        assert_eq!(position.half_move_clock(), 99);
        assert_eq!(position.full_move_number(), 50);
    }

    #[test]
    fn test_castling_flags() {
        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights(), [[true; 2]; 2]);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights(), [[false; 2]; 2]);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQ - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize], true);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize], true);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize], false);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize], false);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w kq - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize], false);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize], false);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize], true);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize], true);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Qk - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize], false);
        assert_eq!(position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize], true);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize], true);
        assert_eq!(position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize], false);
    }


    #[test]
    fn test_cannot_castle_out_of_check() {
        let fen = "r3k2r/p1pp1pb1/bn2Qnp1/2qPN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQkq - 3 2";
        let position = Position::from(fen);
        let moves = generate(&position);
        let castling_moves: Vec<&ChessMove> =
            moves.iter().filter(|chess_move| matches!(chess_move, CastlingMove { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(castling_moves.iter().filter_map(|chess_move| { position.make_move(chess_move) }).count(), 0);
    }

    #[test]
    fn test_cannot_castle_through_check() {
        let fen = "r3k3/8/8/8/7B/8/8/4K3 b q - 0 1";
        let mut position = Position::from(fen);
        let moves = generate(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, CastlingMove { .. })).collect();
        assert_eq!(castling_moves.len(), 1);
        assert_eq!(castling_moves.iter().filter_map(|chess_move| { position.make_move(chess_move) }).count(), 0);
    }

    #[test]
    fn test_ep_capture_square_is_set_after_double_pawn_move() {
        let mut position = Position::from(NEW_GAME_FEN);
        position.make_raw_move(sq!("e2"), sq!("e4"), None);
        let moves = generate(&position);
    }
}
