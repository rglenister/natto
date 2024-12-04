use crate::board::Board;

struct ArrayBoard {
    squares: [i32; 64]
}

impl Board for ArrayBoard {
    fn get_piece(&self, row: usize, col: usize) -> crate::board::Piece {
        todo!()
    }

    fn put_piece(&self, piece: crate::board::Piece, row: usize, col: usize) {
        todo!()
    }
}