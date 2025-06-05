use crate::core::board::Board;
use crate::core::board::{CASTLING_METADATA, KING_HOME_SQUARE};
use crate::core::board::BoardSide;
use crate::core::board::BoardSide::{KingSide, QueenSide};
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType::{King, Pawn, Rook};
use crate::core::piece::{Piece, PieceColor};
use crate::core::r#move::Move::{Basic, Castling, EnPassant, Promotion};
use crate::core::r#move::{Move, RawMove};
use crate::core::move_generator::{is_en_passant_capture_possible, king_attacks_finder, square_attacks_finder};
use crate::util::util::distance;
use crate::core::move_generator;
use once_cell::sync::Lazy;
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::fmt;
use crate::util::{fen, util};

include!("../util/generated_macro.rs");

pub const NEW_GAME_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

struct PositionHashes {
    pub board_hashes_table: [[[u64; PositionHashes::NUM_SQUARES]; PositionHashes::NUM_PIECE_TYPES]; PositionHashes::NUM_COLORS],
    pub castling_hashes_table: [u64; PositionHashes::NUM_CASTLING_STATES],
    pub side_to_move_hashes_table: [u64; PositionHashes::NUM_COLORS],
    pub en_passant_capture_square_hashes_table: [u64; PositionHashes::NUM_SQUARES],
}
impl PositionHashes {
    const NUM_SQUARES: usize = 64;
    const NUM_PIECE_TYPES: usize = 6;
    const NUM_COLORS: usize = 2;
    const NUM_CASTLING_STATES: usize = 16;
}

static POSITION_HASHES: Lazy<PositionHashes> = Lazy::new(|| {
    let seed: u64 = 49;
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);

    let mut board_hashes_table: [[[u64; PositionHashes::NUM_SQUARES]; PositionHashes::NUM_PIECE_TYPES]; PositionHashes::NUM_COLORS] = [[[0; PositionHashes::NUM_SQUARES]; PositionHashes::NUM_PIECE_TYPES]; PositionHashes::NUM_COLORS];
    for color in 0..PositionHashes::NUM_COLORS {
        for piece_type in 0..PositionHashes::NUM_PIECE_TYPES {
            for square in 0..PositionHashes::NUM_SQUARES {
                board_hashes_table[color][piece_type][square] = rng.random::<u64>();
            }
        }
    }

    let mut castling_hashes_table: [u64; PositionHashes::NUM_CASTLING_STATES] = [0; PositionHashes::NUM_CASTLING_STATES];
    for state in 0..PositionHashes::NUM_CASTLING_STATES {
        castling_hashes_table[state] = rng.random::<u64>();
    }

    let mut side_to_move_hashes_table: [u64; PositionHashes::NUM_COLORS] = [0; PositionHashes::NUM_COLORS];
    for state in 0..PositionHashes::NUM_COLORS {
        side_to_move_hashes_table[state] = rng.random::<u64>();
    }

    let mut en_passant_capture_square_hashes_table: [u64; PositionHashes::NUM_SQUARES] = [0; PositionHashes::NUM_SQUARES];
    for square in 0..PositionHashes::NUM_SQUARES {
        en_passant_capture_square_hashes_table[square] = rng.random::<u64>();
    }

    PositionHashes { board_hashes_table, castling_hashes_table, side_to_move_hashes_table, en_passant_capture_square_hashes_table }
});

#[derive(Copy, Clone, Debug, Default, Eq)]
pub struct Position {
    board: Board,
    side_to_move: PieceColor,
    castling_rights: [[bool; 2]; 2],
    en_passant_capture_square: Option<usize>,
    half_move_clock: usize,
    full_move_number: usize,
    hash_code: u64,
}

impl From<&str> for Position {
    fn from(fen: &str) -> Self {
        fen::parse(fen.to_string()).unwrap()
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.board, fen::write(self))
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.board == other.board &&
        self.side_to_move == other.side_to_move &&
        self.castling_rights == other.castling_rights &&
        is_en_passant_capture_possible(self) == is_en_passant_capture_possible(other)
    }
}

impl Position {
    pub(crate) fn new(
        board: Board,
        side_to_move: PieceColor,
        fen_castling_rights: String,
        en_passant_capture_square: Option<usize>,
        half_move_clock: usize,
        full_move_number: usize,
    ) -> Self {
        let mut position = Self {
            board,
            side_to_move,
            castling_rights: Position::create_castling_rights(fen_castling_rights.clone()),
            en_passant_capture_square,
            half_move_clock,
            full_move_number,
            hash_code: 0
        };
        position.hash_code = position.create_initial_hash();
        position
    }


    pub fn new_game() -> Position {
        Position::from(NEW_GAME_FEN)
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    pub fn side_to_move(&self) -> PieceColor {
        self.side_to_move
    }

    pub fn opposing_side(&self) -> PieceColor {
        if self.side_to_move == White {Black} else {White}
    }

    pub fn castling_rights(&self) -> [[bool; 2]; 2] {
        self.castling_rights
    }

    pub fn castling_rights_as_u64(&self) -> u64 {
        (self.castling_rights[White as usize][KingSide as usize] as u64) |
        ((self.castling_rights[White as usize][QueenSide as usize] as u64) << 1) |
        ((self.castling_rights[Black as usize][KingSide as usize] as u64) << 2) |
        ((self.castling_rights[Black as usize][QueenSide as usize] as u64) << 3)
    }

    pub fn en_passant_capture_square(&self) -> Option<usize> {
        self.en_passant_capture_square
    }

    pub fn half_move_clock(&self) -> usize {
        self.half_move_clock
    }

    pub fn full_move_number(&self) -> usize {
        self.full_move_number
    }

    pub fn hash_code(&self) -> u64 {
        self.hash_code
    }

    fn create_initial_hash(&self) -> u64 {
        let mut initial_hash: u64 = self.get_board_hash();
        initial_hash ^= POSITION_HASHES.side_to_move_hashes_table[self.side_to_move as usize];
        initial_hash ^= POSITION_HASHES.castling_hashes_table[self.castling_rights_as_u64() as usize];

        if is_en_passant_capture_possible(self) {
            initial_hash ^= POSITION_HASHES.en_passant_capture_square_hashes_table[self.en_passant_capture_square.unwrap()];
        }
        initial_hash
    }

    fn get_board_hash(&self) -> u64 {
        let mut result: u64 = 0;
        self.board.process_pieces(|piece_color, piece_type, square_index| {
            result ^= POSITION_HASHES.board_hashes_table[piece_color as usize][piece_type as usize][square_index];
        });
        result
    }

    fn put_piece(&mut self, square_index: usize, piece: Piece) {
        self.remove_piece(square_index);
        self.board.put_piece(square_index, piece.clone());
        self.hash_code ^= POSITION_HASHES.board_hashes_table[piece.piece_color as usize][piece.piece_type as usize][square_index];
    }

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece> {
        let piece = self.board.remove_piece(square_index);
        if let Some(piece) = piece {
            self.hash_code ^= POSITION_HASHES.board_hashes_table[piece.piece_color as usize][piece.piece_type as usize][square_index];
            Some(piece)
        } else {
            None
        }
    }

    fn move_piece(&mut self, from_square_index: usize, to_square_index: usize) -> Piece {
        let piece = self.remove_piece(from_square_index).unwrap();
        self.put_piece(to_square_index, piece.clone());
        piece
    }

    pub fn make_raw_move(&self, raw_move: &RawMove) -> Option<(Self, Move)> {
        let chess_move = util::find_generated_move(move_generator::generate_moves(self), raw_move);
        self.make_move(&chess_move?)
    }

    pub fn make_move(&self, chess_move: &Move) -> Option<(Self, Move)> {
        let mut new_position = *self;

        new_position.en_passant_capture_square = None;

        match chess_move {
            Basic { base_move } => {
                do_basic_move(&mut new_position, base_move.from, base_move.to, base_move.capture);
            }
            EnPassant { base_move, capture_square: _ } => {
                do_basic_move(&mut new_position, base_move.from, base_move.to, true);
                let forward_pawn_increment: i32 = if self.side_to_move == White {-8} else {8};
                new_position.remove_piece((self.en_passant_capture_square.unwrap() as i32 + forward_pawn_increment)as usize);
            }
            Castling { base_move, board_side } => {
                let castling_metadata = &CASTLING_METADATA[self.side_to_move as usize][*board_side as usize];
                if king_attacks_finder(&new_position, self.side_to_move) == 0 &&
                            square_attacks_finder(&new_position, self.opposing_side(), castling_metadata.king_through_square) == 0 {
                    do_basic_move(&mut new_position, base_move.from, base_move.to, false);
                    let castling_meta_data = &CASTLING_METADATA[self.side_to_move as usize][*board_side as usize];
                    new_position.move_piece(castling_meta_data.rook_from_square, castling_meta_data.rook_to_square);
                    new_position.castling_rights[self.side_to_move as usize] = [false, false];
                } else {
                    return None;
                }
            }
            Promotion { base_move, promote_to } => {
                new_position.remove_piece(base_move.from);
                new_position.put_piece(base_move.to, Piece { piece_color: self.side_to_move(), piece_type: *promote_to });
                new_position.half_move_clock = 0;
            }
        }

        fn do_basic_move(new_position: &mut Position, from: usize, to: usize, capture: bool) {
            let piece = new_position.move_piece(from, to);
            let piece_type = piece.piece_type;
            if piece_type == Pawn && distance(from as isize, to as isize) == 2 {
                new_position.en_passant_capture_square = Some((from + to) / 2);
            } else if piece_type == King && from == KING_HOME_SQUARE[new_position.side_to_move() as usize] {
                new_position.castling_rights[new_position.side_to_move as usize] = [false, false];
            } else if piece_type == Rook {
                if from == CASTLING_METADATA[new_position.side_to_move() as usize][KingSide as usize].rook_from_square {
                    new_position.castling_rights[new_position.side_to_move as usize][KingSide as usize] = false;
                } else if from == CASTLING_METADATA[new_position.side_to_move() as usize][QueenSide as usize].rook_from_square {
                    new_position.castling_rights[new_position.side_to_move as usize][QueenSide as usize] = false;
                }
            }
            if capture || piece_type == Pawn {
                new_position.half_move_clock = 0;
            } else {
                new_position.half_move_clock += 1;
            }
        }

        if king_attacks_finder(&new_position, self.side_to_move()) == 0 {
            // it's a valid move because it doesn't leave the side making the move in check
            self.update_hash_code(chess_move, &mut new_position);
            Some((new_position, *chess_move))
        } else {
            None
        }
    }

    fn update_hash_code(&self, chess_move: &Move, new_position: &mut Position) {
        new_position.side_to_move = if self.side_to_move == White {
            Black
        } else {
            new_position.full_move_number += 1;
            White
        };
        new_position.hash_code ^= POSITION_HASHES.side_to_move_hashes_table[White as usize];
        new_position.hash_code ^= POSITION_HASHES.side_to_move_hashes_table[Black as usize];

        if new_position.castling_rights != self.castling_rights {
            new_position.hash_code ^= POSITION_HASHES.castling_hashes_table[self.castling_rights_as_u64() as usize];
            new_position.hash_code ^= POSITION_HASHES.castling_hashes_table[new_position.castling_rights_as_u64() as usize];
        }
        // en passant moves are only included in the hash if the relevant pawn can actually be captured en passant
        if is_en_passant_capture_possible(self) {
            // remove the old en passant from the hash only if an en passant capture could be made because it won't have been added to the hash
            new_position.hash_code ^= POSITION_HASHES.en_passant_capture_square_hashes_table[self.en_passant_capture_square.unwrap()];
        }
        if is_en_passant_capture_possible(new_position) {
            // add the new en passant square to the hash only if an en passant capture can actually be made
            new_position.hash_code ^= POSITION_HASHES.en_passant_capture_square_hashes_table[new_position.en_passant_capture_square.unwrap()];
        }
        if new_position.hash_code != new_position.create_initial_hash() {
            panic!("Hash code mismatch after move: {}", chess_move);
        }
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
    use crate::core::piece::PieceType::Queen;
    use crate::core::move_generator::generate_moves;
    use super::*;
    
    #[test]
    fn test_general_usability() {
        let position: Position =
            Position::new(
                Board::new(),
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
        assert_eq!(position.castling_rights()[White as usize][KingSide as usize], true);
        assert_eq!(position.castling_rights()[White as usize][QueenSide as usize], true);
        assert_eq!(position.castling_rights()[Black as usize][KingSide as usize], false);
        assert_eq!(position.castling_rights()[Black as usize][QueenSide as usize], false);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w kq - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights()[White as usize][KingSide as usize], false);
        assert_eq!(position.castling_rights()[White as usize][QueenSide as usize], false);
        assert_eq!(position.castling_rights()[Black as usize][KingSide as usize], true);
        assert_eq!(position.castling_rights()[Black as usize][QueenSide as usize], true);

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Qk - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(position.castling_rights()[White as usize][KingSide as usize], false);
        assert_eq!(position.castling_rights()[White as usize][QueenSide as usize], true);
        assert_eq!(position.castling_rights()[Black as usize][KingSide as usize], true);
        assert_eq!(position.castling_rights()[Black as usize][QueenSide as usize], false);
    }


    #[test]
    fn test_cannot_castle_out_of_check() {
        let fen = "r3k2r/p1pp1pb1/bn2Qnp1/2qPN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQkq - 3 2";
        let position = Position::from(fen);
        let moves = generate_moves(&position);
        let castling_moves: Vec<&Move> =
            moves.iter().filter(|chess_move| matches!(chess_move, Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(castling_moves.iter().filter_map(|chess_move| { position.make_move(chess_move) }).count(), 0);
    }

    #[test]
    fn test_cannot_castle_through_check() {
        let fen = "r3k3/8/8/8/7B/8/8/4K3 b q - 0 1";
        let position = Position::from(fen);
        let moves = generate_moves(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 1);
        assert_eq!(castling_moves.iter().filter_map(|chess_move| { position.make_move(chess_move) }).count(), 0);
    }

    #[test]
    fn test_ep_capture_square_is_set_after_double_white_pawn_move() {
        let position_1 = Position::from(NEW_GAME_FEN);
        let (position_2, _) = position_1.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position_2.en_passant_capture_square, Some(sq!("e3")));
        let (position_3, _) = position_2.make_raw_move(&RawMove::new(sq!("b8"), sq!("c6"), None)).unwrap();
        assert_eq!(position_3.en_passant_capture_square, None);
    }

    #[test]
    fn test_ep_capture_square_is_set_after_double_black_pawn_move() {
        let position_1 = Position::from(NEW_GAME_FEN);
        let (position_2, _cm) = position_1.make_raw_move(&RawMove::new(sq!("e2"), sq!("e3"), None)).unwrap();
        assert_eq!(position_2.en_passant_capture_square, None);
        let (position_3, _cm) = position_2.make_raw_move(&RawMove::new(sq!("a7"), sq!("a5"), None)).unwrap();
        assert_eq!(position_3.en_passant_capture_square, Some(sq!("a6")));
        let (position_4, _cm) = position_3.make_raw_move(&RawMove::new(sq!("c2"), sq!("c3"), None)).unwrap();
        assert_eq!(position_4.en_passant_capture_square, None);
    }

    #[test]
    fn test_castling_rights_lost_after_castling() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        assert_eq!(position.castling_rights[White as usize], [true, true]);
        assert_eq!(position.castling_rights[Black as usize], [true, true]);
        assert_eq!(position.castling_rights_as_u64(), 15);

        let moves = generate_moves(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        let positions: Vec<_> = castling_moves.iter().filter_map(|chess_move| { position.make_move(chess_move) }).collect();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].0.castling_rights[White as usize], [false, false]);
        assert_eq!(positions[1].0.castling_rights[White as usize], [false, false]);
        assert_eq!(positions[1].0.castling_rights_as_u64(), 12);
    }

    #[test]
    fn test_castling_rights_lost_after_moving_king() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        assert_eq!(position.castling_rights[White as usize], [true, true]);
        assert_eq!(position.castling_rights[Black as usize], [true, true]);
        assert_eq!(position.castling_rights_as_u64(), 15);
        let moves = generate_moves(&position);
        let king_moves: Vec<_> = util::filter_moves_by_from_square(moves, sq!("e1"));
        assert_eq!(king_moves.len(), 7);
        let new_position = position.make_move(&king_moves[0]).unwrap();
        assert_eq!(new_position.0.castling_rights[White as usize], [false, false]);
        assert_eq!(new_position.0.castling_rights[Black as usize], [true, true]);
        assert_eq!(new_position.0.castling_rights_as_u64(), 12);
    }
    #[test]
    fn test_castling_rights_lost_after_moving_rook() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position_1 = Position::from(fen);
        assert_eq!(position_1.castling_rights[White as usize], [true, true]);
        assert_eq!(position_1.castling_rights[Black as usize], [true, true]);

        let position_2 = position_1.make_raw_move(&RawMove::new(sq!("a1"), sq!("a2"), None)).unwrap();
        assert_eq!(position_2.0.castling_rights[White as usize], [true, false]);
        assert_eq!(position_2.0.castling_rights[Black as usize], [true, true]);
        assert_eq!(position_2.0.castling_rights_as_u64(), 13);

        let position_3 = position_1.make_raw_move(&RawMove::new(sq!("h1"), sq!("h2"), None)).unwrap();
        assert_eq!(position_3.0.castling_rights[White as usize], [false, true]);
        assert_eq!(position_3.0.castling_rights[Black as usize], [true, true]);
        assert_eq!(position_3.0.castling_rights_as_u64(), 14);
    }

    #[test]
    fn test_full_move_counter_incremented_after_black_move() {
        let position_1 = Position::from(NEW_GAME_FEN);
        assert_eq!(position_1.full_move_number, 1);
        let position_2 = position_1.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position_2.0.full_move_number, 1);
        let position_3 = position_2.0.make_raw_move(&RawMove::new(sq!("e7"), sq!("e5"), None)).unwrap();
        assert_eq!(position_3.0.full_move_number, 2);
    }

    #[test]
    fn test_half_move_counter_incrementation() {
        let position_1 = Position::from(NEW_GAME_FEN);
        assert_eq!(position_1.half_move_clock, 0);
        let position_2 = position_1.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position_2.0.half_move_clock, 0);
        let position_3 = position_2.0.make_raw_move(&RawMove::new(sq!("e7"), sq!("e5"), None)).unwrap();
        assert_eq!(position_3.0.half_move_clock, 0);
        let position_4 = position_3.0.make_raw_move(&RawMove::new(sq!("g1"), sq!("f3"), None)).unwrap();
        assert_eq!(position_4.0.half_move_clock, 1);
        let position_5 = position_4.0.make_raw_move(&RawMove::new(sq!("d7"), sq!("d6"), None)).unwrap();
        assert_eq!(position_5.0.half_move_clock, 0);
        let position_6 = position_5.0.make_raw_move(&RawMove::new(sq!("b1"), sq!("c3"), None)).unwrap();
        assert_eq!(position_6.0.half_move_clock, 1);
        let position_7 = position_6.0.make_raw_move(&RawMove::new(sq!("d8"), sq!("g5"), None)).unwrap();
        assert_eq!(position_7.0.half_move_clock, 2);
        let position_8 = position_7.0.make_raw_move(&RawMove::new(sq!("f3"), sq!("g5"), None)).unwrap();
        assert_eq!(position_8.0.half_move_clock, 0);
    }

    #[test]
    fn test_promotion_move_resets_half_move_counter() {
        let fen = "7k/4P3/8/8/8/8/5p2/K3N3 b - - 10 1";
        let position_1 = Position::from(fen);
        assert_eq!(position_1.half_move_clock, 10);

        // non capture promotion
        let position_2 =  position_1.make_raw_move(&RawMove::new(sq!("f2"), sq!("f1"), Some(Queen))).unwrap();
        assert_eq!(position_2.0.half_move_clock, 0);

        // capturing promotion
        let position_3 =  position_1.make_raw_move(&RawMove::new(sq!("f2"), sq!("e1"), Some(Queen))).unwrap();
        assert_eq!(position_3.0.half_move_clock, 0);

    }
}
