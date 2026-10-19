use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use postcard::to_vec;
use rq2_telemetry_parser::{
    Command, CommandProcessor, DataProcessor, EncodedFrameWriter, FeedIncomingData,
    IncomingDataProcessor, Request, ResponseProcessor, SelfContainedFrameParser, Write,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

use tokio_serial::{available_ports, SerialPortBuilderExt, SerialPortType::UsbPort, SerialStream};
use tracing::{debug, error};

#[derive(ValueEnum, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Module {
    Lora1,
    Lora2,
}

impl From<Module> for rq2_telemetry_parser::Module {
    fn from(value: Module) -> Self {
        match value {
            Module::Lora1 => rq2_telemetry_parser::Module::Lora1,
            Module::Lora2 => rq2_telemetry_parser::Module::Lora2,
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Port to open
    serial_port: Option<String>,
    /// Module to refer to
    #[arg(long)]
    destination: Module,
    /// Speed
    #[arg(short, default_value_t = 115200)]
    serial_speed: u32,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ping,
    StartSending,
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
enum TestDataProcessorError {}

impl DataProcessor for TestDataProcessor {
    type Error = TestDataProcessorError;

    fn process_data<'a>(
        &mut self,
        _source: rq2_telemetry_parser::Module,
        data: &'a [u8],
    ) -> Result<(), Self::Error> {
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

async fn send_command(port: &mut SerialStream, command: Command) -> eyre::Result<()> {
    let output: heapless::Vec<u8, 256> = to_vec(&command)?;
    let mut write_helper = SerialPortWriteHelper { port };
    let mut encoded_writer = EncodedFrameWriter::new(&mut write_helper);
    encoded_writer
        .write(&output)
        .await
        .map_err(|_| eyre::eyre!("Can't write data!"))
}

async fn receive_command(
    port: &mut SerialStream,
    incoming_processor: &mut impl FeedIncomingData,
    command_processor: &mut impl ResponseProcessor<TestDataProcessorError>,
) -> eyre::Result<()> {
    loop {
        let mut buffer = [0; 1];
        match timeout(Duration::from_millis(100), port.read(&mut buffer)).await {
            Ok(_) => {
                match incoming_processor
                    .feed(buffer[0])
                    .await
                    .map_err(|e| eyre::eyre!("dispatch error: {:#?}", e))?
                {
                    Some(response) => {
                        let response = command_processor
                            .process_response_or_timeout(Some(response))
                            .await
                            .map_err(|e| eyre::eyre!("response processing error: {:#?}", e))?;
                        println!("{:#?}", response);
                        break;
                    }
                    None => {}
                }
            }
            Err(_) => {
                let _ = command_processor
                    .process_response_or_timeout(None)
                    .await
                    .map_err(|e| eyre::eyre!("response processing error: {:#?}", e))?;
            }
        }
    }
    Ok(())
}

async fn work(args: Cli, serial_port: String) -> eyre::Result<()> {
    debug!("serial_port: {serial_port}");
    let mut port = tokio_serial::new(serial_port, args.serial_speed).open_native_async()?;
    let mut parser = SelfContainedFrameParser::new();
    let mut tdp = TestDataProcessor::default();
    let mut command_processor = CommandProcessor::new(&mut tdp);
    let mut incoming_processor = IncomingDataProcessor::new(&mut parser);
    let command = command_processor
        .create_and_expect_command(
            args.destination.into(),
            match args.command {
                Commands::Ping => Request::Ping,
                Commands::StartSending => Request::Send,
            },
        )
        .await
        .map_err(|_| eyre::eyre!("Can't process request!"))?;
    send_command(&mut port, command).await?;
    receive_command(&mut port, &mut incoming_processor, &mut command_processor).await
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
