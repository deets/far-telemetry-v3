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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Mode {
    Idle,
    Tx,
    RxContinuous,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Bandwidth {
    _200KHz,
    _400KHz,
    _800KHz,
    _1600KHz,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum SpreadingFactor {
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Response {
    Ack,
    /// Report the current mode we are in
    Status(Mode),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Timeout,
    SerializationError(postcard::Error),
    UnexpectedCommandId(u8),
    NotAResponse,
    NoResponseExpected,
    WrongDestination,
    UnexpectedResponse,
}

impl From<postcard::Error> for Error {
    fn from(value: postcard::Error) -> Self {
        SerializationError(value)
    }
}

trait ResponseProcessor {
    type Error;

    fn process_response<'a>(&mut self, command: Command<'a>) -> Result<Response, Self::Error>;
}

/// The communication model to the telemetry
/// module has three key properties the CommandProcessor is
/// designed to uphold:
///
///  - At any time a payload can come through, which must be passed on to
///    the rest of the application.
///  - There is always only ever *one* control command
///    in flight for which usually an answer is expected.
///  - A timeout can occur and means an error state for
///    which higher levels are responsible, it's not the job
///    of CommandProcessor to deal with this e.g. by retries.
struct CommandProcessor {
    command_id: u8,
    expected: Option<Expectation>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ResponseKind {
    Status,
    Configure,
}

impl From<Request> for ResponseKind {
    fn from(value: Request) -> Self {
        match value {
            Request::Status => ResponseKind::Status,
            Request::Configure { .. } => ResponseKind::Configure,
        }
    }
}

struct Expectation {
    command_id: u8,
    destination: Module,
    response_kind: ResponseKind,
}

impl CommandProcessor {
    fn new() -> Self {
        Self {
            command_id: 0,
            expected: None,
        }
    }

    fn next_command_id(&mut self) -> u8 {
        let res = self.command_id + 1;
        self.command_id = (self.command_id + 1) % 254;
        res
    }

    async fn send(
        &mut self,
        destination: Module,
        request: Request,
        writer: &mut impl Write<Error = Error>,
    ) -> Result<(), Error> {
        let command_id = self.next_command_id();
        self.expected = Some(Expectation {
            command_id,
            destination,
            response_kind: request.into(),
        });

        let command = Command {
            destination,
            payload: Payload::Request {
                command_id,
                request,
            },
        };
        let mut buffer: Vec<u8, 128> = to_vec(&command)?;
        writer.write(&buffer).await
    }
}

impl ResponseProcessor for CommandProcessor {
    type Error = Error;

    fn process_response<'a>(&mut self, command: Command<'a>) -> Result<Response, Self::Error> {
        match command.payload {
            Payload::Data(items) => todo!(),
            Payload::Request {
                command_id,
                request,
            } => Err(Error::NotAResponse),
            Payload::Response {
                command_id,
                response,
            } => {
                if let Some(expectation) = &self.expected {
                    expectation.validate_response_matches_expectation(
                        command.destination,
                        command_id,
                        response,
                    )?;
                    self.expected = None;
                    Ok(response)
                } else {
                    Err(Error::NoResponseExpected)
                }
            }
        }
    }
}

impl Expectation {
    fn validate_response_matches_expectation(
        &self,
        destination: Module,
        command_id: u8,
        response: Response,
    ) -> Result<(), Error> {
        if self.command_id != command_id {
            return Err(Error::UnexpectedCommandId(command_id));
        }
        if self.destination != destination {
            return Err(Error::WrongDestination);
        }
        if match (self.response_kind, response) {
            (ResponseKind::Status, Response::Status(_)) => true,
            (ResponseKind::Configure, Response::Ack) => true,
            _ => false,
        } {
            Ok(())
        } else {
            Err(Error::UnexpectedResponse)
        }
    }
}

#[derive(Debug, PartialEq)]
enum DispatcherError<E> {
    FrameError(crate::FrameError),
    SevenBitEncodingError(crate::SevenBitTranscoderError),
    PostcardError(postcard::Error),
    ResponseError(E),
}

/// Processing incoming bytes from uart according
/// to both our framing and 7-bit encoding to
/// yield Responses.
struct IncomingDataDispatcher<'a, 'b, E, RP: ResponseProcessor<Error = E>, FP: FrameProcessor> {
    processor: &'a mut RP,
    frame_parser: &'b mut FP,
    working: [u8; 128],
}

impl<'a, 'b, E, RP, FP> IncomingDataDispatcher<'a, 'b, E, RP, FP>
where
    RP: ResponseProcessor<Error = E>,
    FP: FrameProcessor,
{
    pub fn new(processor: &'a mut RP, frame_parser: &'b mut FP) -> Self {
        Self {
            processor,
            frame_parser,
            working: [0; 128],
        }
    }

    pub async fn feed(&mut self, data: u8) -> Result<(), DispatcherError<E>> {
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

    fn process_frame(&mut self, frame: &[u8]) -> Result<Response, DispatcherError<E>> {
        let decoded = seven_bit_decode(&mut self.working, frame)
            .map_err(DispatcherError::SevenBitEncodingError)?;
        let command: Command = from_bytes(decoded).map_err(DispatcherError::PostcardError)?;
        self.processor
            .process_response(command)
            .map_err(DispatcherError::ResponseError)
    }
}

#[cfg(test)]
mod tests {
    use crate::encode_frame;

    use super::*;
    extern crate std;
    extern crate tokio;

    #[derive(Debug, Copy, Clone, PartialEq)]
    enum TestResponseProcessorError {
        NothingWorks,
    }

    struct TestResponseProcessor {
        count: usize,
    }

    impl ResponseProcessor for TestResponseProcessor {
        type Error = TestResponseProcessorError;

        fn process_response<'a>(&mut self, command: Command<'a>) -> Result<Response, Self::Error> {
            match command.payload {
                Payload::Data(items) => todo!(),
                Payload::Request {
                    command_id,
                    request,
                } => todo!(),
                Payload::Response {
                    command_id,
                    response,
                } => {
                    self.count += 1;
                    return Ok(response);
                }
            }
            Err(TestResponseProcessorError::NothingWorks)
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
    async fn command_processor_send_test() {
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
        assert!(command_processor
            .send(Module::Lora1, request, &mut writer)
            .await
            .is_ok());
    }
}
