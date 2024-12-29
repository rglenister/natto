use crate::chess_move::ChessMove;
use crate::position::Position;
use crate::board::{PieceColor, PieceType};
use crate::chess_move::ChessMove::BasicMove;

fn generate_move_list(position: Position) -> Vec<ChessMove> {
    println!("Generating move list for position:");
    println!("{}", position.to_string());
    println!();

    let mut move_list: Vec<ChessMove> = vec!();
    move_list.extend(generate_pawn_moves(position));
    move_list
}

fn generate_pawn_moves(position: Position) -> Vec<ChessMove> {
    let side_to_move: PieceColor = position.side_to_move();
    let board = position.board();
    let pawn_bitboard : u64 = board.bitboard_by_color_and_piece_type(side_to_move, PieceType::Pawn);
    let white_pieces_bitboard = board.bitboard_by_color(PieceColor::White);
    let black_pieces_bitboard = board.bitboard_by_color(PieceColor::White);
    let all_pieces_bitboard = white_pieces_bitboard | black_pieces_bitboard;
    let mut move_list: Vec<ChessMove> = Vec::new();
    let pawn_step: i32 = if side_to_move == PieceColor::White {8} else {-8};
    bit_positions(pawn_bitboard).iter().for_each(|pawn_index| {
        let destination_square = pawn_index + pawn_step;
        if side_to_move == PieceColor::White && all_pieces_bitboard & 1 << destination_square == 0
                || side_to_move == PieceColor::Black && all_pieces_bitboard & 1 >> destination_square == 0 {
            move_list.push(BasicMove {from: *pawn_index as usize, to: destination_square as usize, capture: false});
        }
    });
    move_list
}

fn bit_positions(mut bits: u64) -> Vec<i32> {
    let mut bit_positions = Vec::new();

    while bits != 0 {
        let pos = bits.trailing_zeros(); // Find position of least significant set bit
        bit_positions.push(pos as i32);
        bits &= bits - 1; // Clear the least significant set bit
    }
    bit_positions
}
fn search(position: Position, depth: u32) {
    let mut move_list: Vec<ChessMove> = vec!();
    move_list.extend(generate_move_list(position));
}

#[cfg(test)]
mod tests {
    use serde_derive::Deserialize;

    use std::error::Error;
    use crate::fen;
    use crate::search::generate_move_list;

    use std::fs;

    #[derive(Deserialize, Debug)]

    struct FenTestCase {
        depth: usize,
        nodes: usize,
        fen: String,
    }

    #[test]
    fn test_fens() {
        let test_cases = load_fens().unwrap();
        assert_eq!(test_cases.len(), 7);
        for test in test_cases {
            let position = fen::parse(test.fen);
            generate_move_list(position);
        }
    }

    fn load_fens() -> Result<Vec<FenTestCase>, Box<dyn Error>> {
        let file = fs::read_to_string("src/test_data/fen_test_data.json")?;
        let test_cases = json5::from_str(file.as_str())?;
        Ok(test_cases)
    }

    #[test]
    fn test_pawn_moves() {

    }
}
