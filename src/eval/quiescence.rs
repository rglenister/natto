use crate::board::{Board, PieceType};
use crate::eval::evaluation;
use crate::eval::evaluation::PIECE_SCORES;
use crate::move_generator;
use crate::position::Position;
use crate::r#move::{BaseMove, Move};

fn quiescence_search(position: &Position, mut alpha: isize, beta: isize) -> isize {
    let static_eval = evaluation::score_pieces(position);
    if static_eval >= beta {
        return beta;
    }
    if static_eval > alpha {
        alpha = static_eval;
    }

    let capture_moves = generate_sorted_captures(&position);

    for mov in capture_moves {
        // ensure the move is legal
        if let Some(next_position) = position.make_move(&mov) {
            // Skip captures that lose material using SEE
            if !static_exchange_evaluation(position, &mov) {
                continue;
            }
            let score = -quiescence_search(&next_position.0, -beta, -alpha);
            if score >= beta {
                return beta; // Prune and return
            }
            if score > alpha {
                alpha = score;
            }
        }
    }
    alpha
}

fn generate_sorted_captures(position: &Position) -> Vec<Move> {
    let mut capture_moves = move_generator::generate_capture_moves(position);

    // Sort captures using a heuristic, such as MVV-LVA (Most Valuable Victim, Least Valuable Attacker)
    capture_moves.sort_by(|a, b| rank_capture_move(position, a).cmp(&rank_capture_move(position, b)));
    capture_moves
}

fn static_exchange_evaluation(position: &Position, mov: &Move) -> bool {
    let base_move = mov.get_base_move();
    let attacker_value = piece_value(position, base_move.from);
    let victim_value = piece_value(position, base_move.to);

    // Basic check: Is capturing worth it?
    if attacker_value < victim_value {
        return true; // Capturing a stronger piece is always profitable
    }

    // If attacker value >= victim value, ensure the recapture sequence doesn't backfire
    // Simulating material exchange sequence:
    let mut current_material_gain = victim_value - attacker_value;
    let mut current_attackers = find_attackers(position, base_move.to);

    while !current_attackers.is_empty() {
        // Swap attacker/defender perspective for minimax-style evaluation
        current_material_gain = -current_material_gain;

        // Get the next best attacker
        let next_attacker = current_attackers.pop();

        if let Some(next_attacker_piece) = next_attacker {
            // Adjust the material exchange
            current_material_gain -= piece_value(position, next_attacker_piece as usize);
        }

        // If no profitable exchange, stop
        if current_material_gain < 0 {
            return false;
        }
    }

    true // Profitable exchange
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

// Find all attackers on a given square
fn find_attackers(position: &Position, square_index: usize) -> Vec<PieceType> {
    move_generator::generate_moves(position)
        .into_iter()
        .filter(|mv| mv.get_base_move().to == square_index)
        .flat_map(|mv| position.board().get_piece(mv.get_base_move().from))
        .map(|p| p.piece_type)
        .collect()
}
