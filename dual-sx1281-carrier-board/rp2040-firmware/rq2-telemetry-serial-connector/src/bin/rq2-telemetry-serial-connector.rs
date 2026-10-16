use clap::{Parser, Subcommand};
use eyre::bail;
use postcard::to_vec;
use rq2_telemetry_parser::{
    Bandwidth, CodingRate, CommandProcessor, DataProcessor, EncodedFrameWriter,
    Error as CommandError, IncomingDataProcessor, Module, Request, SelfContainedFrameParser,
    SpreadingFactor, Write,
};
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

use tokio_serial::{
    available_ports, SerialPortBuilderExt, SerialPortInfo, SerialPortType::UsbPort, SerialStream,
};
use tracing::{debug, error};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Port to open
    serial_port: Option<String>,
    /// Speed
    #[arg(short, default_value_t = 115200)]
    serial_speed: u32,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ping,
}

fn default_port() -> eyre::Result<String> {
    let mut res = vec![];
    for port in available_ports()? {
        if let UsbPort(_) = port.port_type {
            res.push(port);
        }
    }
    if res.len() == 1 {
        return Ok(res.get(0).unwrap().port_name.clone());
    }
    eyre::bail!("No serial port")
}

#[derive(Default)]
struct TestDataProcessor {
    data: std::vec::Vec<std::vec::Vec<u8>>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum TestDataProcessorError {
    SerialError,
}

impl DataProcessor for TestDataProcessor {
    type Error = TestDataProcessorError;

    fn process_data<'a>(&mut self, source: Module, data: &'a [u8]) -> Result<(), Self::Error> {
        let mut data_vec = std::vec![];
        data_vec.extend_from_slice(data);
        self.data.push(data_vec);
        Ok(())
    }
}

struct SerialPortWriteHelper<'a> {
    port: &'a mut SerialStream,
}

impl Write for SerialPortWriteHelper<'_> {
    type Error = std::io::Error;

    async fn write(&mut self, buffer: &[u8]) -> Result<(), Self::Error> {
        self.port.write_all(buffer).await
    }
}

async fn work(args: Cli, serial_port: String) -> eyre::Result<()> {
    debug!("serial_port: {serial_port}");
    let mut port = tokio_serial::new(serial_port, args.serial_speed).open_native_async()?;
    let mut parser = SelfContainedFrameParser::new();
    let mut tdp = TestDataProcessor::default();
    let mut processor = CommandProcessor::new(&mut tdp);
    let mut dispatcher = IncomingDataProcessor::new(&mut parser);
    let request = Request::Ping;
    let mut write_helper = SerialPortWriteHelper { port: &mut port };
    let mut encoded_writer = EncodedFrameWriter::new(&mut write_helper);
    let command = processor
        .send(Module::Lora1, request)
        .await
        .map_err(|_| eyre::eyre!("Can't process request!"))?;
    let output: heapless::Vec<u8, 256> = to_vec(&command).unwrap();
    encoded_writer
        .write(&output)
        .await
        .map_err(|_| eyre::eyre!("Can't write data!"))?;
    drop(encoded_writer);
    drop(write_helper);
    loop {
        let mut buffer = [0; 1];
        port.read(&mut buffer).await?;
        match dispatcher
            .feed(buffer[0])
            .await
            .map_err(|e| eyre::eyre!("dispatch error: {:#?}", e))?
        {
            Some(response) => {
                let response = processor
                    .process_response_or_timeout(Some(response))
                    .await
                    .map_err(|e| eyre::eyre!("response processing error: {:#?}", e))?;
                println!("{:#?}", response);
                break;
            }
            None => {}
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    env_logger::init();
    let port = cli.serial_port.clone().or_else(|| default_port().ok());
    match port {
        Some(port) => {
            work(cli, port).await?;
        }
        None => {
            error!("No port given or found");
        }
    }
    Ok(())
}
