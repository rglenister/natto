use std::error::Error;
use shakmaty::{Chess, fen, CastlingMode, EnPassantMode, Move, Position, Role};
use shakmaty_syzygy::{Tablebase, Wdl, Dtz, MaybeRounded, SyzygyError};
use std::sync::OnceLock;
use shakmaty::fen::Fen;
use crate::chessboard::piece::PieceType;
use crate::r#move::RawMove;

static TABLEBASE: OnceLock<Tablebase<shakmaty::Chess>> = OnceLock::new();

pub fn init_tablebases(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut tables = Tablebase::<Chess>::new();
    tables.add_directory(path)?;
    TABLEBASE
        .set(tables)
        .map_err(|e| format!("Failed to set TABLEBASES - {:?}", e))?;
    Ok(())
}

pub fn retrieve_best_move(fen: String) -> Result<RawMove, Box<dyn Error>> {
    let fen: Fen = fen.parse().map_err(|e| e)?;
    let position: Chess = fen.into_position(CastlingMode::Standard).map_err(|e| e)?;
    let best_move = best_move(&position).map_err(|e| e)?.ok_or("No best move found")?.0;
    let promotion_piece: Option<PieceType> = match best_move {
        Move::Normal { promotion, .. } => { 
            match promotion {
                None => None,
                Some(piece) => {
                    match piece {
                        Role::Queen => Some(PieceType::Queen),
                        Role::Rook => Some(PieceType::Rook),
                        Role::Bishop => Some(PieceType::Bishop),
                        Role::Knight => Some(PieceType::Knight),
                        _ => None,
                    }
                }
            }
        }
        _ => { None }
    };
    Ok(RawMove::new(best_move.from().unwrap() as usize, best_move.to() as usize, promotion_piece))
}
fn best_move(position: &Chess) -> Result<Option<(Move, MaybeRounded<Dtz>)>, Box<dyn Error>> {
    let result = TABLEBASE
        .get()
        .ok_or("Tablebases not initialized")?
        .best_move(position)
        .map_err(|_| Box::<dyn Error>::from("No moves found in tablebases"))?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use std::env;
    use dotenv::dotenv;
    use crate::config::CONFIG;
    use super::*;
    
    fn init() {
        dotenv().ok();
        let path = env::var("ENGINE_TABLEBASE_DIR").unwrap();
        init_tablebases(path.as_str());;
    }
    
    fn do_retrieve_best_move(fen: &str) -> RawMove {
        init();
        retrieve_best_move(fen.to_string()).unwrap()
    }
    
    #[test]
    fn test_tablebases_initialization() {
        assert!(TABLEBASE.get().is_none());
        init();
        assert!(TABLEBASE.get().is_some());
    }

    #[test]
    fn test_get_best_move() {
        assert_eq!(do_retrieve_best_move("8/8/8/k7/6R1/7R/8/4K3 w - - 0 1"), RawMove::new(23, 17, None));
    }
}
