use crate::seven_bit_decode;
use crate::FrameParser;
use crate::FrameProcessor;
use crate::FrameResult;
use crate::FrameResult::Nothing;
use crate::Read;
use crate::SelfContainedFrameParser;
use crate::Write;
use heapless::Vec;
use postcard::{from_bytes, to_vec};
use serde::{Deserialize, Serialize};
use Error::SerializationError;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Mode {
    Idle,
    Tx,
    RxContinuous,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Bandwidth {
    _200KHz,
    _400KHz,
    _800KHz,
    _1600KHz,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum SpreadingFactor {
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Module {
    Lora1,
    Lora2,
}

/// A command sent or received from the telemetry
/// module.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Command<'a> {
    destination: Module,
    #[serde(borrow)]
    payload: Payload<'a>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Payload<'a> {
    Data(&'a [u8]),
    Request { command_id: u8, request: Request },
    Response { command_id: u8, response: Response },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Request {
    /// Ask for the current mode we're in
    Status,
    /// Configure the modulation and packet parameters.
    /// This is only accepted when in Mode::Idle.
    Configure {
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
        coding_rate: CodingRate,
        /// Given in Megahertz
        frequency: usize,
    },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Response {
    Ack,
    /// Report the current mode we are in
    Status(Mode),
}

struct CommandProcessor {
    command_id: u8,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    SomethingWentWrong,
    SerializationError(postcard::Error),
}

impl From<postcard::Error> for Error {
    fn from(value: postcard::Error) -> Self {
        SerializationError(value)
    }
}

impl CommandProcessor {
    fn new() -> Self {
        Self { command_id: 0 }
    }

    fn next_command_id(&mut self) -> u8 {
        let res = self.command_id + 1;
        self.command_id = (self.command_id + 1) % 254;
        res
    }

    async fn send_sync(
        &mut self,
        request: Request,
        writer: &mut impl Write<Error = Error>,
        reader: &mut impl Read<Error = Error>,
    ) -> Result<Response, Error> {
        let mut buffer: Vec<u8, 128> = to_vec(&request)?;
        writer.write(&buffer).await?;
        let bytes_read = reader.read_all(&mut buffer).await?;
        let res: Response = from_bytes(&buffer)?;
        Ok(res)
    }
}

trait ResponseProcessor {
    type Error;

    fn process_response<'a>(&mut self, command: Command<'a>) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq)]
enum DispatcherError {
    FrameError(crate::FrameError),
    SevenBitEncodingError(crate::SevenBitTranscoderError),
    PostcardError(postcard::Error),
}

/// Processing incoming bytes from uart according
/// to both our framing and 7-bit encoding to
/// yield Commands (that technically should be responses)
/// and delegates these for processing.
struct IncomingDataDispatcher<
    'a,
    'b,
    RP: ResponseProcessor<Error = DispatcherError>,
    FP: FrameProcessor,
> {
    processor: &'a mut RP,
    frame_parser: &'b mut FP,
    working: [u8; 128],
}

impl<'a, 'b, RP, FP> IncomingDataDispatcher<'a, 'b, RP, FP>
where
    RP: ResponseProcessor<Error = DispatcherError>,
    FP: FrameProcessor,
{
    pub fn new(processor: &'a mut RP, frame_parser: &'b mut FP) -> Self {
        Self {
            processor,
            frame_parser,
            working: [0; 128],
        }
    }

    pub async fn feed(&mut self, data: u8) -> Result<(), DispatcherError> {
        let frame_result = self.frame_parser.feed(data);
        let mut frame_copy = [0; 128];
        match frame_result {
            Ok(FrameResult::Frame(frame)) => {
                frame_copy[0..frame.len()].copy_from_slice(frame);
                drop(frame);
                self.process_frame(&frame_copy)?;
            }
            Ok(Nothing) => {}
            Err(frame_error) => return Err(DispatcherError::FrameError(frame_error)),
        };
        Ok(())
    }

    fn process_frame(&mut self, frame: &[u8]) -> Result<(), DispatcherError> {
        let decoded = seven_bit_decode(&mut self.working, frame)
            .map_err(DispatcherError::SevenBitEncodingError)?;
        let command: Command = from_bytes(decoded).map_err(DispatcherError::PostcardError)?;
        self.processor.process_response(command)
    }
}

#[cfg(test)]
mod tests {
    use crate::encode_frame;

    use super::*;
    extern crate std;
    extern crate tokio;

    struct TestResponseProcessor {
        count: usize,
    }

    impl ResponseProcessor for TestResponseProcessor {
        type Error = DispatcherError;

        fn process_response<'a>(&mut self, command: Command<'a>) -> Result<(), Self::Error> {
            self.count += 1;
            Ok(())
        }
    }

    fn serialize_command<'a>(buffer: &'a mut [u8], command: &Command) -> &'a [u8] {
        let output: Vec<u8, 256> = to_vec(&command).unwrap();
        let mut encoded_buffer = [0; 127];
        let encoded = crate::framing::seven_bit_encode(&mut encoded_buffer, &output)
            .expect("Can't encode 7-bit");
        crate::framing::encode_frame(buffer, encoded).expect("Can't generate frame")
    }

    #[tokio::test]
    async fn test_incoming_data_dispatch() {
        let mut processor = TestResponseProcessor { count: 0 };
        let mut parser = SelfContainedFrameParser::new();
        let mut dispatcher = IncomingDataDispatcher::new(&mut processor, &mut parser);
        let response = Command {
            destination: Module::Lora1,
            payload: Payload::Response {
                command_id: 1,
                response: Response::Ack,
            },
        };
        let mut buffer = [0; 1024];
        let data = serialize_command(&mut buffer, &response);
        for c in data {
            dispatcher.feed(*c).await.expect("Can't dispatch command");
        }
        assert_eq!(processor.count, 1);
    }

    #[test]
    fn test_serialization() {
        let data = b"DIEZ WAS HERE";
        let command = Command {
            destination: Module::Lora1,
            payload: Payload::Data(data.as_slice()),
        };
        let output: Vec<u8, 256> = to_vec(&command).unwrap();
        std::println!(
            "{:?}, len data: {}, output len: {}",
            output,
            data.len(),
            output.len()
        );

        let command = Command {
            destination: Module::Lora1,
            payload: Payload::Request {
                command_id: 1,
                request: Request::Configure {
                    bandwidth: Bandwidth::_400KHz,
                    spreading_factor: SpreadingFactor::_12,
                    coding_rate: CodingRate::_4_8,
                    frequency: 2450,
                },
            },
        };
        let output: Vec<u8, 256> = to_vec(&command).unwrap();
        std::println!(
            "{:?}, len data: {}, output len: {}",
            output,
            data.len(),
            output.len()
        );
    }

    struct BufferWriter {
        buffer: Vec<u8, 1024>,
    }

    impl BufferWriter {
        fn new() -> Self {
            let buffer: Vec<u8, 1024> = Vec::new();
            Self { buffer }
        }
    }

    impl Write for BufferWriter {
        type Error = super::Error;

        async fn write(&mut self, buffer: &[u8]) -> Result<(), Self::Error> {
            self.buffer.extend_from_slice(buffer).expect("Buffer full");
            Ok(())
        }
    }

    struct BufferReader<'a> {
        buffer: &'a [u8],
    }

    impl Read for BufferReader<'_> {
        type Error = super::Error;

        async fn read_all(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
            buffer[0..self.buffer.len()].copy_from_slice(self.buffer);
            Ok(self.buffer.len())
        }
    }

    #[tokio::test]
    async fn command_processor_send_sync_test() {
        let mut command_processor = CommandProcessor::new();
        let mut writer = BufferWriter::new();
        let request = Request::Configure {
            bandwidth: Bandwidth::_400KHz,
            spreading_factor: SpreadingFactor::_12,
            coding_rate: CodingRate::_4_8,
            frequency: 2450,
        };
        let response = Response::Ack;
        let response: Vec<u8, 256> = to_vec(&response).unwrap();
        let mut reader = BufferReader { buffer: &response };
        assert_eq!(
            command_processor
                .send_sync(request, &mut writer, &mut reader)
                .await,
            Ok(Response::Ack)
        );
    }
}
