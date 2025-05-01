use arrayvec::ArrayVec;
use strum::IntoEnumIterator;
use crate::board::{Board, PieceColor, PieceType};
use crate::board::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::eval::evaluation::{evaluate, score_pieces, PIECE_SCORES};
use crate::eval::search::{MAXIMUM_SCORE, MAXIMUM_SEARCH_DEPTH};
use crate::{move_generator, util};
use crate::position::Position;
use crate::r#move::{Move};

include!("../util/generated_macro.rs");

pub fn quiescence_search(position: &Position, depth: isize, alpha: isize, beta: isize) -> isize {
    if move_generator::is_check(position) {
        // If in check: must respond with evasions
        let mut best_score = -MAXIMUM_SCORE + depth;
        for mov in move_generator::generate_moves(position) {
            if let Some(new_position) = position.make_move(&mov) {
                let score = -quiescence_search(&new_position.0, depth + 1, -beta, -alpha);
                best_score = best_score.max(score);
                if best_score >= beta {
                    break;
                }
            }
        }
        return best_score;
    }

    // Static evaluation when not in check
    let mut stand_pat = score_pieces(position);
    if stand_pat >= beta {
        return stand_pat;
    }
    let mut alpha = alpha.max(stand_pat);

    // 1. Captures
    let captures = generate_sorted_captures(position);

    for mov in captures {
        if !good_capture(position, &mov) {
            continue; // Skip bad captures by SEE
        }
        if let Some(next_position) = position.make_move(&mov) {
            let score = -quiescence_search(&next_position.0, depth + 1, -beta, -alpha);
            if score >= beta {
                return score;
            }
            alpha = alpha.max(score);
        }
    }

    // 2. Non-capture checks (optional but very strong tactically)
//    let mut checks = move_generator::generate_checks();
    //checks.sort_by_key(|mv| rank_capture_move(position, mv)); // Optional ordering

    // for mov in checks {
    //     if mov.get_base_move.capture {
    //         continue; // Already handled captures
    //     }
    //     if let Some(next_position) = new_pos.make_move(mv) && !new_pos.in_check() {
    //         let score = -quiescence_search(&new_pos, -beta, -alpha);
    //         if score >= beta {
    //             return score;
    //         }
    //         alpha = alpha.max(score);
    //     }
    // }

    alpha
}

fn good_capture(position: &Position, mov: &Move) -> bool {
    static_exchange_evaluation(position, &mov) >= 0
}

// with delta pruning
fn static_exchange_evaluation(position: &Position, mv: &Move) -> isize {
    let attacked_square = mv.get_base_move().to;
    let attacking_square = mv.get_base_move().from;
    let attacking_piece = piece_on(position, attacking_square);

    let mut gain: ArrayVec<isize, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
    let mut attacked_piece = piece_on(position, attacked_square);
    gain.push(PIECE_SCORES[attacked_piece as usize]);

    let mut occupied = position.board().bitboard_all_pieces();
    let mut attackers = attackers_to(&position, attacked_square, occupied);
    let mut side_to_move = position.side_to_move();

    // Remove moving piece from occupied and attackers
    occupied ^= 1 << attacking_square;
    attackers[side_to_move as usize] ^= 1 << attacking_square;
    if let Some(discovered_attacker_square) = find_discovered_attacker(position, attacked_square as isize, attacking_square as isize, side_to_move, occupied) {
        attackers[side_to_move as usize] ^= 1 << discovered_attacker_square;
    }

    attacked_piece = attacking_piece;
    let mut depth = 0;
    side_to_move = !side_to_move;
    while let Some(next_attacking_square) = select_least_valuable_attacker(position, side_to_move, attackers[side_to_move as usize]) {
        let next_attacking_piece = piece_on(position, next_attacking_square);
        occupied ^= 1 << next_attacking_square;

        // Update attackers (X-rays etc.)
//        attackers = attackers_to(position, target_square, occupied);
        attackers[side_to_move as usize] ^= 1 << next_attacking_square;

        depth += 1;
        let last_gain = gain[depth - 1];
        gain.push(PIECE_SCORES[attacked_piece as usize] - last_gain);

        // **Delta pruning: early abort**
        // if side_to_move == position.side_to_move() {
        //     // Our move: maximize
        //     if gain[depth] < 0 {
        //         break; // Already worse, stop
        //     }
        // } else {
        //     // Opponent's move: minimize
        //     if -gain[depth] <= gain[depth - 1] {
        //         break; // No way to recover, stop
        //     }
        // }

        if let Some(discovered_attacker_square) = find_discovered_attacker(position, attacked_square as isize, next_attacking_square as isize, side_to_move, occupied) {
            attackers[side_to_move as usize] ^= 1 << discovered_attacker_square;
        }
        attacked_piece = next_attacking_piece;
        side_to_move = !side_to_move;
    }

    // Walk back to find best gain
    // while depth > 0 {
    //     gain[depth - 1] = -gain[depth - 1].max(-gain[depth]);
    //     depth -= 1;
    // }
    while depth > 0 {
        if gain[depth - 1] > -gain[depth] {
            gain[depth - 1] = -gain[depth];
        }
        depth -= 1;
    }
    gain[0]
}

fn piece_on(position: &Position, source_square: usize) -> PieceType {
    position.board().get_piece(source_square).unwrap().piece_type
}

fn attackers_to(position: &Position, target_index: usize, occupied: u64) -> [u64; 2] {
    let white_attackers = move_generator::square_attacks_finder(position, PieceColor::White, target_index) & occupied;
    let black_attackers = move_generator::square_attacks_finder(position, PieceColor::Black, target_index) & occupied;
    [white_attackers, black_attackers]
}

fn select_least_valuable_attacker(position: &Position, attacking_color: PieceColor, attackers: u64) -> Option<usize> {
    let bitboards = position.board().bitboards_for_color(attacking_color);
    for piece_type in PieceType::iter() {
        let attackers_with_piece_type = attackers & (bitboards[piece_type as usize]);
        if (attackers_with_piece_type) != 0 {
            return Some(attackers_with_piece_type.trailing_zeros() as usize);
        }
    }
    None
}

fn generate_sorted_captures(position: &Position) -> Vec<Move> {
    let mut capture_moves = move_generator::generate_basic_capture_moves(position);
    // Sort captures using MVV-LVA most valuable - victim least valuable attacker
    capture_moves.sort_by(|a, b| rank_capture_move(position, b).cmp(&rank_capture_move(position, a)));
    capture_moves
}

// Heuristic for capture moves ordering: Most Valuable Victim, Least Valuable Attacker
fn rank_capture_move(position: &Position, mov: &Move) -> isize {
    let base_move = mov.get_base_move();
    let attacker_value = piece_value(position, base_move.from);
    let victim_value = piece_value(position, base_move.to);
    victim_value - attacker_value // Prefer capturing higher value pieces with lower-value ones
}

fn piece_value(position: &Position, square_index: usize) -> isize {
    if position.board().get_piece(square_index).is_none() {
        return 100;
    }
    PIECE_SCORES[position.board().get_piece(square_index).unwrap().piece_type as usize]
}

fn find_discovered_attacker(position: &Position, target_square: isize, previous_attacker_square: isize, side_to_move: PieceColor, occupied: u64) -> Option<isize> {
    if let Some(square_increment) = find_square_increment(target_square, previous_attacker_square) {
        let piece_type = if square_increment.abs() == 8 || square_increment == 0 { Rook } else { Bishop };
        let mut square_index = previous_attacker_square + square_increment;
        while util::on_board(previous_attacker_square, square_index) {
            if (1 << square_index) & occupied != 0 {
                let bitboards_for_color = position.board().bitboards_for_color(side_to_move);
                let bitboard = bitboards_for_color[piece_type as usize] | bitboards_for_color[Queen as usize];
                if (bitboard & (1 << square_index)) != 0 {
                    return Some(square_index);
                }
            }
            square_index += square_increment;
        }
    }
    None
}

fn find_square_increment(from_square: isize, to_square: isize) -> Option<isize> {
    let square_delta = to_square - from_square;
    let distance = util::distance(from_square, to_square);
    let square_increment = square_delta / distance;
    if from_square + square_increment * distance == to_square {
        Some(square_increment)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::max;
    use crate::board::PieceColor::{Black, White};
    use crate::board::PieceType::{Bishop, King, Pawn, Rook};
    use crate::r#move::BaseMove;
    use super::*;
    #[test]
    fn test_rank_capture_move() {
        let fen = "4k3/8/2n2Q2/1P6/8/8/8/2R1K2B w - - 0 1";
        let position: Position = Position::from(fen);
        let moves = generate_sorted_captures(&position);
        assert_eq!(moves.len(), 4);
        assert_eq!(rank_capture_move(&position, &moves[0]), 200);
        assert_eq!(rank_capture_move(&position, &moves[1]), 0);
        assert_eq!(rank_capture_move(&position, &moves[2]), -200);
        assert_eq!(rank_capture_move(&position, &moves[3]), -600);
    }

    #[test]
    fn test_generate_sorted_captures() {
        let fen = "4k3/8/2n2Q2/1P6/8/8/8/2R1K2B w - - 0 1";
        let position: Position = Position::from(fen);
        let moves = generate_sorted_captures(&position);
        assert_eq!(moves.len(), 4);
        assert_eq!(moves[0].get_base_move().from, sq!("b5"));
        assert_eq!(moves[1].get_base_move().from, sq!("h1"));
        assert_eq!(moves[2].get_base_move().from, sq!("c1"));
        assert_eq!(moves[3].get_base_move().from, sq!("f6"));
    }

    #[test]
    fn test_piece_value() {
        let position: Position = Position::default();
        assert_eq!(piece_value(&position, sq!("e1")), 10000);
        assert_eq!(piece_value(&position, sq!("d1")), 900);
        assert_eq!(piece_value(&position, sq!("h2")), 100);
        assert_eq!(piece_value(&position, sq!("h8")), 500);
    }

    #[test]
    fn test_attackers_to() {
        let fen = "4k3/1p6/2b4r/1B1Pn3/8/8/8/2R1K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let occupied = position.board().bitboard_all_pieces();
        let attackers = attackers_to(&position, sq!("c6"), occupied);

        let white_attackers = attackers[White as usize];
        assert_eq!(white_attackers.count_ones(), 3);
        assert_ne!(white_attackers & (1 << sq!("b5")), 0);
        assert_ne!(white_attackers & (1 << sq!("c1")), 0);
        assert_ne!(white_attackers & (1 << sq!("d5")), 0);

        let black_attackers = attackers[Black as usize];
        assert_eq!(black_attackers.count_ones(), 3);
        assert_ne!(black_attackers & (1 << sq!("b7")), 0);
        assert_ne!(black_attackers & (1 << sq!("e5")), 0);
        assert_ne!(black_attackers & (1 << sq!("h6")), 0);
    }

    #[test]
    fn test_select_least_valuable_attacker() {
        let fen = "4k3/1p6/2b4r/1B1Pn3/8/8/8/2R1K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let occupied = position.board().bitboard_all_pieces();
        let attackers = attackers_to(&position, sq!("c6"), occupied);

        let white_attackers = attackers[White as usize];
        let square_index = select_least_valuable_attacker(&position, White, white_attackers);
        assert_eq!(square_index, Some(sq!("d5")));

        let black_attackers = attackers[Black as usize];
        let square_index = select_least_valuable_attacker(&position, Black, black_attackers);
        assert_eq!(square_index, Some(sq!("b7")));
    }

    #[test]
    fn test_static_exchange_evaluation() {
        let fen = "4k3/8/2n5/1P6/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 300); // should be +300?

        let fen = "4k3/1p6/2p5/1B6/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), -200); // should be -200

        let fen = "4k3/1p6/2b5/1B6/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 0); // should be zero

        let fen = "4k3/1p6/2b5/1B1P4/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("d5"), to: sq!("c6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 300); // should be +300
    }

    #[test]
    fn test_see_double_rooks_attacking_double_rooks() {
        // a winning capture that static SEE misses because the doubled rook isn't directly attacking the enemy rook
        let fen = "3r4/4bk2/8/8/8/8/3R4/3RK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);
        
        // undoubling the rooks produces the correct result
        let fen = "R2r4/4bk2/8/8/8/8/3R4/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);
        
        // a losing capture because SEE misses the doubled rooks
        let fen = "3r4/4bk2/3P4/8/8/8/3R4/3RK3 b - - 0 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), -200);
        
        // a winning capture because SE
        let fen = "3r4/4bk2/3P4/8/8/8/8/3RK3 b - - 0 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 100);

        let fen = "3r4/3br3/7k/8/3R4/3R4/8/3QK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov = Move::Basic { base_move: BaseMove { from: sq!("d4"), to: sq!("d7"), capture: true }};
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);
    }

    // #[test]
    // fn test_score_array() {
    //     let mut gain = [0; 7];
    //     gain[0] = 1;
    //     gain[1] = 8;
    //
    //     let mut depth = 2;
    //
    //     while depth > 0 {
    //         gain[depth - 1] = -gain[depth - 1].max(-gain[depth]);
    //         depth -= 1;
    //     }        // while depth > 1 {
    //     //     depth -= 1;
    //     //     if gain[depth - 1] > -gain[depth] {
    //     //         gain[depth - 1] = -gain[depth];
    //     //     };
    //     // }
    //     assert_eq!(-gain[0].max(-gain[1]), 22);
    // }
    #[test]
    fn test_find_discovered_attacker() {
        let fen = "3r4/4bk2/8/8/8/8/3R4/3RK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let square_index = find_discovered_attacker(&position, sq!("d8"), sq!("d2"), White, position.board().bitboard_all_pieces());
        assert_eq!(square_index, Some(sq!("d1")));

        let fen = "4k3/5r2/8/3B3b/8/1Q6/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let square_index = find_discovered_attacker(&position, sq!("f7"), sq!("d5"), White, position.board().bitboard_all_pieces());
        assert_eq!(square_index, Some(sq!("b3")));
    }
    #[test]
    fn test_find_square_increment() {
        assert_eq!(find_square_increment(sq!("a1"), sq!("a2")), Some(8));
        assert_eq!(find_square_increment(sq!("a1"), sq!("a8")), Some(8));
        assert_eq!(find_square_increment(sq!("a8"), sq!("a1")), Some(-8));
        assert_eq!(find_square_increment(sq!("a1"), sq!("a2")), Some(8));
        assert_eq!(find_square_increment(sq!("a1"), sq!("b2")), Some(9));
        assert_eq!(find_square_increment(sq!("a2"), sq!("b1")), Some(-7));
        assert_eq!(find_square_increment(sq!("a2"), sq!("b5")), None);
        assert_eq!(find_square_increment(sq!("h8"), sq!("h6")), Some(-8));
        assert_eq!(find_square_increment(sq!("h8"), sq!("g1")), None);
        assert_eq!(find_square_increment(sq!("a6"), sq!("c4")), Some(-7));
        assert_eq!(find_square_increment(sq!("c4"), sq!("a6")), Some(7));
    }
}

