
use regex::Regex;

fn parse(fen: String) {
    let re =
        Regex::new(r"(?P<board>[pnbrqkPNBRQK12345678/]+) (?P<side_to_move>[wb]) (?P<castling_rights>K?Q?k?q?-?) (?P<en_passant_target_square>[a-h][1-8]|-) (?P<halfmove_clock>\d+) (?P<fullmove_number>\d+)").unwrap();
    if let Some(captures) = re.captures(&fen) {
        println!("The result is {:?}", captures);
        let board = expand_board(captures.name("board").unwrap().as_str());
        let side_to_move = captures.name("side_to_move").unwrap().as_str();
        let castling_rights = captures.name("castling_rights").unwrap().as_str();
        let en_passant_target_square = captures.name("en_passant_target_square").unwrap().as_str();
        let move_clock = captures.name("halfmove_clock").unwrap().as_str();
        let fullmove_number = captures.name("fullmove_number").unwrap().as_str();

        println!("board = {:?}", board);
        assert_eq!(board, "RNBQKBNRPPPPPPPP                                pppppppprnbqkbnr");
        assert_eq!(board.len(), 64);
    };
}

fn expand_board(fen_board: &str) -> String {
    let expanded = digits_to_spaces(fen_board);
    return reverse_rows(&expanded);
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
    return rows_reversed.join("");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        parse(fen.to_string());
    }
}
