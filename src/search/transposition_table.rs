use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::engine::config;

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
    pub fn new(size_in_mb: usize) -> Self {
        let requested_num_entries = size_in_mb * 1024 * 1024 / size_of::<TTEntry>();
        let rounded_num_entries = Self::prev_power_of_two(requested_num_entries);
        let table_size_in_bytes = rounded_num_entries * size_of::<TTEntry>();
        log::info!("Creating transposition table with size {rounded_num_entries} ({rounded_num_entries:#X}) from a requested maximum size of {} Mib", size_in_mb);
        log::info!(
            "Transposition table created. Total memory used is {} MiB ({:.2} GiB)",
            table_size_in_bytes / (1024 * 1024),
            Self::bytes_to_gib(table_size_in_bytes)
        );
        let table = vec![TTEntry::default(); rounded_num_entries].into_boxed_slice();
        Self { table }
    }

    pub fn new_using_config() -> Self {
        Self::new(config::get_hash_size())
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

    pub fn prev_power_of_two(configured_hash_size: usize) -> usize {
        if configured_hash_size == 0 {
            return 0;
        }
        let msb_pos = usize::BITS - 1 - configured_hash_size.leading_zeros();
        1usize << msb_pos
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
        assert_eq!(table.table.len(), 65536);
    }
    #[test]
    fn test_large_table_creation() {
        assert_eq!(1 << 25, 33_554_432);
        let table = TranspositionTable::new(2048);
        assert_eq!(table.table.len(), 67108864);
        assert_eq!(table.table.len(), 0x4000000);
        assert!(table.table.len().is_power_of_two());

        let memory_used = table.table.len() * size_of::<TTEntry>();
        assert_eq!(memory_used, 1610612736);
        let memory_used_mb = memory_used / 1024 / 1024;
        assert_eq!(memory_used_mb, 1536);
        assert!(memory_used / 1024 / 1024 < 2048);
    }

    #[test]
    fn test_do_stuff_with_table() {
        let mut table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 65536);

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
        assert_eq!(table.table.len(), 131072);
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
        assert_eq!(config::get_hash_size(), 1);
        assert_eq!(ttable.table.len(), 32768);
        assert_eq!(size_of::<TTEntry>(), 24);
        assert_eq!(ttable.table.len() * size_of::<TTEntry>(), 786432);
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

    #[test]
    fn test_prev_power_of_two() {
        assert_eq!(TranspositionTable::prev_power_of_two(0), 0);
        assert_eq!(TranspositionTable::prev_power_of_two(1), 1);
        assert_eq!(TranspositionTable::prev_power_of_two(2), 2);
        assert_eq!(TranspositionTable::prev_power_of_two(3), 2);
        assert_eq!(TranspositionTable::prev_power_of_two(4), 4);
        assert_eq!(TranspositionTable::prev_power_of_two(500), 256);
        assert_eq!(TranspositionTable::prev_power_of_two(2048), 2048);
        assert_eq!(TranspositionTable::prev_power_of_two(2050), 2048);
    }

    #[test]
    fn test_bytes_to_gib() {
        assert_eq!(format!("{:.2}", TranspositionTable::bytes_to_gib(1_000_000_000)), "0.93");
        assert_eq!(format!("{:.2}", TranspositionTable::bytes_to_gib(100_000_000)), "0.09");
        assert_eq!(format!("{:.2}", TranspositionTable::bytes_to_gib(10_000_000_000)), "9.31");
    }
}
