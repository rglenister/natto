use crate::core::move_gen;
use crate::core::piece::{PieceColor, PieceType};
use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::eval::evaluation::{score_position, PIECE_SCORES};
use crate::search::move_ordering::order_quiescence_moves;
use crate::search::negamax::{SearchContext, MAXIMUM_SCORE, MAXIMUM_SEARCH_DEPTH};
use crate::utils::util;
use arrayvec::ArrayVec;
use strum::IntoEnumIterator;

include!("../utils/generated_macro.rs");

pub const QUIESCENCE_MAXIMUM_SCORE: isize = MAXIMUM_SCORE / 2;

pub fn quiescence_search(
    position: &mut Position,
    ply: isize,
    search_context: &SearchContext,
    alpha: isize,
    beta: isize,
) -> isize {
    if ply > 100 {
        return 0;
    }
    search_context.node_counter.increment();
    if move_gen::is_check(position) {
        // If in check: must respond with evasions
        let mut best_score = -QUIESCENCE_MAXIMUM_SCORE + ply;
        for mov in move_gen::generate_moves(position) {
            if let Some(undo_move_info) = position.make_move(&mov) {
                let score = -quiescence_search(position, ply + 1, search_context, -beta, -alpha);
                position.unmake_move(&undo_move_info);
                best_score = best_score.max(score);
                if best_score >= beta {
                    break;
                }
            }
        }
        return best_score;
    }

    // Static evaluation when not in check
    let stand_pat = score_position(position);
    if stand_pat >= beta {
        return stand_pat;
    }
    let mut alpha = alpha.max(stand_pat);

    // 1. Captures
    let captures = generate_sorted_quiescence_moves(position);

    for mov in captures {
        if matches!(mov, Move::Basic { .. }) && !good_capture(position, &mov) {
            continue; // Skip bad captures by SEE
        }
        if let Some(undo_move_info) = position.make_move(&mov) {
            let score = -quiescence_search(position, ply + 1, search_context, -beta, -alpha);
            position.unmake_move(&undo_move_info);
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
    static_exchange_evaluation(position, mov) >= 0
}

// with delta pruning
fn static_exchange_evaluation(position: &Position, mv: &Move) -> isize {
    let attacked_square = mv.get_base_move().to as usize;
    let attacking_square = mv.get_base_move().from as usize;
    let attacking_piece = piece_on(position, attacking_square);

    let mut gain: ArrayVec<isize, MAXIMUM_SEARCH_DEPTH> = ArrayVec::new();
    let mut attacked_piece = piece_on(position, attacked_square);
    gain.push(PIECE_SCORES[attacked_piece as usize]);

    let mut occupied = position.board().bitboard_all_pieces();
    let mut attackers = attackers_to(position, attacked_square, occupied);
    let mut side_to_move = position.side_to_move();

    // Remove moving piece from occupied and attackers
    occupied ^= 1 << attacking_square;
    attackers[side_to_move as usize] ^= 1 << attacking_square;
    if let Some(discovered_attacker_square) = find_discovered_attacker(
        position,
        attacked_square as isize,
        attacking_square as isize,
        side_to_move,
        occupied,
    ) {
        attackers[side_to_move as usize] ^= 1 << discovered_attacker_square;
    }

    attacked_piece = attacking_piece;
    let mut depth = 0;
    side_to_move = !side_to_move;
    while let Some(next_attacking_square) =
        select_least_valuable_attacker(position, side_to_move, attackers[side_to_move as usize])
    {
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

        if let Some(discovered_attacker_square) = find_discovered_attacker(
            position,
            attacked_square as isize,
            next_attacking_square as isize,
            side_to_move,
            occupied,
        ) {
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
    let white_attackers =
        move_gen::square_attacks_finder(position, PieceColor::White, target_index) & occupied;
    let black_attackers =
        move_gen::square_attacks_finder(position, PieceColor::Black, target_index) & occupied;
    [white_attackers, black_attackers]
}

fn select_least_valuable_attacker(
    position: &Position,
    attacking_color: PieceColor,
    attackers: u64,
) -> Option<usize> {
    let bitboards = position.board().bitboards_for_color(attacking_color);
    for piece_type in PieceType::iter() {
        let attackers_with_piece_type = attackers & (bitboards[piece_type as usize]);
        if (attackers_with_piece_type) != 0 {
            return Some(attackers_with_piece_type.trailing_zeros() as usize);
        }
    }
    None
}

fn generate_sorted_quiescence_moves(position: &Position) -> Vec<Move> {
    let mut quiescence_moves = move_gen::generate_moves_for_quiescence(position);
    order_quiescence_moves(position, &mut quiescence_moves);
    quiescence_moves
}

fn find_discovered_attacker(
    position: &Position,
    target_square: isize,
    previous_attacker_square: isize,
    side_to_move: PieceColor,
    occupied: u64,
) -> Option<isize> {
    if let Some(square_increment) = find_square_increment(target_square, previous_attacker_square) {
        let piece_type = if square_increment.abs() == 8 || square_increment == 0 {
            PieceType::Rook
        } else {
            PieceType::Bishop
        };
        let mut square_index = previous_attacker_square + square_increment;
        while util::on_board(previous_attacker_square, square_index) {
            if (1 << square_index) & occupied != 0 {
                let bitboards_for_color = position.board().bitboards_for_color(side_to_move);
                let bitboard = bitboards_for_color[piece_type as usize]
                    | bitboards_for_color[PieceType::Queen as usize];
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
    let square_increment = square_delta / distance as isize;
    if from_square + square_increment * distance as isize == to_square {
        Some(square_increment)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::piece::PieceColor::{Black, White};
    use crate::core::r#move::BaseMove;

    #[test]
    fn test_generate_sorted_captures() {
        let fen = "4k3/8/2n2Q2/1P6/8/8/8/2R1K2B w - - 0 1";
        let position: Position = Position::from(fen);
        let moves = generate_sorted_quiescence_moves(&position);
        assert_eq!(moves.len(), 4);
        assert_eq!(moves[0].get_base_move().from, sq!("b5"));
        assert_eq!(moves[1].get_base_move().from, sq!("h1"));
        assert_eq!(moves[2].get_base_move().from, sq!("c1"));
        assert_eq!(moves[3].get_base_move().from, sq!("f6"));
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
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);

        let fen = "4k3/1p6/2p5/1B6/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), -200);

        let fen = "4k3/1p6/2b5/1B6/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("b5"), to: sq!("c6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 0);

        let fen = "4k3/1p6/2b5/1B1P4/8/8/8/4K3 w - - 1 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("d5"), to: sq!("c6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);
    }

    #[test]
    fn test_see_double_rooks_attacking_double_rooks() {
        // a winning capture that static SEE misses because the doubled rook isn't directly attacking the enemy rook
        let fen = "3r4/4bk2/8/8/8/8/3R4/3RK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);

        // undoubling the rooks produces the correct result
        let fen = "R2r4/4bk2/8/8/8/8/3R4/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("d2"), to: sq!("d8"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);

        // a losing capture because SEE misses the doubled rooks
        let fen = "3r4/4bk2/3P4/8/8/8/3R4/3RK3 b - - 0 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), -200);

        // a winning capture because SE
        let fen = "3r4/4bk2/3P4/8/8/8/8/3RK3 b - - 0 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("e7"), to: sq!("d6"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 100);

        let fen = "3r4/3br3/7k/8/3R4/3R4/8/3QK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let mov =
            Move::Basic { base_move: BaseMove { from: sq!("d4"), to: sq!("d7"), capture: true } };
        assert_eq!(static_exchange_evaluation(&position, &mov), 300);
    }

    #[test]
    fn test_find_discovered_attacker() {
        let fen = "3r4/4bk2/8/8/8/8/3R4/3RK3 w - - 0 1";
        let position: Position = Position::from(fen);
        let square_index = find_discovered_attacker(
            &position,
            sq!("d8"),
            sq!("d2"),
            White,
            position.board().bitboard_all_pieces(),
        );
        assert_eq!(square_index, Some(sq!("d1")));

        let fen = "4k3/5r2/8/3B3b/8/1Q6/8/4K3 w - - 0 1";
        let position: Position = Position::from(fen);
        let square_index = find_discovered_attacker(
            &position,
            sq!("f7"),
            sq!("d5"),
            White,
            position.board().bitboard_all_pieces(),
        );
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

    mod q_search {
        use super::*;
        use crate::core::r#move::Move::{Basic, EnPassant, Promotion};
        use crate::search::move_ordering::MoveOrderer;
        use crate::search::negamax::SearchParams;
        use crate::search::transposition_table::TranspositionTable;
        use std::sync::Arc;

        fn create_search_context(transposition_table: &mut TranspositionTable) -> SearchContext<'_> {
            SearchContext::new(
                transposition_table,
                &SearchParams { allocated_time_millis: 0, max_depth: 0, max_nodes: 0 },
                Arc::new(Default::default()),
                vec![],
                MoveOrderer::new(),
                0,
            )
        }

        #[test]
        fn test_only_kings() {
            let fen = "4k3/8/8/8/8/8/8/4K3 w - - 0 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, -1);
        }

        #[test]
        fn test_queening_by_capturing() {
            let fen = "4q3/3P4/8/8/8/7k/8/4K3 w - - 0 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, 903);
        }

        #[test]
        fn test_multiple_capture_options() {
            let fen = "5rk1/2q2pbp/1p2pnp1/pP1pP3/P2P1P2/2N2BN1/6PP/R2Q1RK1 w - - 0 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, 961);
        }

        #[test]
        fn test_white_king_under_attack() {
            let fen = "8/8/8/8/4k3/8/8/4K2r w - - 0 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, -550);
        }

        #[test]
        fn test_no_good_capture() {
            let fen = "r4rk1/pp3ppp/2n1b3/3p4/3P4/2N5/PP2BPPP/3R1RK1 b - - 1 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, -14);
        }

        #[test]
        fn test_good_capture() {
            let fen = "r4rk1/pp3ppp/2n1b3/3q4/3P4/2N5/PP2BPPP/3R1RK1 b - - 1 1";
            let mut position: Position = Position::from(fen);
            let score = quiescence_search(
                &mut position,
                0,
                &create_search_context(&mut TranspositionTable::new_using_config()),
                -MAXIMUM_SCORE,
                MAXIMUM_SCORE,
            );
            assert_eq!(score, 788);
        }

        #[test]
        fn test_generated_sorted_quiescence_moves() {
            let fen = "8/4k3/Q7/8/4Pp2/8/3K2p1/r1N2Q1R b - e3 0 1";
            let position: Position = Position::from(fen);
            let quiescence_moves = generate_sorted_quiescence_moves(&position);
            assert_eq!(quiescence_moves.len(), 15);

            println!("{:?}", quiescence_moves);

            let move_0 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("f1"), capture: true },
                promote_to: PieceType::Queen,
            };
            assert_eq!(quiescence_moves[0], move_0);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_0), 17);

            let move_1 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("f1"), capture: true },
                promote_to: PieceType::Rook,
            };
            assert_eq!(quiescence_moves[1], move_1);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_1), 13);

            let move_2 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("h1"), capture: true },
                promote_to: PieceType::Queen,
            };
            assert_eq!(quiescence_moves[2], move_2);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_2), 13);

            let move_3 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("f1"), capture: true },
                promote_to: PieceType::Knight,
            };
            assert_eq!(quiescence_moves[3], move_3);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_3), 11);

            let move_4 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("f1"), capture: true },
                promote_to: PieceType::Bishop,
            };
            assert_eq!(quiescence_moves[4], move_4);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_4), 11);

            let move_5 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("h1"), capture: true },
                promote_to: PieceType::Rook,
            };
            assert_eq!(quiescence_moves[5], move_5);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_5), 9);

            let move_6 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("g1"), capture: false },
                promote_to: PieceType::Queen,
            };
            assert_eq!(quiescence_moves[6], move_6);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_6), 8);

            let move_7 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("h1"), capture: true },
                promote_to: PieceType::Knight,
            };
            assert_eq!(quiescence_moves[7], move_7);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_7), 7);

            let move_8 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("h1"), capture: true },
                promote_to: PieceType::Bishop,
            };
            assert_eq!(quiescence_moves[8], move_8);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_8), 7);

            let move_9 =
                Basic { base_move: BaseMove { from: sq!("a1"), to: sq!("a6"), capture: true } };
            assert_eq!(quiescence_moves[9], move_9);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_9), 4);

            let move_10 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("g1"), capture: false },
                promote_to: PieceType::Rook,
            };
            assert_eq!(quiescence_moves[10], move_10);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_10), 4);

            let move_11 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("g1"), capture: false },
                promote_to: PieceType::Knight,
            };
            assert_eq!(quiescence_moves[11], move_11);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_11), 2);

            let move_12 = Promotion {
                base_move: BaseMove { from: sq!("g2"), to: sq!("g1"), capture: false },
                promote_to: PieceType::Bishop,
            };
            assert_eq!(quiescence_moves[12], move_12);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_12), 2);

            let move_13 = EnPassant {
                base_move: BaseMove { from: sq!("f4"), to: sq!("e3"), capture: true },
                capture_square: sq!("e4"),
            };
            assert_eq!(quiescence_moves[13], move_13);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_13), 0);

            let move_14 =
                Basic { base_move: BaseMove { from: sq!("a1"), to: sq!("c1"), capture: true } };
            assert_eq!(quiescence_moves[14], move_14);
            assert_eq!(MoveOrderer::mvv_lva_score(&position, &move_14), -2);
        }
    }
}
