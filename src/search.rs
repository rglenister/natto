use crate::position::Position;




fn search(position: Position, depth: u32) {

}

#[cfg(test)]
mod tests {
    use serde_derive::Deserialize;

    use std::error::Error;
    use crate::{fen, move_generator};

    use std::fs;

    #[derive(Deserialize, Debug)]

    struct FenTestCase {
        depth: usize,
        nodes: usize,
        fen: String,
    }

    #[test]
    fn test_fens() {
        let test_cases = load_fens().unwrap();
        assert_eq!(test_cases.len(), 7);
        for test in test_cases {
            let position = fen::parse(test.fen);
            let moves = move_generator::generate(position);
            assert_eq!(moves.len(), test.nodes);
            break
        }
    }

    fn load_fens() -> Result<Vec<FenTestCase>, Box<dyn Error>> {
        let file = fs::read_to_string("src/test_data/fen_test_data.json")?;
        let test_cases = json5::from_str(file.as_str())?;
        Ok(test_cases)
    }
}
