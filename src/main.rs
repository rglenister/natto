pub mod core;
pub mod eval;
pub mod utils;

pub mod uci;

mod book;
mod search;
use crate::uci::uci_interface;

fn main() {
    uci_interface::run();
}
