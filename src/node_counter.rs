use crate::move_generator;
use crate::position::Position;

unsafe fn count_nodes(position: &Position, mut node_counter: usize) -> usize {
    move_generator::generate(position)
        .iter()
        .flat_map(|cm| position.make_move(cm))
        .collect::<Vec<_>>()
//        .map(|pos| count_nodes(&pos, node_counter))
        .iter().count()
}
