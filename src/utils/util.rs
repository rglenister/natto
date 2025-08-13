use crate::core::piece::PieceColor;
use crate::core::piece::PieceColor::{Black, White};
use crate::core::piece::PieceType;
use crate::core::position::Position;
use crate::core::r#move::{Move, RawMove};
use crate::core::{board, piece, r#move};
use crate::search::negamax::RepetitionKey;
use once_cell::sync::Lazy;
use regex::Regex;
use std::ops::Add;

include!("generated_macro.rs");

static RAW_MOVE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(^(?P<from>[a-h][0-8])(?P<to>[a-h][0-8])(?P<promote_to>[nbrq])?$)").unwrap());

pub fn create_color(initial: &str) -> Option<PieceColor> {
    if initial == "w" {
        Some(White)
    } else if initial == "b" {
        Some(Black)
    } else {
        None
    }
}

pub fn parse_square(square: &str) -> Option<usize> {
    if square == "-" {
        None
    } else {
        let row = square.chars().nth(1).expect("Invalid square").to_digit(10).expect("Invalid square");
        let col_char = square.chars().next().expect("Invalid square");
        let col = col_char as u32 - 'a' as u32;
        Some(((row - 1) * 8 + col).try_into().unwrap())
    }
}

pub fn format_square(square_index: usize) -> String {
    if square_index < board::NUMBER_OF_SQUARES {
        ((b'a' + (square_index % 8) as u8) as char).to_string().add(&(square_index / 8 + 1).to_string())
    } else {
        "Invalid square".to_string()
    }
}

pub(crate) fn distance(square_index_1: isize, square_index_2: isize) -> usize {
    let square_1_row = square_index_1 / 8;
    let square_1_col = square_index_1 % 8;

    let square_2_row = square_index_2 / 8;
    let square_2_col = square_index_2 % 8;

    let row_difference = (square_2_row - square_1_row).unsigned_abs();
    let col_difference = (square_2_col - square_1_col).unsigned_abs();

    row_difference.max(col_difference)
}

pub fn on_board(square_from: isize, square_to: isize) -> bool {
    (0..64).contains(&square_to) && (square_to % 8 - square_from % 8).abs() <= 2
}

pub fn print_bitboard(bitboard: u64) {
    for row in (0..8).rev() {
        for col in 0..8 {
            let square_index = row * 8 + col;
            let bit = (bitboard >> square_index) & 1;
            if bit == 1 {
                print!("1 ");
            } else {
                print!("- ");
            }
        }
        println!()
    }
    println!();
    println!("{bitboard:0x}");
    println!("{bitboard:064b}");
    println!();
}

pub fn process_bits<F>(mut bitmap: u64, mut func: F)
where
    F: FnMut(u64),
{
    while bitmap != 0 {
        func(bitmap.trailing_zeros() as u64);
        bitmap &= bitmap - 1;
    }
}

pub fn bit_indexes(bitmap: u64) -> Vec<u64> {
    let mut indexes: Vec<u64> = Vec::new();
    process_bits(bitmap, |index: u64| indexes.push(index));
    indexes
}

pub fn filter_bits<F>(bitmap: u64, filter_fn: F) -> u64
where
    F: Fn(u64) -> bool,
{
    let mut result: u64 = 0;
    process_bits(bitmap, |index: u64| {
        if filter_fn(index) {
            result |= 1 << index;
        }
    });
    result
}

pub const fn column_bitboard(file: usize) -> u64 {
    0x0101010101010101u64 << file
}

pub const fn row_bitboard(row: usize) -> u64 {
    0x00000000000000ffu64 << (row * 8)
}

pub fn filter_moves_by_from_square(moves: Vec<Move>, from_square: usize) -> Vec<Move> {
    moves.into_iter().filter(|mov| mov.get_base_move().from == from_square as u8).collect::<Vec<Move>>()
}

pub fn find_generated_move(moves: Vec<Move>, raw_move: &RawMove) -> Option<Move> {
    moves.into_iter().find(|mov| match mov {
        Move::Basic { base_move } | Move::EnPassant { base_move, .. } | Move::Castling { base_move, .. } => {
            base_move.from == raw_move.from && base_move.to == raw_move.to
        }
        Move::Promotion { base_move, promote_to, .. } => {
            base_move.from == raw_move.from
                && base_move.to == raw_move.to
                && Some(promote_to) == raw_move.promote_to.as_ref()
        }
    })
}

pub fn replay_move_string(position: &Position, raw_moves_string: String) -> Option<Vec<(Position, Move)>> {
    replay_raw_moves(position, &moves_string_to_raw_moves(raw_moves_string)?)
}

pub fn replay_moves(position: &Position, moves: &[Move]) -> Option<Vec<(Position, Move)>> {
    replay_raw_moves(position, &r#move::convert_moves_to_raw(moves))
}

pub fn replay_raw_moves(position: &Position, raw_moves: &[RawMove]) -> Option<Vec<(Position, Move)>> {
    let result: Option<Vec<(Position, Move)>> =
        raw_moves.iter().try_fold(Vec::new(), |mut acc: Vec<(Position, Move)>, rm: &RawMove| {
            let mut current_position = if !acc.is_empty() { acc.last().unwrap().0 } else { *position };
            if let Some(undo_move_info) = current_position.make_raw_move(rm) {
                acc.push((current_position, undo_move_info.mov));
                return Some(acc);
            }
            None
        });
    result
}

pub fn create_move_list(position: &Position, raw_moves_string: String) -> Option<Vec<Move>> {
    Some(replay_move_string(position, raw_moves_string)?.into_iter().map(|(_, mov)| mov).collect::<Vec<Move>>())
}

pub fn create_repetition_keys(position: &Position, raw_moves_string: String) -> Option<Vec<RepetitionKey>> {
    std::iter::once(RepetitionKey::new(position))
        .chain(replay_move_string(position, raw_moves_string)?.into_iter().map(|(pos, _)| RepetitionKey::new(&pos)))
        .collect::<Vec<RepetitionKey>>()
        .into()
}

pub fn parse_initial_moves(raw_move_strings: Vec<String>) -> Option<Vec<RawMove>> {
    let result: Option<Vec<RawMove>> =
        raw_move_strings.iter().try_fold(Vec::new(), |mut acc: Vec<RawMove>, rms: &String| {
            match parse_move(rms.clone()) {
                Some(raw_chess_move) => {
                    acc.push(raw_chess_move);
                    Some(acc)
                }
                None => None,
            }
        });
    result
}

pub fn moves_string_to_raw_moves(moves: String) -> Option<Vec<RawMove>> {
    let moves_vec: Vec<String> = moves.split_whitespace().map(String::from).collect();
    let raw_chess_moves = parse_initial_moves(moves_vec)?;
    Some(raw_chess_moves)
}

pub fn parse_move(raw_move_string: String) -> Option<RawMove> {
    let captures = RAW_MOVE_REGEX.captures(&raw_move_string);
    captures.map(|captures| {
        let promote_to = captures
            .name("promote_to")
            .map(|m| piece::PieceType::from_char(m.as_str().to_string().chars().next().unwrap()));
        RawMove::new(
            parse_square(captures.name("from").unwrap().as_str()).unwrap() as u8,
            parse_square(captures.name("to").unwrap().as_str()).unwrap() as u8,
            promote_to.map(|item| item.unwrap()),
        )
    })
}

pub fn is_piece_pinned(position: &Position, blocking_piece_square: isize, attacking_color: PieceColor) -> bool {
    is_blocking_attack_to_square(
        position,
        position.board().king_square(!attacking_color) as isize,
        blocking_piece_square,
        attacking_color,
    )
}
// pub fn is_blocking_attack_to_square(position: &Position, target_piece_square: isize, blocking_piece_square: isize, attacking_color: PieceColor) -> bool {
//     let board = position.board();
//     let occupied_squares = board.bitboard_all_pieces();
//     if let Some(piece_type) =
//         if blocking_piece_square / 8 == target_piece_square / 8 || blocking_piece_square % 8 == target_piece_square % 8 {
//             Some(Rook)
//         } else if (blocking_piece_square / 8 - target_piece_square / 8).abs() == (blocking_piece_square % 8 - target_piece_square % 8).abs() {
//             Some(Bishop)
//         } else {
//             None
//         } {
//         let attacked_squares = get_sliding_moves_by_piece_type_and_square_index(&piece_type, blocking_piece_square as usize, occupied_squares);
//         if attacked_squares & (1 << target_piece_square) != 0 {
//             let square_increment = (blocking_piece_square - target_piece_square) / distance(target_piece_square, blocking_piece_square) as isize;
//             let mut square_from = blocking_piece_square;
//             let mut square_to = blocking_piece_square + square_increment;
//             while on_board(square_from, square_to) {
//                 if (1 << square_to) & occupied_squares != 0 {
//                     let piece = board.get_piece(square_to as usize).unwrap();
//                     return piece.piece_color == attacking_color && [piece_type, Queen].contains(&piece.piece_type)
//                 }
//                 square_from = square_to;
//                 square_to += square_increment;
//             }
//         }
//     };
//     false
// }

pub fn is_blocking_attack_to_square(
    position: &Position,
    target_piece_square: isize,
    blocking_piece_square: isize,
    attacking_color: PieceColor,
) -> bool {
    let board = position.board();
    let occupied_squares = board.bitboard_all_pieces();
    let dx = target_piece_square % 8 - blocking_piece_square % 8;
    let dy = target_piece_square / 8 - blocking_piece_square / 8;
    if let Some(piece_type) = if dx == 0 || dy == 0 {
        Some(PieceType::Rook)
    } else if dx.abs() == dy.abs() {
        Some(PieceType::Bishop)
    } else {
        None
    } {
        let square_increment = (blocking_piece_square - target_piece_square)
            / distance(target_piece_square, blocking_piece_square) as isize;
        let mut square_from = target_piece_square;
        let mut square_to = target_piece_square + square_increment;
        let mut reached_blocking_square = false;
        while on_board(square_from, square_to) {
            if !reached_blocking_square {
                if square_to == blocking_piece_square {
                    // should check that blocking square is actually occupied?
                    reached_blocking_square = true;
                } else if (1 << square_to) & occupied_squares != 0 {
                    return false;
                }
            } else if (1 << square_to) & occupied_squares != 0 {
                let piece = board.get_piece(square_to as usize).unwrap();
                return piece.piece_color == attacking_color
                    && [piece_type, PieceType::Queen].contains(&piece.piece_type);
            }
            square_from = square_to;
            square_to += square_increment;
        }
    };
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::board::Board;
    use crate::core::piece::{Piece, PieceType};
    use crate::core::r#move::BaseMove;
    use crate::core::r#move::Move;
    use crate::utils::fen;

    #[test]
    fn test_bit_indexes() {
        let result = bit_indexes(1 << 0 | 1 << 1 | 1 << 32 | 1 << 63);
        assert_eq!(result.len(), 4);
        assert_eq!(result, vec![0, 1, 32, 63]);
    }

    #[test]
    fn test_filter_bits() {
        assert_eq!(
            filter_bits(!0, |index| index / 8 == 0),
            1 << 0 | 1 << 1 | 1 << 2 | 1 << 3 | 1 << 4 | 1 << 5 | 1 << 6 | 1 << 7
        );
    }

    #[test]
    fn test_create_color() {
        assert_eq!(None, create_color("a"));
        assert_eq!(Some(Black), create_color("b"));
        assert_eq!(Some(White), create_color("w"));
    }

    #[test]
    fn test_parse_square() {
        assert_eq!(parse_square("a1").unwrap(), 0);
        assert_eq!(parse_square("a2").unwrap(), 8);
        assert_eq!(parse_square("e3").unwrap(), 20);
        assert_eq!(parse_square("h7").unwrap(), 55);
        assert_eq!(parse_square("h8").unwrap(), 63);
    }

    #[test]
    fn test_format_square() {
        assert_eq!(format_square(0), "a1");
        assert_eq!(format_square(8), "a2");
        assert_eq!(format_square(20), "e3");
        assert_eq!(format_square(62), "g8");
        assert_eq!(format_square(63), "h8");
    }

    #[test]
    fn test_distance() {
        assert_eq!(distance(0, 0), 0);
        assert_eq!(distance(0, 1), 1);
        assert_eq!(distance(6, 7), 1);
        assert_eq!(distance(7, 8), 7);
        assert_eq!(distance(60, 68), 1);
    }
    #[test]
    fn test_print_bitboard() {
        let board: u64 = 1 as u64;
        print_bitboard(board);

        let board: u64 = (1 as u64) << 63;
        print_bitboard(board);
    }

    #[test]
    fn test_print_board() {
        let mut board = Board::new();
        board.put_piece(0, Piece { piece_color: White, piece_type: PieceType::Rook });
        board.put_piece(63, Piece { piece_color: Black, piece_type: PieceType::Rook });
    }

    #[test]
    fn test_bit_indices() {
        let selector_mask: u64 = 0b10100101;
        let indices = bit_indexes(selector_mask);
        assert!(indices.len().eq(&4));
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(indices.contains(&5));
        assert!(indices.contains(&7));
    }

    #[test]
    fn test_count_bits() {
        let number: u64 = 0xff00fff;
        let count = number.count_ones();
        assert!(count.eq(&20));
    }

    #[test]
    fn test_find_generated_basic_move() {
        let mut moves: Vec<Move> = vec![];
        moves.push(Move::Basic { base_move: { BaseMove { from: 1, to: 2, capture: false } } });
        moves.push(Move::Basic { base_move: { BaseMove { from: 3, to: 4, capture: false } } });
        let matched_move = find_generated_move(moves, &RawMove::new(1, 2, None));
        assert_eq!(matched_move.unwrap(), Move::Basic { base_move: BaseMove { from: 1, to: 2, capture: false } });
    }

    #[test]
    fn test_find_generated_promotion_move() {
        let mut moves: Vec<Move> = vec![];
        moves.push(Move::Basic { base_move: { BaseMove { from: 1, to: 2, capture: false } } });
        moves.push(Move::Basic { base_move: { BaseMove { from: 3, to: 4, capture: false } } });
        moves.push(Move::Promotion {
            base_move: { BaseMove { from: 3, to: 9, capture: false } },
            promote_to: PieceType::Queen,
        });
        moves.push(Move::Promotion {
            base_move: { BaseMove { from: 3, to: 9, capture: false } },
            promote_to: PieceType::Rook,
        });
        moves.push(Move::Promotion {
            base_move: { BaseMove { from: 3, to: 9, capture: false } },
            promote_to: PieceType::Knight,
        });
        let matched_move = find_generated_move(moves, &RawMove::new(3, 9, Some(PieceType::Rook)));
        assert_eq!(
            matched_move.unwrap(),
            Move::Promotion { base_move: { BaseMove { from: 3, to: 9, capture: false } }, promote_to: PieceType::Rook }
        );
    }

    #[test]
    fn test_parse_initial_moves() {
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string())),
            Some(vec!(RawMove { from: sq!("e2"), to: sq!("e4"), promote_to: None }))
        );
        assert_eq!(
            parse_initial_moves(vec!("e2e4".to_string(), "e7e5".to_string())),
            Some(vec!(
                RawMove { from: sq!("e2"), to: sq!("e4"), promote_to: None },
                RawMove { from: sq!("e7"), to: sq!("e5"), promote_to: None }
            ))
        );

        assert_eq!(parse_initial_moves(vec!("i2e4".to_string(), "e7e5".to_string())), None);
    }

    #[test]
    fn test_parse_move() {
        assert_eq!(
            parse_move("a1b1".to_string()).unwrap(),
            RawMove { from: sq!("a1"), to: sq!("b1"), promote_to: None }
        );
        assert_eq!(
            parse_move("h8a1n".to_string()).unwrap(),
            RawMove { from: sq!("h8"), to: sq!("a1"), promote_to: Some(PieceType::Knight) }
        );
        assert_eq!(
            parse_move("h8a1b".to_string()).unwrap(),
            RawMove { from: sq!("h8"), to: sq!("a1"), promote_to: Some(PieceType::Bishop) }
        );
        assert_eq!(
            parse_move("a1b1r".to_string()).unwrap(),
            RawMove { from: sq!("a1"), to: sq!("b1"), promote_to: Some(PieceType::Rook) }
        );
        assert_eq!(
            parse_move("a1b1q".to_string()).unwrap(),
            RawMove { from: sq!("a1"), to: sq!("b1"), promote_to: Some(PieceType::Queen) }
        );

        assert_eq!(parse_move("a1b1k".to_string()), None);
        assert_eq!(parse_move("".to_string()), None);
        assert_eq!(parse_move("i8h8".to_string()), None);
    }

    #[test]
    fn test_replay_moves() {
        let position = Position::new_game();
        let moves = "e2e4 e7e5".to_string();
        let result = replay_move_string(&position, moves);
        assert!(result.is_some());
        let moves = result.unwrap();
        assert_eq!(moves.len(), 2);
        let last_position = moves.last().unwrap().0;
        let fen = fen::write(&last_position);
        assert_eq!(fen, "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2".to_string());

        let position = Position::new_game();
        let moves = "e2e4 e6e5".to_string();
        let result = replay_move_string(&position, moves);
        assert!(result.is_none());
    }

    #[test]
    fn test_create_repetition_keys() {
        let mut position = Position::new_game();
        let moves = "e2e4 e7e5".to_string();
        let result = create_repetition_keys(&position, moves);
        assert!(result.is_some());
        let repetition_keys = result.unwrap();
        assert_eq!(repetition_keys.len(), 3);

        assert_eq!(repetition_keys[0], RepetitionKey::new(&position));

        position.make_raw_move(&RawMove::new(sq!("e2"), sq!("e4"), None)).unwrap();
        assert_eq!(repetition_keys[1], RepetitionKey::new(&position));

        position.make_raw_move(&RawMove::new(sq!("e7"), sq!("e5"), None)).unwrap();
        assert_eq!(repetition_keys[2], RepetitionKey::new(&position));
    }

    #[test]
    fn test_create_repetition_keys_no_moves() {
        let position = Position::new_game();
        let moves = "".to_string();
        let result = create_repetition_keys(&position, moves);
        assert!(result.is_some());
        let repetition_keys = result.unwrap();
        assert_eq!(repetition_keys.len(), 1);
    }

    #[test]
    fn test_column_bitboard() {
        for column_index in 0..8 {
            let bitboard = column_bitboard(column_index);
            for i in 0..64 {
                assert_eq!(1 << i & bitboard != 0, column_index == i % 8);
            }
        }
    }

    #[test]
    fn test_row_bitboard() {
        for row_index in 0..8 {
            let bitboard = row_bitboard(row_index);
            print_bitboard(bitboard);
            for i in 0..64 {
                assert_eq!(1 << i & bitboard != 0, row_index == i / 8);
            }
            for i in 0..64 {
                assert_eq!(1 << i & bitboard != 0, row_index == i / 8);
            }
        }
    }

    mod pinning {
        use super::*;
        #[test]
        fn test_is_piece_pinned() {
            let fen: &str = "R1n1k3/8/8/8/8/8/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("c8"), PieceColor::White), true);
        }

        #[test]
        fn test_is_piece_pinned2() {
            let fen: &str = "Q1n1k3/8/8/8/8/8/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("c8"), Black), false);
        }

        #[test]
        fn test_is_piece_pinned3() {
            let fen: &str = "B1n1k3/8/8/8/8/8/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("c8"), PieceColor::White), false);
        }

        #[test]
        fn test_is_piece_pinned4() {
            let fen: &str = "RBn1k3/8/8/8/8/8/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("c8"), PieceColor::White), false);
        }

        #[test]
        fn test_is_piece_pinned5() {
            let fen: &str = "Q2nk3/8/8/8/8/8/8/4K3 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("d8"), PieceColor::White), true);
        }

        #[test]
        fn test_is_piece_pinned6() {
            let fen: &str = "3nk3/8/8/1q6/8/3N4/8/5K2 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("d3"), PieceColor::Black), true);
        }

        #[test]
        fn test_is_piece_pinned7() {
            let fen: &str = "3nk3/8/5r2/1b6/8/3N1R2/8/5K2 w - - 0 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("d3"), PieceColor::Black), true);
            assert_eq!(is_piece_pinned(&position, sq!("e3"), PieceColor::Black), false);
            assert_eq!(is_piece_pinned(&position, sq!("f3"), PieceColor::Black), true);
        }

        #[test]
        fn test_is_piece_pinned8() {
            let fen: &str = "4k2K/8/8/8/8/8/1R6/b7 b - - 1 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("b2"), PieceColor::Black), true);
        }

        #[test]
        fn test_is_piece_pinned9() {
            let fen: &str = "4k2K/8/8/8/8/8/1R6/r7 b - - 1 1";
            let position: Position = Position::from(fen);
            assert_eq!(is_piece_pinned(&position, sq!("b2"), PieceColor::Black), false);
        }
    }
}
