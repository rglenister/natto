
use regex::Regex;
use crate::board;
use crate::board::{Board, Piece};
use crate::position::Position;
use crate::util::create_color;
use crate::util::parse_square;


pub fn parse<T: Board>(fen: String) -> Position<T> {
    let re =
        Regex::new(r"(?P<board>[pnbrqkPNBRQK12345678/]+) (?P<side_to_move>[wb]) (?P<castling_rights>K?Q?k?q?-?) (?P<en_passant_target_square>[a-h][1-8]|-) (?P<halfmove_clock>\d+) (?P<fullmove_number>\d+)").unwrap();
    if let Some(captures) = re.captures(&fen) {
        let board_str = expand_board(captures.name("board").unwrap().as_str());
        let side_to_move = captures.name("side_to_move").unwrap().as_str();
        let castling_rights = captures.name("castling_rights").unwrap().as_str();
        let en_passant_target_square = captures.name("en_passant_target_square").unwrap().as_str();
        let halfmove_clock: usize = captures.name("halfmove_clock").unwrap().as_str().parse().expect("it matched the regular expression");
        let fullmove_number: usize = captures.name("fullmove_number").unwrap().as_str().parse().expect("it matched the regular expression");

        let mut board: T = T::new();
        for i in 0..board::NUMBER_OF_SQUARES {
            let ch = board_str.chars().nth(i).expect("it's ok");
            if !ch.is_whitespace() {
                board.put_piece(i, Piece::from_char(ch).expect("it's ok"));
            }
        }
        return Position::new(
            board,
            create_color(side_to_move).expect("it matched the regular expression"),
            castling_rights.to_string(),
            parse_square(en_passant_target_square),
            halfmove_clock,
            fullmove_number
        );
    } else { panic!("{}", format!("Could not parse fen {}", fen)); };
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
    use crate::bit_board::BitBoard;
    use super::*;
    use crate::board::PieceColor::White;
    use crate::position;

    #[test]
    fn test_parse() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let position: position::Position<BitBoard> = parse(fen.to_string());

        assert_eq!(position.side_to_move(), White);
        assert_eq!(position.castling_rights(), "KQkq");
        assert_eq!(position.en_passant_target(), None);
        assert_eq!(position.half_move_clock(), 0);
        assert_eq!(position.full_move_number(), 1);
    }
}
