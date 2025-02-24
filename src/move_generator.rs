use crate::position::Position;
use std::cell::RefCell;
use std::option::Option;
use std::collections::HashMap;
use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use strum::IntoEnumIterator;
use crate::bit_board::BitBoard;
use crate::board::{BoardSide, PieceColor, PieceType};
use crate::board::PieceColor::{Black, White};
use crate::board::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::chess_move::{BaseMove, ChessMove};
use crate::chess_move::ChessMove::{BasicMove, CastlingMove, EnPassantMove, PromotionMove};
use crate::{bit_board, util};
use crate::util::on_board;

include!("util/generated_macro.rs");

struct MoveProcessor {
    moves: Vec<ChessMove>,
}

impl MoveProcessor {
    fn get_closure(&mut self) -> Box<dyn FnMut(&ChessMove) -> Option<()> + '_> {
        let moves = RefCell::new(&mut self.moves);

        Box::new(move |cm: &ChessMove| -> Option<()> {
            moves.borrow_mut().push(*cm);
            Some(())
        })
    }

    fn get_moves(&self) -> Vec<ChessMove> {
        self.moves.clone()
    }
}
pub fn generate(position: &Position) -> Vec<ChessMove> {
    let mut moves: Vec<ChessMove> = vec!();
    let mut non_capture_moves: Vec<ChessMove> = vec!();
    let mut process_move = |chess_move: &ChessMove| -> Option<()> {
        if chess_move.get_base_move().capture {
            moves.push(*chess_move);
        } else {
            non_capture_moves.push(*chess_move);
        }
        Some(())
    };
    generate_moves_for_position(&position, &mut process_move);
    moves.extend(non_capture_moves);
    moves
}

pub fn has_legal_move(position: &Position) -> bool {
    let mut found_legal_move: bool = false;
    let mut process_move = |chess_move: &ChessMove| -> Option<()> {
        if !found_legal_move {
            found_legal_move = position.make_move(&chess_move).is_some();
        }
        if found_legal_move { None } else { Some(()) }
    };
    generate_moves_for_position(&position, &mut process_move);
    found_legal_move
}

fn generate_moves_for_position<F>(position: &Position, process_move: &mut F) -> Option<()> where
    F: FnMut(&ChessMove) -> Option<()> {
    let board: &BitBoard = position.board();
    let occupied_squares = board.bitboard_all_pieces();
    let friendly_squares = board.bitboard_by_color(position.side_to_move());
    let bitboards: [u64; 6] = board.bitboards_for_color(position.side_to_move());

    generate_pawn_moves(&position, bitboards[PieceType::Pawn as usize], occupied_squares, RefCell::new(&mut *process_move))?;
    get_non_sliding_moves_by_piece_type(Knight, bitboards[Knight as usize].try_into().unwrap(), occupied_squares, friendly_squares, process_move)?;
    get_sliding_moves_by_piece_type(Bishop, bitboards[Bishop as usize], occupied_squares, friendly_squares, process_move)?;
    get_sliding_moves_by_piece_type(Rook, bitboards[Rook as usize], occupied_squares, friendly_squares, process_move)?;

    // combine bishop and rook to derive the queen moves
    get_sliding_moves_by_piece_type(Bishop, bitboards[Queen as usize], occupied_squares, friendly_squares, process_move)?;
    get_sliding_moves_by_piece_type(Rook, bitboards[Queen as usize], occupied_squares, friendly_squares, process_move)?;

    generate_king_moves(&position, bitboards[King as usize], occupied_squares, friendly_squares, process_move)?;
    Some(())
}

pub fn get_non_sliding_moves_by_piece_type<F>(
    piece_type: PieceType,
    square_indexes: usize,
    occupied_squares: u64,
    friendly_squares: u64,
    process_move: &mut F,
) -> Option<()>
where F: FnMut(&ChessMove) -> Option<()> {
    let mut quit = false;
    util::process_bits(square_indexes.try_into().unwrap(), |square_index| {
        let destinations = NON_SLIDING_PIECE_MOVE_TABLE[&piece_type][square_index as usize];
        if generate_moves_for_destinations(square_index as usize, destinations, occupied_squares, friendly_squares, process_move).is_none() {
            quit = true;
        }
    });
    if quit { None } else { Some(()) }
}

pub fn get_sliding_moves_by_piece_type<F>(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
    process_move: &mut F
) -> Option<()>
where F: FnMut(&ChessMove) -> Option<()> {
    util::process_bits(square_indexes, |square_index| {
        let valid_moves = get_sliding_moves_by_piece_type_and_square_index(&piece_type, square_index, occupied_squares);
        generate_moves_for_destinations(square_index as usize, valid_moves, occupied_squares, friendly_squares, process_move);
    });
    Some(())
}

fn get_sliding_moves_by_piece_type_and_square_index(piece_type: &PieceType, square_index: u64, occupied_squares: u64) -> u64 {
    let table_entry = &SLIDING_PIECE_MOVE_TABLE[piece_type][square_index as usize];
    let occupied_blocking_squares_bitboard = occupied_squares & table_entry.blocking_squares_bitboard;
    let table_entry_bitboard_index = occupied_blocking_squares_bitboard.pext(table_entry.blocking_squares_bitboard);
    let valid_moves = *table_entry.moves_bitboard.get(table_entry_bitboard_index as usize).unwrap();
    valid_moves
}

fn generate_moves_for_destinations<F>(from: usize, destinations: u64, occupied_squares: u64, friendly_squares: u64, process_move: &mut F) -> Option<()>
where F: FnMut(&ChessMove) -> Option<()> {
    let mut quit = false;
    util::process_bits(destinations, |to: u64| {
        if !quit && friendly_squares & 1 << to == 0 {
            if process_move(&BasicMove { base_move: BaseMove::new(from, to as usize,occupied_squares & 1 << to != 0)}).is_none() {
                quit = true;
            }
        }
    });
    if !quit { Some(()) } else { None }
}

static PAWN_ATTACKS_TABLE: Lazy<HashMap<&'static PieceColor, [u64; 64]>> = Lazy::new(|| {
    let mut table = HashMap::new();
    // the white table contains the attacks by white pawns on the black king
    table.insert(&White, generate_move_table([-7, -9]));
    // the black table contains the attacks by black pawns on the white king
    table.insert(&Black, generate_move_table([7, 9]));
    fn generate_move_table(increments: [i32; 2]) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        for square_index in 0..64 {
            let move_squares: u64 = generate_move_bitboard(
                square_index,
                (&increments).to_vec(),
                0,
                false,
                false,
            );
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    return table;
});

static PIECE_INCREMENTS_TABLE: Lazy<HashMap<&'static PieceType, Vec<i32>>> = Lazy::new(|| {
    let mut table = HashMap::new();
    table.insert(&PieceType::Knight, vec![10, 17, 15, 6, -10, -17, -15, -6]);
    table.insert(&PieceType::Bishop, vec![9, 7, -9, -7]);
    table.insert(&PieceType::Rook, vec![1, 8, -1, -8]);
    table.insert(&PieceType::Queen, vec![9, 7, -9, -7, 1, 8, -1, -8]);
    table.insert(&PieceType::King, vec![9, 7, -9, -7, 1, 8, -1, -8]);
    table
});

static NON_SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, [u64; 64]>> = Lazy::new(|| {
    let move_table = [PieceType::Knight, PieceType::King]
        .into_iter().map(|piece_type| (piece_type.clone(), generate_move_table(piece_type))).collect();
    fn generate_move_table(piece_type: PieceType) -> [u64; 64] {
        let mut squares: [u64; 64] = [0; 64];
        let increments = PIECE_INCREMENTS_TABLE.get(&piece_type).unwrap();
        for square_index in 0..64 {
            let move_squares: u64 = generate_move_bitboard(
                square_index,
                (&increments).to_vec(),
                0,
                false,
                false,
            );
            squares[square_index as usize] = move_squares;
        }
        squares
    }
    return move_table;
});

struct TableEntry {
    blocking_squares_bitboard: u64,
    moves_bitboard: Vec<u64>,
}

static SLIDING_PIECE_MOVE_TABLE: Lazy<HashMap<PieceType, Vec<TableEntry>>> = Lazy::new(|| {
    let move_table = [PieceType::Bishop, PieceType::Rook]
        .into_iter().map(|piece_type| (piece_type.clone(), generate_move_table(piece_type))).collect();
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

    return move_table;
});

/// Pre-calculates the bitmaps
fn generate_move_bitboard(
    source_square: i32,
    increments: Vec<i32>,
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
        source_square: i32,
        blocking_pieces_bitboard: u64,
        increment: i32,
        generating_blocking_square_mask: bool,
        sliding: bool,
    ) -> u64 {
        let destination_square: i32 = source_square + increment;
        if on_board(source_square, destination_square) &&
            (!generating_blocking_square_mask || on_board(destination_square, destination_square + increment)) {
            let result = 1 << destination_square;
            if sliding && blocking_pieces_bitboard & 1 << destination_square == 0 {
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

fn generate_king_moves<F>(position: &Position, square_indexes: u64, occupied_squares: u64, friendly_squares: u64, process_move: &mut F) -> Option<()>
where F: FnMut(&ChessMove) -> Option<()> {
    get_non_sliding_moves_by_piece_type(King, 1 << square_indexes.trailing_zeros(), occupied_squares, friendly_squares, process_move)?;
    let moves = BoardSide::iter()
            .filter(|board_side| position.can_castle(position.side_to_move(), board_side))
            .map(|board_side| { &bit_board::CASTLING_METADATA[position.side_to_move() as usize][board_side as usize] })
            .map(|cmd| CastlingMove { base_move: BaseMove::new(cmd.king_from_square, cmd.king_to_square, false), board_side: cmd.board_side })
            .collect::<Vec<_>>();
    for cm in moves {
        if process_move(&cm).is_none() {
            return None;
        }
    }
    Some(())
}

fn generate_pawn_moves<F>(position: &Position, square_indexes: u64, occupied_squares: u64, process_move: RefCell<F>) -> Option<()>
where F: FnMut(&ChessMove) -> Option<()> {
    let opposition_pieces_bitboard = position.board().bitboard_by_color(position.opposing_side());
    let forward_increment: i32 = if position.side_to_move() == White { 8 } else { -8 };
    let mut quit = false;

    util::process_bits(square_indexes, |square_index| {
        let mut create_moves = |from: usize, to: usize, capture: bool| -> Option<()> {
            if BitBoard::rank(to, position.side_to_move()) != 7 {
                if process_move.borrow_mut()(&BasicMove { base_move: BaseMove::new(from, to, capture) }).is_none() {
                    return None;
                }
            } else {
                for piece_type in [Knight, Bishop, Rook, Queen] {
                    if process_move.borrow_mut()(&PromotionMove { base_move: { BaseMove::new(from, to, capture) }, promote_to: piece_type }).is_none() {
                        return None;
                    }
                }
            }
            Some(())
        };

        let mut generate_forward_moves = |square_index: i32| -> Option<()> {
            let one_step_forward = square_index + forward_increment;
            if occupied_squares & 1 << one_step_forward == 0 {
                if create_moves((square_index as usize).try_into().unwrap(), one_step_forward.try_into().unwrap(), false).is_none() {
                    return None;
                }
                if BitBoard::rank(square_index.try_into().unwrap(), position.side_to_move()) == 1 && occupied_squares & (1 << one_step_forward + forward_increment) == 0 {
                    if create_moves((square_index as usize).try_into().unwrap(), ((one_step_forward + forward_increment) as usize).try_into().unwrap(), false).is_none() {
                        return None;
                    }
                }
            }
            Some(())
        };

        let generate_en_passant = |square_index: u64| -> Option<()> {
            let ep_move = position.en_passant_capture_square()
                .map(|sq| sq as i32 - forward_increment)
                .filter(|ep_square| BitBoard::is_along_side(square_index.try_into().unwrap(), (*ep_square as i32).try_into().unwrap()))
                .map(|ep_square| { EnPassantMove { base_move: { BaseMove::new(square_index as usize, (ep_square as i32 + forward_increment) as usize, true) }, capture_square: ep_square as usize } });
            if ep_move.is_some() && process_move.borrow_mut()(&ep_move.unwrap()).is_none() {
                None
            } else {
                Some(())
            }
        };

        let is_valid_capture = |from: i32, to: i32| -> bool {
            on_board(from, to) && opposition_pieces_bitboard & 1 << to != 0
        };

        let mut generate_standard_captures = |square_index: u64| -> Option<()> {
            for increment in [forward_increment + 1, forward_increment - 1] {
                let to = square_index as i32 + increment;
                if is_valid_capture(square_index.try_into().unwrap(), to) {
                    if create_moves((square_index as usize).try_into().unwrap(), to.try_into().unwrap(), true).is_none() {
                        return None;
                    }
                }
            }
            Some(())
        };

        quit = generate_forward_moves(square_index as i32).is_none()
                || generate_standard_captures(square_index).is_none()
                || generate_en_passant(square_index).is_none()
    });
    if quit { None } else { Some(()) }
}

pub fn square_attacks_finder(position: &Position, attacking_color: PieceColor, square_index: i32) -> u64 {
    let occupied_squares = position.board().bitboard_all_pieces();
    let enemy_squares = position.board().bitboard_by_color(attacking_color);
    let enemy_queens = position.board().bitboard_by_color_and_piece_type(attacking_color, Queen);
    let mut attacking_squares = 0;
    for piece_type in [Bishop, Rook] {
        let moves = get_sliding_moves_by_piece_type_and_square_index(&piece_type, square_index.try_into().unwrap(), occupied_squares);
        let possible_attackers = moves & enemy_squares;
        util::process_bits(possible_attackers, |square_index| {
            if (enemy_queens | position.board().bitboard_by_color_and_piece_type(attacking_color, piece_type)) & 1 << square_index != 0 {
                attacking_squares |= 1 << square_index;
            }
        })
    }
    attacking_squares |= non_sliding_piece_attacks(position, King, attacking_color, square_index);
    attacking_squares |= non_sliding_piece_attacks(position, Knight, attacking_color, square_index);

    let pawn_attack_squares = PAWN_ATTACKS_TABLE[&attacking_color][square_index as usize];
    let attacking_pawns = position.board().bitboard_by_color_and_piece_type(attacking_color, Pawn);
    let attacking_pawn = pawn_attack_squares & attacking_pawns;
    attacking_squares |= attacking_pawn;

    attacking_squares
}

fn non_sliding_piece_attacks(position: &Position, attacking_piece_type: PieceType, attacking_color: PieceColor, square_index: i32) -> u64 {
    let moves = NON_SLIDING_PIECE_MOVE_TABLE[&attacking_piece_type][square_index as usize];
    let enemy_squares = position.board().bitboard_by_color_and_piece_type(attacking_color, attacking_piece_type);
    moves & enemy_squares
}

pub fn king_attacks_finder(position: &Position, king_color: PieceColor) -> u64 {
    square_attacks_finder(
        position, if king_color == White {Black} else {White}, position.board().king_square(king_color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::ChessMove::CastlingMove;
    use crate::move_generator::generate;

    use super::*;

    use crate::board::BoardSide::{KingSide, QueenSide};
    use crate::chess_move::BaseMove;
    use crate::fen;
    use super::*;

    /// Verifies that a knight in a corner square can move to the expected squares
    #[test]
    fn test_knight_on_corner_square() {
        let mut move_processor = MoveProcessor { moves: vec!() };
        get_non_sliding_moves_by_piece_type(PieceType::Knight, 1 << 0, 0, 0, &mut move_processor.get_closure());
        let moves = move_processor.moves;
        assert_eq!(
            moves,
            vec!(BasicMove { base_move: { BaseMove::new(0, 10,  false) }},
                 BasicMove {  base_move: { BaseMove::new(0, 17, false)}})
        );
    }

    /// Verifies that a knight cannot capture a friendly piece
    #[test]
    fn test_knight_attacking_friendly_piece() {
        let mut move_processor = MoveProcessor { moves: vec!() };
        get_non_sliding_moves_by_piece_type(PieceType::Knight, 1 << 0, 0, 1 << 10, &mut move_processor.get_closure());
        let moves = move_processor.moves;
        assert_eq!(
            moves,
            vec!(BasicMove { base_move: { BaseMove::new(0, 17, false)}})
        );
    }

    /// Verifies that a knight can capture an enemy piece
    #[test]
    fn test_knight_attacking_enemy_piece() {
        let mut move_processor = MoveProcessor { moves: vec!() };
        get_non_sliding_moves_by_piece_type(PieceType::Knight, 1 << 0, 1 << 10, 0, &mut move_processor.get_closure());
        let moves = move_processor.moves;
        assert_eq!(
            moves,
            vec!(BasicMove { base_move: { BaseMove::new(0, 10, true )}},
                 BasicMove { base_move: { BaseMove::new(0, 17, false )}})
        );
    }

    #[test]
    fn test_king_lookup_table() {
        let mut move_processor = MoveProcessor { moves: vec!() };
        get_non_sliding_moves_by_piece_type(PieceType::King, 1 << 0, 0, 0, &mut move_processor.get_closure());
        let moves = move_processor.moves;
        assert_eq!(
            moves,
            vec!(BasicMove { base_move: { BaseMove::new(0, 1, false)}},
                 BasicMove { base_move: { BaseMove::new(0, 8, false)}},
                 BasicMove { base_move: { BaseMove::new(0, 9, false)}})
        );
    }

    /// 20 moves are generated from the initial position
    #[test]
    fn test_move_count_from_initial_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position = Position::from(fen);
        let moves = generate(&position);
        assert_eq!(moves.len(), 20);
    }

    #[test]
    fn test_white_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 10);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(10, 18, false)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new( 10, 26, false)});
    }

    /// Black pawns can make single or double moves from their home squares
    #[test]
    fn test_black_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 53);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(53, 45, false)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(53, 37, false)});
    }

    /// White pawns can be completely blocked
    #[test]
    fn test_white_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// Black pawns can be completely blocked
    #[test]
    fn test_black_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// White pawns can be blocked from making a double move
    #[test]
    fn test_white_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 10);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(10, 18, false)});
    }

    /// Black pawns can be blocked from making a double move
    #[test]
    fn test_black_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 53);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(),BasicMove { base_move: BaseMove::new(53, 45, false)});
    }

    /// White pawns can capture
    #[test]
    fn test_white_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate(&position);
        assert_eq!(all_moves.len(), 13);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 19);
        assert_eq!(moves.len(), 3);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(19, 28, true)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(19, 26, true)});
        assert_eq!(*moves.get(2).unwrap(), BasicMove { base_move: BaseMove::new(19, 27, false)});

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 23);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(23, 30, true)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(23, 31, false)});

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 37);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(37, 46, true)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(37, 45, false)});

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 44);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(44, 52, false)});
    }

    /// Black pawns can capture
    #[test]
    fn test_black_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 b - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate(&position);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 32);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(32, 24, false)});
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 26);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { base_move: BaseMove::new(26, 19, true)});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(26, 18, false)});
    }

    /// White pawns can capture en passant
    #[test]
    fn test_white_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/4PpP1/8/8/8/4K3 w - f6 0 1";
        let position = Position::from(fen);
        let all_moves = generate(&position);

        assert_eq!(all_moves.len(), 9);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 36);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), EnPassantMove { base_move: BaseMove::new(36, 45, true), capture_square: 37});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(36, 44, false)});

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 38);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), EnPassantMove { base_move: BaseMove::new(38, 45, true), capture_square: 37});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(38, 46, false)});
    }

    /// Black pawns can capture en passant
    #[test]
    fn test_black_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/8/4pPp1/8/8/4K3 b - f3 0 1";
        let position = Position::from(fen);
        let all_moves = generate(&position);

        assert_eq!(all_moves.len(), 9);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 28);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), EnPassantMove { base_move: BaseMove::new(28, 21, true), capture_square: 29});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(28, 20, false)});

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 30);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), EnPassantMove { base_move: BaseMove::new(30, 21, true), capture_square: 29});
        assert_eq!(*moves.get(1).unwrap(), BasicMove { base_move: BaseMove::new(30, 22, false)});
    }

    /// White pawns can be promoted
    #[test]
    fn test_white_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 50);
        assert_eq!(moves.len(), 4);
        assert_eq!(*moves.get(0).unwrap(), PromotionMove { base_move: BaseMove::new(50, 58, false), promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { base_move: BaseMove::new(50, 58, false), promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { base_move: BaseMove::new(50, 58, false), promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { base_move: BaseMove::new(50, 58, false), promote_to: Queen });
    }

    /// Black pawns can be promoted
    #[test]
    fn test_black_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 14);
        assert_eq!(moves.len(), 4);

        assert_eq!(*moves.get(0).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Queen });
    }

    /// Black pawns can be promoted by capturing
    #[test]
    fn test_pawns_can_be_promoted_by_capturing() {
        let fen = "4k3/2P5/8/5b2/8/8/6p1/4KB1N b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), 14);
        assert_eq!(moves.len(), 12);

        assert_eq!(*moves.get(0).unwrap(), PromotionMove { base_move: BaseMove::new(14, 7, true), promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { base_move: BaseMove::new(14, 7, true), promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { base_move: BaseMove::new(14, 7, true), promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { base_move: BaseMove::new(14, 7, true), promote_to: Queen });

        assert_eq!(*moves.get(4).unwrap(), PromotionMove { base_move: BaseMove::new(14, 5, true), promote_to: Knight });
        assert_eq!(*moves.get(5).unwrap(), PromotionMove { base_move: BaseMove::new(14, 5, true), promote_to: Bishop });
        assert_eq!(*moves.get(6).unwrap(), PromotionMove { base_move: BaseMove::new(14, 5, true), promote_to: Rook });
        assert_eq!(*moves.get(7).unwrap(), PromotionMove { base_move: BaseMove::new(14, 5, true), promote_to: Queen });

        assert_eq!(*moves.get(8).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Knight });
        assert_eq!(*moves.get(9).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Bishop });
        assert_eq!(*moves.get(10).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Rook });
        assert_eq!(*moves.get(11).unwrap(), PromotionMove { base_move: BaseMove::new(14, 6, false), promote_to: Queen });

    }

    /// Test white king moves
    #[test]
    fn test_white_king_moves() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), sq!("e1"));
        assert_eq!(moves.len(), 7);
        let castling_moves: Vec<&ChessMove> =
            moves.iter().filter(|chess_move| matches!(chess_move, CastlingMove { .. }))
                .collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(**castling_moves.get(0).unwrap(), CastlingMove { base_move: BaseMove::new(sq!("e1"), sq!("g1"), false), board_side: KingSide });
        assert_eq!(**castling_moves.get(1).unwrap(), CastlingMove { base_move: BaseMove::new(sq!("e1"), sq!("c1"), false), board_side: QueenSide });
    }

    /// Test black king moves
    #[test]
    fn test_black_king_moves() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(&position), sq!("e8"));
        assert_eq!(moves.len(), 7);
        let castling_moves: Vec<&ChessMove> =
            moves.iter().filter(|chess_move| matches!(chess_move, CastlingMove { .. }))
                .collect();
        assert_eq!(castling_moves.len(), 2);
        assert_eq!(**castling_moves.get(0).unwrap(), CastlingMove { base_move: BaseMove::new(sq!("e8"), sq!("g8"), false), board_side: KingSide });
        assert_eq!(**castling_moves.get(1).unwrap(), CastlingMove { base_move: BaseMove::new(sq!("e8"), sq!("c8"), false), board_side: QueenSide });
    }

    /// Test square attacks finder
    #[test]
    fn test_king_attacks_finder_using_white_rook_and_bishop() {
        let fen = "4k2R/8/8/8/B7/8/8/4K3 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 2);
        assert_eq!(attacking_squares[0], 24);
        assert_eq!(attacking_squares[1], 63);
    }

    #[test]
    fn test_king_attacks_finder_using_black_rook_and_bishop() {
        let fen = "4k3/8/8/b7/8/8/8/1r2K3 w - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, White);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 2);
        assert_eq!(attacking_squares[0], 1);
        assert_eq!(attacking_squares[1], 32);
    }

    #[test]
    fn test_king_attacks_finder_using_white_queen() {
        let fen = "4k3/8/2Q5/8/8/8/8/4K3 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 42);
    }

    #[test]
    fn test_king_attacks_finder_using_black_knight() {
        let fen = "4k3/8/8/8/8/3n4/2N5/4K3 w - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, White);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 19);
    }

    #[test]
    fn test_king_attacks_finder_using_first_json_example() {
        let fen = "r6r/1b2k1bq/8/8/7B/8/8/R3K2R b KQ - 3 2";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 31);
    }

    #[test]
    fn test_king_attacks_finder_using_white_pawn() {
        let fen = "8/8/8/1K3k2/4P3/8/8/8 b - - 0 1";
        let mut position = Position::from(fen);
        let attacking_square_indexes = king_attacks_finder(&mut position, Black);
        let attacking_squares = util::bit_indexes(attacking_square_indexes);
        assert_eq!(attacking_squares.len(), 1);
        assert_eq!(attacking_squares[0], 28);
    }

    #[test]
    fn test_pawn_attacks_table() {
        assert_eq!(PAWN_ATTACKS_TABLE[&Black][0], 1 << 9);
        assert_eq!(PAWN_ATTACKS_TABLE[&Black][1], 1 << 8 | 1 << 10);

        assert_eq!(PAWN_ATTACKS_TABLE[&White][63], 1 << 54);
        assert_eq!(PAWN_ATTACKS_TABLE[&White][62], 1 << 53 | 1 << 55);

        assert_eq!(PAWN_ATTACKS_TABLE[&White][31], 1 << 22);
        assert_eq!(PAWN_ATTACKS_TABLE[&Black][31], 1 << 38);
    }

}

