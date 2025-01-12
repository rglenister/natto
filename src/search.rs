use std::sync::atomic::{AtomicUsize, Ordering};
use crate::position::Position;

// Define a static atomic counter
static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn increment_node_counter() {
    NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn get_node_count() -> usize {
    NODE_COUNTER.load(Ordering::SeqCst)
}

fn reset_node_counter() {
    NODE_COUNTER.store(0, Ordering::SeqCst);
}

fn search(position: &Position, max_depth: i32) {
    do_search(position, 0, max_depth);
}

fn do_search(position: &Position, depth: u32, max_depth: i32) {
    increment_node_counter();

}

#[cfg(test)]
mod tests {
    use serial_test::serial;
use serde_derive::Deserialize;

    use std::error::Error;
    use crate::{fen, move_generator, node_counter};

    use std::fs;
    use crate::position::{Position, NEW_GAME_FEN};

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
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 119060324);
    }

    #[test]
    #[serial]
    fn test_fen_1() {
        let position = fen::parse("r6r/1b2k1bq/8/8/7B/8/8/R3K2R b KQ - 3 2".to_string());
        let moves = move_generator::generate(&position);
        let count = moves.iter().filter_map(|chess_move| position.make_move(chess_move)).count();
        assert_eq!(count, 8);
    }
    #[test]
    #[serial]
    fn test_fen_2() {
        let position = fen::parse("8/8/8/2k5/2pP4/8/B7/4K3 b - d3 0 3".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 8);
    }

    #[test]
    #[serial]
    fn test_fen_3() {
        let position = fen::parse("r1bqkbnr/pppppppp/n7/8/8/P7/1PPPPPPP/RNBQKBNR w KQkq - 2 2".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 19);
    }

    #[test]
    #[serial]
    fn test_fen_4() {
        let position = fen::parse("r3k2r/p1pp1pb1/bn2Qnp1/2qPN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQkq - 3 2".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 5);
    }

    #[test]
    #[serial]
    fn test_fen_5() {
        let position = fen::parse("2kr3r/p1ppqpb1/bn2Qnp1/3PN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQ - 3 2".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 44);
    }

    #[test]
    #[serial]
    fn test_fen_6() {
        let position = fen::parse("rnb2k1r/pp1Pbppp/2p5/q7/2B5/8/PPPQNnPP/RNB1K2R w KQ - 3 9".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 39);
    }

    #[test]
    #[serial]
    fn test_fen_7() {
        let position = fen::parse("2r5/3pk3/8/2P5/8/2K5/8/8 w - - 5 4".to_string());
        let count = node_counter::count_nodes(&position, 1);
        assert_eq!(count, 9);
    }

    #[test]
    #[serial]
    fn test_fen_8() {
        let position = fen::parse("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8".to_string());
        let count = node_counter::count_nodes(&position, 3);
        assert_eq!(count, 62379);
    }

    #[test]
    #[serial]
    fn test_fen_9() {
        let position = fen::parse("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10".to_string());
        let count = node_counter::count_nodes(&position, 3);
        assert_eq!(count, 89890);
    }

    #[test]
    #[serial]
    fn test_fen_10() {
        // exception
        let position = fen::parse("3k4/3p4/8/K1P4r/8/8/8/8 b - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 1134888);
    }

    #[test]
    #[serial]
    fn test_fen_11() {
        let position = fen::parse("8/8/4k3/8/2p5/8/B2P2K1/8 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 1015133);
    }

    #[test]
    #[serial]
    fn test_fen_12() {
        let position = fen::parse("8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 1440467);
    }

    #[test]
    #[serial]
    fn test_fen_13() {
        // castling flags??
        let position = fen::parse("5k2/8/8/8/8/8/8/4K2R w K - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 661072);
    }

    #[test]
    #[serial]
    fn test_fen_14() {
        // castling flags??
        let position = fen::parse("3k4/8/8/8/8/8/8/R3K3 w Q - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 803711);
    }

    #[test]
    #[serial]
    fn test_fen_15() {
        // ok
        let position = fen::parse("r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 4);
        assert_eq!(count, 1274206);
    }

    #[test]
    #[serial]
    fn test_fen_16() {
        // ok
        let position = fen::parse("r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 4);
        assert_eq!(count, 1720476);
    }

    #[test]
    #[serial]
    fn test_fen_17() {
        // castling flags?
        let position = fen::parse("2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 3821001);
    }

    #[test]
    #[serial]
    fn test_fen_18() {
        let position = fen::parse("8/8/1P2K3/8/2n5/1q6/8/5k2 b - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 5);
        assert_eq!(count, 1004658);
    }

    #[test]
    #[serial]
    fn test_fen_19() {
        let position = fen::parse("4k3/1P6/8/8/8/8/K7/8 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 217342);
    }

    #[test]
    #[serial]
    fn test_fen_20() {
        let position = fen::parse("8/P1k5/K7/8/8/8/8/8 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 92683);
    }

    #[test]
    #[serial]
    fn test_fen_21() {
        let position = fen::parse("K1k5/8/P7/8/8/8/8/8 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 6);
        assert_eq!(count, 2217);
    }

    #[test]
    #[serial]
    fn test_fen_22() {
        let position = fen::parse("8/k1P5/8/1K6/8/8/8/8 w - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 7);
        assert_eq!(count, 567584);
    }

    #[test]
    #[serial]
    fn test_fen_23() {
        let position = fen::parse("8/8/2k5/5q2/5n2/8/5K2/8 b - - 0 1".to_string());
        let count = node_counter::count_nodes(&position, 4);
        assert_eq!(count, 23527);
    }

    #[test]
    #[serial]
    fn test_fens() {
        let test_cases = load_fens().unwrap();
        let mut test_number = 0;
        for test in test_cases {
            let position = fen::parse(test.fen);
            let count = node_counter::count_nodes(&position, test.depth as i32);
            assert_eq!(count, test.nodes, "Test {}",  test_number);
            test_number += 1;
        }
    }

    fn load_fens() -> Result<Vec<FenTestCase>, Box<dyn Error>> {
        let file = fs::read_to_string("src/test_data/fen_test_data.json")?;
        let test_cases = json5::from_str(file.as_str())?;
        Ok(test_cases)
    }
}
