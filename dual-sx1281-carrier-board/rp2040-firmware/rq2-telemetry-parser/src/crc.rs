#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CrcError {
    BufferTooSmall,
    FormatError,
    WrongCrc(u8),
}

/// Check the CRC contained as last two bytes in the same buffer.  The
/// CRC is encoded in ASCII HEX uppercase, so we don't violate the
/// framing protocol that uses the MSB to establish frame boundaries.
/// However the algorithm itself doesnt' care about the actual
/// bytes being compliant themselves.
/// Implementation chosen from https://medo64.com/posts/selecting-8-bit-crc
pub fn check_crc(buffer: &[u8]) -> Result<(), CrcError> {
    if buffer.len() < 3 {
        return Err(CrcError::BufferTooSmall);
    }
    let given_crc =
        decode_nibble(buffer[buffer.len() - 2])? << 4 | decode_nibble(buffer[buffer.len() - 1])?;

    let mut crc = 0;
    for c in &buffer[0..buffer.len() - 2] {
        crc ^= c;
        for _ in 0..8 {
            if crc & 0x80 > 0 {
                crc = crc << 1 ^ 0x2F;
            } else {
                crc <<= 1;
            }
        }
    }
    if given_crc == crc {
        Ok(())
    } else {
        Err(CrcError::WrongCrc(crc))
    }
}

fn decode_nibble(input: u8) -> Result<u8, CrcError> {
    match input {
        v @ b'0'..=b'9' => Ok(v - b'0'),
        v @ b'A'..=b'F' => Ok(v - b'A' + 10),
        _ => Err(CrcError::FormatError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_small_buffer() {
        let buffer = [0; 1];
        assert_eq!(check_crc(&buffer), Err(CrcError::BufferTooSmall));
    }

    #[test]
    fn test_correct_crc() {
        let buffer = b"DIEZ WAS HERE56";
        assert_eq!(check_crc(buffer.as_slice()), Ok(()));
    }

    #[test]
    fn test_wrong_crc() {
        let buffer = b"DIEZ WAS HERE57";
        assert_eq!(check_crc(buffer.as_slice()), Err(CrcError::WrongCrc(86)));
    }

    #[test]
    fn test_malformed() {
        let buffer = b"DIEZ WAS HERE5X";
        assert_eq!(check_crc(buffer.as_slice()), Err(CrcError::FormatError));
        let buffer = b"DIEZ WAS HEREX5";
        assert_eq!(check_crc(buffer.as_slice()), Err(CrcError::FormatError));
    }

    #[test]
    fn test_nibble_decoding() {
        // Very simple test
        let mut correct = 0;
        for i in 0..=255 {
            if decode_nibble(i).is_ok() {
                correct += 1;
            }
        }
        assert_eq!(correct, 16);
    }
}
