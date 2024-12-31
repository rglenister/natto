use crate::board::PieceType;
use crate::util::on_board;
use crate::util::print_bitboard;
use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use crate::bit_board::BitBoard;
use crate::board::PieceColor::White;
use crate::board::PieceType::{Bishop, King, Knight, Queen, Rook};
use crate::chess_move::ChessMove;
use crate::chess_move::ChessMove::{BasicMove, EnPassantMove};
use crate::position::Position;
use crate::util;

pub fn generate(position: Position) -> Vec<ChessMove> {
    let mut moves: Vec<ChessMove> = vec![];
    let board: &BitBoard = position.board();
    let occupied_squares = board.bitboard_all_pieces();
    let friendly_squares = board.bitboard_by_color(position.side_to_move());
    let bitboards: [u64; 6] = board.bitboards_for_color(position.side_to_move());

    moves.extend(generate_pawn_moves(&position, bitboards[PieceType::Pawn as usize], occupied_squares, friendly_squares));
    get_moves_by_piece_type(Knight, bitboards[Knight as usize].try_into().unwrap(), occupied_squares, friendly_squares);
    get_sliding_moves_by_piece_type(Bishop, bitboards[Bishop as usize], occupied_squares, friendly_squares);
    get_sliding_moves_by_piece_type(Rook, bitboards[Rook as usize], occupied_squares, friendly_squares);
    get_sliding_moves_by_piece_type(Queen, bitboards[Queen as usize], occupied_squares, friendly_squares);
    generate_king_moves(bitboards[King as usize], occupied_squares, friendly_squares);
    moves
}

pub fn get_moves_by_piece_type(
    piece_type: PieceType,
    square_indexes: usize,
    occupied_squares: u64,
    friendly_squares: u64,
) -> Vec<ChessMove> {
    let mut moves = vec!();
    util::process_bits(square_indexes.try_into().unwrap(), |square_index| {
        let destinations = *NON_SLIDING_PIECE_MOVE_TABLE
            .get(&piece_type)
            .unwrap()
            .get(square_index as usize)
            .unwrap();

        moves.extend(generate_moves(piece_type.clone(), square_index as usize, destinations, occupied_squares, friendly_squares));
    });
    moves
}

pub fn get_sliding_moves_by_piece_type(
    piece_type: PieceType,
    square_indexes: u64,
    occupied_squares: u64,
    friendly_squares: u64,
) -> Vec<ChessMove> {
    let mut moves = vec!();
    util::process_bits(square_indexes, |square_index| {
        let table_entry = SLIDING_PIECE_MOVE_TABLE
            .get(&piece_type)
            .unwrap()
            .get(square_index as usize)
            .unwrap();

        let occupied_blocking_squares_bitboard = occupied_squares & table_entry.blocking_squares_bitboard;
        let table_entry_bitboard_index = occupied_blocking_squares_bitboard.pext(table_entry.blocking_squares_bitboard);
        *table_entry.moves_bitboard.get(table_entry_bitboard_index as usize).unwrap();
        moves.extend(generate_moves(piece_type.clone(), square_index as usize, table_entry.moves_bitboard[0], occupied_squares, friendly_squares));
    });
    moves
}

fn generate_moves(piece_type: PieceType, from: usize, destinations: u64, occupied_squares: u64 , friendly_squares: u64) -> Vec<ChessMove> {
    let mut moves: Vec<ChessMove> = vec![];
    util::process_bits(destinations, |to: u64| {
        moves.push(BasicMove { from, to: to as usize, capture: friendly_squares & 1 << to == 0});
    });
    moves
}


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
    let move_table = [PieceType::Bishop, PieceType::Rook, PieceType::Queen]
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

fn generate_king_moves(square_indexes: u64, occupied_squares: u64, friendly_squares: u64) -> u64 {
    get_moves_by_piece_type(King, 1 << square_indexes, occupied_squares, friendly_squares);
    // add castling moves
    0
}

fn generate_pawn_moves(position: &Position, square_indexes: u64, occupied_squares: u64, friendly_squares: u64) -> Vec<ChessMove> {
    let all_pieces_bitboard = position.board().bitboard_all_pieces();
    let opposition_pieces_bitboard = position.board().bitboard_by_color(position.opposing_side());
    let forward_increment: i32 = if position.side_to_move() == White { 8 } else { -8 };
    let capture_increments = [forward_increment + 1, forward_increment - 1];
    let indexes = util::bit_indexes(square_indexes);

    let mut moves: Vec<ChessMove> = vec!();
    for square_index in indexes {
        generate_forward_moves(&position, all_pieces_bitboard, square_index.try_into().unwrap(), forward_increment);
        generate_standard_captures(&moves, &position, square_index, capture_increments, opposition_pieces_bitboard);
        generate_en_passant(square_index, position.en_passant_target(), forward_increment);
        fn create_moves(mut moves: Vec<ChessMove>, position: Position, from: i32, to: i32, capture: bool) -> Vec<ChessMove> {
            if BitBoard::rank(to, position.side_to_move()) != 7 {
                vec!()
            } else {
                vec!()
                // moves
                // [Knight, Bishop, Rook, Queen].iter().map(|piece_type| { PromotionMove { from, to, capture, promote_to: piece_type}}).collect();
            }
        }

        fn generate_forward_moves(position: &Position, all_pieces_bitboard: u64, square_index: i32, forward_increment: i32) {
            let mut moves: Vec<ChessMove> = vec!();
            let one_step_forward = square_index + forward_increment;
            if all_pieces_bitboard & 1 << one_step_forward == 0 {
                moves.push(BasicMove { from: square_index as usize, to: one_step_forward as usize, capture: false });
                if BitBoard::rank(square_index, position.side_to_move()) == 2 {
                    moves.push(BasicMove { from: square_index as usize, to: (one_step_forward + forward_increment) as usize, capture: false });
                }
            }
        }
        fn generate_en_passant(square_index: u64, en_passant_capture_square: Option<usize>, forward_increment: i32) -> Option<ChessMove> {
            en_passant_capture_square
                .filter(|ep_square| BitBoard::is_along_side(square_index.try_into().unwrap(), *ep_square as i32))
                .map(|ep_square| { EnPassantMove { from: square_index as usize, to: (ep_square as i32 + forward_increment) as usize, capture: true, capture_square: ep_square as usize } })
        }

        fn generate_standard_captures(mut moves: &Vec<ChessMove>, position: &Position, square_index: u64, capture_increments: [i32; 2], opposition_pieces_bitboard: u64) {
            // capture_increments.map(|increment| {
            //     (square_index + increment)
            //         .filter(|&to| is_valid_capture(square_index, to, opposition_pieces_bitboard))
            //         .flat_map(|to| create_moves((&moves).to_vec(), &position, square_index, to, true))
            // });
        }

        fn is_valid_capture(from: i32, to: i32, opposition_pieces_bitboard: u64) -> bool {
            util::on_board(from, to) && opposition_pieces_bitboard << to != 0
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_knight_lookup_table() {
        // print_bitboard(get_moves_by_piece_type(PieceType::Knight, 0, 0, 0));
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::Knight, 0, 0, 0),
        //     1 << 10 | 1 << 17
        // );
        // print_bitboard(get_moves_by_piece_type(PieceType::Knight, 36, 0, 0));
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::Knight, 36, 0, 0),
        //     1 << 46 | 1 << 53 | 1 << 51 | 1 << 42 | 1 << 26 | 1 << 19 | 1 << 21 | 1 << 30
        // );
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::Knight, 63, 0, 0),
        //     1 << 53 | 1 << 46
        // );
    }
    #[test]
    fn test_king_lookup_table() {
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::King, 0, 0, 0),
        //     1 << 9 | 1 << 1 | 1 << 8
        // );
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::King, 7, 0, 0),
        //     1 << 14 | 1 << 15 | 1 << 6
        // );
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::King, 1, 0, 0),
        //     1 << 10 | 1 << 8 | 1 << 2 | 1 << 9 | 1 << 0
        // );
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::King, 2, 0, 0),
        //     1 << 11 | 1 << 9 | 1 << 3 | 1 << 10 | 1 << 1
        // );
        // assert_eq!(
        //     get_moves_by_piece_type(PieceType::King, 9, 0, 0),
        //     1 << 18 | 1 << 16 | 1 << 0 | 1 << 2 | 1 << 10 | 1 << 17 | 1 << 8 | 1 << 1
        // );
    }

    #[test]
    fn test_bishop_lookup_table1() {
        // let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0, 0);
        // print_bitboard(a);
        // assert_eq!(
        //     get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0, 0),
        //     1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54 | 1 << 63
        // );
    }
    #[test]
    fn test_bishop_lookup_table2() {
        // let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0, 0);
        // print_bitboard(a);
        // assert_eq!(
        //     get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 0, 0),
        //     1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54 | 1 << 63
        // );
    }
    #[test]
    fn test_bishop_lookup_table3() {
        // let a = get_sliding_moves_by_piece_type(PieceType::Bishop, 0, 1 << 54, 0);
        // print_bitboard(a);
        // assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }

    #[test]
    fn test_queen_lookup_table1() {
        //let a = get_sliding_moves_by_piece_type(PieceType::Queen, 0, 1 << 3 | 1 << 32 | 1 << 18, 0);
        //print_bitboard(a);
        //        assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }

    #[test]
    fn test_queen_lookup_table2() {
        let a = get_sliding_moves_by_piece_type(PieceType::Queen, 0, 0, 0);
        //print_bitboard(a);
        //        assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }

    #[test]
    fn test_queen_lookup_table3() {
        //let a = get_sliding_moves_by_piece_type(PieceType::Queen, 35, 0, 0);
        //print_bitboard(a);
        //        assert_eq!(a, 1 << 9 | 1 << 18 | 1 << 27 | 1 << 36 | 1 << 45 | 1 << 54);
    }
}
