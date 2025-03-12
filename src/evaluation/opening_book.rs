use crate::chess_move::RawChessMove;
use crate::util;
use rand::{rng, Rng};
use reqwest;
use serde::{Deserialize, Serialize};
use std::error::Error;
use log::{error, info};


pub fn get_opening_move(fen: &str) -> Result<RawChessMove, String> {
    let opening_moves = fetch_opening_moves(fen).map_err(|e| e.to_string())?;
    if opening_moves.len() > 0 {
        let move_string = weighted_random_move(&opening_moves);
        util::parse_move(move_string.clone()).ok_or_else(|| {
            let msg = format!("Unable to parse move string: [{}]", move_string);
            error!("{}", msg);
            msg
        })
    } else {
        let msg = format!("No opening moves found for {}", fen);
        info!("{}", msg);
        Err(msg)
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


fn weighted_random_move(moves: &[LiChessMoveData]) -> String {
    let total_games: u32 = moves.iter().map(|m| (m.white + m.black + m.draws) as u32).sum();

    let mut rng = rng();
    let mut pick = rng.random_range(0..total_games);

    for mv in moves {
        let move_count = mv.white + mv.black + mv.draws;
        if pick < move_count as u32 {
            return mv.uci.clone();
        }
        pick -= move_count as u32;
    }
    moves[0].uci.clone()
}

#[cfg(test)]
mod tests {
    use crate::evaluation::opening_book::get_opening_move;

    #[test]
    fn test_get_opening_move() {
        let opening_move = get_opening_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let opening_move = opening_move.unwrap();
        assert!(opening_move.promote_to.is_none());
    }

    #[test]
    fn test_get_opening_move_empty_response() {
        let result = get_opening_move("r1b1k1n1/p1p1p1p1/8/8/8/8/1P1P1P1P/R1B1K1N1 w KQkq - 0 1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No opening moves found for r1b1k1n1/p1p1p1p1/8/8/8/8/1P1P1P1P/R1B1K1N1 w KQkq - 0 1");
    }
}