use crate::chess_move::RawChessMove;
use crate::util;
use rand::{rng, Rng};
use reqwest;
use serde::{Deserialize, Serialize};
use std::error::Error;

pub const MAXIMUM_BOOK_DEPTH: usize = 8;


pub fn get_opening_move(fen: &str, depth: usize) -> Result<RawChessMove, String> {
    if depth <= MAXIMUM_BOOK_DEPTH {
        let opening_moves = fetch_opening_moves(fen).map_err(|e| e.to_string())?; // Calls the corrected synchronous function
        weighted_random_move(&opening_moves)
            .and_then(|mv| util::parse_move(mv))
            .ok_or_else(|| "Unable to parse move string".to_string())
    } else {
        Err(format!("Depth {} is too high", depth))
    }
}


#[derive(Serialize, Deserialize)]
struct LiChessMoveData {
    uci: String,
    white: isize,
    draws: isize,
    black: isize,
}

#[derive(Deserialize)]
struct LiChessOpeningResponse {
    moves: Vec<LiChessMoveData>,
}

fn fetch_opening_moves(fen: &str) -> Result<Vec<LiChessMoveData>, Box<dyn Error>> {
    let url = format!("https://explorer.lichess.ovh/masters?fen={}", fen);

    // Perform the HTTP request synchronously using reqwest
    let response: LiChessOpeningResponse = reqwest::blocking::get(&url)?
        .json()?; // Parse the response as JSON into the LiChessOpeningResponse struct

    Ok(response.moves) // Return the list of moves
}


fn weighted_random_move(moves: &[LiChessMoveData]) -> Option<String> {
    if moves.is_empty() {
        return None;
    }

    let total_games: u32 = moves.iter().map(|m| (m.white + m.black + m.draws) as u32).sum();

    let mut rng = rng();
    let mut pick = rng.random_range(0..total_games);

    for m in moves {
        let move_count = m.white + m.black + m.draws;
        if pick < move_count as u32 {
            return Some(m.uci.clone());
        }
        pick -= move_count as u32;
    }
    Some(moves[0].uci.clone())
}

#[cfg(test)]
mod tests {
    use crate::evaluation::opening_book::get_opening_move;

    #[test]
    fn test_get_opening_move() {
        let opening_move = get_opening_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 1);
        let opening_move = opening_move.unwrap();
        assert!(opening_move.promote_to.is_none());
    }
}