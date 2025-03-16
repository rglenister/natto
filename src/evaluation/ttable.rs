use std::sync::atomic::{AtomicU64, Ordering};
use crate::board::{PieceColor, PieceType};
use crate::board::BoardSide::{KingSide, QueenSide};
use crate::board::PieceType::{Bishop, Knight, Queen, Rook};
use crate::chess_move::{BaseMove, ChessMove};
use crate::chess_move::ChessMove::{Basic, Castling, EnPassant, Promotion};

//#[derive(Clone, Copy, Debug, PartialEq, Eq)]
//pub struct Move(u16); // Assume a move is stored as a 16-bit integer

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub zobrist: u64,
    pub best_move: ChessMove,
    pub depth: u8,
    pub score: i32,
    pub bound: BoundType, // Bound type
}

pub struct TranspositionTable {
    table: Vec<AtomicU64>,
    size: usize,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "Size must be a power of 2");
//        let table = vec![AtomicU64::new(0); size * 2]; // Using 2 u64 per entry
        let table = (0..size * 2).map(|_| AtomicU64::new(0)).collect(); // Using 2 u64 per entry
        Self { table, size }
    }

    pub fn store(&self, zobrist: u64, best_move: ChessMove, depth: u8, score: i32, bound: BoundType) {
        let index = (zobrist as usize) % self.size;
        let packed = Self::pack_entry(zobrist, best_move, depth, score, bound);
        self.table[index * 2].store(packed.0, Ordering::Relaxed);
        self.table[index * 2 + 1].store(packed.1, Ordering::Relaxed);
    }

    pub fn retrieve(&self, zobrist: u64) -> Option<TTEntry> {
        let index = (zobrist as usize) % self.size;
        let packed1 = self.table[index * 2].load(Ordering::Relaxed);
        let packed2 = self.table[index * 2 + 1].load(Ordering::Relaxed);
        Self::unpack_entry(packed1, packed2).filter(|e| e.zobrist == zobrist)
    }

    fn pack_entry(zobrist: u64, best_move: ChessMove, depth: u8, score: i32, bound: BoundType) -> (u64, u64) {
        let packed1 = zobrist;
        let packed2 = Self::pack_move(best_move) | ((depth as u64) << 21) | (((score as u64) << 29) & (0xffffffff << 29)) | ((bound as u64) << 61);
        (packed1, packed2)
    }
    
    fn pack_move(best_move: ChessMove) -> u64 {
        fn pack_base_move_and_type(base_move: BaseMove, move_type: u64) -> u64 {
            // 20 19 18 17 16 15 14 13 12 11 10 09 08 07 06 05 04 03 02 01 00
            // |                |   to  square    |c||move |e   x   t   r   a
            //                                                    |type|
            ((base_move.from as u64 & 0x3ff) << 15) | ((base_move.to as u64 & 0x3f) << 9) | ((base_move.capture as u64 & 1) << 8) | ((move_type & 0x03) << 6)
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
    
    fn unpack_mv(move_packed: u64) -> ChessMove {
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

    fn unpack_entry(packed1: u64, packed2: u64) -> Option<TTEntry> {
        let zobrist = packed1;
        let best_move = Self::unpack_mv(packed2);
        let depth = ((packed2 >> 21) & 0xFF) as u8;
        let score = ((packed2 >> 29) & 0xFFFFFFFF) as i32;
        let bound = match (packed2 >> 61) & 0xFF {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            2 => BoundType::UpperBound,
            _ => return None,
        };
        Some(TTEntry { zobrist, best_move, depth, score, bound })
    }
}

#[cfg(test)]
mod tests {
    use crate::chess_move::BaseMove;
    use crate::chess_move::ChessMove::Basic;
    use crate::evaluation::ttable::BoundType::LowerBound;
    use crate::evaluation::ttable::{ChessMove, TranspositionTable};
    use crate::position::Position;

    struct BasicMove {}

    #[test]
    fn do_stuff_with_table() {
        let table = TranspositionTable::new(1 << 1);
        assert_eq!(table.table.len(), 2 * 2);
        assert_eq!(table.size, 2);
        
        let position = Position::new_game();
        table.store(position.hash_code(), Basic { base_move: BaseMove{from: 63, to: 0, capture: true }},8, -100000, LowerBound);
        let entry = table.retrieve(position.hash_code()).unwrap();
        assert_eq!(entry.zobrist, position.hash_code());
        assert_eq!(entry.best_move, Basic { base_move: BaseMove { from: 63, to: 0, capture: true } });
        assert_eq!(entry.depth, 8);
        assert_eq!(entry.score, -100000);
        assert_eq!(entry.bound, LowerBound);
    }
    
}