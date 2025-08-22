use crate::core::board::BoardSide;
use crate::core::piece::PieceType;
use crate::core::position::Position;
use crate::core::r#move::{BaseMove, Move};
use crate::uci::config;
use crate::search::negamax::Search;
pub use crate::search::negamax::MAXIMUM_SCORE;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub zobrist: u64,
    pub best_move: Option<Move>,
    pub depth: u8,
    pub score: i32,
    pub bound_type: BoundType,
}

pub struct TranspositionTable {
    table: Vec<AtomicU64>,
    size: usize,
}

impl TranspositionTable {
    pub fn new(size_in_mb: usize) -> Self {
        let entry_size = 2 * size_of::<AtomicU64>();
        let requested_num_entries = size_in_mb * 1024 * 1024 / entry_size;
        let actual_num_entries = Self::prev_power_of_two(requested_num_entries);
        let table_size_in_bytes = actual_num_entries * entry_size;
        log::info!("Creating transposition table with size {actual_num_entries} ({actual_num_entries:#X}) from a requested maximum size of {size_in_mb} Mib");
        log::info!(
            "Transposition table created. Total memory used is {} MiB ({:.2} GiB)",
            table_size_in_bytes / (1024 * 1024),
            Self::bytes_to_gib(table_size_in_bytes)
        );
        let table = (0..actual_num_entries * 2).map(|_| AtomicU64::new(0)).collect(); // Using 2 u64 per entry
        Self { table, size: actual_num_entries }
    }

    pub fn new_using_config() -> Self {
        Self::new(config::get_hash_size())
    }

    pub fn insert(
        &self,
        position: &Position,
        depth: u8,
        alpha: i32,
        beta: i32,
        score: i32,
        mov: Option<Move>,
    ) {
        let bound_type = if Search::is_terminal_score(score) {
            BoundType::Exact
        } else if score <= alpha {
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
                        && ((bound_type == BoundType::Exact
                            && current_entry.bound_type != BoundType::Exact)
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

    fn store(
        &self,
        zobrist: u64,
        best_move: Option<Move>,
        depth: u8,
        score: i32,
        bound: BoundType,
    ) {
        let index = (zobrist as usize) % self.size;
        let packed = Self::pack_entry(zobrist, best_move, depth, score, bound);
        self.table[index * 2].store(packed.0, Ordering::Relaxed);
        self.table[index * 2 + 1].store(packed.1, Ordering::Relaxed);
    }

    pub fn probe(&self, zobrist: u64) -> Option<TTEntry> {
        let index = (zobrist as usize) % self.size;
        let packed1 = self.table[index * 2].load(Ordering::Relaxed);
        if packed1 == zobrist {
            let packed2 = self.table[index * 2 + 1].load(Ordering::Relaxed);
            Self::unpack_entry(packed1, packed2)
        } else {
            None
        }
    }

    fn prev_power_of_two(configured_hash_size: usize) -> usize {
        if configured_hash_size == 0 {
            return 0;
        }
        let msb_pos = usize::BITS - 1 - configured_hash_size.leading_zeros();
        1usize << msb_pos
    }

    fn bytes_to_gib(bytes: usize) -> f64 {
        bytes as f64 / (1024 * 1024 * 1024) as f64
    }

    #[allow(dead_code)]
    pub fn item_count(&self) -> usize {
        let mut count = 0;
        for i in 0..self.size {
            if self.table[i * 2].load(Ordering::Relaxed) != 0 {
                count += 1;
            }
        }
        count
    }
    pub fn clear(&self) {
        for atomic in &self.table {
            atomic.store(0, Ordering::Relaxed);
        }
    }

    fn pack_move(best_move: Move) -> u64 {
        fn pack_base_move_and_type(base_move: BaseMove, move_type: u64) -> u64 {
            // 20 19 18 17 16 15 14 13 12 11 10 09 08 07 06 05 04 03 02 01 00
            // |   from square  |   to  square    |c||move |e   x   t   r   a
            //                                       |type|| non basic  moves
            ((base_move.from as u64 & 0x3f) << 15)
                | ((base_move.to as u64 & 0x3f) << 9)
                | ((base_move.capture as u64 & 1) << 8)
                | ((move_type & 0x03) << 6)
        }

        match best_move {
            Move::Basic { base_move } => pack_base_move_and_type(base_move, 0),
            Move::EnPassant { base_move, capture_square } => {
                pack_base_move_and_type(base_move, 1) | (capture_square as u64 & 0x3f)
            }
            Move::Promotion { base_move, promote_to } => {
                pack_base_move_and_type(base_move, 2) | (promote_to as u64 & 0x3f)
            }
            Move::Castling { base_move, board_side } => {
                pack_base_move_and_type(base_move, 3) | (board_side as u64 & 0x3f)
            }
        }
    }

    fn unpack_mv(move_packed: u64) -> Move {
        let from = ((move_packed >> 15) & 0x3F) as usize;
        let to = ((move_packed >> 9) & 0x3F) as usize;
        let is_capture = ((move_packed >> 8) & 1) != 0;
        let move_type = (move_packed >> 6) & 3;

        match move_type {
            0 => Move::Basic {
                base_move: BaseMove { from: from as u8, to: to as u8, capture: is_capture },
            },
            1 => Move::EnPassant {
                base_move: BaseMove { from: from as u8, to: to as u8, capture: is_capture },
                capture_square: (move_packed & 0x3F) as u8,
            },
            2 => Move::Promotion {
                base_move: BaseMove { from: from as u8, to: to as u8, capture: is_capture },
                promote_to: match move_packed & 0x3F {
                    1 => PieceType::Knight,
                    2 => PieceType::Bishop,
                    3 => PieceType::Rook,
                    4 => PieceType::Queen,
                    _ => panic!("Invalid promotion type"),
                },
            },
            3 => Move::Castling {
                base_move: BaseMove { from: from as u8, to: to as u8, capture: is_capture },
                board_side: match move_packed & 0x3F {
                    0 => BoardSide::KingSide,
                    1 => BoardSide::QueenSide,
                    _ => panic!("Invalid castling side"),
                },
            },
            _ => panic!("Invalid move type"),
        }
    }

    fn pack_entry(
        zobrist: u64,
        best_move: Option<Move>,
        depth: u8,
        score: i32,
        bound: BoundType,
    ) -> (u64, u64) {
        let packed1 = zobrist;
        let packed2 = if let Some(best_move) = best_move { Self::pack_move(best_move) } else { 0 }
            | ((depth as u64) << 21)
            | (((score + MAXIMUM_SCORE) as u64 & 0x0FFFFFFF) << 29)
            | ((bound as u64) << 57);
        (packed1, packed2)
    }

    fn unpack_entry(packed1: u64, packed2: u64) -> Option<TTEntry> {
        let zobrist = packed1;
        let has_move = (packed2 & 0x1fffff) != 0;
        let best_move = if has_move { Some(Self::unpack_mv(packed2)) } else { None };
        let depth = ((packed2 >> 21) & 0xFF) as u8;
        let score = ((packed2 >> 29) & 0x0FFFFFFF) as i32 - MAXIMUM_SCORE;
        let bound = match (packed2 >> 57) & 0x0F {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            2 => BoundType::UpperBound,
            _ => panic!("Invalid bound"),
        };
        Some(TTEntry { zobrist, best_move, depth, score, bound_type: bound })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::transposition_table::BoundType::LowerBound;

    #[test]
    fn test_small_table_creation() {
        let t_table = TranspositionTable::new(1);
        assert_eq!(t_table.table.len(), 131072);
        assert_eq!(t_table.size, 131072 / 2);
    }
    #[test]
    fn test_large_table_creation() {
        let t_table = TranspositionTable::new(1024);
        assert_eq!(t_table.table.len(), 134217728);
        assert_eq!(t_table.size, 134217728 / 2);
        assert_eq!(size_of::<AtomicU64>(), 8);
    }

    #[test]
    fn test_do_stuff_with_table() {
        let t_table = TranspositionTable::new(1 << 1);

        let position = Position::new_game();
        t_table.store(
            position.hash_code(),
            Option::from(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }),
            8,
            -100,
            LowerBound,
        );
        let entry = t_table.probe(position.hash_code()).unwrap();
        assert_eq!(entry.zobrist, position.hash_code());
        assert_eq!(
            entry.best_move,
            Some(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } })
        );
        assert_eq!(entry.depth, 8);
        assert_eq!(entry.score, -100);
        assert_eq!(entry.bound_type, LowerBound);
    }

    #[test]
    fn test_item_count() {
        let t_table = TranspositionTable::new(1);
        let position = Position::new_game();
        t_table.store(
            position.hash_code(),
            Option::from(Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }),
            8,
            -100,
            LowerBound,
        );
        assert_eq!(t_table.item_count(), 1);
        t_table.clear();
        assert_eq!(t_table.item_count(), 0);
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

    mod entry_packing {
        use super::*;
        use crate::search::transposition_table::BoundType::Exact;

        #[test]
        fn test_pack_unpack() {
            let position = Position::new_game();
            let packed1 = position.hash_code();
            let packed = TranspositionTable::pack_entry(
                packed1,
                Option::from(Move::Basic {
                    base_move: BaseMove { from: 12, to: 16, capture: true },
                }),
                2,
                21,
                Exact,
            );
            let unpacked = TranspositionTable::unpack_entry(packed.0, packed.1).unwrap();
            assert_eq!(unpacked.zobrist, packed1);
            assert_eq!(
                unpacked.best_move,
                Some(Move::Basic { base_move: BaseMove { from: 12, to: 16, capture: true } })
            );
            assert_eq!(unpacked.depth, 2);
            assert_eq!(unpacked.score, 21);
            assert_eq!(unpacked.bound_type, Exact);
        }

        #[test]
        fn test_pack_unpack_without_move() {
            let zobrist: u64 = 123456;
            let packed = TranspositionTable::pack_entry(zobrist, None, 2, -21, Exact);
            let unpacked = TranspositionTable::unpack_entry(packed.0, packed.1).unwrap();
            assert_eq!(unpacked.zobrist, zobrist);
            assert_eq!(unpacked.best_move, None);
            assert_eq!(unpacked.depth, 2);
            assert_eq!(unpacked.score, -21);
            assert_eq!(unpacked.bound_type, Exact);
        }

        mod move_packing {
            use super::*;
            use crate::core::board::BoardSide::KingSide;
            use crate::core::piece::PieceType::Rook;
            use crate::core::r#move::Move::{Castling, EnPassant, Promotion};
            #[test]
            fn test_basic_move() {
                let basic_move =
                    Move::Basic { base_move: BaseMove { from: 63, to: 0, capture: false } };
                let packed = TranspositionTable::pack_move(basic_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(basic_move, unpacked);
            }
            #[test]
            fn test_en_passant_move() {
                let en_passant_move = EnPassant {
                    base_move: BaseMove { from: 63, to: 0, capture: true },
                    capture_square: 40,
                };
                let packed = TranspositionTable::pack_move(en_passant_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(en_passant_move, unpacked);
            }
            #[test]
            fn test_promotion_move() {
                let promotion_move = Promotion {
                    base_move: BaseMove { from: 63, to: 0, capture: false },
                    promote_to: Rook,
                };
                let packed = TranspositionTable::pack_move(promotion_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(promotion_move, unpacked);
            }
            #[test]
            fn test_castling_move() {
                // capture must be set to false otherwise the test will fail - castling never captures
                let castling_move = Castling {
                    base_move: BaseMove { from: 63, to: 0, capture: false },
                    board_side: KingSide,
                };
                let packed = TranspositionTable::pack_move(castling_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(castling_move, unpacked);
            }
        }
    }
}
