#![no_std]

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameResult<'a> {
    Nothing,
    Frame(&'a [u8]),
}

pub struct FrameParser<'a> {
    /// The working buffer to operate on
    buffer: &'a mut [u8],
    /// The size of the current frame that we expect.
    expected: usize,
    /// The actual bytes received so far in this frame.
    received: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameError {
    StrayInput,
    ShortFrame,
}
impl<'a> FrameParser<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            expected: 0,
            received: 0,
        }
    }

    pub fn feed(&mut self, input: u8) -> Result<FrameResult<'_>, FrameError> {
        // We got a frame delimiter
        if input & 128 == 128 {
            let is_short_frame = self.expected != 0;
            self.expected = input as usize & 0x7f;
            self.received = 0;
            if is_short_frame {
                return Err(FrameError::ShortFrame);
            } else {
                return Ok(FrameResult::Nothing);
            }
        }
        if self.received < self.expected {
            self.buffer[self.received] = input;
            self.received += 1;
            if self.received == self.expected {
                let expected = self.expected;
                self.expected = 0;
                self.received = 0;
                return Ok(FrameResult::Frame(&self.buffer[0..expected]));
            } else {
                return Ok(FrameResult::Nothing);
            }
        }
        Err(FrameError::StrayInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer);
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Frame(&[65])));
    }

    #[test]
    fn test_two_frames() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer);
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Frame(&[65])));
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
    }

    #[test]
    fn test_to_short_frame_yields_short_frame_error() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer);
        assert_eq!(parser.feed(128 + 2), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(128 + 1), Err(FrameError::ShortFrame));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
    }

    #[test]
    fn test_stray_bytes_error_after_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer);
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
        assert_eq!(parser.feed(b'C'), Err(FrameError::StrayInput));
    }

    #[test]
    fn test_stray_bytes_before_first_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer);
        assert_eq!(parser.feed(b'C'), Err(FrameError::StrayInput));
    }
}
