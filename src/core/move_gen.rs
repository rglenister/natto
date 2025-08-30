use crate::core::board;
use crate::core::board::BoardSide;
use crate::core::piece::{PieceColor, PieceType};
use crate::core::position::Position;
use crate::core::r#move::BaseMove;
use crate::core::r#move::Move;
use crate::utils::bitboard_iterator::BitboardIterator;
use crate::utils::util;
use arrayvec::ArrayVec;
use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use strum::IntoEnumIterator;

include!("../utils/generated_macro.rs");

pub fn generate_moves(position: &Position) -> Vec<Move> {
    let mut move_generator = MoveGeneratorImpl::new(*position, MoveListMoveProcessor::new());
    move_generator.generate();
    move_generator.move_processor.get_result().clone()
}

pub fn generate_moves_for_quiescence(position: &Position) -> Vec<Move> {
    let mut move_processor = MoveListMoveProcessor::new();
    move_processor
        .set_filter(|mov| mov.get_base_move().capture || matches!(mov, Move::Promotion { .. }));
    let mut move_generator = MoveGeneratorImpl::new(*position, move_processor);
    move_generator.generate();
    move_generator.move_processor.get_result()
}

pub fn has_legal_move(position: &Position) -> bool {
    get_first_legal_move(position).is_some()
}

pub fn get_first_legal_move(position: &Position) -> Option<Move> {
    let mut move_generator = MoveGeneratorImpl {
        position: *position,
        move_processor: HasLegalMoveProcessor::new(*position),
        occupied_squares: position.board().bitboard_all_pieces(),
        friendly_squares: position.board().bitboard_by_color(position.side_to_move()),
    };
    move_generator.generate();
    move_generator.move_processor.get_result()
}

pub fn square_attacks_finder_empty_board(
    position: &Position,
    attacking_color: PieceColor,
    square_index: usize,
) -> u64 {
    square_attacks_finder_internal(position, attacking_color, square_index, 0)
}

pub fn square_attacks_finder(
    position: &Position,
    attacking_color: PieceColor,
    square_index: usize,
) -> u64 {
    square_attacks_finder_internal(
        position,
        attacking_color,
        square_index,
        position.board().bitboard_all_pieces(),
    )
}

pub fn get_sliding_moves_by_piece_type_and_square_index(
    piece_type: &PieceType,
    square_index: usize,
    occupied_squares: u64,
) -> u64 {
    let table_entry = &SLIDING_PIECE_MOVE_TABLE[*piece_type as usize][square_index];
    let occupied_blocking_squares_bitboard =
        occupied_squares & table_entry.blocking_squares_bitboard;
    let table_entry_bitboard_index =
        occupied_blocking_squares_bitboard.pext(table_entry.blocking_squares_bitboard);
    let valid_moves = *table_entry.moves_bitboard.get(table_entry_bitboard_index as usize).unwrap();
    valid_moves
}

pub fn is_en_passant_capture_possible(position: &Position) -> bool {
    if let Some(en_passant_capture_square) = position.en_passant_capture_square() {
        position.board().bitboard_by_color_and_piece_type(position.side_to_move(), PieceType::Pawn)
            & PAWN_ATTACKS_TABLE[position.opposing_side() as usize][en_passant_capture_square]
            != 0
    } else {
        false
    }
}
pub fn non_sliding_piece_attacks_empty_board(piece_type: PieceType, square_index: usize) -> u64 {
    NON_SLIDING_PIECE_MOVE_TABLE[piece_type as usize][square_index]
}

pub fn non_sliding_piece_attacks(
    position: &Position,
    attacking_piece_type: PieceType,
    attacking_color: PieceColor,
    square_index: usize,
) -> u64 {
    let moves = NON_SLIDING_PIECE_MOVE_TABLE[attacking_piece_type as usize][square_index];
    let enemy_squares =
        position.board().bitboard_by_color_and_piece_type(attacking_color, attacking_piece_type);
    moves & enemy_squares
}

pub fn king_attacks_finder(position: &Position, king_color: PieceColor) -> u64 {
    square_attacks_finder(position, king_color.opposite(), position.board().king_square(king_color))
}

pub fn king_attacks_finder_empty_board(position: &Position, king_color: PieceColor) -> u64 {
    square_attacks_finder_empty_board(
        position,
        king_color.opposite(),
        position.board().king_square(king_color),
    )
}

pub fn check_count(position: &Position) -> usize {
    king_attacks_finder(position, position.side_to_move()).count_ones() as usize
}

pub fn is_check(position: &Position) -> bool {
    check_count(position) > 0
}

pub fn squares_attacked_by_pawn(piece_color: PieceColor, pawn_square_index: usize) -> u64 {
    PAWN_ATTACKS_TABLE[piece_color as usize][pawn_square_index]
}

static PAWN_ATTACKS_TABLE: Lazy<[[u64; 64]; 2]> = Lazy::new(|| {
    let mut table = [[0u64; 64]; 2];
    // the white table contains the attacks by white pawns on a square
    table[0].clone_from_slice(&generate_move_table([7, 9]));
    // the black table contains the attacks by black pawns on a square
    table[1].clone_from_slice(&generate_move_table([-7, -9]));
    fn generate_move_table(increments: [isize; 2]) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        for square_index in 0..64 {
            let move_squares: u64 =
                generate_move_bitboard(square_index, (increments).to_vec(), 0, false, false);
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    table
});

static PIECE_INCREMENTS_TABLE: Lazy<[Vec<isize>; 6]> = Lazy::new(|| {
    let mut table: [Vec<isize>; 6] = Default::default();
    table[PieceType::Pawn as usize] = vec![];
    table[PieceType::Knight as usize] = vec![10, 17, 15, 6, -10, -17, -15, -6];
    table[PieceType::Bishop as usize] = vec![9, 7, -9, -7];
    table[PieceType::Rook as usize] = vec![1, 8, -1, -8];
    table[PieceType::Queen as usize] =
        [table[PieceType::Bishop as usize].clone(), table[PieceType::Rook as usize].clone()]
            .concat();
    table[PieceType::King as usize] = table[PieceType::Queen as usize].clone();
    table
});

static NON_SLIDING_PIECE_MOVE_TABLE: Lazy<[[u64; 64]; 6]> = Lazy::new(|| {
    let mut table = [[0u64; 64]; 6];
    for piece_type in [PieceType::Knight, PieceType::King] {
        table[piece_type as usize].clone_from_slice(generate_move_table(piece_type).as_slice());
    }

    fn generate_move_table(piece_type: PieceType) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        let increments = PIECE_INCREMENTS_TABLE.get(piece_type as usize).unwrap();
        for square_index in 0..64 {
            let move_squares: u64 =
                generate_move_bitboard(square_index, increments.to_vec(), 0, false, false);
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    table
});

#[derive(Clone)]
struct TableEntry {
    blocking_squares_bitboard: u64,
    moves_bitboard: Vec<u64>,
}

static SLIDING_PIECE_MOVE_TABLE: Lazy<[Vec<TableEntry>; 6]> = Lazy::new(|| {
    let mut table: [Vec<TableEntry>; 6] = Default::default();

    for piece_type in [PieceType::Bishop, PieceType::Rook] {
        table[piece_type as usize] = generate_move_table(piece_type);
    }

    fn generate_move_table(piece_type: PieceType) -> Vec<TableEntry> {
        let mut squares: Vec<TableEntry> = Vec::new();
        for square_index in 0..64 {
            let blocking_squares_bitboard: u64 = generate_move_bitboard(
                square_index,
                PIECE_INCREMENTS_TABLE[piece_type as usize].clone(),
                0,
                true,
                true,
            );
            let n_ones = blocking_squares_bitboard.count_ones() as u64;
            let table_size: u64 = 2_i32.pow((n_ones as i32).try_into().unwrap()) as u64;
            let mut moves_bitboard: Vec<u64> = Vec::new();
            for table_index in 0..table_size {
                let blocking_pieces_bitboard: u64 = table_index.pdep(blocking_squares_bitboard);
                let sliding_move_bitboard = generate_move_bitboard(
                    square_index,
                    PIECE_INCREMENTS_TABLE.get(piece_type as usize).unwrap().clone(),
                    blocking_pieces_bitboard,
                    false,
                    true,
                );
                moves_bitboard.push(sliding_move_bitboard);
            }
            let table_entry: TableEntry = TableEntry { blocking_squares_bitboard, moves_bitboard };
            squares.push(table_entry);
        }
        squares
    }

    table
});

trait MoveProcessor {
    type Output;
    fn process_move(&mut self, move_: Move);
    fn continue_processing(&mut self) -> bool;
    fn get_result(&self) -> Self::Output;
}

const MOVE_LIST_LENGTH: usize = 250;

struct MoveListMoveProcessor {
    capture_moves: ArrayVec<Move, MOVE_LIST_LENGTH>,
    non_capture_moves: ArrayVec<Move, MOVE_LIST_LENGTH>,
    move_filter: Box<dyn Fn(&Move) -> bool>,
}

struct HasLegalMoveProcessor {
    position: Position,
    legal_move: Option<Move>,
}

impl MoveProcessor for MoveListMoveProcessor {
    type Output = Vec<Move>;

    fn process_move(&mut self, mov: Move) {
        if (self.move_filter)(&mov) {
            if mov.get_base_move().capture {
                self.capture_moves.push(mov);
            } else {
                self.non_capture_moves.push(mov);
            }
        }
    }

    fn continue_processing(&mut self) -> bool {
        true
    }

    fn get_result(&self) -> Self::Output {
        let mut moves = self.capture_moves.clone();
        moves.extend(self.non_capture_moves.iter().cloned());
        moves.into_iter().collect()
    }
}
impl MoveProcessor for HasLegalMoveProcessor {
    type Output = Option<Move>;
    fn process_move(&mut self, mov: Move) {
        if self.legal_move.is_none() && self.position.make_move(&mov).is_some() {
            self.legal_move = Some(mov);
        }
    }

    fn continue_processing(&mut self) -> bool {
        self.legal_move.is_none()
    }

    fn get_result(&self) -> Option<Move> {
        self.legal_move
    }
}

impl MoveListMoveProcessor {
    fn new() -> Self {
        MoveListMoveProcessor {
            capture_moves: ArrayVec::new(),
            non_capture_moves: ArrayVec::new(),
            move_filter: Box::new(|_| true),
        }
    }

    fn set_filter(&mut self, filter: impl Fn(&Move) -> bool + 'static) {
        self.move_filter = Box::new(filter)
    }
}
impl HasLegalMoveProcessor {
    fn new(position: Position) -> Self {
        HasLegalMoveProcessor { position, legal_move: None }
    }
}

struct MoveGeneratorImpl<P: MoveProcessor> {
    position: Position,
    move_processor: P,
    occupied_squares: u64,
    friendly_squares: u64,
}

impl<P: MoveProcessor + std::any::Any> MoveGeneratorImpl<P> {
    fn new(position: Position, move_processor: P) -> Self {
        let occupied_squares = position.board().bitboard_all_pieces();
        let friendly_squares = position.board().bitboard_by_color(position.side_to_move());
        MoveGeneratorImpl { position, move_processor, occupied_squares, friendly_squares }
    }
    fn generate(&mut self)
    where
        P: MoveProcessor,
    {
        let board: &board::Board = self.position.board();
        let bitboards: [u64; 6] = board.bitboards_for_color(self.position.side_to_move());
        for piece_type in [
            PieceType::King,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::Pawn,
        ] {
            if self.move_processor.continue_processing() {
                self.generate_moves_for_piece_type(piece_type, bitboards[piece_type as usize]);
            } else {
                break;
            }
        }
    }
    fn generate_moves_for_piece_type(&mut self, piece_type: PieceType, bitboard: u64) {
        match piece_type {
            PieceType::Pawn => {
                generate_pawn_moves::<P, <P as MoveProcessor>::Output>(
                    &self.position,
                    bitboard,
                    self.occupied_squares,
                    &mut self.move_processor,
                );
            }
            PieceType::Knight => {
                get_non_sliding_moves_by_piece_type::<<P as MoveProcessor>::Output>(
                    PieceType::Knight,
                    bitboard,
                    self.occupied_squares,
                    self.friendly_squares,
                    &mut self.move_processor,
                );
            }
            PieceType::Bishop | PieceType::Rook => {
                get_sliding_moves_by_piece_type(
                    piece_type,
                    bitboard,
                    self.occupied_squares,
                    self.friendly_squares,
                    &mut self.move_processor,
                );
            }
            PieceType::Queen => {
                for piece_type in [PieceType::Bishop, PieceType::Rook] {
                    get_sliding_moves_by_piece_type(
                        piece_type,
                        bitboard,
                        self.occupied_squares,
                        self.friendly_squares,
                        &mut self.move_processor,
                    );
                }
            }
            PieceType::King => {
                generate_king_moves::<<P as MoveProcessor>::Output>(
                    &self.position,
                    bitboard,
                    self.occupied_squares,
                    self.friendly_squares,
                    &mut self.move_processor,
                );
            }
        }
    }
}

fn get_non_sliding_moves_by_piece_type<U>(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output = U> + Sized),
) {
    let square_iterator = BitboardIterator::new(square_indexes);
    for square_index in square_iterator {
        let destinations = NON_SLIDING_PIECE_MOVE_TABLE[piece_type as usize][square_index];
        generate_moves_for_destinations(
            square_index,
            destinations,
            occupied_squares,
            friendly_squares,
            move_processor,
        );
    }
}
fn get_sliding_moves_by_piece_type<T>(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output = T> + Sized),
) {
    let square_iterator = BitboardIterator::new(square_indexes);
    for square_index in square_iterator {
        let valid_moves = get_sliding_moves_by_piece_type_and_square_index(
            &piece_type,
            square_index,
            occupied_squares,
        );
        generate_moves_for_destinations(
            square_index,
            valid_moves,
            occupied_squares,
            friendly_squares,
            move_processor,
        );
    }
}

fn generate_moves_for_destinations<T>(
    from: usize,
    destinations: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output = T> + Sized),
) {
    let square_iterator = BitboardIterator::new(destinations);
    for to in square_iterator {
        if friendly_squares & (1 << to) == 0 {
            move_processor.process_move(Move::Basic {
                base_move: BaseMove::new(from as u8, to as u8, occupied_squares & (1 << to) != 0),
            });
        }
    }
}

/// Pre-calculates the bitmaps
fn generate_move_bitboard(
    source_square: isize,
    increments: Vec<isize>,
    blocking_pieces_bitboard: u64,
    generating_blocking_square_mask: bool,
    sliding: bool,
) -> u64 {
    let bitboards: Vec<_> = increments
        .into_iter()
        .map(|increment| {
            generate_move_bitboard_for_increment(
                source_square,
                blocking_pieces_bitboard,
                increment,
                generating_blocking_square_mask,
                sliding,
            )
        })
        .collect();
    return bitboards.iter().fold(0, |acc: u64, bitboard: &u64| acc | bitboard);

    fn generate_move_bitboard_for_increment(
        source_square: isize,
        blocking_pieces_bitboard: u64,
        increment: isize,
        generating_blocking_square_mask: bool,
        sliding: bool,
    ) -> u64 {
        let destination_square: isize = source_square + increment;
        if util::on_board(source_square, destination_square)
            && (!generating_blocking_square_mask
                || util::on_board(destination_square, destination_square + increment))
        {
            let result = 1 << destination_square;
            if sliding && blocking_pieces_bitboard & (1 << destination_square) == 0 {
                result
                    | generate_move_bitboard_for_increment(
                        destination_square,
                        blocking_pieces_bitboard,
                        increment,
                        generating_blocking_square_mask,
                        sliding,
                    )
            } else {
                result
            }
        } else {
            0
        }
    }
}

fn generate_king_moves<U>(
    position: &Position,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output = U> + Sized),
) {
    get_non_sliding_moves_by_piece_type::<U>(
        PieceType::King,
        1 << square_indexes.trailing_zeros(),
        occupied_squares,
        friendly_squares,
        move_processor,
    );
    BoardSide::iter()
        .filter(|board_side| position.can_castle(position.side_to_move(), board_side))
        .map(|board_side| {
            &board::CASTLING_METADATA[position.side_to_move() as usize][board_side as usize]
        })
        .map(|cmd| Move::Castling {
            base_move: BaseMove::new(cmd.king_from_square as u8, cmd.king_to_square as u8, false),
            board_side: cmd.board_side,
        })
        .for_each(|mv| move_processor.process_move(mv));
}

fn generate_pawn_moves<P, U>(
    position: &Position,
    square_indexes: u64,
    occupied_squares: u64,
    move_processor: &mut P,
) where
    P: MoveProcessor<Output = U> + Sized,
{
    let create_moves = |from: usize, to: usize, capture: bool, move_processor: &mut P| {
        if board::Board::rank(to, position.side_to_move()) != 7 {
            move_processor.process_move(Move::Basic {
                base_move: BaseMove::new(from as u8, to as u8, capture),
            });
        } else {
            for piece_type in
                [PieceType::Queen, PieceType::Knight, PieceType::Rook, PieceType::Bishop]
            {
                move_processor.process_move(Move::Promotion {
                    base_move: { BaseMove::new(from as u8, to as u8, capture) },
                    promote_to: piece_type,
                });
            }
        }
    };

    let side_to_move = position.side_to_move();
    let opposing_side = !side_to_move;
    let opposing_side_bitboard = position.board().bitboard_by_color(opposing_side);
    let pawn_increment: isize = if side_to_move == PieceColor::White { 8 } else { -8 };

    let square_iterator = BitboardIterator::new(square_indexes);
    for square_index in square_iterator {
        // generate forward moves
        let one_step_forward: u64 = 1 << ((square_index as isize + pawn_increment) as usize);
        if occupied_squares & one_step_forward == 0 {
            create_moves(
                square_index,
                one_step_forward.trailing_zeros() as usize,
                false,
                move_processor,
            );
            if board::Board::rank(square_index, side_to_move) == 1 {
                let two_steps_forward =
                    1 << ((square_index as isize + pawn_increment * 2) as usize);
                if (occupied_squares & two_steps_forward) == 0 {
                    create_moves(
                        square_index,
                        two_steps_forward.trailing_zeros() as usize,
                        false,
                        move_processor,
                    );
                }
            }
        }

        // generate standard captures
        let attacked_squares = PAWN_ATTACKS_TABLE[side_to_move as usize][square_index];
        let attacked_opposing_piece_squares = attacked_squares & opposing_side_bitboard;
        for attacked_square_index in BitboardIterator::new(attacked_opposing_piece_squares) {
            create_moves(square_index, attacked_square_index, true, move_processor);
        }

        // generate en passant capture
        if let Some(ep_square) = position.en_passant_capture_square() {
            if ((1 << ep_square) & attacked_squares) != 0 {
                let ep_move = Move::EnPassant {
                    base_move: BaseMove::new(square_index as u8, ep_square as u8, true),
                    capture_square: (ep_square as isize - pawn_increment) as usize as u8,
                };
                move_processor.process_move(ep_move);
            }
        }
    }
}

fn square_attacks_finder_internal(
    position: &Position,
    attacking_color: PieceColor,
    square_index: usize,
    occupied_squares: u64,
) -> u64 {
    let enemy_squares = position.board().bitboard_by_color(attacking_color);
    let enemy_queens =
        position.board().bitboard_by_color_and_piece_type(attacking_color, PieceType::Queen);
    let mut attacking_squares = 0;
    for piece_type in [PieceType::Bishop, PieceType::Rook] {
        let moves = get_sliding_moves_by_piece_type_and_square_index(
            &piece_type,
            square_index,
            occupied_squares,
        );
        let possible_attackers = moves & enemy_squares;
        let square_iterator = BitboardIterator::new(possible_attackers);
        for square_index in square_iterator {
            if (enemy_queens
                | position.board().bitboard_by_color_and_piece_type(attacking_color, piece_type))
                & (1 << square_index)
                != 0
            {
                attacking_squares |= 1 << square_index;
            }
        }
    }
    attacking_squares |=
        non_sliding_piece_attacks(position, PieceType::King, attacking_color, square_index);
    attacking_squares |=
        non_sliding_piece_attacks(position, PieceType::Knight, attacking_color, square_index);

    let pawn_attack_squares = PAWN_ATTACKS_TABLE[!attacking_color as usize][square_index];
    let attacking_pawns =
        position.board().bitboard_by_color_and_piece_type(attacking_color, PieceType::Pawn);
    let attacking_pawn = pawn_attack_squares & attacking_pawns;
    attacking_squares |= attacking_pawn;

    attacking_squares
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::move_gen::generate_moves;
    use crate::core::r#move::BaseMove;

    /// 20 moves are generated from the initial position
    #[test]
    fn test_move_count_from_initial_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position = Position::from(fen);
        let moves = generate_moves(&position);
        assert_eq!(moves.len(), 20);
    }

    #[test]
    fn test_white_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 10);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(10, 18, false) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(10, 26, false) });
    }

    /// Black pawns can make single or double moves from their home squares
    #[test]
    fn test_black_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 53);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(53, 45, false) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(53, 37, false) });
    }

    /// White pawns can be completely blocked
    #[test]
    fn test_white_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// Black pawns can be completely blocked
    #[test]
    fn test_black_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// White pawns can be blocked from making a double move
    #[test]
    fn test_white_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 10);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(10, 18, false) });
    }

    /// Black pawns can be blocked from making a double move
    #[test]
    fn test_black_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 53);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(53, 45, false) });
    }

    /// White pawns can capture
    #[test]
    fn test_white_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);
        assert_eq!(all_moves.len(), 13);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 19);
        assert_eq!(moves.len(), 3);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(19, 26, true) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(19, 28, true) });
        assert_eq!(*moves.get(2).unwrap(), Move::Basic { base_move: BaseMove::new(19, 27, false) });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 23);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(23, 30, true) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(23, 31, false) });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 37);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(37, 46, true) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(37, 45, false) });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 44);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(44, 52, false) });
    }

    /// Black pawns can capture
    #[test]
    fn test_black_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 b - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 32);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(32, 24, false) });
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 26);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Move::Basic { base_move: BaseMove::new(26, 19, true) });
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(26, 18, false) });
    }

    #[test]
    fn test_is_en_passant_capture_possible() {
        let fen = "4k3/8/8/4PpP1/8/8/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        assert_eq!(is_en_passant_capture_possible(&position), false);

        let fen = "4k3/8/8/4PpP1/8/8/8/4K3 w - f6 0 1";
        let position = Position::from(fen);
        assert_eq!(is_en_passant_capture_possible(&position), true);

        let fen = "4k3/8/8/5p2/8/8/8/4K3 w - f6 0 1";
        let position = Position::from(fen);
        assert_eq!(is_en_passant_capture_possible(&position), false);
    }

    /// White pawns can capture en passant
    #[test]
    fn test_white_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/4PpP1/8/8/8/4K3 w - f6 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);

        assert_eq!(all_moves.len(), 9);
        assert_eq!(is_en_passant_capture_possible(&position), true);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 36);
        assert_eq!(moves.len(), 2);
        assert_eq!(
            *moves.get(0).unwrap(),
            Move::EnPassant { base_move: BaseMove::new(36, 45, true), capture_square: 37 }
        );
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(36, 44, false) });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 38);
        assert_eq!(moves.len(), 2);
        assert_eq!(
            *moves.get(0).unwrap(),
            Move::EnPassant { base_move: BaseMove::new(38, 45, true), capture_square: 37 }
        );
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(38, 46, false) });
    }

    /// Black pawns can capture en passant
    #[test]
    fn test_black_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/8/4pPp1/8/8/4K3 b - f3 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);

        assert_eq!(all_moves.len(), 9);
        assert_eq!(is_en_passant_capture_possible(&position), true);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 28);
        assert_eq!(moves.len(), 2);
        assert_eq!(
            *moves.get(0).unwrap(),
            Move::EnPassant { base_move: BaseMove::new(28, 21, true), capture_square: 29 }
        );
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(28, 20, false) });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 30);
        assert_eq!(moves.len(), 2);
        assert_eq!(
            *moves.get(0).unwrap(),
            Move::EnPassant { base_move: BaseMove::new(30, 21, true), capture_square: 29 }
        );
        assert_eq!(*moves.get(1).unwrap(), Move::Basic { base_move: BaseMove::new(30, 22, false) });
    }

    /// White pawns can be promoted
    #[test]
    fn test_white_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 50);
        assert_eq!(moves.len(), 4);
        assert_eq!(
            *moves.get(0).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(50, 58, false),
                promote_to: PieceType::Queen
            }
        );
        assert_eq!(
            *moves.get(1).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(50, 58, false),
                promote_to: PieceType::Knight
            }
        );
        assert_eq!(
            *moves.get(2).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(50, 58, false),
                promote_to: PieceType::Rook
            }
        );
        assert_eq!(
            *moves.get(3).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(50, 58, false),
                promote_to: PieceType::Bishop
            }
        );
    }

    /// Black pawns can be promoted
    #[test]
    fn test_black_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 14);
        assert_eq!(moves.len(), 4);

        assert_eq!(
            *moves.get(0).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Queen
            }
        );
        assert_eq!(
            *moves.get(1).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Knight
            }
        );
        assert_eq!(
            *moves.get(2).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 6, false), promote_to: PieceType::Rook }
        );
        assert_eq!(
            *moves.get(3).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Bishop
            }
        );
    }

    /// Black pawns can be promoted by capturing
    #[test]
    fn test_pawns_can_be_promoted_by_capturing() {
        let fen = "4k3/2P5/8/5b2/8/8/6p1/4KB1N b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 14);
        assert_eq!(moves.len(), 12);

        assert_eq!(
            *moves.get(0).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 5, true), promote_to: PieceType::Queen }
        );
        assert_eq!(
            *moves.get(1).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 5, true),
                promote_to: PieceType::Knight
            }
        );
        assert_eq!(
            *moves.get(2).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 5, true), promote_to: PieceType::Rook }
        );
        assert_eq!(
            *moves.get(3).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 5, true),
                promote_to: PieceType::Bishop
            }
        );

        assert_eq!(
            *moves.get(4).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 7, true), promote_to: PieceType::Queen }
        );
        assert_eq!(
            *moves.get(5).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 7, true),
                promote_to: PieceType::Knight
            }
        );
        assert_eq!(
            *moves.get(6).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 7, true), promote_to: PieceType::Rook }
        );
        assert_eq!(
            *moves.get(7).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 7, true),
                promote_to: PieceType::Bishop
            }
        );

        assert_eq!(
            *moves.get(8).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Queen
            }
        );
        assert_eq!(
            *moves.get(9).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Knight
            }
        );
        assert_eq!(
            *moves.get(10).unwrap(),
            Move::Promotion { base_move: BaseMove::new(14, 6, false), promote_to: PieceType::Rook }
        );
        assert_eq!(
            *moves.get(11).unwrap(),
            Move::Promotion {
                base_move: BaseMove::new(14, 6, false),
                promote_to: PieceType::Bishop
            }
        );
    }

    /// Test white king moves
    #[test]
    fn test_white_king_moves() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), sq!("e1"));
        assert_eq!(moves.len(), 7);
        let castling_moves: Vec<&Move> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(
            **castling_moves.get(0).unwrap(),
            Move::Castling {
                base_move: BaseMove::new(sq!("e1"), sq!("g1"), false),
                board_side: BoardSide::KingSide
            }
        );
        assert_eq!(
            **castling_moves.get(1).unwrap(),
            Move::Castling {
                base_move: BaseMove::new(sq!("e1"), sq!("c1"), false),
                board_side: BoardSide::QueenSide
            }
        );
    }

    /// Test black king moves
    #[test]
    fn test_black_king_moves() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), sq!("e8"));
        assert_eq!(moves.len(), 7);
        let castling_moves: Vec<&Move> =
            moves.iter().filter(|chess_move| matches!(chess_move, Move::Castling { .. })).collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(
            **castling_moves.get(0).unwrap(),
            Move::Castling {
                base_move: BaseMove::new(sq!("e8"), sq!("g8"), false),
                board_side: BoardSide::KingSide
            }
        );
        assert_eq!(
            **castling_moves.get(1).unwrap(),
            Move::Castling {
                base_move: BaseMove::new(sq!("e8"), sq!("c8"), false),
                board_side: BoardSide::QueenSide
            }
        );
    }

    /// Test square attacks finder
    #[test]
    fn test_king_attacks_finder_using_white_rook_and_bishop() {
        let fen = "4k2R/8/8/8/B7/8/8/4K3 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 2);
        assert_eq!(attacking_squares[0], 24);
        assert_eq!(attacking_squares[1], 63);
    }

    #[test]
    fn test_king_attacks_finder_using_black_rook_and_bishop() {
        let fen = "4k3/8/8/b7/8/8/8/1r2K3 w - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::White);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 2);
        assert_eq!(attacking_squares[0], 1);
        assert_eq!(attacking_squares[1], 32);
    }

    #[test]
    fn test_king_attacks_finder_using_white_queen() {
        let fen = "4k3/8/2Q5/8/8/8/8/4K3 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 42);
    }

    #[test]
    fn test_king_attacks_finder_using_black_knight() {
        let fen = "4k3/8/8/8/8/3n4/2N5/4K3 w - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::White);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 19);
    }

    #[test]
    fn test_king_attacks_finder_using_first_json_example() {
        let fen = "r6r/1b2k1bq/8/8/7B/8/8/R3K2R b KQ - 3 2";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 31);
    }

    #[test]
    fn test_king_attacks_finder_using_white_pawn() {
        let fen = "8/8/8/1K3k2/4P3/8/8/8 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, PieceColor::Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 28);
    }

    #[test]
    fn test_king_attacks_finder_empty_board() {
        let fen = "5rk1/5p1p/8/8/2B5/8/8/4K1R1 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes =
            king_attacks_finder_empty_board(&mut position, PieceColor::Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 2);
        assert!(attacking_squares.contains(&6) && attacking_squares.contains(&26));
    }

    #[test]
    fn test_pawn_attacks_table() {
        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::White as usize][0], 1 << 9);
        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::White as usize][1], 1 << 8 | 1 << 10);

        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::Black as usize][63], 1 << 54);
        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::Black as usize][62], 1 << 53 | 1 << 55);

        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::Black as usize][31], 1 << 22);
        assert_eq!(PAWN_ATTACKS_TABLE[PieceColor::White as usize][31], 1 << 38);
    }

    #[test]
    fn test_squares_attacked_by_pawn() {
        assert_eq!(squares_attacked_by_pawn(PieceColor::Black, 63), 1 << 54);
        assert_eq!(squares_attacked_by_pawn(PieceColor::White, 31), 1 << 38);
    }

    #[test]
    fn test_generate_moves_for_quiescence() {
        let fen = "8/4k3/Q7/8/4Pp2/8/3K2p1/r6R b - e3 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);
        assert_eq!(all_moves.len(), 30);

        let quiescence_moves = generate_moves_for_quiescence(&position);
        assert_eq!(quiescence_moves.len(), 11);

        // basic captures
        assert!(quiescence_moves
            .contains(&Move::Basic { base_move: BaseMove::new(sq!("a1"), sq!("a6"), true) }));
        assert!(quiescence_moves
            .contains(&Move::Basic { base_move: BaseMove::new(sq!("a1"), sq!("h1"), true) }));

        // en-passant
        assert!(quiescence_moves.contains(&Move::EnPassant {
            base_move: BaseMove::new(sq!("f4"), sq!("e3"), true),
            capture_square: sq!("e4")
        }));

        // promotions without capture
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("g1"), false),
            promote_to: PieceType::Knight
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("g1"), false),
            promote_to: PieceType::Bishop
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("g1"), false),
            promote_to: PieceType::Rook
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("g1"), false),
            promote_to: PieceType::Queen
        }));

        // promotions with capture
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("h1"), true),
            promote_to: PieceType::Knight
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("h1"), true),
            promote_to: PieceType::Bishop
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("h1"), true),
            promote_to: PieceType::Rook
        }));
        assert!(quiescence_moves.contains(&Move::Promotion {
            base_move: BaseMove::new(sq!("g2"), sq!("h1"), true),
            promote_to: PieceType::Queen
        }));
    }
}
