use clap::{Parser, Subcommand, ValueEnum};
use eyre::bail;
use postcard::to_vec;
use rq2_telemetry_parser::{
    Command, CommandProcessor, DataProcessor, EncodedFrameWriter, FeedIncomingData,
    IncomingDataProcessor, Request, Response, ResponseProcessor, SelfContainedFrameParser, Write,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{sleep, timeout},
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
    #[arg(long)]
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

#[derive(ValueEnum, Clone, Copy, PartialEq, Debug)]
enum SpreadingFactor {
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
}

impl From<SpreadingFactor> for rq2_telemetry_parser::SpreadingFactor {
    fn from(value: SpreadingFactor) -> Self {
        match value {
            SpreadingFactor::_6 => rq2_telemetry_parser::SpreadingFactor::_6,
            SpreadingFactor::_7 => rq2_telemetry_parser::SpreadingFactor::_7,
            SpreadingFactor::_8 => rq2_telemetry_parser::SpreadingFactor::_8,
            SpreadingFactor::_9 => rq2_telemetry_parser::SpreadingFactor::_9,
            SpreadingFactor::_10 => rq2_telemetry_parser::SpreadingFactor::_10,
            SpreadingFactor::_11 => rq2_telemetry_parser::SpreadingFactor::_11,
            SpreadingFactor::_12 => rq2_telemetry_parser::SpreadingFactor::_12,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Debug)]
enum Bandwidth {
    _200KHz,
    _400KHz,
    _800KHz,
    _1600KHz,
}

impl From<Bandwidth> for rq2_telemetry_parser::Bandwidth {
    fn from(value: Bandwidth) -> Self {
        match value {
            Bandwidth::_200KHz => rq2_telemetry_parser::Bandwidth::_200KHz,
            Bandwidth::_400KHz => rq2_telemetry_parser::Bandwidth::_400KHz,
            Bandwidth::_800KHz => rq2_telemetry_parser::Bandwidth::_800KHz,
            Bandwidth::_1600KHz => rq2_telemetry_parser::Bandwidth::_1600KHz,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Debug)]
enum CodingRate {
    _4_5,
    _4_6,
    _4_7,
    _4_8,
}

impl From<CodingRate> for rq2_telemetry_parser::CodingRate {
    fn from(value: CodingRate) -> Self {
        match value {
            CodingRate::_4_5 => rq2_telemetry_parser::CodingRate::_4_5,
            CodingRate::_4_6 => rq2_telemetry_parser::CodingRate::_4_6,
            CodingRate::_4_7 => rq2_telemetry_parser::CodingRate::_4_7,
            CodingRate::_4_8 => rq2_telemetry_parser::CodingRate::_4_8,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    Ping,
    StartSending,
    Configure {
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        output_power: i32,
        frequency: usize,
    },
    Send {
        payload: String,
    },
    Receive,
    /// Ask the module to go back to idle
    Stop,
    /// Setup both modules (ignoring destination)
    /// to the same configuration, and put one
    /// into Tx, the other into RxContinuous.
    /// Then send out packets and receive them.
    AirLoop {
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        output_power: i32,
        frequency: usize,
    },
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
) -> eyre::Result<Response> {
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
                        if let Some(response) = command_processor
                            .process_response_or_timeout(Some(response))
                            .await
                            .map_err(|e| eyre::eyre!("response processing error: {:#?}", e))?
                        {
                            return Ok(response);
                        }
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
}

async fn transact(
    port: &mut SerialStream,
    destination: Module,
    request: Request,
) -> eyre::Result<Response> {
    let mut parser = SelfContainedFrameParser::new();
    let mut tdp = TestDataProcessor::default();
    let mut command_processor = CommandProcessor::new(&mut tdp);
    let mut incoming_processor = IncomingDataProcessor::new(&mut parser);
    let command = command_processor
        .create_and_expect_command(destination.into(), request)
        .await
        .map_err(|_| eyre::eyre!("Can't process request!"))?;
    send_command(port, command).await?;
    receive_command(port, &mut incoming_processor, &mut command_processor).await
}

async fn send_payload(
    port: &mut SerialStream,
    destination: Module,
    payload: &[u8],
) -> eyre::Result<()> {
    let mut data: heapless::Vec<u8, 128> = heapless::Vec::new();
    data.extend_from_slice(payload)
        .expect("Buffer overflow in send_payload");
    let command = Command {
        destination: destination.into(),
        payload: rq2_telemetry_parser::Payload::Data(data),
    };
    send_command(port, command).await
}

async fn receive_payload(port: &mut SerialStream) -> eyre::Result<()> {
    let mut parser = SelfContainedFrameParser::new();
    let mut incoming_processor = IncomingDataProcessor::new(&mut parser);
    loop {
        let mut buffer = [0; 1];
        match timeout(Duration::from_secs(2), port.read(&mut buffer)).await {
            Ok(_) => match incoming_processor.feed(buffer[0]).await {
                Ok(Some(command)) => match command {
                    Command {
                        payload: rq2_telemetry_parser::Payload::Data(data),
                        ..
                    } => {
                        println!(
                            "received payload: {:?} from {:?}",
                            &data, command.destination
                        );
                        return Ok(());
                    }
                    _ => {
                        bail!("Received stray command: {:#?}", command);
                    }
                },
                Ok(None) => {}
                Err(_) => bail!("Error parsing incoming payload"),
            },
            Err(_) => bail!("Timeout waiting for incoming payload"),
        }
    }
}

async fn run_airloop(port: &mut SerialStream, airloop: Commands) -> eyre::Result<()> {
    if let Commands::AirLoop {
        spreading_factor,
        bandwidth,
        coding_rate,
        output_power,
        frequency,
    } = airloop
    {
        // Just reset the modules, ignore error
        let _ = transact(port, Module::Lora1, Request::Stop).await;
        let _ = transact(port, Module::Lora2, Request::Stop).await;

        let configuration_request = Request::Configure {
            bandwidth: bandwidth.into(),
            spreading_factor: spreading_factor.into(),
            coding_rate: coding_rate.into(),
            output_power,
            frequency,
        };
        transact(port, Module::Lora1, configuration_request).await?;
        transact(port, Module::Lora1, Request::Send).await?;
        transact(port, Module::Lora2, configuration_request).await?;
        transact(port, Module::Lora2, Request::Receive).await?;
        loop {
            send_payload(port, Module::Lora1, b"This is a test").await?;
            receive_payload(port).await?;
            sleep(Duration::from_secs(2)).await;
        }
    } else {
        unreachable!();
    }
}

async fn work(args: Cli, serial_port: String) -> eyre::Result<()> {
    debug!("serial_port: {serial_port}");
    let mut port = tokio_serial::new(serial_port, args.serial_speed).open_native_async()?;
    let mut parser = SelfContainedFrameParser::new();
    match args.command {
        airloop @ Commands::AirLoop { .. } => run_airloop(&mut port, airloop).await,
        Commands::Send { payload } => {
            let mut payload_vector: heapless::Vec<u8, 128> = heapless::Vec::new();
            payload_vector
                .extend_from_slice(&payload.into_bytes())
                .map_err(|_| eyre::eyre!("Can't create payload"))?;
            send_command(
                &mut port,
                Command {
                    destination: args.destination.into(),
                    payload: rq2_telemetry_parser::Payload::Data(payload_vector),
                },
            )
            .await?;
            Ok(())
        }
        _ => {
            let mut tdp = TestDataProcessor::default();
            let mut command_processor = CommandProcessor::new(&mut tdp);
            let mut incoming_processor = IncomingDataProcessor::new(&mut parser);
            let command = command_processor
                .create_and_expect_command(
                    args.destination.into(),
                    match args.command {
                        Commands::Ping => Request::Ping,
                        Commands::StartSending => Request::Send,
                        Commands::Configure {
                            spreading_factor,
                            bandwidth,
                            coding_rate,
                            output_power,
                            frequency,
                        } => Request::Configure {
                            bandwidth: bandwidth.into(),
                            spreading_factor: spreading_factor.into(),
                            coding_rate: coding_rate.into(),
                            output_power,
                            frequency,
                        },
                        Commands::Receive => Request::Receive,
                        Commands::Stop => Request::Stop,
                        Commands::AirLoop { .. } => unreachable!(),
                        Commands::Send { .. } => unreachable!(),
                    },
                )
                .await
                .map_err(|_| eyre::eyre!("Can't process request!"))?;
            debug!("command: {:#?}", command);
            send_command(&mut port, command).await?;
            println!(
                "{:#?}",
                receive_command(&mut port, &mut incoming_processor, &mut command_processor).await?
            );
            Ok(())
        }
    }
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
