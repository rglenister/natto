use std::sync::atomic::{AtomicU64, Ordering};
use dotenv::var;
use log::{error, info};
use once_cell::sync::Lazy;
use crate::chessboard::board::BoardSide::{KingSide, QueenSide};
use crate::chessboard::piece::PieceType::{Bishop, Knight, Queen, Rook};
use crate::config::CONFIG;
use crate::r#move::{BaseMove, Move};
use crate::r#move::Move::{Basic, Castling, EnPassant, Promotion};
pub use crate::search::negamax::MAXIMUM_SCORE;
use crate::position::Position;

pub static TRANSPOSITION_TABLE: Lazy<TranspositionTable> = Lazy::new(|| {
    let hash_size = DEFAULT_HASH_SIZE;
    info!("Creating transposition table with size {} ({:#X})", hash_size, hash_size);
    let transposition_table = TranspositionTable::new(hash_size);
    info!("Transposition table created");
    transposition_table
});

const DEFAULT_HASH_SIZE: usize = 1 << 25;

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
    pub depth: usize,
    pub score: isize,
    pub bound_type: BoundType,
}

pub struct TranspositionTable {
    table: Vec<AtomicU64>,
    size: usize,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "Size must be a power of 2");
        let table = (0..size * 2).map(|_| AtomicU64::new(0)).collect(); // Using 2 u64 per entry
        let table = Self { table, size };
        ensure_physical_memory::<AtomicU64>(&table.table);
        table
    }
    
    pub fn insert(&self, position: &Position, depth: usize, alpha: isize, beta: isize, score: isize, mov: Option<Move>) {
        let bound_type = if score <= alpha {
            BoundType::UpperBound
        } else if score >= beta {
            BoundType::LowerBound
        } else {
            BoundType::Exact
        };
        let do_store = {
            if let Some(current_entry) = self.probe(position.hash_code()) {
                depth > current_entry.depth ||
                    (depth == current_entry.depth &&
                        ((bound_type == BoundType::Exact && current_entry.bound_type != BoundType::Exact) ||
                            (bound_type == BoundType::LowerBound && current_entry.bound_type == BoundType::UpperBound)))
            } else {
                true
            }
        };
        if do_store {
            self.store(position.hash_code(), mov, depth as u8, score as i32, bound_type);
            //#[cfg(debug_assertions)]
            if cfg!(debug_assertions)
            {
                let entry = self.probe(position.hash_code()).unwrap();
                println!("Debug info: TranspositionTable size is {}", self.item_count());
                assert_eq!(entry.zobrist, position.hash_code());
                assert_eq!(entry.best_move, mov);
                assert_eq!(entry.depth, depth);
                assert_eq!(entry.score, score);
                assert_eq!(entry.bound_type, bound_type);
            }
        }
    }

    fn store(&self, zobrist: u64, best_move: Option<Move>, depth: u8, score: i32, bound: BoundType) {
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
            ((base_move.from as u64 & 0x3f) << 15) | ((base_move.to as u64 & 0x3f) << 9) | ((base_move.capture as u64 & 1) << 8) | ((move_type & 0x03) << 6)
        }

        match best_move {
            Basic { base_move } => {
                pack_base_move_and_type(base_move, 0)
            }
            EnPassant { base_move, capture_square } => {
                pack_base_move_and_type(base_move, 1) | (capture_square as u64 & 0x3f)
            }
            Promotion { base_move, promote_to } => {
                pack_base_move_and_type(base_move, 2) | (promote_to as u64 & 0x3f)
            }
            Castling { base_move, board_side } => {
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
            0 => Basic {
                base_move: BaseMove { from, to, capture: is_capture },
            },
            1 => EnPassant {
                base_move: BaseMove { from, to, capture: is_capture },
                capture_square: (move_packed & 0x3F) as usize,
            },
            2 => Promotion {
                base_move: BaseMove { from, to, capture: is_capture },
                promote_to: match move_packed & 0x3F {
                    1 => Knight,
                    2 => Bishop,
                    3 => Rook,
                    4 => Queen,
                    _ => panic!("Invalid promotion type"),
                }
            },
            3 => Castling {
                base_move: BaseMove { from, to, capture: false },
                board_side: match move_packed & 0x3F {
                    0 => KingSide,
                    1 => QueenSide,
                    _ => panic!("Invalid castling side"),
                }
            },
            _ => panic!("Invalid move type"),
        }
    }

    fn pack_entry(zobrist: u64, best_move: Option<Move>, depth: u8, score: i32, bound: BoundType) -> (u64, u64) {
        let packed1 = zobrist;
        let packed2 =
            if best_move.is_some() { Self::pack_move(best_move.unwrap()) } else { 0 } |
                ((depth as u64) << 21) |
                (((score + MAXIMUM_SCORE as i32) as u64 & 0x0FFFFFFF) << 29) |
                ((bound as u64) << 57) |
                ((0u64) << 61);
        (packed1, packed2)
    }

    fn unpack_entry(packed1: u64, packed2: u64) -> Option<TTEntry> {
        let zobrist = packed1;
        let has_move = (packed2 & 0x1fffff) != 0;
        let best_move =  if has_move { Some(Self::unpack_mv(packed2)) } else { None };
        let depth = ((packed2 >> 21) & 0xFF) as u8;
        let score = ((packed2 >> 29) & 0x0FFFFFFF) as i32 - MAXIMUM_SCORE as i32;
        let bound = match (packed2 >> 57) & 0x0F {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            2 => BoundType::UpperBound,
            _ => panic!("Invalid bound"),
        };
        Some(TTEntry { zobrist, best_move, depth: depth as usize, score: score as isize, bound_type: bound })
    }
}

pub fn ensure_physical_memory<T>(data: &[AtomicU64]) {
    for atomic in data {
        atomic.store(0, Ordering::Relaxed); // Write to every entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::transposition_table::BoundType::{LowerBound, UpperBound};
    use crate::position::Position;
    
    #[test]
    fn test_small_table_creation() {
        let table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 2 * 2);
        assert_eq!(table.size, 2);
    }
    #[test]
    fn test_large_table_creation() {
        assert_eq!(1 << 25, 33_554_432);
        let table = TranspositionTable::new(1 << 25);
        assert_eq!(table.table.len(), (1 << 25) * 2);
        assert_eq!(table.size, 1 << 25);
        assert_eq!(table.table.len() * std::mem::size_of::<AtomicU64>(), 1 << 29);
        assert_eq!(1 << 29, 536_870_912);
    }

    #[test]
    fn test_do_stuff_with_table() {
        let table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 2 * 2);
        assert_eq!(table.size, 2);

        let position = Position::new_game();
        table.store(position.hash_code(), Option::from(Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }), 8, -100, LowerBound);
        let entry = table.probe(position.hash_code()).unwrap();
        assert_eq!(entry.zobrist, position.hash_code());
        assert_eq!(entry.best_move, Some(Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }));
        assert_eq!(entry.depth, 8);
        assert_eq!(entry.score, -100);
        assert_eq!(entry.bound_type, LowerBound);
    }
    #[test]
    fn test_item_count() {
        let table = TranspositionTable::new(1 << 2);
        assert_eq!(table.table.len(), 4 * 2);
        assert_eq!(table.size, 4);
        assert_eq!(table.item_count(), 0);
        let position = Position::new_game();
        table.store(position.hash_code(), Option::from(Basic { base_move: BaseMove { from: 63, to: 0, capture: true } }), 8, -100, LowerBound);
        assert_eq!(table.item_count(), 1);
        table.clear();
        assert_eq!(table.item_count(), 0);
    }

    mod entry_packing {
        use crate::search::transposition_table::BoundType::Exact;
        use crate::game::GameStatus::DrawnByInsufficientMaterial;
        use super::*;

        #[test]
        fn test_pack_unpack() {
            let position = Position::new_game();
            let packed1 = position.hash_code();
            let packed = TranspositionTable::pack_entry(
                packed1, Option::from(Basic { base_move: BaseMove { from: 12, to: 16, capture: true } }), 2, 21, Exact);
            let unpacked = TranspositionTable::unpack_entry(packed.0, packed.1).unwrap();
            assert_eq!(unpacked.zobrist, packed1);
            assert_eq!(unpacked.best_move, Some(Basic { base_move: BaseMove { from: 12, to: 16, capture: true } }));
            assert_eq!(unpacked.depth, 2);
            assert_eq!(unpacked.score, 21);
            assert_eq!(unpacked.bound_type, Exact);
        }

        #[test]
        fn test_pack_unpack_without_move() {
            let zobrist: u64 = 123456;
            let packed = TranspositionTable::pack_entry(
                zobrist, None, 2, -21, Exact);
            let unpacked = TranspositionTable::unpack_entry(packed.0, packed.1).unwrap();
            assert_eq!(unpacked.zobrist, zobrist);
            assert_eq!(unpacked.best_move, None);
            assert_eq!(unpacked.depth, 2);
            assert_eq!(unpacked.score, -21);
            assert_eq!(unpacked.bound_type, Exact);
        }

        mod move_packing {
            use super::*;
            use crate::chessboard::board::BoardSide::KingSide;
            use crate::chessboard::piece::PieceType::Rook;
            use crate::r#move::Move::{Castling, EnPassant, Promotion};
            #[test]
            fn test_basic_move() {
                let basic_move = Basic { base_move: BaseMove{from: 63, to: 0, capture: false }};
                let packed = TranspositionTable::pack_move(basic_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(basic_move, unpacked);
            }
            #[test]
            fn test_en_passant_move() {
                let en_passant_move = EnPassant { base_move: BaseMove{from: 63, to: 0, capture: true }, capture_square: 40};
                let packed = TranspositionTable::pack_move(en_passant_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(en_passant_move, unpacked);
            }
            #[test]
            fn test_promotion_move() {
                let promotion_move = Promotion { base_move: BaseMove{from: 63, to: 0, capture: false }, promote_to: Rook};
                let packed = TranspositionTable::pack_move(promotion_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(promotion_move, unpacked);
            }
            #[test]
            fn test_castling_move() {
                // capture must be set to false otherwise the test will fail - castling never captures
                let castling_move = Castling { base_move: BaseMove{from: 63, to: 0, capture: false }, board_side: KingSide};
                let packed = TranspositionTable::pack_move(castling_move);
                let unpacked = TranspositionTable::unpack_mv(packed);
                assert_eq!(castling_move, unpacked);
            }

        }

    }
}