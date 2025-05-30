use std::error::Error;
#[macro_use]
use shakmaty::{Chess, fen, CastlingMode, EnPassantMode, Move, Position, Role};
use shakmaty_syzygy::{Tablebase, Wdl, Dtz, MaybeRounded, SyzygyError};
use std::sync::OnceLock;
use shakmaty::fen::Fen;
use crate::chessboard::piece::PieceType;
use crate::r#move::RawMove;

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

pub fn retrieve_best_move(fen: &str) -> Result<Option<RawMove>, Box<dyn Error>> {
    let fen: Fen = fen.parse()
        .map_err(|e| format!("Failed to parse FEN: {}", e))?;
    
    let position: Chess = fen.into_position(CastlingMode::Standard)
        .map_err(|e| format!("Failed to create position from FEN: {}", e))?;

    let tablebases = TABLEBASES.get().ok_or("Tablebases not initialized")?;
    
    let (best_move, _) = tablebases.best_move(&position)
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

    Ok(Some(RawMove::new(from, to, promotion_piece)))
}

fn best_move(position: &Chess) -> Result<Option<(Move, MaybeRounded<Dtz>)>, Box<dyn Error>> {
    let tablebases = TABLEBASES.get().ok_or_else(|| "Tablebases not initialized")?;
    let best_move_result = tablebases
        .best_move(position)
        .map_err(|_| Box::<dyn Error>::from("No moves found in tablebases"))?;

    Ok(best_move_result)
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
    
    fn do_retrieve_best_move(fen: &str) -> Result<Option<RawMove>, Box<dyn Error>> {
        init();
        retrieve_best_move(fen)
    }
    
    #[test]
    fn test_tablebases_initialization() {
        assert!(TABLEBASES.get().is_none());
        init();
        assert!(TABLEBASES.get().is_some());
    }

    #[test]
    fn test_easy_get_best_move() {
        assert_eq!(do_retrieve_best_move("8/8/8/k7/6R1/7R/8/4K3 w - - 0 1").unwrap(), Some(RawMove::new(sq!("h3"), sq!("b3"), None)));
    }

    #[test]
    fn test_gets_best_move_with_only_kings() {
        assert_eq!(do_retrieve_best_move("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap(), Some(RawMove::new(sq!("e1"), sq!("d1"), None)));
    }

    #[test]
    fn test_get_best_move_promotion_queen() {
        assert_eq!(do_retrieve_best_move("8/2P5/8/5k2/3K4/8/6p1/8 b - - 0 1").unwrap(), Some(RawMove::new(sq!("g2"), sq!("g1"), Some(PieceType::Queen))));
    }

    #[test]
    fn test_get_best_move_promotion_to_knight() {
        assert_eq!(do_retrieve_best_move("8/5P1k/R7/8/8/8/8/b3K3 w - - 0 1").unwrap(), Some(RawMove::new(sq!("f7"), sq!("f8"), Some(PieceType::Knight))));
    }

    #[test]
    fn test_no_best_move_for_start_position() {
        assert_eq!(do_retrieve_best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").err().unwrap().to_string(), "No moves found in tablebases: too many pieces");
    }

    #[test]
    fn test_no_best_move_for_six_pieces() {
        assert_eq!(do_retrieve_best_move("8/5P1k/R7/8/2r5/8/8/b3K3 w - - 0 1").err().unwrap().to_string(), "No moves found in tablebases: too many pieces");
    }

    #[test]
    fn test_empty_fen() {
        assert_eq!(
            do_retrieve_best_move("8/8/8/8/8/8/8/8 w - - 0 1").err().unwrap().to_string(),
                   "Failed to create position from FEN: illegal position: empty board, missing king"
        );
    }

    #[test]
    fn test_one_pawn_fen() {
        assert_eq!(
            do_retrieve_best_move("4k3/8/8/8/8/7P/8/3K4 w - - 0 1").unwrap(), Some(RawMove::new(sq!("d1"), sq!("c1"), None))
        );
    }

}
