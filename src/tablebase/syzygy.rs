use std::error::Error;
#[macro_use]
use shakmaty::{Chess, fen, CastlingMode, EnPassantMode, Move, Role};
use shakmaty_syzygy::{Tablebase, Wdl, Dtz, MaybeRounded, SyzygyError, AmbiguousWdl};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TablebaseError {
    #[error("Tablebase query error: {0}")]
    Query(String),
}
use std::sync::OnceLock;
use log::{info, warn};
use shakmaty::fen::Fen;
use crate::fen::write;
use crate::chessboard::piece::PieceType;
use crate::position::Position;
use crate::r#move::RawMove;
use crate::tablebase;

pub const MAXIMUM_NUMBER_OF_PIECES: usize = 5;

static TABLEBASES: OnceLock<Tablebase<shakmaty::Chess>> = OnceLock::new();

pub fn init_tablebases(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut tables = Tablebase::<Chess>::new();
    tables.add_directory(path)?;
    TABLEBASES
        .set(tables)
        .map_err(|e| format!("Failed to set TABLEBASES - {:?}", e))?;
    Ok(())
}



pub fn query_tablebase(position: &Position) -> Result<Option<(RawMove, MaybeRounded<Dtz>, AmbiguousWdl)>, Box<dyn Error>> {
    let number_of_pieces = position.board().get_total_number_of_pieces();
    if number_of_pieces > MAXIMUM_NUMBER_OF_PIECES {
        info!(
            "Skipping tablebase query - the position has {} pieces but the maximum supported is {}",
            number_of_pieces, MAXIMUM_NUMBER_OF_PIECES
        );
        return Ok(None);
    }

    let fen = write(position);
    match tablebase::syzygy::do_query(&fen) {
        Ok(Some(best_move)) => {
            info!("The move from syzygy is {:?}", best_move);
            Ok(Some(best_move))
        },
        Ok(None) => {
            info!("No move could be found in the tablebase for this position!");
            Ok(None)
        },
        Err(e) => {
            Err(Box::new(TablebaseError::Query(format!("Error querying tablebase: {}", e))))
        }
    }
}

fn do_query(fen_str: &str) -> Result<Option<(RawMove, MaybeRounded<Dtz>, AmbiguousWdl)>, Box<dyn Error>> {
    let fen: Fen = fen_str.parse()
        .map_err(|e| format!("Failed to parse fen {}: {}", fen_str, e))?;
    
    let position: Chess = fen.into_position(CastlingMode::Standard)
        .map_err(|e| format!("Failed to create position from fen {}: {}", fen_str, e))?;

    let tablebases = TABLEBASES.get().ok_or("Tablebases not initialized")?;

    let wdl = tablebases.probe_wdl(&position).map_err(|e| format!("Failed to probe wdl for fen {}: {}", fen_str, e))?;

    let (best_move, dtz) = tablebases.best_move(&position)
        .map_err(|e| format!("No moves found in tablebases: {}", e))?
        .ok_or("No best move found")?;

    let promotion_piece = if let Move::Normal { promotion: Some(piece), .. } = best_move {
        match piece {
            Role::Queen => Some(PieceType::Queen),
            Role::Rook => Some(PieceType::Rook),
            Role::Bishop => Some(PieceType::Bishop),
            Role::Knight => Some(PieceType::Knight),
            _ => None,
        }
    } else {
        None
    };

    let from = best_move.from()
        .ok_or("Invalid move: missing `from` square")? as usize;
    let to = best_move.to() as usize;

    Ok(Some((RawMove::new(from, to, promotion_piece), dtz, wdl)))
}

#[cfg(test)]
mod tests {
    use std::env;
    use dotenv::dotenv;
    use super::*;

    include!("../chess_util/generated_macro.rs");

    fn init() {
        dotenv().ok();
        let path = env::var("ENGINE_TABLEBASE_DIR").unwrap();
        init_tablebases(path.as_str());;
    }
    
    fn do_retrieve_best_move(fen: &str) -> Result<Option<(RawMove, MaybeRounded<Dtz>, AmbiguousWdl)>, Box<dyn Error>> {
        init();
        do_query(fen)
    }
    
    #[test]
    fn test_tablebases_initialization() {
        assert!(TABLEBASES.get().is_none());
        init();
        assert!(TABLEBASES.get().is_some());
    }

    #[test]
    fn test_easy_get_best_move() {
        assert_eq!(do_retrieve_best_move("8/8/8/k7/6R1/7R/8/4K3 w - - 0 1").unwrap().map(|(m, _, _)| m), Some(RawMove::new(sq!("h3"), sq!("b3"), None)));
    }

    #[test]
    fn test_gets_best_move_with_only_kings() {
        assert_eq!(do_retrieve_best_move("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap().map(|(m, _, _)| m), Some(RawMove::new(sq!("e1"), sq!("d1"), None)));
    }

    #[test]
    fn test_get_best_move_promotion_queen() {
        assert_eq!(do_retrieve_best_move("8/2P5/8/5k2/3K4/8/6p1/8 b - - 0 1").unwrap().map(|(m, _, _)| m), Some(RawMove::new(sq!("g2"), sq!("g1"), Some(PieceType::Queen))));
    }

    #[test]
    fn test_get_best_move_promotion_to_knight() {
        assert_eq!(do_retrieve_best_move("8/5P1k/R7/8/8/8/8/b3K3 w - - 0 1").unwrap().map(|(m, _, _)| m), Some(RawMove::new(sq!("f7"), sq!("f8"), Some(PieceType::Knight))));
    }

    #[test]
    fn test_no_best_move_for_start_position() {
        assert_eq!(do_retrieve_best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").err().unwrap().to_string(), "Failed to probe wdl for fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1: too many pieces");
    }

    #[test]
    fn test_no_best_move_for_six_pieces() {
        assert_eq!(do_retrieve_best_move("8/5P1k/R7/8/2r5/8/8/b3K3 w - - 0 1").err().unwrap().to_string(), "Failed to probe wdl for fen 8/5P1k/R7/8/2r5/8/8/b3K3 w - - 0 1: too many pieces");
    }

    #[test]
    fn test_empty_fen() {
        assert_eq!(
            do_retrieve_best_move("8/8/8/8/8/8/8/8 w - - 0 1").err().unwrap().to_string(),
                   "Failed to create position from fen 8/8/8/8/8/8/8/8 w - - 0 1: illegal position: empty board, missing king"
        );
    }

    #[test]
    fn test_one_pawn_fen() {
        assert_eq!(
            do_retrieve_best_move("4k3/8/8/8/8/7P/8/3K4 w - - 0 1").unwrap().map(|(m, _, _)| m), Some(RawMove::new(sq!("d1"), sq!("c1"), None))
        );
    }

}
