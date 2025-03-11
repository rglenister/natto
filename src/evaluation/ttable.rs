use std::sync::atomic::{AtomicU64, Ordering};

/// Represents a move (simplified for this example)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move(u16); // Assume a move is stored as a 16-bit integer

/// Enum representing the bound type of a transposition table entry
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

/// A single entry in the transposition table
#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub zobrist: u64,   // Hash of the position
    pub best_move: Move, // Best move found in this position
    pub depth: u8,       // Search depth
    pub score: i32,      // Evaluation score
    pub bound: BoundType, // Bound type
}

/// Transposition Table
pub struct TranspositionTable {
    table: Vec<AtomicU64>, // Raw memory for storing entries
    size: usize,            // Number of entries (must be a power of 2)
}

impl TranspositionTable {
    /// Create a new transposition table with the given size (must be a power of 2)
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "Size must be a power of 2");
//        let table = vec![AtomicU64::new(0); size * 2]; // Using 2 u64 per entry
        let table = (0..size * 2).map(|_| AtomicU64::new(0)).collect(); // Using 2 u64 per entry
        Self { table, size }
    }

    /// Stores an entry in the table
    pub fn store(&self, zobrist: u64, best_move: Move, depth: u8, score: i32, bound: BoundType) {
        let index = (zobrist as usize) % self.size;
        let packed = Self::pack_entry(zobrist, best_move, depth, score, bound);
        self.table[index * 2].store(packed.0, Ordering::Relaxed);
        self.table[index * 2 + 1].store(packed.1, Ordering::Relaxed);
    }

    /// Retrieves an entry from the table
    pub fn retrieve(&self, zobrist: u64) -> Option<TTEntry> {
        let index = (zobrist as usize) % self.size;
        let packed1 = self.table[index * 2].load(Ordering::Relaxed);
        let packed2 = self.table[index * 2 + 1].load(Ordering::Relaxed);
        Self::unpack_entry(packed1, packed2).filter(|e| e.zobrist == zobrist)
    }

    /// Packs TTEntry data into two u64 values
    fn pack_entry(zobrist: u64, best_move: Move, depth: u8, score: i32, bound: BoundType) -> (u64, u64) {
        let packed1 = zobrist;
        let packed2 = (best_move.0 as u64) | ((depth as u64) << 16) | ((score as u64) << 24) | ((bound as u64) << 56);
        (packed1, packed2)
    }

    /// Unpacks TTEntry data from two u64 values
    fn unpack_entry(packed1: u64, packed2: u64) -> Option<TTEntry> {
        let zobrist = packed1;
        let best_move = Move((packed2 & 0xFFFF) as u16);
        let depth = ((packed2 >> 16) & 0xFF) as u8;
        let score = ((packed2 >> 24) & 0xFFFFFFFF) as i32;
        let bound = match (packed2 >> 56) & 0xFF {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            2 => BoundType::UpperBound,
            _ => return None,
        };
        Some(TTEntry { zobrist, best_move, depth, score, bound })
    }
}