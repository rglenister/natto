use crate::chess_move::ChessMove;

use std::collections::BTreeMap;

pub struct SortedMoveList {
    moves_to_score_map: BTreeMap<ChessMove, isize>,
}

impl SortedMoveList {
    pub fn new(moves: &[ChessMove]) -> SortedMoveList {
        let mut sorted_move_list = SortedMoveList { 
            moves_to_score_map: BTreeMap::new()
        };
        sorted_move_list.add_moves(moves);
        sorted_move_list
    }
    
    pub fn add_moves(&mut self, moves: &[ChessMove]) {
        moves.iter().for_each(|move_to_add| {
            self.moves_to_score_map.insert(*move_to_add, 0);
        })
    }
    
    pub fn update_score(&mut self, chess_move: &ChessMove, score: isize) {
        self.moves_to_score_map.insert(*chess_move, score);
    }
    
    pub fn entries(&self) -> Vec<(&ChessMove, isize)> {
        let mut entries: Vec<(&ChessMove, isize)> = self.moves_to_score_map
            .iter()
            .map(|(chess_move, &score)| (chess_move, score))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries
    }
    pub fn get_all_moves(&self) -> Vec<ChessMove> {
        self.entries().into_iter().map(|entry| *entry.0).collect()
    }
}



#[cfg(test)]
mod tests {
    use crate::chess_move::{BaseMove, ChessMove};
    use crate::sorted_move_list::SortedMoveList;

    #[test]
    fn test_add_zero_moves() {
        let mut sorted_move_list = SortedMoveList::new(&vec!());
        assert_eq!(sorted_move_list.get_all_moves().len(), 0);
    }
    #[test]
    fn test_add_one_move() {
        let chess_move = ChessMove::Basic { base_move: { BaseMove::new(1, 2, false)} };
        let mut sorted_move_list = SortedMoveList::new(&vec!(chess_move.clone() ));
        sorted_move_list.add_moves(&vec!(chess_move));
        assert_eq!(sorted_move_list.get_all_moves().len(), 1);
        assert_eq!(sorted_move_list.get_all_moves().first().unwrap(), &chess_move);
    }

    #[test]
    fn test_add_two_moves() {
        let chess_move_1 = ChessMove::Basic { base_move: { BaseMove::new(1, 2, false)} };
        let chess_move_2 = ChessMove::Basic { base_move: { BaseMove::new(1, 3, false)} };
        let mut sorted_move_list = SortedMoveList::new(&vec!(chess_move_1.clone(), chess_move_2.clone() ));
        sorted_move_list.update_score(&chess_move_1, 1);
        sorted_move_list.update_score(&chess_move_2, 2);
        assert_eq!(sorted_move_list.get_all_moves().len(), 2);
        assert_eq!(sorted_move_list.get_all_moves().get(0).unwrap(), &chess_move_2);
        assert_eq!(sorted_move_list.get_all_moves().get(1).unwrap(), &chess_move_1);

        assert_eq!(sorted_move_list.get_all_moves().len(), 2);
        sorted_move_list.update_score(&chess_move_2, 0);
        assert_eq!(sorted_move_list.get_all_moves().len(), 2);
        assert_eq!(sorted_move_list.get_all_moves().get(0).unwrap(), &chess_move_1);
        assert_eq!(sorted_move_list.get_all_moves().get(1).unwrap(), &chess_move_2);
    }

    #[test]
    fn test_equal_moves() {
        let chess_move_1 = ChessMove::Basic { base_move: { BaseMove::new(1, 2, false)} };
        let chess_move_2 = ChessMove::Basic { base_move: { BaseMove::new(1, 3, false)} };
        let chess_move_3 = ChessMove::Basic { base_move: { BaseMove::new(1, 2, false)} };
        let chess_move_4 = ChessMove::Basic { base_move: { BaseMove::new(1, 2, true)} };
        assert_eq!(chess_move_1, chess_move_3);
        assert_ne!(chess_move_1, chess_move_4);
        assert_ne!(chess_move_4, chess_move_1);
        assert_ne!(chess_move_1, chess_move_2);
        assert_ne!(chess_move_2, chess_move_1);
    }
}

