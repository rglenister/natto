use crate::core::board::Board;
use crate::core::board::BoardSide;
use crate::core::piece::{Piece, PieceColor, PieceType};
use crate::core::r#move::{BaseMove, Move, RawMove};
use crate::core::{board, move_gen};
use crate::utils::{fen, util};
use once_cell::sync::Lazy;
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::fmt;

include!("../utils/generated_macro.rs");

pub const NEW_GAME_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

struct PositionHashes {
    pub board_hashes_table: [[[u64; PositionHashes::NUM_SQUARES]; PositionHashes::NUM_PIECE_TYPES];
        PositionHashes::NUM_COLORS],
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
    fn create_random_value_array<const N: usize>(rng: &mut Xoshiro256PlusPlus) -> [u64; N] {
        core::array::from_fn(|_| rng.random::<u64>())
    }

    let seed: u64 = 49;
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);

    let mut board_hashes_table: [[[u64; PositionHashes::NUM_SQUARES];
        PositionHashes::NUM_PIECE_TYPES];
        PositionHashes::NUM_COLORS] = [[[0; PositionHashes::NUM_SQUARES];
        PositionHashes::NUM_PIECE_TYPES];
        PositionHashes::NUM_COLORS];
    for color in 0..board_hashes_table.len() {
        for piece_type in 0..board_hashes_table[0].len() {
            for square in 0..board_hashes_table[0][0].len() {
                board_hashes_table[color][piece_type][square] = rng.random::<u64>();
            }
        }
    }

    let castling_hashes_table =
        create_random_value_array::<{ PositionHashes::NUM_CASTLING_STATES }>(&mut rng);
    let side_to_move_hashes_table =
        create_random_value_array::<{ PositionHashes::NUM_COLORS }>(&mut rng);
    let en_passant_capture_square_hashes_table =
        create_random_value_array::<{ PositionHashes::NUM_SQUARES }>(&mut rng);
    PositionHashes {
        board_hashes_table,
        castling_hashes_table,
        side_to_move_hashes_table,
        en_passant_capture_square_hashes_table,
    }
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
    castled: [bool; 2],
}

#[derive(Debug, Default)]
pub struct UndoMoveInfo {
    pub mov: Move,
    pub captured_piece_type: Option<PieceType>,
    pub old_castling_rights: [[bool; 2]; 2],
    pub old_en_passant_capture_square: Option<usize>,
    pub old_is_en_passant_capture_possible: bool,
    pub old_half_move_clock: usize,
    pub old_full_move_number: usize,
    pub old_side_to_move: PieceColor,
    pub old_zobrist_hash: u64,
}

impl UndoMoveInfo {
    pub fn new(position: &Position, mov: Move) -> Self {
        Self {
            mov,
            captured_piece_type: None,
            old_castling_rights: position.castling_rights,
            old_en_passant_capture_square: position.en_passant_capture_square,
            old_is_en_passant_capture_possible: move_gen::is_en_passant_capture_possible(position),
            old_half_move_clock: position.half_move_clock,
            old_full_move_number: position.full_move_number,
            old_side_to_move: position.side_to_move,
            old_zobrist_hash: position.hash_code,
        }
    }
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
        self.board == other.board
            && self.side_to_move == other.side_to_move
            && self.castling_rights == other.castling_rights
            && self.en_passant_capture_square == other.en_passant_capture_square
            && move_gen::is_en_passant_capture_possible(self)
                == move_gen::is_en_passant_capture_possible(other)
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
            hash_code: 0,
            castled: [false, false],
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
        if self.side_to_move == PieceColor::White {
            PieceColor::Black
        } else {
            PieceColor::White
        }
    }

    pub fn castling_rights(&self) -> [[bool; 2]; 2] {
        self.castling_rights
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

    pub fn has_castled(&self, piece_color: PieceColor) -> bool {
        self.castled[piece_color as usize]
    }

    pub fn is_drawn_by_fifty_moves_rule(&self) -> bool {
        self.half_move_clock >= 100
    }

    fn create_initial_hash(&self) -> u64 {
        let mut initial_hash: u64 = self.get_board_hash();
        initial_hash ^= POSITION_HASHES.side_to_move_hashes_table[self.side_to_move as usize];
        initial_hash ^= POSITION_HASHES.castling_hashes_table
            [Position::castling_rights_as_u8(&self.castling_rights) as usize];

        if move_gen::is_en_passant_capture_possible(self) {
            initial_hash ^= POSITION_HASHES.en_passant_capture_square_hashes_table
                [self.en_passant_capture_square.unwrap()];
        }
        initial_hash
    }

    fn get_board_hash(&self) -> u64 {
        let mut result: u64 = 0;
        self.board.process_pieces(|piece_color, piece_type, square_index| {
            result ^= POSITION_HASHES.board_hashes_table[piece_color as usize][piece_type as usize]
                [square_index];
        });
        result
    }

    fn put_piece(&mut self, square_index: usize, piece: Piece) {
        self.remove_piece(square_index);
        self.board.put_piece(square_index, piece.clone());
        self.hash_code ^= POSITION_HASHES.board_hashes_table[piece.piece_color as usize]
            [piece.piece_type as usize][square_index];
    }

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece> {
        if let Some(piece) = self.board.remove_piece(square_index) {
            self.hash_code ^= POSITION_HASHES.board_hashes_table[piece.piece_color as usize]
                [piece.piece_type as usize][square_index];
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

    pub fn make_raw_move(&mut self, raw_move: &RawMove) -> Option<UndoMoveInfo> {
        let mov = util::find_generated_move(move_gen::generate_moves(self), raw_move)?;
        self.make_move(&mov)
    }

    pub fn make_move(&mut self, mov: &Move) -> Option<UndoMoveInfo> {
        fn make_en_passant_move(
            position: &mut Position,
            undo_move_info: &mut UndoMoveInfo,
            base_move: &BaseMove,
        ) {
            undo_move_info.captured_piece_type = Some(PieceType::Pawn);
            make_basic_move(
                position,
                undo_move_info,
                base_move.from as usize,
                base_move.to as usize,
                true,
            );
            let forward_pawn_increment: i32 =
                if position.side_to_move == PieceColor::White { -8 } else { 8 };
            position.remove_piece(
                (undo_move_info.old_en_passant_capture_square.unwrap() as i32
                    + forward_pawn_increment) as usize,
            );
        }
        fn make_castling_move(
            position: &mut Position,
            undo_move_info: &mut UndoMoveInfo,
            base_move: &BaseMove,
            board_side: &BoardSide,
        ) {
            undo_move_info.captured_piece_type = None;
            make_basic_move(
                position,
                undo_move_info,
                base_move.from as usize,
                base_move.to as usize,
                false,
            );
            let castling_meta_data =
                &board::CASTLING_METADATA[position.side_to_move as usize][*board_side as usize];
            position
                .move_piece(castling_meta_data.rook_from_square, castling_meta_data.rook_to_square);
            position.castling_rights[position.side_to_move as usize] = [false, false];
            position.castled[position.side_to_move as usize] = true;
        }

        fn make_promotion_move(
            position: &mut Position,
            undo_move_info: &mut UndoMoveInfo,
            base_move: &BaseMove,
            promote_to: &PieceType,
        ) {
            undo_move_info.captured_piece_type = position
                .remove_piece(undo_move_info.mov.get_base_move().to as usize)
                .map(|piece| piece.piece_type);
            position.remove_piece(base_move.from as usize);
            position.put_piece(
                base_move.to as usize,
                Piece { piece_color: position.side_to_move(), piece_type: *promote_to },
            );
            position.half_move_clock = 0;
        }

        fn make_basic_move(
            position: &mut Position,
            undo_move_info: &mut UndoMoveInfo,
            from: usize,
            to: usize,
            capture: bool,
        ) {
            undo_move_info.captured_piece_type = position
                .board()
                .get_piece(undo_move_info.mov.get_base_move().to as usize)
                .map(|piece| piece.piece_type);
            let piece = position.move_piece(from, to);
            let piece_type = piece.piece_type;
            if piece_type == PieceType::Pawn && util::distance(from as isize, to as isize) == 2 {
                position.en_passant_capture_square = Some((from + to) / 2);
            } else if piece_type == PieceType::King
                && from == board::KING_HOME_SQUARE[position.side_to_move as usize]
            {
                position.castling_rights[position.side_to_move as usize] = [false, false];
            } else if piece_type == PieceType::Rook {
                if from
                    == board::CASTLING_METADATA[position.side_to_move as usize]
                        [BoardSide::KingSide as usize]
                        .rook_from_square
                {
                    position.castling_rights[position.side_to_move as usize]
                        [BoardSide::KingSide as usize] = false;
                } else if from
                    == board::CASTLING_METADATA[position.side_to_move as usize]
                        [BoardSide::QueenSide as usize]
                        .rook_from_square
                {
                    position.castling_rights[position.side_to_move as usize]
                        [BoardSide::QueenSide as usize] = false;
                }
            }
            if capture || piece_type == PieceType::Pawn {
                position.half_move_clock = 0;
            } else {
                position.half_move_clock += 1;
            }
        }
        #[allow(unused_variables)]
        let original_position: Position;
        #[cfg(debug_assertions)]
        {
            original_position = *self;
        }
        let mut undo_move_info = UndoMoveInfo::new(self, *mov);
        self.en_passant_capture_square = None;

        match mov {
            Move::Basic { base_move } => {
                make_basic_move(
                    self,
                    &mut undo_move_info,
                    base_move.from as usize,
                    base_move.to as usize,
                    base_move.capture,
                );
            }
            Move::EnPassant { base_move, capture_square: _ } => {
                make_en_passant_move(self, &mut undo_move_info, base_move);
            }
            Move::Castling { base_move, board_side } => {
                make_castling_move(self, &mut undo_move_info, base_move, board_side);
            }
            Move::Promotion { base_move, promote_to } => {
                make_promotion_move(self, &mut undo_move_info, base_move, promote_to);
            }
        }

        self.side_to_move = if self.side_to_move == PieceColor::White {
            PieceColor::Black
        } else {
            self.full_move_number += 1;
            PieceColor::White
        };
        if move_gen::king_attacks_finder(self, !self.side_to_move) == 0 {
            // it's a valid move because it doesn't leave the side making the move in check
            self.update_hash_code(&undo_move_info);

            #[cfg(debug_assertions)]
            {
                let mut temp_new_position = *self;
                temp_new_position.unmake_move(&undo_move_info);
                assert_eq!(format!("{temp_new_position:?}"), format!("{original_position:?}"));
            }

            Some(undo_move_info)
        } else {
            self.unmake_move(&undo_move_info);
            None
        }
    }

    pub fn unmake_move(&mut self, undo_move_info: &UndoMoveInfo) {
        let mov = &undo_move_info.mov;

        match mov {
            Move::Basic { base_move } => {
                self.move_piece(base_move.to as usize, base_move.from as usize);
                if let Some(piece_type) = undo_move_info.captured_piece_type {
                    self.put_piece(
                        base_move.to as usize,
                        Piece { piece_color: self.side_to_move, piece_type },
                    );
                }
            }
            Move::EnPassant { base_move, capture_square } => {
                self.move_piece(base_move.to as usize, base_move.from as usize);
                self.put_piece(
                    *capture_square as usize,
                    Piece { piece_color: self.side_to_move, piece_type: PieceType::Pawn },
                );
            }
            Move::Castling { base_move, board_side } => {
                self.move_piece(base_move.to as usize, base_move.from as usize);
                let castling_metadata =
                    &board::CASTLING_METADATA[!self.side_to_move as usize][*board_side as usize];
                self.move_piece(
                    castling_metadata.rook_to_square,
                    castling_metadata.rook_from_square,
                );
                self.castled[!self.side_to_move as usize] = false;
            }
            Move::Promotion { base_move, .. } => {
                self.remove_piece(base_move.to as usize);
                self.put_piece(
                    base_move.from as usize,
                    Piece { piece_color: !self.side_to_move, piece_type: PieceType::Pawn },
                );
                if let Some(piece_type) = undo_move_info.captured_piece_type {
                    self.put_piece(
                        base_move.to as usize,
                        Piece { piece_color: self.side_to_move, piece_type },
                    );
                }
            }
        }
        self.castling_rights = undo_move_info.old_castling_rights;
        self.en_passant_capture_square = undo_move_info.old_en_passant_capture_square;
        self.half_move_clock = undo_move_info.old_half_move_clock;
        self.full_move_number = undo_move_info.old_full_move_number;
        self.hash_code = undo_move_info.old_zobrist_hash;
        self.side_to_move = undo_move_info.old_side_to_move;
    }

    fn update_hash_code(&mut self, undo_move_info: &UndoMoveInfo) {
        self.hash_code ^= POSITION_HASHES.side_to_move_hashes_table[PieceColor::White as usize];
        self.hash_code ^= POSITION_HASHES.side_to_move_hashes_table[PieceColor::Black as usize];

        if self.castling_rights != undo_move_info.old_castling_rights {
            self.hash_code ^= POSITION_HASHES.castling_hashes_table
                [Position::castling_rights_as_u8(&undo_move_info.old_castling_rights) as usize];
            self.hash_code ^= POSITION_HASHES.castling_hashes_table
                [Position::castling_rights_as_u8(&self.castling_rights) as usize];
        }
        // en passant moves are only included in the hash if the relevant pawn can actually be captured en passant
        if undo_move_info.old_en_passant_capture_square.is_some()
            && undo_move_info.old_is_en_passant_capture_possible
        {
            // remove the old en passant from the hash only if an en passant capture could be made because it won't have been added to the hash
            self.hash_code ^= POSITION_HASHES.en_passant_capture_square_hashes_table
                [undo_move_info.old_en_passant_capture_square.unwrap()];
        }
        if move_gen::is_en_passant_capture_possible(self) {
            // add the new en passant square to the hash only if an en passant capture can actually be made
            self.hash_code ^= POSITION_HASHES.en_passant_capture_square_hashes_table
                [self.en_passant_capture_square.unwrap()];
        }

        #[cfg(debug_assertions)]
        {
            if self.hash_code != self.create_initial_hash() {
                panic!("Hash code mismatch after move: {}", undo_move_info.mov);
            }
        }
    }

    pub fn can_castle(&self, piece_color: PieceColor, board_side: &BoardSide) -> bool {
        if self.castling_rights[piece_color as usize][*board_side as usize]
            && self.board.can_castle(piece_color, board_side)
        {
            let castling_metadata =
                &board::CASTLING_METADATA[piece_color as usize][*board_side as usize];
            return move_gen::king_attacks_finder(self, self.side_to_move) == 0
                && move_gen::square_attacks_finder(
                    self,
                    self.opposing_side(),
                    castling_metadata.king_through_square,
                ) == 0;
        }
        false
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

    fn castling_rights_as_u8(castling_rights: &[[bool; 2]; 2]) -> u8 {
        (castling_rights[PieceColor::White as usize][BoardSide::KingSide as usize] as u8)
            | ((castling_rights[PieceColor::White as usize][BoardSide::QueenSide as usize] as u8)
                << 1)
            | ((castling_rights[PieceColor::Black as usize][BoardSide::KingSide as usize] as u8)
                << 2)
            | ((castling_rights[PieceColor::Black as usize][BoardSide::QueenSide as usize] as u8)
                << 3)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::move_gen::generate_moves;
    use crate::core::piece::PieceType::Queen;

    #[test]
    fn test_general_usability() {
        let position: Position =
            Position::new(Board::new(), PieceColor::Black, "KQkq".to_string(), Some(31), 99, 50);

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
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize],
            true
        );
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize],
            true
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize],
            false
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize],
            false
        );

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w kq - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize],
            false
        );
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize],
            false
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize],
            true
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize],
            true
        );

        let fen: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Qk - 0 1";
        let position: Position = Position::from(fen);
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::KingSide as usize],
            false
        );
        assert_eq!(
            position.castling_rights()[PieceColor::White as usize][BoardSide::QueenSide as usize],
            true
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::KingSide as usize],
            true
        );
        assert_eq!(
            position.castling_rights()[PieceColor::Black as usize][BoardSide::QueenSide as usize],
            false
        );
    }

    #[test]
    fn test_cannot_castle_out_of_check() {
        let fen = "r3k2r/p1pp1pb1/bn2Qnp1/2qPN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQkq - 3 2";
        let mut position = Position::from(fen);
        let moves = generate_moves(&position);
        let castling_moves: Vec<&Move> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 0);
        assert_eq!(
            castling_moves
                .iter()
                .filter_map(|chess_move| { position.make_move(chess_move) })
                .count(),
            0
        );
    }

    #[test]
    fn test_cannot_castle_through_check() {
        let fen = "r3k3/8/8/8/7B/8/8/4K3 b q - 0 1";
        let mut position = Position::from(fen);
        let moves = generate_moves(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 0);
        assert_eq!(
            castling_moves
                .iter()
                .filter_map(|chess_move| { position.make_move(chess_move) })
                .count(),
            0
        );
    }

    #[test]
    fn test_ep_capture_square_is_set_after_double_white_pawn_move() {
        let mut position = Position::new_game();
        position.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position.en_passant_capture_square, Some(sq!("e3")));
        position.make_raw_move(&RawMove::new(sq!("b8"), sq!("c6"), None)).unwrap();
        assert_eq!(position.en_passant_capture_square, None);
    }

    #[test]
    fn test_ep_capture_square_is_set_after_double_black_pawn_move() {
        let mut position = Position::new_game();
        position.make_raw_move(&RawMove::new(sq!("e2"), sq!("e3"), None)).unwrap();
        assert_eq!(position.en_passant_capture_square, None);
        position.make_raw_move(&RawMove::new(sq!("a7"), sq!("a5"), None)).unwrap();
        assert_eq!(position.en_passant_capture_square, Some(sq!("a6")));
        position.make_raw_move(&RawMove::new(sq!("c2"), sq!("c3"), None)).unwrap();
        assert_eq!(position.en_passant_capture_square, None);
    }

    #[test]
    fn test_castling_rights_lost_after_castling() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        assert_eq!(position.castling_rights[PieceColor::White as usize], [true, true]);
        assert_eq!(position.castling_rights[PieceColor::Black as usize], [true, true]);
        assert_eq!(Position::castling_rights_as_u8(&position.castling_rights), 15);

        let moves = generate_moves(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);

        let mut position_0 = position.clone();
        position_0.make_move(&castling_moves[0]).unwrap();
        assert_eq!(position_0.castling_rights[PieceColor::White as usize], [false, false]);

        let mut position_1 = position.clone();
        position_1.make_move(&castling_moves[0]).unwrap();
        assert_eq!(position_1.castling_rights[PieceColor::White as usize], [false, false]);
        assert_eq!(Position::castling_rights_as_u8(&position_1.castling_rights), 12);
    }

    #[test]
    fn test_castling_rights_lost_after_moving_king() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let mut position = Position::from(fen);
        assert_eq!(position.castling_rights[PieceColor::White as usize], [true, true]);
        assert_eq!(position.castling_rights[PieceColor::Black as usize], [true, true]);
        assert_eq!(Position::castling_rights_as_u8(&position.castling_rights), 15);
        let moves = generate_moves(&position);
        let king_moves: Vec<_> = util::filter_moves_by_from_square(moves, sq!("e1"));
        assert_eq!(king_moves.len(), 7);
        let _ = position.make_move(&king_moves[0]).unwrap();
        assert_eq!(position.castling_rights[PieceColor::White as usize], [false, false]);
        assert_eq!(position.castling_rights[PieceColor::Black as usize], [true, true]);
        assert_eq!(Position::castling_rights_as_u8(&position.castling_rights), 12);
    }
    #[test]
    fn test_castling_rights_lost_after_moving_rook() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let original_position = Position::from(fen);
        assert_eq!(original_position.castling_rights[PieceColor::White as usize], [true, true]);
        assert_eq!(original_position.castling_rights[PieceColor::Black as usize], [true, true]);

        let mut position = original_position.clone();
        position.make_raw_move(&RawMove::new(sq!("a1"), sq!("a2"), None)).unwrap();
        assert_eq!(position.castling_rights[PieceColor::White as usize], [true, false]);
        assert_eq!(position.castling_rights[PieceColor::Black as usize], [true, true]);
        assert_eq!(Position::castling_rights_as_u8(&position.castling_rights), 13);

        let mut position = original_position.clone();
        position.make_raw_move(&RawMove::new(sq!("h1"), sq!("h2"), None)).unwrap();
        assert_eq!(position.castling_rights[PieceColor::White as usize], [false, true]);
        assert_eq!(position.castling_rights[PieceColor::Black as usize], [true, true]);
        assert_eq!(Position::castling_rights_as_u8(&position.castling_rights), 14);
    }

    #[test]
    fn test_full_move_counter_incremented_after_black_move() {
        let mut position = Position::new_game();
        assert_eq!(position.full_move_number, 1);
        position.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position.full_move_number, 1);
        let _ = position.make_raw_move(&RawMove::new(sq!("e7"), sq!("e5"), None)).unwrap();
        assert_eq!(position.full_move_number, 2);
    }

    #[test]
    fn test_half_move_counter_incrementation() {
        let mut position = Position::new_game();
        assert_eq!(position.half_move_clock, 0);
        position.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(position.half_move_clock, 0);
        position.make_raw_move(&RawMove::new(sq!("e7"), sq!("e5"), None)).unwrap();
        assert_eq!(position.half_move_clock, 0);
        position.make_raw_move(&RawMove::new(sq!("g1"), sq!("f3"), None)).unwrap();
        assert_eq!(position.half_move_clock, 1);
        position.make_raw_move(&RawMove::new(sq!("d7"), sq!("d6"), None)).unwrap();
        assert_eq!(position.half_move_clock, 0);
        position.make_raw_move(&RawMove::new(sq!("b1"), sq!("c3"), None)).unwrap();
        assert_eq!(position.half_move_clock, 1);
        position.make_raw_move(&RawMove::new(sq!("d8"), sq!("g5"), None)).unwrap();
        assert_eq!(position.half_move_clock, 2);
        position.make_raw_move(&RawMove::new(sq!("f3"), sq!("g5"), None)).unwrap();
        assert_eq!(position.half_move_clock, 0);
    }

    #[test]
    fn test_promotion_move_resets_half_move_counter() {
        let fen = "7k/4P3/8/8/8/8/5p2/K3N3 b - - 10 1";
        let original_position = Position::from(fen);
        assert_eq!(original_position.half_move_clock, 10);

        // non capture promotion
        let mut position = original_position.clone();
        position.make_raw_move(&RawMove::new(sq!("f2"), sq!("f1"), Some(Queen))).unwrap();
        assert_eq!(position.half_move_clock, 0);

        // capturing promotion
        let mut position = original_position.clone();
        position.make_raw_move(&RawMove::new(sq!("f2"), sq!("e1"), Some(Queen))).unwrap();
        assert_eq!(position.half_move_clock, 0);
    }

    #[test]
    fn test_castling_move_sets_castled_flag() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        assert_eq!(position.has_castled(PieceColor::White), false);
        assert_eq!(position.has_castled(PieceColor::Black), false);

        let moves = generate_moves(&position);
        let castling_moves: Vec<_> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        let mut position_0 = position.clone();
        position_0.make_move(&castling_moves[0]).unwrap();

        let mut position_1 = position.clone();
        position_1.make_move(&castling_moves[1]).unwrap();
    }

    #[test]
    fn test_unmake_basic_move() {
        let fen = "4k3/8/8/6n1/4R3/8/8/4K3 b - - 0 1";
        let original_position = Position::from(fen);

        // no capture
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("g5"), sq!("e6"), None)).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        // with capture
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("g5"), sq!("e4"), None)).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        // still in check
        let mut position = original_position.clone();
        let undo_move_info = position.make_raw_move(&RawMove::new(sq!("g5"), sq!("f3"), None));
        assert!(undo_move_info.is_none());
        assert_eq!(format!("{:?}", original_position), format!("{:?}", original_position));
    }

    #[test]
    fn test_unmake_en_passant_move() {
        let fen = "4k3/8/8/4pP2/8/8/8/4K3 w - e6 0 1";
        let original_position = Position::from(fen);

        // with capture
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("f5"), sq!("e6"), None)).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));
    }

    #[test]
    fn test_unmake_castling_move() {
        let fen = "r3k3/8/8/8/8/8/8/4K3 b q - 0 1";
        let original_position = Position::from(fen);

        // legal castling
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("e8"), sq!("c8"), None)).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        let fen = "r3k3/8/8/8/8/8/8/2R1K3 b q - 0 1";
        let original_position = Position::from(fen);

        // castling into check
        let mut position = original_position.clone();
        let undo_move_info = position.make_raw_move(&RawMove::new(sq!("e8"), sq!("c8"), None));
        assert!(undo_move_info.is_none());
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));
    }

    #[test]
    fn test_unmake_promotion_move() {
        let fen = "2n1k3/1P6/8/8/8/8/8/4K3 w - - 0 1";
        let original_position = Position::from(fen);

        // no capture
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("b7"), sq!("b8"), Some(Queen))).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        // with capture
        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("b7"), sq!("c8"), Some(Queen))).unwrap();
        position.unmake_move(&undo_move_info);
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        // still in check
        let fen = "2n1k3/1P6/8/8/7b/8/8/4K3 w - - 0 1";
        let original_position = Position::from(fen);

        let mut position = original_position.clone();
        let undo_move_info =
            position.make_raw_move(&RawMove::new(sq!("b7"), sq!("c8"), Some(Queen)));
        assert!(undo_move_info.is_none());
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));
    }

    #[test]
    fn test_castling_through_check_does_not_mutate_position() {
        let fen = "4k3/8/b7/8/8/8/8/4K2R w K - 0 1";
        let mut original_position = Position::from(fen);
        let mut position = original_position.clone();
        let undo_move_info = position.make_raw_move(&RawMove::new(sq!("e1"), sq!("g1"), None));
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));

        // now set the enpassant square
        let fen = "4k3/8/b7/2pP4/8/8/8/4K2R w K c6 0 1";
        let mut original_position = Position::from(fen);
        let mut position = original_position.clone();
        let undo_move_info = position.make_raw_move(&RawMove::new(sq!("e1"), sq!("g1"), None));
        assert_eq!(format!("{:?}", original_position), format!("{:?}", position));
    }
}
