use strum::IntoEnumIterator;
use crate::board::{Board, PieceColor, PieceType};
use crate::board::PieceType::{Knight, Pawn};
use crate::eval::evaluation::{evaluate, score_pieces, PIECE_SCORES};
use crate::eval::search::{MAXIMUM_SCORE};
use crate::move_generator;
use crate::position::Position;
use crate::r#move::{Move};

include!("../util/generated_macro.rs");

fn quiescence_search(position: &Position, alpha: isize, beta: isize) -> isize {
    if move_generator::is_check(position) {
        // If in check: must respond with evasions
        let mut best_score = -MAXIMUM_SCORE;
        for mov in move_generator::generate_moves(position) {
            if let Some(new_position) = position.make_move(&mov) {
                let score = -quiescence_search(&new_position.0, -beta, -alpha);
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
            let score = -quiescence_search(&next_position.0, -beta, -alpha);
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

fn static_exchange_eval(position: &Position, mov: &Move) -> isize {
    let base_move = mov.get_base_move();
    let target_square = base_move.to;
    let captured_piece = position.board().get_piece(target_square).unwrap().piece_type;

    let mut gain = Vec::new();
    gain.push(PIECE_SCORES[captured_piece as usize]); // First gain: captured piece value

    let mut occupied = position.board().bitboard_all_pieces();
    let mut attackers = attackers_to(position, target_square, occupied);

    let mut side_to_move = position.side_to_move();

    // Remove the moving piece from occupied and attackers
    occupied ^= 1 << base_move.from;
    attackers[0] ^= 1 << base_move.from;

    let mut depth = 0;

    loop {
        // Find the least valuable attacker for side to move
        if let Some(from_square) = select_least_valuable_attacker(position, position.side_to_move(), attackers[side_to_move as usize]) {
            // Update occupied and attackers
            let piece_type = position.board().get_piece(from_square).unwrap().piece_type;
            occupied ^= 1 << from_square;

            // Update attackers due to new X-ray attacks (like rooks, bishops, queens behind pawns)
            attackers = attackers_to(position, target_square, occupied);

            depth += 1;
            let last_gain = gain[depth - 1];
            gain.push(PIECE_SCORES[piece_type as usize] - last_gain);

            // Switch side
            side_to_move = side_to_move.opposite();
        } else {
            break; // No more attackers
        }
    }

    // Now, walk back through the gains to find best result
    while depth > 0 {
        gain[depth - 1] = -gain[depth - 1].max(-gain[depth]);
        depth -= 1;
    }

    gain[0]
}
fn good_capture(position: &Position, mov: &Move) -> bool {
    static_exchange_eval(position,&mov) >= 0
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
    let mut capture_moves = move_generator::generate_capture_moves(position);
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
    PIECE_SCORES[position.board().get_piece(square_index).unwrap().piece_type as usize]
}

#[cfg(test)]
mod tests {
    use crate::board::PieceColor::{Black, White};
    use crate::board::PieceType::{Bishop, King, Pawn, Rook};
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
        // let fen = "4k3/8/2n5/1P6/8/8/8/4K3 w - - 1 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), true);
        //
        // let fen = "4k3/1p6/2p5/1B6/8/8/8/4K3 w - - 1 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false);
        //
        // let fen = "4k3/1p6/2b5/1B6/8/8/8/4K3 w - - 1 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false);
        //
        // let fen = "4k3/1p6/2b5/1B1P4/8/8/8/4K3 w - - 1 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("d5"), to: sq!("c6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), true);
    }

    #[test]
    fn test_see_double_rooks_attacking_double_rooks() {
        // // a winning capture that SEE misses because the doubled rook isn't directly attacking the enemy rook
        // let fen = "3r4/4bk2/8/8/8/8/3R4/3RK3 w - - 0 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false);
        //
        // // undoubling the rooks produces the correct result
        // let fen = "R2r4/4bk2/8/8/8/8/3R4/4K3 w - - 0 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), true); // this really should be false!!!!
        //
        // // a losing capture because SEE misses the doubled rooks
        // let fen = "3r4/4bk2/3P4/8/8/8/3R4/3RK3 b - - 0 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false);
        //
        // // a winning capture because SE
        // let fen = "3r4/4bk2/3P4/8/8/8/8/3RK3 b - - 0 1";
        // let position: Position = Position::from(fen);
        // let mov = Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true }};
        // assert_eq!(static_exchange_evaluation(&position, &mov), false); // this really should be true!!!!
    }
}