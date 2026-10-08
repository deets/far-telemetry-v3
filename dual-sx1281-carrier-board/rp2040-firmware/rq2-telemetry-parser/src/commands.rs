use crate::Read;
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

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    extern crate tokio;

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
