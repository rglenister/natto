
use crate::chessboard::board;
use crate::chessboard::piece::PieceColor::{Black, White};
use crate::chessboard::board::{Board, BoardSide};
use crate::r#move::Move::{Basic, Castling, EnPassant, Promotion};
use crate::r#move::{BaseMove, Move};
use crate::position::Position;

use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use strum::IntoEnumIterator;
use crate::chess_util::bitboard_iterator::BitboardIterator;
use crate::chess_util::util;
use crate::chessboard::piece::{PieceColor, PieceType};

include!("chess_util/generated_macro.rs");


static PAWN_ATTACKS_TABLE: Lazy<HashMap<&'static PieceColor, [u64; 64]>> = Lazy::new(|| {
    let mut table = HashMap::new();
    // the white table contains the attacks by white pawns on a square
    table.insert(&White, generate_move_table([7, 9]));
    // the black table contains the attacks by black pawns on a square
    table.insert(&Black, generate_move_table([-7, -9]));
    fn generate_move_table(increments: [isize; 2]) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        for square_index in 0..64 {
            let move_squares: u64 = generate_move_bitboard(
                square_index,
                (increments).to_vec(),
                0,
                false,
                false,
            );
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    table
});

static PIECE_INCREMENTS_TABLE: Lazy<HashMap<&'static PieceType, Vec<isize>>> = Lazy::new(|| {
    let mut table = HashMap::new();
    table.insert(&PieceType::Knight, vec![10, 17, 15, 6, -10, -17, -15, -6]);
    table.insert(&PieceType::Bishop, vec![9, 7, -9, -7]);
    table.insert(&PieceType::Rook, vec![1, 8, -1, -8]);
    table.insert(&PieceType::Queen, table.get(&PieceType::Bishop).unwrap().iter().chain(table.get(&PieceType::Rook).unwrap()).cloned().collect());
    table.insert(&PieceType::King, table.get(&PieceType::Queen).unwrap().clone());
    table
});

static NON_SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, [u64; 64]>> = Lazy::new(|| {
    let move_table = [PieceType::Knight, PieceType::King]
        .into_iter().map(|piece_type| (piece_type, generate_move_table(piece_type))).collect();
    fn generate_move_table(piece_type: PieceType) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        let increments = PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap();
        for square_index in 0..64 {
            let move_squares: u64 = generate_move_bitboard(
                square_index,
                increments.to_vec(),
                0,
                false,
                false,
            );
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    move_table
});

struct TableEntry {
    blocking_squares_bitboard: u64,
    moves_bitboard: Vec<u64>,
}

static SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, Vec<TableEntry>>> = Lazy::new(|| {
    let move_table = [PieceType::Bishop, PieceType::Rook]
        .into_iter().map(|piece_type| (piece_type, generate_move_table(piece_type))).collect();
    fn generate_move_table(piece_type: PieceType) -> Vec<TableEntry> {
        let mut squares: Vec<TableEntry> = Vec::new();
        for square_index in 0..64 {
            let blocking_squares_bitboard: u64 =
                generate_move_bitboard(
                    square_index,
                    PIECE_INCREMENTS_TABLE[&piece_type].clone(),
                    0,
                    true,
                    true
                );
            let n_ones = blocking_squares_bitboard.count_ones() as u64;
            let table_size: u64 = 2_i32.pow((n_ones as i32).try_into().unwrap()) as u64;
            let mut moves_bitboard: Vec<u64> = Vec::new();
            for table_index in 0..table_size {
                let blocking_pieces_bitboard: u64 = table_index.pdep(blocking_squares_bitboard);
                let sliding_move_bitboard = generate_move_bitboard(
                    square_index,
                    PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap().clone(),
                    blocking_pieces_bitboard,
                    false,
                    true,
                );
                moves_bitboard.push(sliding_move_bitboard);
            }
            let table_entry: TableEntry = TableEntry {
                blocking_squares_bitboard,
                moves_bitboard,
            };
            squares.push(table_entry);
        }
        squares
    }

    move_table
});

trait MoveProcessor {
    type Output;
    fn process_move(&mut self, move_: Move);
    fn continue_processing(&mut self) -> bool;
    fn get_result(&self) -> Self::Output;
}

struct MoveListMoveProcessor {
    capture_moves: Vec<Move>,
    non_capture_moves: Vec<Move>,
}

struct HasLegalMoveProcessor {
    position: Position,
    found_legal_move: bool,
}

impl MoveProcessor for MoveListMoveProcessor {
    type Output = Vec<Move>;

    fn process_move(&mut self, mov: Move) {
        if mov.get_base_move().capture {
            self.capture_moves.push(mov);
        } else {
            self.non_capture_moves.push(mov);
        }
    }

    fn continue_processing(&mut self) -> bool {
        true
    }

    fn get_result(&self) -> Self::Output {
        let mut moves = self.non_capture_moves.clone();
        moves.extend_from_slice(&self.capture_moves);
        moves
    }
}
impl MoveProcessor for HasLegalMoveProcessor {
    type Output = bool;
    fn process_move(&mut self, mov: Move) {
        if !self.found_legal_move {
            self.found_legal_move = self.position.make_move(&mov).is_some();
        }
    }

    fn continue_processing(&mut self) -> bool {
        !self.found_legal_move
    }

    fn get_result(&self) -> bool {
        self.found_legal_move
    }
}

impl MoveListMoveProcessor {
    fn new() -> Self {
        MoveListMoveProcessor {
            capture_moves: vec!(),
            non_capture_moves: vec!(),
        }
    }
}
impl HasLegalMoveProcessor {
    fn new(position: Position) -> Self {
        HasLegalMoveProcessor {
            position,
            found_legal_move: false,
        }
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
        MoveGeneratorImpl {
            position,
            move_processor,
            occupied_squares,
            friendly_squares,
        }
    }
    fn generate(&mut self) where P: MoveProcessor {
        let board: &Board = self.position.board();
        let bitboards: [u64; 6] = board.bitboards_for_color(self.position.side_to_move());
        for piece_type in [PieceType::King, PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
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
                generate_pawn_moves::<P, <P as MoveProcessor>::Output>(&self.position, bitboard, self.occupied_squares, &mut self.move_processor);
            }
            PieceType::Knight => {
                get_non_sliding_moves_by_piece_type::<P, <P as MoveProcessor>::Output>(PieceType::Knight, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
            }
            PieceType::Bishop => {
                get_sliding_moves_by_piece_type(PieceType::Bishop, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
            }
            PieceType::Rook => {
                get_sliding_moves_by_piece_type(PieceType::Rook, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
            }
            PieceType::Queen => {
                get_sliding_moves_by_piece_type(PieceType::Bishop, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
                get_sliding_moves_by_piece_type(PieceType::Rook, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
            }
            PieceType::King => {
                generate_king_moves::<P, <P as MoveProcessor>::Output>(&self.position, bitboard, self.occupied_squares, self.friendly_squares, &mut self.move_processor);
            },
        }
    }
}

pub fn generate_moves(position: &Position) -> Vec<Move> {
    let mut move_generator = MoveGeneratorImpl::new(position.clone(), MoveListMoveProcessor::new()); 
    move_generator.generate();
    move_generator.move_processor.get_result().clone()
}

pub(crate) fn generate_basic_capture_moves(position: &Position) -> Vec<Move> {
    let moves = generate_moves(position);
    moves.into_iter().filter(|mov| mov.get_base_move().capture && matches!(mov, Castling { .. })).collect()
}

pub fn has_legal_move(position: &Position) -> bool {
    let mut move_generator = MoveGeneratorImpl {
        position: position.clone(),
        move_processor: HasLegalMoveProcessor::new(position.clone()),
        occupied_squares: position.board().bitboard_all_pieces(),
        friendly_squares: position.board().bitboard_by_color(position.side_to_move()),
    };
    move_generator.generate();
    move_generator.move_processor.get_result()
}

fn get_non_sliding_moves_by_piece_type<T, U>(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output=U> + Sized)
) {
    let mut iterator = BitboardIterator::new(square_indexes);
    while let Some(square_index) = iterator.next() {
        let destinations = NON_SLIDING_PIECE_MOVE_TABLE[&piece_type][square_index];
        generate_moves_for_destinations(square_index as usize, destinations, occupied_squares, friendly_squares, move_processor);
    }
}
fn get_sliding_moves_by_piece_type<T>(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    move_processor: &mut (impl MoveProcessor<Output=T> + Sized)
) {
    let mut iterator = BitboardIterator::new(square_indexes);
    while let Some(square_index) = iterator.next() {
        let valid_moves = get_sliding_moves_by_piece_type_and_square_index(&piece_type, square_index, occupied_squares);
        generate_moves_for_destinations(square_index as usize, valid_moves, occupied_squares, friendly_squares, move_processor);
    }
}

fn get_sliding_moves_by_piece_type_and_square_index(piece_type: &PieceType, square_index: usize, occupied_squares: u64) -> u64 {
    let table_entry = &SLIDING_PIECE_MOVE_TABLE[piece_type][square_index as usize];
    let occupied_blocking_squares_bitboard = occupied_squares & table_entry.blocking_squares_bitboard;
    let table_entry_bitboard_index = occupied_blocking_squares_bitboard.pext(table_entry.blocking_squares_bitboard);
    let valid_moves = *table_entry.moves_bitboard.get(table_entry_bitboard_index as usize).unwrap();
    valid_moves
}

fn generate_moves_for_destinations<T>(from: usize, destinations: u64, occupied_squares: u64, friendly_squares: u64, move_processor: &mut (impl MoveProcessor<Output=T> + Sized)) {
    let mut iterator = BitboardIterator::new(destinations);
    while let Some(to) = iterator.next() {
        if friendly_squares & (1 << to) == 0 {
            move_processor.process_move(Basic { base_move: BaseMove::new(from, to, occupied_squares & (1 << to) != 0) });
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
    let bitboards: Vec<_> = increments.into_iter().map(|increment| {
        generate_move_bitboard_for_increment(
            source_square,
            blocking_pieces_bitboard,
            increment,
            generating_blocking_square_mask,
            sliding)
    }).collect();
    return bitboards.iter().fold(0, |acc: u64, bitboard: &u64| acc | bitboard);

    fn generate_move_bitboard_for_increment(
        source_square: isize,
        blocking_pieces_bitboard: u64,
        increment: isize,
        generating_blocking_square_mask: bool,
        sliding: bool,
    ) -> u64 {
        let destination_square: isize = source_square + increment;
        if util::on_board(source_square, destination_square) &&
            (!generating_blocking_square_mask || util::on_board(destination_square, destination_square + increment)) {
            let result = 1 << destination_square;
            if sliding && blocking_pieces_bitboard & (1 << destination_square) == 0 {
                result | generate_move_bitboard_for_increment(
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

fn generate_king_moves<T, U>(position: &Position, square_indexes: u64, occupied_squares: u64, friendly_squares: u64, move_processor: &mut (impl MoveProcessor<Output=U> + Sized)) {
    get_non_sliding_moves_by_piece_type::<T, U>(PieceType::King, 1 << square_indexes.trailing_zeros(), occupied_squares, friendly_squares, move_processor);
    BoardSide::iter()
            .filter(|board_side| position.can_castle(position.side_to_move(), board_side))
            .map(|board_side| { &board::CASTLING_METADATA[position.side_to_move() as usize][board_side as usize] })
            .map(|cmd| Castling { base_move: BaseMove::new(cmd.king_from_square, cmd.king_to_square, false), board_side: cmd.board_side })
            .for_each(|mv| move_processor.process_move(mv));
}

fn generate_pawn_moves<P, U>(position: &Position, square_indexes: u64, occupied_squares: u64, move_processor: &mut P)
where
    P: MoveProcessor<Output=U> + Sized,
{
    let create_moves = |from: usize, to: usize, capture: bool, move_processor: &mut P| {
        if Board::rank(to, position.side_to_move()) != 7 {
            move_processor.process_move(Basic { base_move: BaseMove::new(from, to, capture) });
        } else {
            for piece_type in [PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
                move_processor.process_move(Promotion { base_move: { BaseMove::new(from, to, capture) }, promote_to: piece_type });
            }
        }
    };

    let side_to_move = position.side_to_move();
    let opposing_side = !side_to_move;
    let opposing_side_bitboard = position.board().bitboard_by_color(opposing_side);
    let pawn_increment: isize = if side_to_move == White { 8 } else { -8 };

    let mut iterator = BitboardIterator::new(square_indexes);
    while let Some(square_index) = iterator.next() {

        // generate forward moves
        let one_step_forward: u64 = 1 << ((square_index as isize + pawn_increment) as usize);
        if occupied_squares & one_step_forward == 0 {
            create_moves(square_index, one_step_forward.trailing_zeros() as usize, false, move_processor);
            if Board::rank(square_index, side_to_move) == 1 {
                let two_steps_forward = 1 << ((square_index as isize + pawn_increment * 2) as usize);
                if (occupied_squares & two_steps_forward) == 0 {
                    create_moves(square_index, two_steps_forward.trailing_zeros() as usize, false, move_processor);
                }
            }
        }

        // generate standard captures
        let attacked_squares = PAWN_ATTACKS_TABLE[&side_to_move][square_index];
        let attacked_opposing_piece_squares = attacked_squares & opposing_side_bitboard;
        for attacked_square_index in BitboardIterator::new(attacked_opposing_piece_squares) {
            create_moves(square_index, attacked_square_index, true, move_processor);
        }

        // generate en passant capture
        if let Some(ep_square) = position.en_passant_capture_square() {
            if ((1 << ep_square) & attacked_squares) != 0 {
                let ep_move = EnPassant { base_move: BaseMove::new(square_index as usize, ep_square, true), capture_square: (ep_square as isize - pawn_increment) as usize };
                move_processor.process_move(ep_move);
            }
        }
    }
}

pub fn square_attacks_finder(position: &Position, attacking_color: PieceColor, square_index: usize) -> u64 {
    let occupied_squares = position.board().bitboard_all_pieces();
    let enemy_squares = position.board().bitboard_by_color(attacking_color);
    let enemy_queens = position.board().bitboard_by_color_and_piece_type(attacking_color, PieceType::Queen);
    let mut attacking_squares = 0;
    for piece_type in [PieceType::Bishop, PieceType::Rook] {
        let moves = get_sliding_moves_by_piece_type_and_square_index(&piece_type, square_index.try_into().unwrap(), occupied_squares);
        let possible_attackers = moves & enemy_squares;
        let mut iterator = BitboardIterator::new(possible_attackers);
        while let Some(square_index) = iterator.next() {
            if (enemy_queens | position.board().bitboard_by_color_and_piece_type(attacking_color, piece_type)) & (1 << square_index) != 0 {
                attacking_squares |= 1 << square_index;
            }
        }
    }
    attacking_squares |= non_sliding_piece_attacks(position, PieceType::King, attacking_color, square_index);
    attacking_squares |= non_sliding_piece_attacks(position, PieceType::Knight, attacking_color, square_index);

    let pawn_attack_squares = PAWN_ATTACKS_TABLE[&!attacking_color][square_index as usize];
    let attacking_pawns = position.board().bitboard_by_color_and_piece_type(attacking_color, PieceType::Pawn);
    let attacking_pawn = pawn_attack_squares & attacking_pawns;
    attacking_squares |= attacking_pawn;

    attacking_squares
}

pub fn is_en_passant_capture_possible(position: &Position) -> bool {
    if let Some(en_passant_capture_square) = position.en_passant_capture_square() {
        position.board().bitboard_by_color_and_piece_type(position.side_to_move(), PieceType::Pawn) &
            PAWN_ATTACKS_TABLE[&position.opposing_side()][en_passant_capture_square] != 0
    } else {
        false
    }
}
pub fn non_sliding_piece_attacks_empty_board(piece_type: PieceType, square_index: usize) -> u64 {
    NON_SLIDING_PIECE_MOVE_TABLE[&piece_type][square_index]
}

pub fn non_sliding_piece_attacks(position: &Position, attacking_piece_type: PieceType, attacking_color: PieceColor, square_index: usize) -> u64 {
    let moves = NON_SLIDING_PIECE_MOVE_TABLE[&attacking_piece_type][square_index as usize];
    let enemy_squares = position.board().bitboard_by_color_and_piece_type(attacking_color, attacking_piece_type);
    moves & enemy_squares
}

pub fn king_attacks_finder(position: &Position, king_color: PieceColor) -> u64 {
    square_attacks_finder(
        position, king_color.opposite(), position.board().king_square(king_color))
}

pub fn check_count(position: &Position) -> usize {
    king_attacks_finder(position, position.side_to_move()).count_ones() as usize
}

pub fn is_check(position: &Position) -> bool {
    check_count(position) > 0
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
use super::*;
    use crate::r#move::Move::Castling;
    use crate::move_generator::generate_moves;

    use crate::chessboard::board::BoardSide::{KingSide, QueenSide};
    use crate::move_generator;
    use crate::r#move::BaseMove;

    struct MoveProcessor {
        moves: Vec<Move>,
    }

    // impl move_generator::MoveProcessor for MoveProcessor {
    //     type Output = Move;
    // 
    //     fn process_move(&mut self, m: Move) -> Self::Output {
    //         self.moves.push(m);
    //     }
    // 
    //     fn continue_processing(&mut self) -> bool {
    //         todo!()
    //     }
    // 
    //     fn get_result(&self) -> Self::Output {
    //         todo!()
    //     }
    // }
    // 
    // impl MoveProcessor {
    //     fn get_closure(&mut self) -> Box<dyn FnMut(&Move) + '_> {
    //         let moves = RefCell::new(&mut self.moves);
    // 
    //         Box::new(move |cm: &Move| {
    //             moves.borrow_mut().push(*cm);
    //         })
    //     }
    // 
    //     fn get_moves(&self) -> Vec<Move> {
    //         self.moves.clone()
    //     }
    // }

    /// Verifies that a knight in a corner square can move to the expected squares
    // #[test]
    // fn test_knight_on_corner_square() {
    //     let mut move_processor = MoveProcessor { moves: vec!() };
    //     get_non_sliding_moves_by_piece_type::<Move, Basic>(PieceType::Knight, 1 << 0, 0, 0, &mut move_processor);
    //     let moves = move_processor.moves;
    //     assert_eq!(
    //         moves,
    //         vec!(Basic { base_move: { BaseMove::new(0, 10, false) } },
    //              Basic { base_move: { BaseMove::new(0, 17, false) } })
    //     );
    // }
    // 
    // /// Verifies that a knight cannot capture a friendly piece
    // #[test]
    // fn test_knight_attacking_friendly_piece() {
    //     let mut move_processor = MoveProcessor { moves: vec!() };
    //     get_non_sliding_moves_by_piece_type::<Move, Basic>(PieceType::Knight, 1 << 0, 0, 1 << 10, &mut move_processor);
    //     let moves = move_processor.moves;
    //     assert_eq!(
    //         moves,
    //         vec!(Basic { base_move: { BaseMove::new(0, 17, false) } })
    //     );
    // }
    // 
    // /// Verifies that a knight can capture an enemy piece
    // #[test]
    // fn test_knight_attacking_enemy_piece() {
    //     let mut move_processor = MoveProcessor { moves: vec!() };
    //     get_non_sliding_moves_by_piece_type::<Move, Basic>(PieceType::Knight, 1 << 0, 1 << 10, 0, &mut move_processor);
    //     let moves = move_processor.moves;
    //     assert_eq!(
    //         moves,
    //         vec!(Basic { base_move: { BaseMove::new(0, 10, true) } },
    //              Basic { base_move: { BaseMove::new(0, 17, false) } })
    //     );
    // }
    // 
    // #[test]
    // fn test_king_lookup_table() {
    //     let mut move_processor = MoveProcessor { moves: vec!() };
    //     get_non_sliding_moves_by_piece_type::<Move, Basic>(PieceType::King, 1 << 0, 0, 0, &mut move_processor);
    //     let moves = move_processor.moves;
    //     assert_eq!(
    //         moves,
    //         vec!(Basic { base_move: { BaseMove::new(0, 1, false) } },
    //              Basic { base_move: { BaseMove::new(0, 8, false) } },
    //              Basic { base_move: { BaseMove::new(0, 9, false) } })
    //     );
    // }

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
        assert_eq!(*moves.get(0).unwrap(), Basic { base_move: BaseMove::new(10, 18, false) });
        assert_eq!(*moves.get(1).unwrap(), Basic { base_move: BaseMove::new(10, 26, false) });
    }

    /// Black pawns can make single or double moves from their home squares
    #[test]
    fn test_black_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 53);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), Basic { base_move: BaseMove::new(53, 45, false) });
        assert_eq!(*moves.get(1).unwrap(), Basic { base_move: BaseMove::new(53, 37, false) });
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
        assert_eq!(*moves.get(0).unwrap(), Basic { base_move: BaseMove::new(10, 18, false) });
    }

    /// Black pawns can be blocked from making a double move
    #[test]
    fn test_black_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate_moves(&position), 53);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), Basic { base_move: BaseMove::new(53, 45, false) });
    }

    /// White pawns can capture
    #[test]
    fn test_white_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate_moves(&position);
        assert_eq!(all_moves.len(), 13);
    }
}