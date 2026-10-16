use core::marker::PhantomData;

use crate::encode_frame;
use crate::seven_bit_decode;
use crate::seven_bit_encode;
use crate::FrameError;
use crate::FrameParser;
use crate::FrameProcessor;
use crate::FrameResult;
use crate::FrameResult::Nothing;
use crate::Read;
use crate::SelfContainedFrameParser;
use crate::SevenBitTranscoderError;
use crate::Write;

#[cfg(test)]
use core::time::Duration;
#[cfg(test)]
use mock_instant::global::Instant;

#[cfg(all(not(test), not(feature = "use_std_time")))]
use embassy_time::Duration;
#[cfg(all(not(test), not(feature = "use_std_time")))]
use embassy_time::Instant;

#[cfg(all(not(test), feature = "use_std_time"))]
extern crate std;
#[cfg(all(not(test), feature = "use_std_time"))]
use std::time::Duration;
#[cfg(all(not(test), feature = "use_std_time"))]
use std::time::Instant;

use heapless::Vec;
use postcard::{from_bytes, to_vec};
use serde::{Deserialize, Serialize};
use Error::DataProcessingError;
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
    pub destination: Module,
    #[serde(borrow)]
    pub payload: Payload<'a>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Payload<'a> {
    Data(&'a [u8]),
    Request { command_id: u8, request: Request },
    Response { command_id: u8, response: Response },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Request {
    /// Just a test command
    Ping,
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
    Pong,
    /// A bunch of requests just need acknowledegment
    Ack,
    /// Report the current mode we are in
    Status(Mode),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Timeout,
    SerializationError(postcard::Error),
    UnexpectedCommandId(u8),
    NotAResponse,
    NoResponseExpected,
    WrongDestination,
    UnexpectedResponse,
    DataProcessed,
    DataProcessingError(E),
}

impl<E> From<postcard::Error> for Error<E> {
    fn from(value: postcard::Error) -> Self {
        SerializationError(value)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ResponseProcessorResult {
    Nothing,
    DataProcessed,
    Response(Response),
}

pub trait ResponseProcessor {
    type Error;

    /// Process a received response. Must be called
    /// regularly with None to check for a timeout.
    async fn process_response_or_timeout<'a>(
        &mut self,
        command: Option<Command<'a>>,
    ) -> Result<ResponseProcessorResult, Self::Error>;
}

pub trait DataProcessor {
    type Error;

    fn process_data<'a>(&mut self, source: Module, data: &'a [u8]) -> Result<(), Self::Error>;
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
pub struct CommandProcessor<'a, E, DP: DataProcessor<Error = E>> {
    command_id: u8,
    expected: Option<Expectation<E>>,
    data_processor: &'a mut DP,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ResponseKind {
    Status,
    Configure,
    Pong,
}

impl From<Request> for ResponseKind {
    fn from(value: Request) -> Self {
        match value {
            Request::Status => ResponseKind::Status,
            Request::Configure { .. } => ResponseKind::Configure,
            Request::Ping => ResponseKind::Pong,
        }
    }
}

struct Expectation<E> {
    command_id: u8,
    destination: Module,
    response_kind: ResponseKind,
    when: Instant,
    phantom_data: PhantomData<E>,
}

impl<'dp, E, DP: DataProcessor<Error = E>> CommandProcessor<'dp, E, DP> {
    pub fn new(data_processor: &'dp mut DP) -> Self {
        Self {
            command_id: 0,
            expected: None,
            data_processor,
        }
    }

    fn next_command_id(&mut self) -> u8 {
        let res = self.command_id + 1;
        self.command_id = (self.command_id + 1) % 254;
        res
    }

    pub async fn send(
        &mut self,
        destination: Module,
        request: Request,
    ) -> Result<Command<'_>, Error<E>> {
        let command_id = self.next_command_id();
        self.expected = Some(Expectation {
            command_id,
            destination,
            response_kind: request.into(),
            when: Instant::now(),
            phantom_data: PhantomData,
        });

        Ok(Command {
            destination,
            payload: Payload::Request {
                command_id,
                request,
            },
        })
    }
}

const RESPONSE_EXPECTATION_TIMEOUT: Duration = Duration::from_secs(1);

impl<E, DP: DataProcessor<Error = E>> ResponseProcessor for CommandProcessor<'_, E, DP> {
    type Error = Error<E>;

    async fn process_response_or_timeout<'a>(
        &mut self,
        command: Option<Command<'a>>,
    ) -> Result<ResponseProcessorResult, Self::Error> {
        if let Some(command) = command {
            match command.payload {
                Payload::Data(data) => {
                    self.data_processor
                        .process_data(command.destination, data)
                        .map_err(Error::DataProcessingError)?;
                    Ok(ResponseProcessorResult::DataProcessed)
                }
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
                        Ok(ResponseProcessorResult::Response(response))
                    } else {
                        Err(Error::NoResponseExpected)
                    }
                }
            }
        } else {
            if let Some(expectation) = &self.expected {
                if expectation.when.elapsed() > RESPONSE_EXPECTATION_TIMEOUT {
                    self.expected = None;
                    Err(Error::Timeout)
                } else {
                    Ok(ResponseProcessorResult::Nothing)
                }
            } else {
                Ok(ResponseProcessorResult::Nothing)
            }
        }
    }
}

impl<E> Expectation<E> {
    fn validate_response_matches_expectation(
        &self,
        destination: Module,
        command_id: u8,
        response: Response,
    ) -> Result<(), Error<E>> {
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
pub enum DispatcherError<E> {
    FrameError(crate::FrameError),
    SevenBitEncodingError(crate::SevenBitTranscoderError),
    PostcardError(postcard::Error),
    ResponseError(E),
}

/// Processing incoming bytes from uart according
/// to both our framing and 7-bit encoding to
/// yield Responses.
pub struct IncomingDataDispatcher<'a, 'b, E, RP: ResponseProcessor<Error = E>, FP: FrameProcessor> {
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

    pub async fn feed(&mut self, data: u8) -> Result<ResponseProcessorResult, DispatcherError<E>> {
        let frame_result = self.frame_parser.feed(data);
        let mut frame_copy = [0; 128];
        match frame_result {
            Ok(FrameResult::Frame(frame)) => {
                frame_copy[0..frame.len()].copy_from_slice(frame);
                self.process_frame(&frame_copy).await
            }
            Ok(Nothing) => Ok(ResponseProcessorResult::Nothing),
            Err(frame_error) => return Err(DispatcherError::FrameError(frame_error)),
        }
    }

    async fn process_frame(
        &mut self,
        frame: &[u8],
    ) -> Result<ResponseProcessorResult, DispatcherError<E>> {
        let decoded = seven_bit_decode(&mut self.working, frame)
            .map_err(DispatcherError::SevenBitEncodingError)?;
        let command: Command = from_bytes(decoded).map_err(DispatcherError::PostcardError)?;
        self.processor
            .process_response_or_timeout(Some(command))
            .await
            .map_err(DispatcherError::ResponseError)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EncodedFrameWriterError<E> {
    FrameError(FrameError),
    SevenBitTranscoderError(SevenBitTranscoderError),
    WriterError(E),
}

pub struct EncodedFrameWriter<'a, W, E>
where
    W: super::Write<Error = E>,
{
    writer: &'a mut W,
}

impl<'a, W, E> EncodedFrameWriter<'a, W, E>
where
    W: super::Write<Error = E>,
{
    pub fn new(writer: &'a mut W) -> EncodedFrameWriter<'a, W, E> {
        EncodedFrameWriter { writer }
    }
}

impl<W, E> super::Write for EncodedFrameWriter<'_, W, E>
where
    W: super::Write<Error = E>,
{
    type Error = EncodedFrameWriterError<E>;

    async fn write(&mut self, incoming: &[u8]) -> Result<(), Self::Error> {
        let mut seven_bit_buffer = [0; 128];
        let seven_bit_data = seven_bit_encode(&mut seven_bit_buffer, incoming)
            .map_err(EncodedFrameWriterError::SevenBitTranscoderError)?;
        let mut buffer = [0; 128];
        let encoded_frame = encode_frame(&mut buffer, seven_bit_data)
            .map_err(EncodedFrameWriterError::FrameError)?;
        self.writer
            .write(encoded_frame)
            .await
            .map_err(EncodedFrameWriterError::WriterError)
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use mock_instant::global::MockClock;

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

        async fn process_response_or_timeout<'a>(
            &mut self,
            command: Option<Command<'a>>,
        ) -> Result<ResponseProcessorResult, Self::Error> {
            let command = command.unwrap();
            match command.payload {
                Payload::Data(_items) => return Ok(ResponseProcessorResult::DataProcessed),
                Payload::Request {
                    command_id: _,
                    request: _,
                } => todo!(),
                Payload::Response {
                    command_id: _,
                    response,
                } => {
                    self.count += 1;
                    return Ok(ResponseProcessorResult::Response(response));
                }
            };
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
        type Error = super::Error<TestDataProcessorError>;

        async fn write(&mut self, buffer: &[u8]) -> Result<(), Self::Error> {
            self.buffer.extend_from_slice(buffer).expect("Buffer full");
            Ok(())
        }
    }

    struct TestDataProcessor {
        data: std::vec::Vec<std::vec::Vec<u8>>,
    }

    #[derive(Copy, Clone, PartialEq, Debug)]
    enum TestDataProcessorError {}
    impl DataProcessor for TestDataProcessor {
        type Error = TestDataProcessorError;

        fn process_data<'a>(&mut self, _source: Module, data: &'a [u8]) -> Result<(), Self::Error> {
            let mut data_vec = std::vec![];
            data_vec.extend_from_slice(data);
            self.data.push(data_vec);
            Ok(())
        }
    }

    #[tokio::test]
    async fn encoded_frame_write_test() {
        let mut writer = BufferWriter::new();
        let mut encoded_frame_writer = EncodedFrameWriter::new(&mut writer);
        encoded_frame_writer
            .write(b"DIEZ WAS HERE")
            .await
            .expect("Error writing");
        assert_eq!(writer.buffer.len(), 16);
    }

    #[tokio::test]
    async fn command_processor_send_test() {
        let mut tdp = TestDataProcessor { data: std::vec![] };
        let mut command_processor = CommandProcessor::new(&mut tdp);
        let request = Request::Configure {
            bandwidth: Bandwidth::_400KHz,
            spreading_factor: SpreadingFactor::_12,
            coding_rate: CodingRate::_4_8,
            frequency: 2450,
        };
        assert_eq!(
            command_processor.send(Module::Lora1, request).await,
            Ok(Command {
                destination: Module::Lora1,
                payload: Payload::Request {
                    command_id: 1,
                    request
                }
            })
        );
        let response = Command {
            destination: Module::Lora1,
            payload: Payload::Response {
                command_id: 1,
                response: Response::Ack,
            },
        };
        assert_eq!(
            command_processor
                .process_response_or_timeout(Some(response))
                .await,
            Ok(ResponseProcessorResult::Response(Response::Ack))
        );
    }

    #[tokio::test]
    async fn command_processor_send_with_timeout() {
        let mut tdp = TestDataProcessor { data: std::vec![] };
        let mut command_processor = CommandProcessor::new(&mut tdp);
        let request = Request::Configure {
            bandwidth: Bandwidth::_400KHz,
            spreading_factor: SpreadingFactor::_12,
            coding_rate: CodingRate::_4_8,
            frequency: 2450,
        };
        assert!(command_processor.send(Module::Lora1, request).await.is_ok());
        MockClock::advance(Duration::from_secs(15));
        assert_eq!(
            command_processor.process_response_or_timeout(None).await,
            Err(Error::Timeout)
        );
        let response = Command {
            destination: Module::Lora1,
            payload: Payload::Response {
                command_id: 1,
                response: Response::Ack,
            },
        };
        // Ensure we are removing the expectation after a timout
        assert_eq!(
            command_processor
                .process_response_or_timeout(Some(response))
                .await,
            Err(Error::NoResponseExpected)
        );
    }

    #[tokio::test]
    async fn command_processor_send_data() {
        let mut tdp = TestDataProcessor { data: std::vec![] };
        let mut command_processor = CommandProcessor::new(&mut tdp);
        let request = Request::Configure {
            bandwidth: Bandwidth::_400KHz,
            spreading_factor: SpreadingFactor::_12,
            coding_rate: CodingRate::_4_8,
            frequency: 2450,
        };
        assert!(command_processor.send(Module::Lora1, request).await.is_ok());
        let response = Command {
            destination: Module::Lora1,
            payload: Payload::Data(b"DIEZ WAS HERE"),
        };
        assert_eq!(
            command_processor
                .process_response_or_timeout(Some(response))
                .await,
            Ok(ResponseProcessorResult::DataProcessed)
        );
    }
}
