use crate::core::board;
use crate::core::board::Board;
use crate::core::piece::Piece;
use crate::core::position::Position;
use crate::utils::util;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use thiserror::Error;

static FEN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"^(?<board>((?<RankItem>[pnbrqkPNBRQK1-8]{1,8})/?){8})\s+(?<side_to_move>[bw])\s+(?<castling_rights>-|K?Q?k?q?)\s+(?<en_passant_target_square>-|[a-h][3-6])\s+(?<halfmove_clock>\d+)\s+(?<fullmove_number>\d+)\s*$"
).unwrap()
});

#[derive(Debug, Error, PartialEq)]
pub enum ErrorKind {
    #[error("Failed to parse fen: {0}")]
    InvalidFen(String),
}

pub struct Fen {
    pub fen: String,
}

struct FenParts<'a> {
    board: String,
    side_to_move: &'a str,
    castling_rights: &'a str,
    en_passant_target_square: &'a str,
    halfmove_clock: usize,
    fullmove_number: usize,
}

impl From<&Position> for Fen {
    fn from(position: &Position) -> Self {
        Fen { fen: write(position) }
    }
}

impl<'a> TryFrom<Captures<'a>> for FenParts<'a> {
    type Error = ErrorKind;
    fn try_from(captures: Captures<'a>) -> Result<Self, Self::Error> {
        Ok(FenParts {
            board: expand_board(captures.name("board").unwrap().as_str()),
            side_to_move: captures.name("side_to_move").unwrap().as_str(),
            castling_rights: captures.name("castling_rights").unwrap().as_str(),
            en_passant_target_square: captures.name("en_passant_target_square").unwrap().as_str(),
            halfmove_clock: captures.name("halfmove_clock").unwrap().as_str().parse().unwrap(),
            fullmove_number: captures.name("fullmove_number").unwrap().as_str().parse().unwrap(),
        })
        .map_err(|_: std::num::ParseIntError| {
            ErrorKind::InvalidFen(captures.name("fen").unwrap().as_str().to_string())
        })
    }
}
pub fn parse(fen: String) -> Result<Position, ErrorKind> {
    let fen_parts = FEN_REGEX
        .captures(&fen)
        .and_then(|caps| FenParts::try_from(caps).ok())
        .ok_or_else(|| ErrorKind::InvalidFen(fen.clone()))?;

    if fen_parts.board.chars().count() != board::NUMBER_OF_SQUARES {
        return Err(ErrorKind::InvalidFen(fen.clone()));
    }
    let mut board: Board = Board::new();
    for i in 0..board::NUMBER_OF_SQUARES {
        let ch = fen_parts.board.chars().nth(i).unwrap();
        if !ch.is_whitespace() {
            board.put_piece(i, Piece::from_char(ch).unwrap());
        }
    }

    Ok(Position::new(
        board,
        util::create_color(fen_parts.side_to_move).unwrap(),
        fen_parts.castling_rights.to_string(),
        util::parse_square(fen_parts.en_passant_target_square),
        fen_parts.halfmove_clock,
        fen_parts.fullmove_number,
    ))
}

pub fn write(position: &Position) -> String {
    return format!(
        "{} {} {} {} {} {}",
        write_board(position.board()),
        ['w', 'b'][position.side_to_move() as usize],
        get_castling_rights(position),
        position.en_passant_capture_square().map_or("-".to_string(), util::format_square),
        position.half_move_clock(),
        position.full_move_number()
    );

    fn write_board(board: &Board) -> String {
        return (0..64)
            .map(|sq| board.get_piece(sq))
            .map(|p| p.map_or(' ', |p| p.to_char()))
            .collect::<Vec<_>>()
            .chunks(8)
            .rev()
            .map(|c| c.iter().collect::<String>())
            .map(|row| encode_row(row.as_str()))
            .join("/");

        fn encode_row(row: &str) -> String {
            if !row.is_empty() {
                let remaining = row.trim_start_matches(' ');
                let run_length = row.len() - remaining.len();
                if run_length > 0 {
                    run_length.to_string() + &encode_row(remaining)
                } else {
                    remaining.chars().next().unwrap().to_string() + &encode_row(&remaining[1..])
                }
            } else {
                "".to_string()
            }
        }
    }

    fn get_castling_rights(position: &Position) -> String {
        let mut output = String::new();
        if position.castling_rights()[0][0] {
            output.push('K');
        }
        if position.castling_rights()[0][1] {
            output.push('Q');
        }
        if position.castling_rights()[1][0] {
            output.push('k');
        }
        if position.castling_rights()[1][1] {
            output.push('q');
        }
        if output.is_empty() {
            output.push('-');
        }
        output
    }
}

fn expand_board(fen_board: &str) -> String {
    let expanded = digits_to_spaces(fen_board);
    reverse_rows(&expanded)
}

fn digits_to_spaces(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_digit() {
                " ".repeat(c.to_digit(10).unwrap() as usize)
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn reverse_rows(input: &str) -> String {
    let rows: Vec<&str> = input.split("/").collect::<Vec<&str>>();
    let rows_reversed: Vec<_> = rows.iter().cloned().rev().collect();
    rows_reversed.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::piece::PieceColor::White;
    use crate::core::position::NEW_GAME_FEN;

    #[test]
    fn test_parse() {
        let fen = NEW_GAME_FEN.to_string();
        let position = parse(fen);
        assert!(position.is_ok());
        assert_eq!(position.as_ref().unwrap().side_to_move(), White);
        assert_eq!(position.as_ref().unwrap().castling_rights(), [[true; 2]; 2]);
        assert_eq!(position.as_ref().unwrap().en_passant_capture_square(), None);
        assert_eq!(position.as_ref().unwrap().half_move_clock(), 0);
        assert_eq!(position.as_ref().unwrap().full_move_number(), 1);
    }

    #[test]
    fn test_parse_invalid_fen() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 A";
        let position = parse(fen.to_string());
        assert!(position.is_err());
        let error = position.err().unwrap();
        assert_eq!(
            error.to_string(),
            "Failed to parse fen: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 A"
        );
    }
    #[test]
    fn test_write_1() {
        let fen = "r6r/1b2k1bq/8/8/7B/8/8/R3K2R b Kq h3 9 22";
        let position = parse(fen.to_string());
        let result = write(position.as_ref().expect("valid position"));
        assert_eq!(result, fen);
    }

    #[test]
    fn test_write_2() {
        let fen = NEW_GAME_FEN.to_string();
        let position = parse(fen.to_string());
        let result = write(&position.unwrap());
        assert_eq!(result, fen);
    }
}
