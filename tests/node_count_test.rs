use chess_engine::position::{Position, NEW_GAME_FEN};
use chess_engine::{fen, node_counter};

use serde_derive::Deserialize;
use std::error::Error;
use std::fs;
use serial_test::serial;

#[derive(Deserialize, Debug)]

struct FenTestCase {
    depth: usize,
    nodes: usize,
    fen: String,
}

#[test]
#[serial]
fn test_fen_new_game_position() {
    let position = Position::from(NEW_GAME_FEN);
    let node_count_stats = node_counter::count_nodes(&position, 6);
    println!("{:?}", node_count_stats);
    assert_eq!(node_count_stats.node_count, 119060324);
}

#[test]
#[serial]
fn test_fens() {
    let test_cases = load_fens().unwrap();
    let mut test_number = 0;
    for test in test_cases {
        let position = fen::parse(test.fen);
        let node_count_stats = node_counter::count_nodes(&position, test.depth as i32);
        assert_eq!(node_count_stats.node_count, test.nodes, "Test {}",  test_number);
        test_number += 1;
    }
}

fn load_fens() -> Result<Vec<FenTestCase>, Box<dyn Error>> {
    let file = fs::read_to_string("tests/test_data/fen_test_data.json")?;
    let test_cases = json5::from_str(file.as_str())?;
    Ok(test_cases)
}
