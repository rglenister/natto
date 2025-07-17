use arrayvec::ArrayVec;
use std::cmp::Ordering;
use crate::core::r#move::Move;
use crate::core::position::Position;
use crate::core::piece::PieceColor;
use crate::core::piece::PieceType;
use crate::eval::evaluation::PIECE_SCORES;
use crate::search::negamax::MAXIMUM_SEARCH_DEPTH;

// Constants for move scoring
const HASH_MOVE_SCORE: i32 = 20000;
const CAPTURE_SCORE_BASE: i32 = 10000;
const KILLER_FIRST_SLOT_SCORE: i32 = 9000;
const KILLER_SECOND_SLOT_SCORE: i32 = 8000;
const PROMOTION_SCORE: i32 = 7500;
const COUNTERMOVE_SCORE: i32 = 7000;
const CASTLING_SCORE: i32 = 6000;

// Maximum number of killer moves to store per ply
const MAX_KILLER_MOVES: usize = 2;

// Type to represent killer moves for all plies
pub type KillerMoves = [[Option<Move>; MAX_KILLER_MOVES]; MAXIMUM_SEARCH_DEPTH];

// History table: [color][from_square][to_square]
pub type HistoryTable = [[[i32; 64]; 64]; 2];

// Counter move table: [piece_color][piece_type][to_square]
pub type CounterMoveTable = [[Option<Move>; 64]; 12];

pub struct MoveOrderer {
    killer_moves: KillerMoves,
    history_table: HistoryTable,
    counter_moves: CounterMoveTable,
}

impl MoveOrderer {
    pub fn new() -> Self {
        MoveOrderer {
            killer_moves: [[None; MAX_KILLER_MOVES]; MAXIMUM_SEARCH_DEPTH],
            history_table: [[[0; 64]; 64]; 2],
            counter_moves: [[None; 64]; 12],
        }
    }

    pub fn clear(&mut self) {
        self.killer_moves = [[None; MAX_KILLER_MOVES]; MAXIMUM_SEARCH_DEPTH];
        self.history_table = [[[0; 64]; 64]; 2];
        self.counter_moves = [[None; 64]; 12];
    }

    pub fn add_killer_move(&mut self, mov: Move, ply: usize) {
        // Don't add capturing moves or promotions as killers
        if mov.get_base_move().capture || matches!(mov, Move::Promotion { .. }) {
            return;
        }

        // Don't add if already present as the first killer
        if let Some(first_killer) = self.killer_moves[ply][0] {
            if first_killer == mov {
                return;
            }
        }

        // Shift existing killer to second slot and add new one to first slot
        self.killer_moves[ply][1] = self.killer_moves[ply][0];
        self.killer_moves[ply][0] = Some(mov);
    }

    pub fn is_killer_move(&self, mov: &Move, ply: usize) -> bool {
        for slot in 0..MAX_KILLER_MOVES {
            if let Some(killer) = self.killer_moves[ply][slot] {
                if killer == *mov {
                    return true;
                }
            }
        }
        false
    }

    pub fn update_history_score(&mut self, position: &Position, mov: &Move, depth: i32) {
        let side_to_move = position.side_to_move() as usize;
        let from = mov.get_base_move().from as usize;
        let to = mov.get_base_move().to as usize;

        // Bonus based on depth - deeper searches get more weight
        let bonus = depth * depth;

        // Increase the history score, but keep it in a reasonable range
        self.history_table[side_to_move][from][to] += bonus;

        // Prevent overflow by capping at a reasonable value
        if self.history_table[side_to_move][from][to] > 10000 {
            // Age all history entries (divide by 2)
            for c in 0..2 {
                for f in 0..64 {
                    for t in 0..64 {
                        self.history_table[c][f][t] /= 2;
                    }
                }
            }
        }
    }

    pub fn update_countermove(&mut self, position: &Position, last_move: &Move, countermove: Move) {
        if let Some(piece) = position.board().get_piece(last_move.get_base_move().to as usize) {
            let piece_idx = (piece.piece_color as usize * 6) + (piece.piece_type as usize);
            let to_square = last_move.get_base_move().to as usize;
            self.counter_moves[piece_idx][to_square] = Some(countermove);
        }
    }

    pub fn get_countermove(&self, position: &Position, last_move: &Option<&Move>) -> Option<Move> {
        if let Some(last_move) = last_move {
            if let Some(piece) = position.board().get_piece(last_move.get_base_move().to as usize) {
                let piece_idx = (piece.piece_color as usize * 6) + (piece.piece_type as usize);
                let to_square = last_move.get_base_move().to as usize;
                return self.counter_moves[piece_idx][to_square];
            }
        }
        None
    }

    // Score moves for ordering
    pub fn score_moves<T>(&self, position: &Position, hash_move: Option<Move>, moves: &mut T, ply: usize, last_move: Option<&Move>) 
    where 
        T: AsMut<[(Move, i32)]>
    {
        let countermove = self.get_countermove(position, &last_move);
        let moves_slice = moves.as_mut();

        for (mov, score) in moves_slice.iter_mut() {
            *score = self.score_move(position, mov, hash_move, ply, countermove);
        }
    }

    fn score_move(&self, position: &Position, mov: &Move, hash_move: Option<Move>, ply: usize, countermove: Option<Move>) -> i32 {
        // Hash move gets highest priority
        if let Some(hash_move) = hash_move {
            if *mov == hash_move {
                return HASH_MOVE_SCORE;
            }
        }

        let base_move = mov.get_base_move();

        // Captures are scored by MVV-LVA
        if base_move.capture {
            let mut score = CAPTURE_SCORE_BASE;

            // Add MVV-LVA score
            if let Some(victim) = position.board().get_piece(base_move.to as usize) {
                let victim_value = PIECE_SCORES[victim.piece_type as usize];

                if let Some(aggressor) = position.board().get_piece(base_move.from as usize) {
                    let aggressor_value = PIECE_SCORES[aggressor.piece_type as usize];

                    // MVV-LVA = victim value - aggressor value/100 
                    // This ensures higher value victims are prioritized, then lowest value attackers
                    score += victim_value as i32 - (aggressor_value as i32) / 100;
                }
            }

            return score;
        }

        // Check for promotions (non-capturing)
        if let Move::Promotion { promote_to, .. } = mov {
            return PROMOTION_SCORE + PIECE_SCORES[*promote_to as usize] as i32;
        }

        // Check if move is a killer move
        if let Some(killer1) = self.killer_moves[ply][0] {
            if killer1 == *mov {
                return KILLER_FIRST_SLOT_SCORE;
            }
        }

        if let Some(killer2) = self.killer_moves[ply][1] {
            if killer2 == *mov {
                return KILLER_SECOND_SLOT_SCORE;
            }
        }

        // Check for countermove
        if let Some(counter) = countermove {
            if counter == *mov {
                return COUNTERMOVE_SCORE;
            }
        }

        // Check for castling
        if let Move::Castling { .. } = mov {
            return CASTLING_SCORE;
        }

        // Use history score for quiet moves
        let side = position.side_to_move() as usize;
        self.history_table[side][base_move.from as usize][base_move.to as usize]
    }

    // Sort moves based on scores (highest first)
    pub fn sort_moves<T>(moves: &mut T) 
    where 
        T: AsMut<[(Move, i32)]>
    {
        let moves_slice = moves.as_mut();
        moves_slice.sort_by(|a, b| b.1.cmp(&a.1));
    }

    // For SEE (Static Exchange Evaluation)
    pub fn mvv_lva_score(position: &Position, mov: &Move) -> i32 {
        let base_move = mov.get_base_move();
        if !base_move.capture {
            if let Move::Promotion { promote_to, .. } = mov {
                return (PIECE_SCORES[*promote_to as usize] as i32 - PIECE_SCORES[PieceType::Pawn as usize] as i32) / 100;           
            }
            return 0;
        }

        if let Some(victim) = position.board().get_piece(base_move.to as usize) {
            let victim_value = PIECE_SCORES[victim.piece_type as usize];

            if let Some(aggressor) = position.board().get_piece(base_move.from as usize) {
                let aggressor_value = PIECE_SCORES[aggressor.piece_type as usize];
                let difference_value = victim_value as i32 - (aggressor_value as i32);
                if let Move::Promotion { promote_to, .. } = mov {
                    return (PIECE_SCORES[*promote_to as usize] as i32 + difference_value) / 100;
                } else {
                    return difference_value / 100;
                }
            }
        }

        0
    }
}

// Functions for move ordering
pub fn order_moves(position: &Position, moves: &mut Vec<Move>, move_orderer: &MoveOrderer, ply: usize, hash_move: Option<Move>, last_move: &Option<Move>) {
    // Maximum legal moves from any position is ~218, so 256 is safe
    const MAX_MOVES: usize = 256;

    // Create a scored move list on the stack
    let mut scored_moves: ArrayVec<(Move, i32), MAX_MOVES> = ArrayVec::new();
    scored_moves.extend(moves.iter().map(|m| (*m, 0)));

    // Score each move
    move_orderer.score_moves(position, hash_move, &mut scored_moves, ply, Option::from(last_move));

    // Sort by score
    MoveOrderer::sort_moves(&mut scored_moves);

    // Update the original moves list with sorted moves
    moves.clear();
    moves.extend(scored_moves.iter().map(|(m, _)| *m));
}

// Specialized capture ordering for quiescence search
pub fn order_quiescence_moves(position: &Position, moves: &mut Vec<Move>) {
    // Maximum possible captures is much less than legal moves, 64 is very safe
    const MAX_CAPTURES: usize = 250;

    // Create a scored move list on the stack with MVV-LVA scores
    let mut scored_moves: ArrayVec<(Move, i32), MAX_CAPTURES> = ArrayVec::new();
    scored_moves.extend(moves.iter().map(|m| (*m, MoveOrderer::mvv_lva_score(position, m))));

    // Sort by score
    scored_moves.sort_by(|a, b| b.1.cmp(&a.1));

    // Update the original moves list with sorted moves
    moves.clear();
    moves.extend(scored_moves.iter().map(|(m, _)| *m));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::r#move::BaseMove;
    use crate::core::piece::PieceType::*;
    use crate::core::r#move::Move::Basic;
    use crate::core::r#move::Move::Promotion;

    #[test]
    fn test_killer_move_handling() {
        let mut move_orderer = MoveOrderer::new();
        let ply = 2;

        let killer_move1 = Basic { base_move: BaseMove { from: 12, to: 28, capture: false } };
        let killer_move2 = Basic { base_move: BaseMove { from: 11, to: 27, capture: false } };

        move_orderer.add_killer_move(killer_move1, ply);
        assert_eq!(move_orderer.killer_moves[ply][0], Some(killer_move1));
        assert_eq!(move_orderer.killer_moves[ply][1], None);

        move_orderer.add_killer_move(killer_move2, ply);
        assert_eq!(move_orderer.killer_moves[ply][0], Some(killer_move2));
        assert_eq!(move_orderer.killer_moves[ply][1], Some(killer_move1));

        // Adding the same killer should not change anything
        move_orderer.add_killer_move(killer_move2, ply);
        assert_eq!(move_orderer.killer_moves[ply][0], Some(killer_move2));
        assert_eq!(move_orderer.killer_moves[ply][1], Some(killer_move1));

        // Capturing moves should not be added as killers
        let capture_move = Basic { base_move: BaseMove { from: 10, to: 26, capture: true } };
        move_orderer.add_killer_move(capture_move, ply);
        assert_eq!(move_orderer.killer_moves[ply][0], Some(killer_move2));
        assert_eq!(move_orderer.killer_moves[ply][1], Some(killer_move1));
    }

    #[test]
    fn test_is_killer_move() {
        let mut move_orderer = MoveOrderer::new();
        let ply = 3;

        let killer_move1 = Basic { base_move: BaseMove { from: 12, to: 28, capture: false } };
        let killer_move2 = Basic { base_move: BaseMove { from: 11, to: 27, capture: false } };
        let other_move = Basic { base_move: BaseMove { from: 10, to: 26, capture: false } };

        move_orderer.add_killer_move(killer_move1, ply);
        move_orderer.add_killer_move(killer_move2, ply);

        assert!(move_orderer.is_killer_move(&killer_move1, ply));
        assert!(move_orderer.is_killer_move(&killer_move2, ply));
        assert!(!move_orderer.is_killer_move(&other_move, ply));
        assert!(!move_orderer.is_killer_move(&killer_move1, ply + 1));
    }

    #[test]
    fn test_move_scoring() {
        use crate::core::position::Position;

        let mut move_orderer = MoveOrderer::new();
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let position = Position::from(fen);
        let ply = 0;

        // Create sample moves
        let hash_move = Basic { base_move: BaseMove { from: 60, to: 62, capture: false } }; // E1-G1 (castling)
        let capture = Basic { base_move: BaseMove { from: 28, to: 21, capture: true } }; // E5-F6 (capture)
        let killer_move = Basic { base_move: BaseMove { from: 12, to: 28, capture: false } }; // E2-E4
        let quiet_move = Basic { base_move: BaseMove { from: 11, to: 27, capture: false } }; // D2-D4

        // Add killer move
        move_orderer.add_killer_move(killer_move, ply);

        // Add some history for quiet move
        move_orderer.update_history_score(&position, &quiet_move, 3); // depth 3

        let hash_move_score = move_orderer.score_move(&position, &hash_move, Some(hash_move), ply, None);
        let capture_score = move_orderer.score_move(&position, &capture, Some(hash_move), ply, None);
        let killer_score = move_orderer.score_move(&position, &killer_move, Some(hash_move), ply, None);
        let quiet_score = move_orderer.score_move(&position, &quiet_move, Some(hash_move), ply, None);

        // Hash move should be highest
        assert!(hash_move_score > capture_score);
        // Capture should be higher than killer
        assert!(capture_score > killer_score);
        // Killer should be higher than quiet
        assert!(killer_score > quiet_score);
        // Quiet score should be equal to history score
        assert_eq!(quiet_score, 9); // 3*3=9
    }
}
