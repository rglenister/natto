

fn dpec(mut mask: u64, mut source: u64) -> u64 {
    let mut result = 0;
    let mut result_position: u64 = 0;
    while source != 0 {
        let bit = source & 1;
        let mask_trailing_zeros = mask.trailing_zeros() as u64;
        result_position += mask_trailing_zeros;
        result |= bit << result_position;
        mask >>= 1 + mask_trailing_zeros;
        source >>= 1;
        result_position += 1;
    }
    return result;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpec() {
        assert_eq!(dpec(0b1, 0b1), 0b1);
        assert_eq!(dpec(0b11, 0b10), 0b10);
        assert_eq!(dpec(0b110, 0b10), 0b100);
        assert_eq!(dpec(0b1010, 0b11), 0b1010);
        assert_eq!(dpec(0b1001, 0b10), 0b1000);
        assert_eq!(dpec(0b101, 0b11), 0b101);
        assert_eq!(dpec(0b10101, 0b111), 0b10101);
        assert_eq!(dpec(0b100000000000000000000000, 0b1), 0b100000000000000000000000);
        assert_eq!(dpec(0b10100001, 0b111), 0b10100001);
        assert_eq!(dpec(0b101000000000000000000001, 0b111), 0b101000000000000000000001);
        assert_eq!(dpec(0b101000000011000000000001, 0b11111), 0b101000000011000000000001);
        assert_eq!(dpec(0b10, 0b1), 0b10);
        assert_eq!(dpec(0b100, 0b1), 0b100);
        assert_eq!(dpec(0b1001, 0b11), 0b1001);
        assert_eq!(dpec(0b11, 0b11), 0b11);
        assert_eq!(dpec(0b10, 0b1), 0b10);
        assert_eq!(dpec(0b101, 0b11), 0b101);
    }
}