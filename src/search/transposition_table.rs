use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::engine::config;
use log::info;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoundType {
    #[default]
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct TTEntry {
    pub zobrist: u64,
    pub best_move: Option<Move>,
    pub depth: u8,
    pub score: i32,
    pub bound_type: BoundType,
}

pub struct TranspositionTable {
    table: Box<[TTEntry]>,
}

impl TranspositionTable {
    pub fn new(length: usize) -> TranspositionTable {
        let table = vec![TTEntry::default(); length].into_boxed_slice();
        Self { table }
    }

    pub fn new_using_config() -> Self {
        let requested_size_in_mb = config::get_hash_size();
        let requested_num_entries = requested_size_in_mb * 1024 * 1024 / size_of::<TTEntry>();
        let actual_num_entries = Self::round_up_to_nearest_power_of_two(requested_num_entries);
        let table_size_in_bytes = actual_num_entries * size_of::<TTEntry>();
        println!("Creating transposition table with size {actual_num_entries} ({actual_num_entries:#X})");
        println!(
            "Transposition table created. Total memory used is {} MiB ({:.2} GiB)",
            table_size_in_bytes / (1024 * 1024),
            Self::bytes_to_gib(table_size_in_bytes)
        );
        TranspositionTable::new(actual_num_entries)
    }

    pub fn insert(&mut self, position: &Position, depth: u8, alpha: i32, beta: i32, score: i32, mov: Option<Move>) {
        //        return;
        let bound_type = if score <= alpha {
            BoundType::UpperBound
        } else if score >= beta {
            BoundType::LowerBound
        } else {
            BoundType::Exact
        };
        let do_store = {
            if let Some(current_entry) = self.probe(position.hash_code()) {
                depth > current_entry.depth
                    || (depth == current_entry.depth
                        && ((bound_type == BoundType::Exact && current_entry.bound_type != BoundType::Exact)
                            || (bound_type == BoundType::LowerBound
                                && current_entry.bound_type == BoundType::UpperBound)))
            } else {
                true
            }
        };
        if do_store {
            self.store(position.hash_code(), mov, depth, score, bound_type);
            //#[cfg(debug_assertions)]
            if cfg!(debug_assertions) {
                let entry = self.probe(position.hash_code()).unwrap();
                assert_eq!(entry.zobrist, position.hash_code());
                assert_eq!(entry.best_move, mov);
                assert_eq!(entry.depth, depth);
                assert_eq!(entry.score, score);
                assert_eq!(entry.bound_type, bound_type);
            }
        }
    }

    fn store(&mut self, zobrist: u64, best_move: Option<Move>, depth: u8, score: i32, bound_type: BoundType) {
        let entry = TTEntry { zobrist, best_move, depth, score, bound_type };
        let index = (zobrist as usize) % self.table.len();
        self.table[index] = entry;
    }

    pub fn probe(&self, zobrist: u64) -> Option<TTEntry> {
        let index = (zobrist as usize) % self.table.len();
        let entry = self.table[index];
        if entry.zobrist == zobrist {
            Some(entry)
        } else {
            None
        }
    }

    pub fn _clear(&mut self) {
        for entry in &mut self.table {
            entry.zobrist = 0;
        }
    }

    #[allow(dead_code)]
    pub fn item_count(&self) -> usize {
        let mut count = 0;
        for i in 0..self.table.len() {
            if self.table[i].zobrist != 0 {
                count += 1;
            }
        }
        count
    }

    fn round_up_to_nearest_power_of_two(configured_hash_size: usize) -> usize {
        let rounded_up_hash_size = configured_hash_size.next_power_of_two();
        if rounded_up_hash_size != configured_hash_size {
            info!("Hash size rounded up from {configured_hash_size} {configured_hash_size:#X} to {rounded_up_hash_size} {rounded_up_hash_size:#X}");
        }
        rounded_up_hash_size
    }

    fn bytes_to_gib(bytes: usize) -> f64 {
        bytes as f64 / (1024 * 1024 * 1024) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::transposition_table::BoundType::LowerBound;
    use crate::core::r#move::BaseMove;
    
    #[test]
    fn test_small_table_creation() {
        let table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 2);
    }
    #[test]
    fn test_large_table_creation() {
        assert_eq!(1 << 25, 33_554_432);
        let table = TranspositionTable::new(1 << 25);
        assert_eq!(table.table.len(), 1 << 25);
        assert_eq!(1 << 29, 536_870_912);
    }

    #[test]
    fn test_do_stuff_with_table() {
        let mut table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 2);

        let position = Position::new_game();
        table.store(
            position.hash_code(),
            Option::from(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }),
            8,
            -100,
            LowerBound,
        );
        let entry = table.probe(position.hash_code()).unwrap();
        assert_eq!(entry.zobrist, position.hash_code());
        assert_eq!(entry.best_move, Some(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }));
        assert_eq!(entry.depth, 8);
        assert_eq!(entry.score, -100);
        assert_eq!(entry.bound_type, LowerBound);
    }
    #[test]
    fn test_item_count() {
        let mut table = TranspositionTable::new(1 << 2);
        assert_eq!(table.table.len(), 4);
        assert_eq!(table.item_count(), 0);
        let position = Position::new_game();
        table.store(
            position.hash_code(),
            Option::from(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }),
            8,
            -100,
            LowerBound,
        );
        assert_eq!(table.item_count(), 1);
        table._clear();
        assert_eq!(table.item_count(), 0);
    }

    #[test]
    fn test_new_using_config() {
        let mut ttable = TranspositionTable::new_using_config();
        assert_eq!(config::get_hash_size(), 512);
        assert_eq!(ttable.table.len(), 33554432);
        assert_eq!(size_of::<TTEntry>(), 24);
        assert_eq!(ttable.table.len() * size_of::<TTEntry>(), 33554432 * 24);
        assert_eq!(ttable.item_count(), 0);
        let position = Position::new_game();
        ttable.store(
            position.hash_code(),
            Option::from(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }),
            8,
            -100,
            LowerBound,
        );
        assert_eq!(ttable.item_count(), 1);
        ttable._clear();
        assert_eq!(ttable.item_count(), 0);
    }
}
