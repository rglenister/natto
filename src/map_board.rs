use crate::board::{Board, Piece};
use std::collections::HashMap;
use std::fmt::Write;

pub struct MapBoard {
    map: HashMap<usize, Piece>
}

impl MapBoard {
    pub(crate) fn print_board(&mut self) -> String {
        let mut s = String::new();
        for row in (0..8).rev() {
            for col in 0..8 {
                let square_index = row * 8 + col;
                let piece = &self.get_piece(square_index);
                match piece {
                    Some(piece) => {
                        write!(s, "{}", format_args!("{}", piece.to_char())).expect("");
                    }
                    None => {}
                }
            }
            s.write_char('\n').unwrap()
        }
        return s;
    }
}

impl Board for MapBoard {

    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn get_piece(&mut self, square_index: usize) -> Option<Piece> {
        self.map.get(&square_index).cloned()
    }

    fn put_piece(&mut self, square_index: usize, piece: Piece) {
        self.map.insert(square_index, piece.clone());
    }

    fn remove_piece(&mut self, square_index: usize) -> Option<Piece> {
        let option = self.map.remove(&square_index);
        return option;
    }

    fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{PieceColor, PieceType};
    use super::*;

    #[test]
    fn test_get_from_empty_square() {
        let mut map_board: MapBoard = MapBoard::new();
        assert!(map_board.get_piece(0).is_none());
    }

    #[test]
    fn test_get() {
        let mut map_board: MapBoard = MapBoard::new();
        let square_index = 1;
        let piece: Piece = Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight};
        map_board.put_piece(square_index, piece);
        assert!(map_board.get_piece(square_index).is_some());
        let retrieved_piece = map_board.get_piece(square_index).expect("whatever");
        assert_eq!(retrieved_piece.piece_color, PieceColor::White);
        assert_eq!(retrieved_piece.piece_type, PieceType::Knight);
    }

    #[test]
    fn test_remove() {
        let mut map_board: MapBoard = MapBoard::new();
        let square_index = 1;
        assert!(map_board.remove_piece(square_index).is_none());
        let piece: Piece = Piece { piece_color: PieceColor::White, piece_type: PieceType::Knight};
        map_board.put_piece(square_index, piece.clone());
        let piece2: &Piece = &map_board.remove_piece(square_index).expect("Whatwver");
        assert_eq!(piece, piece2.clone());
        assert!(map_board.get_piece(0).is_none());
    }


}