#[cfg(test)]
extern crate std;

use num_integer::Integer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameResult<'a> {
    Nothing,
    Frame(&'a [u8]),
}

#[derive(Default)]
struct FrameParserInner {
    /// The size of the current frame that we expect.
    expected: usize,
    /// The actual bytes received so far in this frame.
    received: usize,
}
pub struct FrameParser<'a> {
    /// The working buffer to operate on
    buffer: &'a mut [u8],
    inner: FrameParserInner,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameError {
    StrayInput,
    ShortFrame,
    BufferCapacityOverrun,
}

impl FrameParserInner {
    fn feed<'a>(&mut self, input: u8, buffer: &'a mut [u8]) -> Result<FrameResult<'a>, FrameError> {
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
            buffer[self.received] = input;
            self.received += 1;
            if self.received == self.expected {
                let expected = self.expected;
                self.expected = 0;
                self.received = 0;
                return Ok(FrameResult::Frame(&buffer[0..expected]));
            } else {
                return Ok(FrameResult::Nothing);
            }
        }
        Err(FrameError::StrayInput)
    }
}

pub trait FrameProcessor {
    fn feed(&mut self, input: u8) -> Result<FrameResult<'_>, FrameError>;
}

impl<'a> FrameParser<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Result<Self, FrameError> {
        if buffer.len() < 127 {
            Err(FrameError::BufferCapacityOverrun)
        } else {
            Ok(Self {
                buffer,
                inner: FrameParserInner::default(),
            })
        }
    }
}

impl FrameProcessor for FrameParser<'_> {
    fn feed(&mut self, input: u8) -> Result<FrameResult<'_>, FrameError> {
        self.inner.feed(input, self.buffer)
    }
}

pub struct SelfContainedFrameParser {
    buffer: [u8; 127],
    inner: FrameParserInner,
}

impl SelfContainedFrameParser {
    pub fn new() -> Self {
        Self {
            buffer: [0; 127],
            inner: FrameParserInner::default(),
        }
    }
}

impl FrameProcessor for SelfContainedFrameParser {
    fn feed(&mut self, input: u8) -> Result<FrameResult<'_>, FrameError> {
        self.inner.feed(input, &mut self.buffer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SevenBitTranscoderError {
    BufferOverflow,
}
/// Our framing forbids the MSB to be set.  To accomodate this, I'm
/// encoding bytes as 7-bit values that are shifted to create full 8
/// bit bytes. This struct helps accomplishing this from both sides.
#[derive(Debug)]
pub struct SevenBitTranscoder<'a> {
    buffer: &'a mut [u8],
}

impl<'a> SevenBitTranscoder<'a> {
    fn new(buffer: &'a mut [u8]) -> SevenBitTranscoder<'a> {
        Self { buffer }
    }

    /// Encode a buffer of 8 bit bytes into a buffer that
    /// guarantees the MSB is always 0, and the bits are
    /// send as a otherwise consecutive stream.
    fn encode(&mut self, incoming: &[u8]) -> Result<&[u8], SevenBitTranscoderError> {
        let (mut byte_count, rest_bits) = (incoming.len() * 8).div_rem(&7_usize);
        if rest_bits > 0 {
            byte_count += 1;
        }
        if byte_count > self.buffer.len() {
            return Err(SevenBitTranscoderError::BufferOverflow);
        }
        // This is a rather stupid implementation, as it accesses
        // every byte usually twice, instead of using a buffer and
        // shifting bits in and out - but I've wasted a bit of time
        // on that, and correctness trumps speed for now.
        for i in 0..byte_count {
            let bit_index_start = i * 7;
            let bit_index_end = bit_index_start + 7;
            let byte_index_start = bit_index_start >> 3;
            let byte_index_end = bit_index_end >> 3;

            // We are fully within one bytes
            let value = if byte_index_start == byte_index_end {
                (incoming[byte_index_start] >> 1) & 0x7F
            } else {
                let upper = (incoming[byte_index_start] << (7 - i)) & 0x7F;
                // it's possible to point beyond the last byte, then fill with zero
                let lower = if byte_index_end < incoming.len() {
                    incoming[byte_index_end] >> (i + 1)
                } else {
                    0
                };
                upper | lower
            };
            self.buffer[i] = value;
        }
        Ok(&self.buffer[0..byte_count])
    }

    /// Decode a buffer with 7-bit bytes into the full 8-bit equivalent.
    fn decode(&mut self, incoming: &[u8]) -> Result<&'a [u8], SevenBitTranscoderError> {
        // let count = incoming.len() * 7 / 8;
        // // A special case that could be considered verboten: encoding
        // // one byte into a 7-bit-based buffer *always* needs two
        // // bytes, as you don't know if the least significant bit is zero or not.
        // // However when decoding you can imply that w/o a byte, it's a zero
        // if count < 1 {
        //     self.buffer[0] = incoming[0] << 1;
        //     return Ok(self.buffer[0..1]);
        // }
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_byte_as_seven_bit_transfer() {
        let mut buffer = [0; 1024];
        let mut encoder = SevenBitTranscoder::new(&mut buffer);
        assert_eq!(encoder.encode(&[0xFF]), Ok([0x7F_u8, 0x40_u8].as_slice()));
        let mut encoder = SevenBitTranscoder::new(&mut buffer);
        assert_eq!(encoder.encode(&[0xFE]), Ok([0x7F_u8, 0x00_u8].as_slice()));
    }

    #[test]
    fn test_two_byte_as_seven_bit_transfer() {
        let mut buffer = [0; 1024];
        let mut encoder = SevenBitTranscoder::new(&mut buffer);
        assert_eq!(
            encoder.encode(&[0xFF, 0xFF]),
            Ok([0x7F_u8, 0x7F_u8, 0x60].as_slice())
        );
        assert_eq!(
            encoder.encode(&[0xFF, 0xFC]),
            Ok([0x7F_u8, 0x7F_u8, 0x00].as_slice())
        );
    }

    #[test]
    fn test_seven_byte_as_seven_bit_transfer() {
        let mut buffer = [0; 1024];
        let mut encoder = SevenBitTranscoder::new(&mut buffer);
        assert_eq!(
            encoder.encode(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            Ok(
                [0x7F_u8, 0x7F_u8, 0x7F_u8, 0x7F_u8, 0x7F_u8, 0x7F_u8, 0x7F_u8, 0x7F_u8,]
                    .as_slice()
            )
        );
    }

    #[test]
    fn test_simple_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer).unwrap();
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Frame(&[65])));
    }

    #[test]
    fn test_two_frames() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer).unwrap();
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Frame(&[65])));
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
    }

    #[test]
    fn test_to_short_frame_yields_short_frame_error() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer).unwrap();
        assert_eq!(parser.feed(128 + 2), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'A'), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(128 + 1), Err(FrameError::ShortFrame));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
    }

    #[test]
    fn test_stray_bytes_error_after_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer).unwrap();
        assert_eq!(parser.feed(128 + 1), Ok(FrameResult::Nothing));
        assert_eq!(parser.feed(b'B'), Ok(FrameResult::Frame(&[66])));
        assert_eq!(parser.feed(b'C'), Err(FrameError::StrayInput));
    }

    #[test]
    fn test_stray_bytes_before_first_frame() {
        let mut buffer = [0; 1024];
        let mut parser = FrameParser::new(&mut buffer).unwrap();
        assert_eq!(parser.feed(b'C'), Err(FrameError::StrayInput));
    }

    #[test]
    fn test_buffer_capacity() {
        let mut buffer = [0; 10];
        assert!(
            FrameParser::new(&mut buffer).is_err_and(|e| e == FrameError::BufferCapacityOverrun)
        );
    }
}
