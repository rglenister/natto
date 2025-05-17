#[cfg(test)]
use super::*;

pub struct BitboardIterator {
    bitboard: u64,
}

impl BitboardIterator {
    pub fn new(bitboard: u64) -> Self {
        BitboardIterator { bitboard }
    }
}

impl Iterator for BitboardIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bitboard == 0 {
            None
        } else {
            let square = self.bitboard.trailing_zeros() as usize;
            self.bitboard &= self.bitboard - 1;
            Some(square)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_bitboard() {
        let mut iterator = BitboardIterator::new(0);
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn test_single_bit() {
        let mut iterator = BitboardIterator::new(1 << 5);
        assert_eq!(iterator.next(), Some(5));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn test_multiple_bits() {
        let mut iterator = BitboardIterator::new(0b1101);
        assert_eq!(iterator.next(), Some(0));
        assert_eq!(iterator.next(), Some(2));
        assert_eq!(iterator.next(), Some(3));
        assert_eq!(iterator.next(), None);
    }
}


