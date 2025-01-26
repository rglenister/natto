use once_cell::sync::Lazy;
use regex::Regex;
use itertools::Itertools;
use crate::bit_board::BitBoard;
use crate::{board, util};
use crate::board::{Board, Piece};
use crate::board::PieceColor::White;
use crate::position::Position;
use crate::util::create_color;
use crate::util::parse_square;


static FEN_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(
    r"(?P<board>[pnbrqkPNBRQK12345678/]+) (?P<side_to_move>[wb]) (?P<castling_rights>K?Q?k?q?-?) (?P<en_passant_target_square>[a-h][1-8]|-) (?P<halfmove_clock>\d+) (?P<fullmove_number>\d+)"
).unwrap());


pub fn parse(fen: String) -> Position {
    if let Some(captures) = FEN_REGEX.captures(&fen) {
        let board_str = expand_board(captures.name("board").unwrap().as_str());
        let side_to_move = captures.name("side_to_move").unwrap().as_str();
        let castling_rights = captures.name("castling_rights").unwrap().as_str();
        let en_passant_target_square = captures.name("en_passant_target_square").unwrap().as_str();
        let halfmove_clock: usize = captures.name("halfmove_clock").unwrap().as_str().parse().expect("it matched the regular expression");
        let fullmove_number: usize = captures.name("fullmove_number").unwrap().as_str().parse().expect("it matched the regular expression");

        let mut board: BitBoard = BitBoard::new();
        for i in 0..board::NUMBER_OF_SQUARES {
            let ch = board_str.chars().nth(i).expect("it's ok");
            if !ch.is_whitespace() {
                board.put_piece(i, Piece::from_char(ch).expect("it's ok"));
            }
        }
        Position::new(
            board,
            create_color(side_to_move).expect("it matched the regular expression"),
            castling_rights.to_string(),
            parse_square(en_passant_target_square),
            halfmove_clock,
            fullmove_number
        )
    } else { panic!("{}", format!("Could not parse fen {}", fen)); }
}

pub fn write(position: &Position) -> String {
    return format!("{} {} {} {} {} {}",
                   write_board(position.board()),
                   if position.side_to_move() == White { "w" } else { "b" },
                   get_castling_rights(position),
                   position.en_passant_capture_square().map_or("-".to_string(), |ep_square| util::format_square(ep_square)),
                   position.half_move_clock(),
                   position.full_move_number());

    fn write_board(board: &BitBoard) -> String {
        return (0..64)
            .map(|sq| board.get_piece(sq))
            .map(|p| if p.is_some() { p.unwrap().to_char() } else { ' ' })
            .collect::<Vec<_>>()
            .chunks(8)
            .rev()
            .map(|c| c.iter().collect::<String>())
            .map(|row| encode_row(&row))
            .join("/");

        fn encode_row(row: &str) -> String {
            if !row.is_empty() {
                let remaining = row.trim_start_matches(|ch: char| ch == ' ');
                let run_length = row.len() - remaining.len();
                if run_length > 0 {
                    run_length.to_string() + &encode_row(remaining)
                } else {
                    remaining.chars().nth(0).unwrap().to_string() + &encode_row(&remaining[1..])
                }
            } else {
                "".to_string()
            }
        }
    }

    fn get_castling_rights(position: &Position) -> String {
        let mut output = String::new();
        if position.castling_rights()[0][0] { output.push_str("K"); }
        if position.castling_rights()[0][1] { output.push_str("Q"); }
        if position.castling_rights()[1][0] { output.push_str("k"); }
        if position.castling_rights()[1][1] { output.push_str("q"); }
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
    let rows : Vec<&str> = input.split("/").collect::<Vec<&str>>();
    let rows_reversed: Vec<_> = rows.iter().cloned().rev().collect();
    rows_reversed.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PieceColor::White;
    use crate::position::NEW_GAME_FEN;

    #[test]
    fn test_parse() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position = parse(fen.to_string());

        assert_eq!(position.side_to_move(), White);
        assert_eq!(position.castling_rights(), [[true; 2]; 2]);
        assert_eq!(position.en_passant_capture_square(), None);
        assert_eq!(position.half_move_clock(), 0);
        assert_eq!(position.full_move_number(), 1);
    }

    #[test]
    fn test_write_1() {
        let fen = "r6r/1b2k1bq/8/8/7B/8/8/R3K2R b Kq h3 9 22";
        let position = parse(fen.to_string());
        let result = write(&position);
        assert_eq!(result, fen);
    }

    #[test]
    fn test_write_2() {
        let fen = NEW_GAME_FEN.to_string();
        let position = parse(fen.to_string());
        let result = write(&position);
        assert_eq!(result, fen);
    }

}
