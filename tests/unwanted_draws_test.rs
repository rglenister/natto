use natto::uci::uci_util;

use natto::search::transposition_table::TranspositionTable;

/// Integration tests for unwanted draws on lichess.org

fn run_test(
    depth: usize,
    base_moves: &str,
    additional_moves: Vec<&str>,
    unwanted_drawing_move: &str,
    expected_non_drawing_move: &str,
) {
    let uci_command = "position startpos moves";
    let uci_positions = additional_moves
        .iter()
        .map(|move_str| format!("{} {} {}", uci_command, base_moves, move_str))
        .collect::<Vec<String>>();

    let transposition_table = TranspositionTable::new(500);
    let depth_str = format!("depth {}", depth);

    uci_positions.iter().for_each(|uci_position| {
        uci_util::run_uci_position_using_t_table(uci_position, &depth_str, &transposition_table);
    });

    let end_position = uci_positions.last().unwrap();

    let search_results =
        uci_util::run_uci_position_using_t_table(&end_position, &depth_str, &transposition_table);

    assert_ne!(search_results.pv[0].to_string(), unwanted_drawing_move);
    assert_eq!(search_results.pv[0].to_string(), expected_non_drawing_move);
}

/// https://lichess.org/exHmRrqX/white#36
#[test]
fn test_engine_does_not_play_drawing_move() {
    let base_moves = "d2d4 e7e6 e2e4 d7d5 b1c3 d5e4 c3e4 c7c6 g1f3 f7f6 f1d3 g7g5 e1g1 g5g4 f3h4 f6f5 e4g5 f8h6 g5e6 c8e6 d3f5 e6f5 h4f5 h6c1 d1g4 c1b2 a1e1 e8d7 f5g7 d7d6 g4g3 d6d7";
    run_test(8, base_moves, vec!["", "g3g4 d7d6", "g3g4 d7d6 g4g3 d6d7"], "g3-g4", "g3-h3");
}

/// https://lichess.org/1yFW1xrydnTb
#[test]
fn test_opponent_is_unable_to_play_drawing_move() {
    let base_moves = "e2e4 e7e6 d2d3 d7d5 d1e2 g8e7 g1f3 c7c5 g2g3 g7g6 h2h4 h7h6 b1c3 d5d4 c3d1 f8g7 h4h5 g6g5 f1h3 e8g8 c1d2 e6e5 h3c8 d8c8 c2c4 b8c6 a1c1 b7b6 a2a3 g7f6 b2b4 g8g7 b4b5 c6d8 a3a4 a7a6 f3h2 a6b5 a4b5 a8a2 h2g4 e7g8 c1b1 d8e6 e2f3 c8a8 b1b2 a2a1 h1f1 a8a3 b2c2 a3b3 c2c1 a1a2 f1h1 f8a8 h1h3 a2c2 c1c2 b3c2 f3e2 a8a2 f2f3 c2b1 h3h1 a2c2 e1f2 b1a2 f2e1";
    run_test(10, base_moves, vec!["", "a2b1 e1f2", "a2b1 e1f2 b1a2 f2e1"], "a2-b1", "a2-a1");
}
