use crate::board::PieceType;
use crate::util::on_board;
use bitintr::{Pdep, Pext};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use crate::bit_board::BitBoard;
use crate::board::PieceColor::White;
use crate::board::PieceType::{Bishop, King, Knight, Queen, Rook};
use crate::chess_move::ChessMove;
use crate::chess_move::ChessMove::{BasicMove, EnPassantMove, PromotionMove};
use crate::position::Position;
use crate::{util};

pub fn generate(position: Position) -> Vec<ChessMove> {
    let mut moves: Vec<ChessMove> = vec![];
    let board: &BitBoard = position.board();
    let occupied_squares = board.bitboard_all_pieces();
    let friendly_squares = board.bitboard_by_color(position.side_to_move());
    let bitboards: [u64; 6] = board.bitboards_for_color(position.side_to_move());

    moves.extend(generate_pawn_moves(&position, bitboards[PieceType::Pawn as usize], occupied_squares, friendly_squares));
    moves.extend(get_moves_by_piece_type(Knight, bitboards[Knight as usize].try_into().unwrap(), occupied_squares, friendly_squares));
//    moves.extend(get_sliding_moves_by_piece_type(Bishop, bitboards[Bishop as usize], occupied_squares, friendly_squares));
//    moves.extend(get_sliding_moves_by_piece_type(Rook, bitboards[Rook as usize], occupied_squares, friendly_squares));
//    moves.extend(get_sliding_moves_by_piece_type(Queen, bitboards[Queen as usize], occupied_squares, friendly_squares));
    moves.extend(generate_king_moves(bitboards[King as usize], occupied_squares, friendly_squares));
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
        let _ = *table_entry.moves_bitboard.get(table_entry_bitboard_index as usize).unwrap();
        moves.extend(generate_moves(piece_type.clone(), square_index as usize, table_entry.moves_bitboard[0], occupied_squares, friendly_squares));
    });
    moves
}

fn generate_moves(piece_type: PieceType, from: usize, destinations: u64, occupied_squares: u64 , friendly_squares: u64) -> Vec<ChessMove> {
    let mut moves: Vec<ChessMove> = vec![];
    util::process_bits(destinations, |to: u64| {
        if friendly_squares & 1 << to == 0 {
            moves.push(BasicMove { from, to: to as usize, capture: occupied_squares & 1 << to != 0});
        }
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

fn generate_king_moves(square_indexes: u64, occupied_squares: u64, friendly_squares: u64) -> Vec<ChessMove> {
    let moves = get_moves_by_piece_type(King, 1 << square_indexes.trailing_zeros(), occupied_squares, friendly_squares);
    // add castling moves
    moves
}

fn generate_pawn_moves(position: &Position, square_indexes: u64, occupied_squares: u64, friendly_squares: u64) -> Vec<ChessMove> {
    let all_pieces_bitboard = position.board().bitboard_all_pieces();
    let opposition_pieces_bitboard = position.board().bitboard_by_color(position.opposing_side());
    let forward_increment: i32 = if position.side_to_move() == White { 8 } else { -8 };
    let capture_increments = [forward_increment + 1, forward_increment - 1];
    let indexes = util::bit_indexes(square_indexes);

    let mut moves: Vec<ChessMove> = vec!();
    for square_index in indexes {
        moves.extend(generate_forward_moves(&position, all_pieces_bitboard, square_index as i32, forward_increment as i32));
        moves.extend(generate_standard_captures(&position, square_index, capture_increments, opposition_pieces_bitboard));
        moves.extend(generate_en_passant(square_index, position.en_passant_capture_square(), forward_increment).into_iter().collect::<Vec<_>>());

        fn create_moves(position: &Position, from: i32, to: i32, capture: bool) -> Vec<ChessMove> {
            if BitBoard::rank(to, position.side_to_move()) != 7 {
                vec!(BasicMove { from: from as usize, to: to as usize, capture})
            } else {
                [Knight, Bishop, Rook, Queen].map(|piece_type| { PromotionMove { from: from as usize, to: to as usize, capture, promote_to: piece_type } }).to_vec()
            }
        }

        fn generate_forward_moves(position: &Position, all_pieces_bitboard: u64, square_index: i32, forward_increment: i32) -> Vec<ChessMove> {
            let mut moves: Vec<ChessMove> = vec!();
            let one_step_forward = square_index + forward_increment;
            if all_pieces_bitboard & 1 << one_step_forward == 0 {
                moves.extend(create_moves(position, (square_index as usize).try_into().unwrap(), one_step_forward, false));
                if BitBoard::rank(square_index, position.side_to_move()) == 1 && all_pieces_bitboard & (1 << one_step_forward + forward_increment) == 0 {
                    moves.extend(create_moves(position, (square_index as usize).try_into().unwrap(), ((one_step_forward + forward_increment) as usize).try_into().unwrap(), false));
                }
            }
            moves
        }
        fn generate_en_passant(square_index: u64, en_passant_capture_square: Option<usize>, forward_increment: i32) -> Option<ChessMove> {
            en_passant_capture_square
                .map(|sq| sq as i32 - forward_increment)
                .filter(|ep_square| BitBoard::is_along_side(square_index.try_into().unwrap(), *ep_square as i32))
                .map(|ep_square| { EnPassantMove { from: square_index as usize, to: (ep_square as i32 + forward_increment) as usize, capture: true, capture_square: ep_square as usize } })
        }

        fn generate_standard_captures(position: &Position, square_index: u64, capture_increments: [i32; 2], opposition_pieces_bitboard: u64) -> Vec<ChessMove> {
           capture_increments.map(|increment| square_index as i32 + increment)
                    .iter().filter(|&to| is_valid_capture(square_index.try_into().unwrap(), *to, opposition_pieces_bitboard))
                    .flat_map(|&to| create_moves(position, (square_index as usize).try_into().unwrap(), to, true)).collect()
        }

        fn is_valid_capture(from: i32, to: i32, opposition_pieces_bitboard: u64) -> bool {
            on_board(from, to) && opposition_pieces_bitboard & 1 << to != 0
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    //    use crate::board::{PieceColor, PieceType};
    use super::*;


    /// Verifies that a knight in a corner square can move to the expected squares
    #[test]
    fn test_knight_on_corner_square() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 1 << 0, 0, 0),
            vec!(BasicMove { from: 0, to: 10, capture: false },
                 BasicMove { from: 0, to: 17, capture: false })
        );
    }

    /// Verifies that a knight cannot capture a friendly piece
    #[test]
    fn test_knight_attacking_friendly_piece() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 1 << 0, 0, 1 << 10),
            vec!(BasicMove { from: 0, to: 17, capture: false })
        );
    }

    /// Verifies that a knight can capture an enemy piece
    #[test]
    fn test_knight_attacking_enemy_piece() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::Knight, 1 << 0, 1 << 10, 0),
            vec!(BasicMove { from: 0, to: 10, capture: true },
                 BasicMove { from: 0, to: 17, capture: false })
        );
    }

    #[test]
    fn test_king_lookup_table() {
        assert_eq!(
            get_moves_by_piece_type(PieceType::King, 1 << 0, 0, 0),
            vec!(BasicMove { from: 0, to: 1, capture: false },
                 BasicMove { from: 0, to: 8, capture: false },
                 BasicMove { from: 0, to: 9, capture: false })
        );
    }

    /// 20 moves are generated from the initial position
    #[test]
    fn test_move_count_from_initial_position() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position = Position::from(fen);
        let moves = generate(position);
        assert_eq!(moves.len(), 20);
    }

    #[test]
    fn test_white_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 10);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 10, to: 18, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 10, to: 26, capture: false });
    }

    /// Black pawns can make single or double moves from their home squares
    #[test]
    fn test_black_pawns_on_home_squares() {
        let fen = "4k3/5p2/8/8/8/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 53);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 53, to: 45, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 53, to: 37, capture: false });
    }

    /// White pawns can be completely blocked
    #[test]
    fn test_white_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// Black pawns can be completely blocked
    #[test]
    fn test_black_pawns_can_be_completely_blocked() {
        let fen = "4k3/5p2/5b2/8/8/2b5/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 50);
        assert_eq!(moves.len(), 0);
    }

    /// White pawns can be blocked from making a double move
    #[test]
    fn test_white_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 10);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 10, to: 18, capture: false });
    }

    /// Black pawns can be blocked from making a double move
    #[test]
    fn test_black_pawns_can_be_blocked_from_making_a_double_move() {
        let fen = "4k3/5p2/8/5b2/2b5/8/2P5/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 53);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 53, to: 45, capture: false });
    }

    /// White pawns can capture
    #[test]
    fn test_white_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 w - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate(position);
        assert_eq!(all_moves.len(), 13);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 19);
        assert_eq!(moves.len(), 3);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 19, to: 27, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 19, to: 28, capture: true });
        assert_eq!(*moves.get(2).unwrap(), BasicMove { from: 19, to: 26, capture: true });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 23);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 23, to: 31, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 23, to: 30, capture: true });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 37);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 37, to: 45, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 37, to: 46, capture: true });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 44);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 44, to: 52, capture: false });
    }

    /// Black pawns can capture
    #[test]
    fn test_black_pawns_can_capture() {
        let fen = "3k4/8/4P1r1/p4P2/2p1n1b1/3P3P/8/4K3 b - - 0 1";
        let position = Position::from(fen);
        let all_moves = generate(position);

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 32);
        assert_eq!(moves.len(), 1);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 32, to: 24, capture: false });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 26);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 26, to: 18, capture: false });
        assert_eq!(*moves.get(1).unwrap(), BasicMove { from: 26, to: 19, capture: true });
    }

    /// White pawns can capture en passant
    #[test]
    fn test_white_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/4PpP1/8/8/8/4K3 w - f6 0 1";
        let position = Position::from(fen);
        let all_moves = generate(position);

        assert_eq!(all_moves.len(), 9);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 36);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 36, to: 44, capture: false });
        assert_eq!(*moves.get(1).unwrap(), EnPassantMove { from: 36, to: 45, capture: true, capture_square: 37 });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 38);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 38, to: 46, capture: false });
        assert_eq!(*moves.get(1).unwrap(), EnPassantMove { from: 38, to: 45, capture: true, capture_square: 37 });
    }

    /// Black pawns can capture en passant
    #[test]
    fn test_black_pawns_can_capture_en_passant() {
        let fen = "4k3/8/8/8/4pPp1/8/8/4K3 b - f3 0 1";
        let position = Position::from(fen);
        let all_moves = generate(position);

        assert_eq!(all_moves.len(), 9);
        let moves = util::filter_moves_by_from_square(all_moves.clone(), 28);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 28, to: 20, capture: false });
        assert_eq!(*moves.get(1).unwrap(), EnPassantMove { from: 28, to: 21, capture: true, capture_square: 29 });

        let moves = util::filter_moves_by_from_square(all_moves.clone(), 30);
        assert_eq!(moves.len(), 2);
        assert_eq!(*moves.get(0).unwrap(), BasicMove { from: 30, to: 22, capture: false });
        assert_eq!(*moves.get(1).unwrap(), EnPassantMove { from: 30, to: 21, capture: true, capture_square: 29 });
    }

    /// White pawns can be promoted
    #[test]
    fn test_white_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 w - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 50);
        assert_eq!(moves.len(), 4);
        assert_eq!(*moves.get(0).unwrap(), PromotionMove { from: 50, to: 58, capture: false, promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { from: 50, to: 58, capture: false, promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { from: 50, to: 58, capture: false, promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { from: 50, to: 58, capture: false, promote_to: Queen });
    }

    /// Black pawns can be promoted
    #[test]
    fn test_black_pawns_can_be_promoted() {
        let fen = "4k3/2P5/8/5b2/2b5/8/6p1/4K3 b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 14);
        assert_eq!(moves.len(), 4);
        assert_eq!(*moves.get(0).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Queen });
    }

    /// Black pawns can be promoted by capturing
    #[test]
    fn test_pawns_can_be_promoted_by_capturing() {
        let fen = "4k3/2P5/8/5b2/8/8/6p1/4KB1N b - - 0 1";
        let position = Position::from(fen);
        let moves = util::filter_moves_by_from_square(generate(position), 14);
        assert_eq!(moves.len(), 12);
        assert_eq!(*moves.get(0).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Knight });
        assert_eq!(*moves.get(1).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Bishop });
        assert_eq!(*moves.get(2).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Rook });
        assert_eq!(*moves.get(3).unwrap(), PromotionMove { from: 14, to: 6, capture: false, promote_to: Queen });
        assert_eq!(*moves.get(4).unwrap(), PromotionMove { from: 14, to: 7, capture: true, promote_to: Knight });
        assert_eq!(*moves.get(5).unwrap(), PromotionMove { from: 14, to: 7, capture: true, promote_to: Bishop });
        assert_eq!(*moves.get(6).unwrap(), PromotionMove { from: 14, to: 7, capture: true, promote_to: Rook });
        assert_eq!(*moves.get(7).unwrap(), PromotionMove { from: 14, to: 7, capture: true, promote_to: Queen });
        assert_eq!(*moves.get(8).unwrap(), PromotionMove { from: 14, to: 5, capture: true, promote_to: Knight });
        assert_eq!(*moves.get(9).unwrap(), PromotionMove { from: 14, to: 5, capture: true, promote_to: Bishop });
        assert_eq!(*moves.get(10).unwrap(), PromotionMove { from: 14, to: 5, capture: true, promote_to: Rook });
        assert_eq!(*moves.get(11).unwrap(), PromotionMove { from: 14, to: 5, capture: true, promote_to: Queen });
    }
}

