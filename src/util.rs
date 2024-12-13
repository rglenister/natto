use crate::board::PieceColor;
use crate::board::PieceColor::White;
use crate::board::PieceColor::Black;

pub fn create_color(initial: &str) -> Option<PieceColor> {
    if initial == "w" { Some(White) } else if initial == "b" { Some(Black) } else { None }
}

pub fn parse_square(square: &str) -> Option<usize> {
    if square == "-" {
        return None
    } else {
        let row = square.chars().nth(1).expect("Invalid square").to_digit(10).expect("Invalid square");
        let col_char = square.chars().nth(0).expect("Invalid square");
        let col = col_char as u32 - 'a' as u32;
        return Some(((row - 1) * 8 + col).try_into().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parse_square("h7").unwrap(), 55);
        assert_eq!(parse_square("h8").unwrap(), 63);
    }
}
