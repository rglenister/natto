use crate::core::move_gen;
use crate::core::position::Position;
use crate::core::r#move::Move;
use crate::utils::node_counter::{NodeCountStats, NodeCounter};
use rayon::prelude::*;

const NODE_COUNTS_AT_DEPTH: [usize; 11] = [
    1,
    20,
    400,
    8902,
    197281,
    4865609,
    119060324,
    3195901860,
    84998978956,
    2439530234167,
    69352859712417,
];

pub fn perf_t() {
    for (depth, node_count) in NODE_COUNTS_AT_DEPTH.iter().enumerate() {
        let stats = count_nodes(&Position::new_game(), depth);
        assert_eq!(*node_count, stats.node_count);
        println!(
            "Depth {} nodes {} nps {}",
            depth, stats.node_count, stats.nodes_per_second
        );
    }
}

pub fn count_nodes(position: &Position, max_depth: usize) -> NodeCountStats {
    let mut node_counter = NodeCounter::new();
    node_counter.reset();
    let count = do_count_nodes::<false>(&mut position.clone(), 0, max_depth);
    node_counter.add(count);
    node_counter.stats()
}

fn do_count_nodes<const USE_PARALLEL_ITERATOR: bool>(
    position: &mut Position,
    depth: usize,
    max_depth: usize,
) -> usize {
    fn process_move<F>(
        position: &mut Position,
        mov: Move,
        depth: usize,
        max_depth: usize,
        do_count_nodes_fn: F,
    ) -> usize
    where
        F: Fn(&mut Position, usize, usize) -> usize,
    {
        if let Some(undo_move_info) = position.make_move(&mov) {
            let count = do_count_nodes_fn(position, depth + 1, max_depth);
            position.unmake_move(&undo_move_info);
            count
        } else {
            0
        }
    }

    if depth < max_depth {
        let moves = move_gen::generate_moves(position);
        if USE_PARALLEL_ITERATOR {
            moves
                .into_par_iter()
                .map(|mov| {
                    process_move(
                        &mut position.clone(),
                        mov,
                        depth,
                        max_depth,
                        do_count_nodes::<false>,
                    )
                })
                .sum()
        } else {
            moves
                .into_iter()
                .map(|mov| {
                    process_move(
                        &mut position.clone(),
                        mov,
                        depth,
                        max_depth,
                        do_count_nodes::<false>,
                    )
                })
                .sum()
        }
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_t() {
        let position = Position::new_game();
        let node_count_stats = count_nodes(&position, 5);
        assert_eq!(node_count_stats.node_count, 4865609);
    }
}
