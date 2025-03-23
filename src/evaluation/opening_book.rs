use std::cell::RefCell;
use std::collections::HashMap;
use crate::{fen, util};
use rand::{rng, Rng};
use reqwest;
use serde::{Deserialize, Serialize};
use log::{debug, error, info};
use thiserror::Error;
use crate::board::{Board, Piece, PieceColor, PieceType};
use crate::board::PieceColor::{Black, White};
use crate::board::PieceType::King;
use crate::chess_move::{ChessMove, RawChessMove};
use crate::move_generator::generate_moves;
use crate::position::Position;
use crate::util::find_generated_move;

include!("../util/generated_macro.rs");

#[derive(Debug, Error)]
#[derive(PartialEq)]
pub enum ErrorKind {
    #[error("No opening moves found")]
    NoOpeningMovesFound,
    #[error("Communications failed: {message}")]
    CommunicationsFailed { message: String },
    #[error("Invalid move string: {move_string}")]
    InvalidMoveString { move_string: String },
    #[error("Illegal move: {raw_chess_move}")]
    IllegalMove { raw_chess_move: RawChessMove },
    #[error("Out of book")]
    OutOfBook,
}

pub trait OpeningBook {
    fn get_opening_move(&self, position: &Position) -> Result<RawChessMove, ErrorKind>;
}

pub struct LiChessOpeningBook {
    out_of_book: RefCell<bool>,
}

impl LiChessOpeningBook {
    pub(crate) fn new() -> LiChessOpeningBook {
        LiChessOpeningBook {
            out_of_book: RefCell::new(false),
        }       
    }
    pub(crate) fn reset(&self) {
        *self.out_of_book.borrow_mut() = false;
    }

}

impl OpeningBook for LiChessOpeningBook {
    fn get_opening_move(&self, position: &Position) -> Result<RawChessMove, ErrorKind> {
        if !*self.out_of_book.borrow() {
            let result = get_opening_move(position);
            match result {
                Ok(book_move) => Ok(book_move),
                Err(e) => {
                    *self.out_of_book.borrow_mut() = true;
                    Err(e)
                },
            }
        } else {
            Err(ErrorKind::OutOfBook)
        }
    }
}
fn get_opening_move(position: &Position) -> Result<RawChessMove, ErrorKind> {
    let fen = fen::write(&position);
    let opening_moves = fetch_opening_moves(&fen)?;
    if opening_moves.len() > 0 {
        let move_string = weighted_random_move(&opening_moves);
        let corrected_move_string= map_castling_move_to_uci_format(&move_string, position);
        let raw_chess_move = parse_move(&corrected_move_string)?;
        validate_move(position, raw_chess_move)?;
        Ok(raw_chess_move)       
    } else {
        Err(ErrorKind::NoOpeningMovesFound {})
    }
}

#[derive(Serialize, Deserialize)]
struct LiChessMoveData {
    uci: String,
    white: isize,
    draws: isize,
    black: isize,
}

#[derive(Deserialize)]
struct LiChessOpeningResponse {
    moves: Vec<LiChessMoveData>,
}

fn fetch_opening_moves(fen: &str) -> Result<Vec<LiChessMoveData>, ErrorKind> {
    let url = format!("https://explorer.lichess.ovh/masters?fen={}", fen);

    let response: LiChessOpeningResponse = reqwest::blocking::get(&url)
        .map_err(|e| ErrorKind::CommunicationsFailed { message: e.to_string() })?
        .json()
        .map_err(|e| ErrorKind::CommunicationsFailed { message: e.to_string() })?;

    Ok(response.moves)
}


fn weighted_random_move(moves: &[LiChessMoveData]) -> String {
    let total_games: u32 = moves.iter().map(|m| (m.white + m.black + m.draws) as u32).sum();

    let mut rng = rng();
    let mut pick = rng.random_range(0..total_games);

    for mv in moves {
        let move_count = mv.white + mv.black + mv.draws;
        if pick < move_count as u32 {
            return mv.uci.clone();
        }
        pick -= move_count as u32;
    }
    moves[0].uci.clone()
}
fn map_castling_move_to_uci_format<'a>(move_string: &'a str, position: &Position) -> &'a str {
    let king_on_home_square = |square_index: usize, piece_color: PieceColor| -> bool {
        position.board().get_piece(square_index) == Some(Piece { piece_color, piece_type: King })
    };
    let white_king_on_home_square = king_on_home_square(sq!("e1"), White);
    let black_king_on_home_square = king_on_home_square(sq!("e8"), Black);
    match move_string {
        "e1h1" if white_king_on_home_square => "e1g1",
        "e1a1" if white_king_on_home_square => "e1c1",
        "e8h8" if black_king_on_home_square => "e8g8",
        "e8a8" if black_king_on_home_square => "e8c8",
        _ => move_string,
    }
}

fn parse_move(move_string: &str) -> Result<RawChessMove, ErrorKind> {
    util::parse_move(move_string.to_string()).ok_or(ErrorKind::InvalidMoveString { move_string: move_string.to_string() })    
}

fn validate_move(position: &Position, raw_chess_move: RawChessMove) -> Result<ChessMove, ErrorKind> {
    find_generated_move(generate_moves(position), &raw_chess_move).ok_or(ErrorKind::IllegalMove { raw_chess_move })
}

#[cfg(test)]
mod tests {
    use crate::evaluation::opening_book::{get_opening_move, map_castling_move_to_uci_format, ErrorKind, LiChessOpeningBook, OpeningBook};
    use crate::position::Position;

    #[test]
    fn test_get_opening_move() {
        let opening_book = LiChessOpeningBook::new();
        let opening_move = opening_book.get_opening_move(&Position::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"));
        let opening_move = opening_move.unwrap();
        assert!(opening_move.promote_to.is_none());
    }

    #[test]
    fn test_opening_book_goes_into_out_of_book_state() {
        let opening_book = LiChessOpeningBook::new();
        let result = opening_book.get_opening_move(&Position::from("k7/8/8/8/8/8/8/K7 w - - 0 1"));
        assert_eq!(result.err().unwrap(), ErrorKind::NoOpeningMovesFound);

        let result = opening_book.get_opening_move(&Position::from("k7/8/8/8/8/8/8/K7 w - - 0 1"));
        assert_eq!(result.err().unwrap(), ErrorKind::OutOfBook);
    }

    #[test]
    fn test_get_opening_move_for_kings_only_position() {
        let opening_book = LiChessOpeningBook::new();
        let result = opening_book.get_opening_move(&Position::from("k7/8/8/8/8/8/8/K7 w - - 0 1"));
        assert_eq!(result.err().unwrap(), ErrorKind::NoOpeningMovesFound);
    }

    #[test]
    fn test_map_castling_move_to_uci_format() {
        let position = Position::new_game();
        assert_eq!(map_castling_move_to_uci_format("a2a3", &position), "a2a3");
        assert_eq!(map_castling_move_to_uci_format("e1g1", &position), "e1g1");
        assert_eq!(map_castling_move_to_uci_format("e1h1", &position), "e1g1");
        assert_eq!(map_castling_move_to_uci_format("e1a1", &position), "e1c1");
        assert_eq!(map_castling_move_to_uci_format("e8h8", &position), "e8g8");
        assert_eq!(map_castling_move_to_uci_format("e8a8", &position), "e8c8");

        let position = Position::from("k7/8/8/8/8/8/8/K7 w - - 0 1");
        assert_eq!(map_castling_move_to_uci_format("a2a3", &position), "a2a3");
        assert_eq!(map_castling_move_to_uci_format("e1g1", &position), "e1g1");
        assert_eq!(map_castling_move_to_uci_format("e1h1", &position), "e1h1");
        assert_eq!(map_castling_move_to_uci_format("e1a1", &position), "e1a1");
        assert_eq!(map_castling_move_to_uci_format("e8h8", &position), "e8h8");
        assert_eq!(map_castling_move_to_uci_format("e8a8", &position), "e8a8");
    }
}
