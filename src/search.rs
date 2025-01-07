use std::sync::atomic::{AtomicUsize, Ordering};
use crate::move_generator;
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

fn search(position: Position, depth: u32, max_depth: i32) {

}

fn do_search(position: &Position, depth: u32, max_depth: i32) {
    increment_node_counter();

}

#[cfg(test)]
mod tests {
    use serde_derive::Deserialize;

    use std::error::Error;
    use crate::{fen, move_generator};

    use std::fs;
    use crate::position::Position;

    #[derive(Deserialize, Debug)]

    struct FenTestCase {
        depth: usize,
        nodes: usize,
        fen: String,
    }

    #[test]
    fn test_fen_1() {
        let position = fen::parse("r6r/1b2k1bq/8/8/7B/8/8/R3K2R b KQ - 3 2".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 8);
    }
    #[test]
    fn test_fen_2() {
        let position = fen::parse("8/8/8/2k5/2pP4/8/B7/4K3 b - d3 0 3".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 8);
    }

    #[test]
    fn test_fen_3() {
        let position = fen::parse("r1bqkbnr/pppppppp/n7/8/8/P7/1PPPPPPP/RNBQKBNR w KQkq - 2 2".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 19);
    }

    #[test]
    fn test_fen_4() {
        let position = fen::parse("r3k2r/p1pp1pb1/bn2Qnp1/2qPN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQkq - 3 2".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn test_fen_5() {
        let position = fen::parse("2kr3r/p1ppqpb1/bn2Qnp1/3PN3/1p2P3/2N5/PPPBBPPP/R3K2R b KQ - 3 2".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 44);
    }

    #[test]
    fn test_fen_6() {
        let position = fen::parse("rnb2k1r/pp1Pbppp/2p5/q7/2B5/8/PPPQNnPP/RNB1K2R w KQ - 3 9".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 39);
    }
    #[test]
    fn test_fen_7() {
        let position = fen::parse("2r5/3pk3/8/2P5/8/2K5/8/8 w - - 5 4".to_string());
        let moves = move_generator::generate(&position);
        let mut count = 0;
        for chess_move in moves {
            let option = position.make_move(&chess_move);
            if (option.is_some()) {
                count += 1;
            }
        }
        assert_eq!(count, 9);
    }

    //    #[test]
//     fn test_fens() {
//         let test_cases = load_fens().unwrap();
//         let mut test_number = 0;
// //        assert_eq!(test_cases.len(), 7);
//         for test in test_cases {
//             let mut count: usize = 0;
//             let position = fen::parse(test.fen);
//             let moves = move_generator::generate(&position);
//             for chess_move in moves {
//                 let option = position.make_move(&chess_move);
//                 if (option.is_some()) {
//                     count += 1;
//                 }
//             }
//             assert_eq!(count, test.nodes);
//             test_number += 1;
//         }
//     }
//
//     fn load_fens() -> Result<Vec<FenTestCase>, Box<dyn Error>> {
//         let file = fs::read_to_string("src/test_data/fen_test_data.json")?;
//         let test_cases = json5::from_str(file.as_str())?;
//         Ok(test_cases)
//     }
}
