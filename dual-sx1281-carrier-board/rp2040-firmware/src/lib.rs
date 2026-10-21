#![no_std]
use defmt::{debug, error};
use embassy_futures::select::select;
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::{SPI0, SPI1};
use embassy_rp::spi::{Async, Spi};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_params::{SpreadingFactor, *};
use lora_phy::mod_traits::RadioKind;
use lora_phy::sx128x::{Sx1281, Sx128x};
use lora_phy::DelayNs;
use lora_phy::LoRa;
use rq2_telemetry_parser::ErrorCode::InvalidCommand;
use rq2_telemetry_parser::{
    Bandwidth as Rq2Bandwidth, CodingRate as Rq2CodingRate, Command, ErrorCode, Mode, Module,
    Request, Response, SpreadingFactor as Rq2SpreadingFactor,
};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

pub mod host_interface;

#[derive(Debug, PartialEq)]
enum Error {
    ConfigurationError,
    RadioError(RadioError),
}

#[derive(Debug)]
struct LoRaConfiguration {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
    coding_rate: CodingRate,
    frequency: usize,
    output_power: i32,
}

impl TryFrom<Request> for LoRaConfiguration {
    type Error = Error;

    fn try_from(value: Request) -> Result<Self, Self::Error> {
        match value {
            Request::Configure {
                bandwidth,
                spreading_factor,
                coding_rate,
                frequency,
                output_power,
            } => {
                error!("incoming frequency:{}", frequency);
                Ok(Self {
                    bandwidth: bw_convert(bandwidth),
                    spreading_factor: sf_convert(spreading_factor),
                    coding_rate: cr_convert(coding_rate),
                    frequency: frequency * 1000_000,
                    output_power,
                })
            }
            _ => Err(Error::ConfigurationError),
        }
    }
}

struct LoraConfigurationManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora: LoRa<RK, DLY>,
    configuration: Option<LoRaConfiguration>,
    destination: Module,
}

struct LoraSendingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora: LoRa<RK, DLY>,
    configuration: LoRaConfiguration,
    destination: Module,
    modulation_params: ModulationParams,
    tx_packet_params: PacketParams,
    output_power: i32,
}

struct LoraReceivingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora: LoRa<RK, DLY>,
    configuration: LoRaConfiguration,
    destination: Module,
    rx_packet_params: PacketParams,
}

enum CommandResult<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    Configuration(LoraConfigurationManager<RK, DLY>),
    Sending(LoraSendingManager<RK, DLY>),
    Receiving(LoraReceivingManager<RK, DLY>),
}

impl From<RadioError> for Error {
    fn from(value: RadioError) -> Self {
        Self::RadioError(value)
    }
}

struct DebugModulationParams<'a> {
    modulation_params: &'a ModulationParams,
}

impl<'a> core::fmt::Debug for DebugModulationParams<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DebugModulationParams")
            .field("spreading_factor", &self.modulation_params.spreading_factor)
            .field("bandwidth", &self.modulation_params.bandwidth)
            .field("coding_rate", &self.modulation_params.coding_rate)
            .field("frequency_in_hz", &self.modulation_params.frequency_in_hz)
            .finish()
    }
}

struct DebugPacketParams<'a> {
    packet_params: &'a PacketParams,
}

impl<'a> core::fmt::Debug for DebugPacketParams<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DebugPacketParams")
            .field("preamble_length", &self.packet_params.preamble_length)
            .field("implicit_header", &self.packet_params.implicit_header)
            .field("payload_length", &self.packet_params.payload_length)
            .field("crc_on", &self.packet_params.crc_on)
            .field("iq_inverted", &self.packet_params.iq_inverted)
            .finish()
    }
}

impl<RK, DLY> LoraSendingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    async fn send(
        mut self,
        receiver: &mut Receiver<'static, NoopRawMutex, Command, 4>,
        sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
        payload_receiver: &mut Receiver<'static, NoopRawMutex, MessageType, 4>,
    ) -> Result<LoraConfigurationManager<RK, DLY>, Error> {
        loop {
            match select(receiver.receive(), payload_receiver.receive()).await {
                embassy_futures::select::Either::First(command) => {
                    match command.payload {
                        rq2_telemetry_parser::Payload::Request {
                            command_id,
                            request,
                        } => match request {
                            Request::Ping => {
                                send_response(sender, self.destination, command_id, Response::Pong)
                                    .await?;
                            }
                            Request::Stop => {
                                send_response(sender, self.destination, command_id, Response::Ack)
                                    .await?;
                                break;
                            }
                            Request::Status => {
                                send_response(
                                    sender,
                                    self.destination,
                                    command_id,
                                    Response::Status(self.status()),
                                )
                                .await?;
                            }
                            _ => {
                                send_response(
                                    sender,
                                    self.destination,
                                    command_id,
                                    Response::Error(ErrorCode::InvalidCommand),
                                )
                                .await?;
                            }
                        },
                        // This is caught by the surrounding host interdace
                        rq2_telemetry_parser::Payload::Data(items) => unreachable!(),
                        rq2_telemetry_parser::Payload::Response { .. } => {
                            error!("Received Response command");
                        }
                    }
                }
                embassy_futures::select::Either::Second(payload) => {
                    self.lora
                        .prepare_for_tx(
                            &self.modulation_params,
                            &mut self.tx_packet_params,
                            self.output_power,
                            &payload,
                        )
                        .await?;
                    self.lora.tx().await?;
                }
            }
        }
        Ok(LoraConfigurationManager {
            lora: self.lora,
            configuration: Some(self.configuration),
            destination: self.destination,
        })
    }
}

impl<RK, DLY> LoraReceivingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    async fn receive(
        mut self,
        receiver: &mut Receiver<'static, NoopRawMutex, Command, 4>,
        sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
        payload_sender: &mut Sender<'static, NoopRawMutex, MessageType, 4>,
    ) -> Result<LoraConfigurationManager<RK, DLY>, Error> {
        error!("Start receiving data");
        loop {
            let mut buffer = [0; 255];
            match select(
                receiver.receive(),
                self.lora.rx(&self.rx_packet_params, &mut buffer),
            )
            .await
            {
                embassy_futures::select::Either::First(command) => {
                    match command.payload {
                        rq2_telemetry_parser::Payload::Request {
                            command_id,
                            request,
                        } => match request {
                            Request::Ping => {
                                send_response(sender, self.destination, command_id, Response::Pong)
                                    .await?;
                            }
                            Request::Stop => {
                                send_response(sender, self.destination, command_id, Response::Ack)
                                    .await?;
                                break;
                            }
                            Request::Status => {
                                send_response(
                                    sender,
                                    self.destination,
                                    command_id,
                                    Response::Status(self.status()),
                                )
                                .await?;
                            }
                            _ => {
                                send_response(
                                    sender,
                                    self.destination,
                                    command_id,
                                    Response::Error(ErrorCode::InvalidCommand),
                                )
                                .await?;
                            }
                        },
                        // This is caught by the surrounding host interdace
                        rq2_telemetry_parser::Payload::Data(items) => unreachable!(),
                        rq2_telemetry_parser::Payload::Response { .. } => {
                            error!("Received Response command");
                        }
                    }
                }
                embassy_futures::select::Either::Second(rx_result) => match rx_result {
                    Ok((len, packet_status)) => {
                        let mut message: MessageType = MessageType::new();
                        match message.extend_from_slice(&buffer[0..len as usize]) {
                            Ok(_) => match payload_sender.try_send(message) {
                                Ok(_) => {}
                                Err(_) => error!("Can't send payload"),
                            },
                            Err(_) => todo!(),
                        }
                    }
                    Err(_) => {
                        error!("Can't receive packet");
                        break;
                    }
                },
            }
        }
        Ok(LoraConfigurationManager {
            lora: self.lora,
            configuration: Some(self.configuration),
            destination: self.destination,
        })
    }
}

impl<RK, DLY> LoraConfigurationManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn set_configuration(&mut self, request: Request) -> Result<(), Error> {
        let configuration = request.try_into()?;
        error!(
            "incoming configuration: {:#?}",
            defmt::Debug2Format(&request)
        );
        self.configuration = Some(configuration);
        Ok(())
    }

    async fn start_sending(
        mut self,
        sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
        command_id: u8,
    ) -> Result<CommandResult<RK, DLY>, Error> {
        if let Some(
            configuration @ LoRaConfiguration {
                bandwidth,
                spreading_factor,
                coding_rate,
                frequency,
                output_power,
            },
        ) = self.configuration
        {
            let modulation_params = self.lora.create_modulation_params(
                spreading_factor,
                bandwidth,
                coding_rate,
                frequency as u32,
            )?;
            let tx_packet_params =
                self.lora
                    .create_tx_packet_params(4, false, true, true, &modulation_params)?;

            send_response(sender, self.destination, command_id, Response::Ack).await?;
            Ok(CommandResult::Sending(LoraSendingManager {
                configuration,
                lora: self.lora,
                destination: self.destination,
                modulation_params,
                tx_packet_params,
                output_power,
            }))
        } else {
            let response = Response::Error(ErrorCode::NoConfiguration);
            send_response(sender, self.destination, command_id, response).await?;
            Ok(CommandResult::Configuration(self))
        }
    }

    async fn start_receiving(
        mut self,
        sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
        command_id: u8,
    ) -> Result<CommandResult<RK, DLY>, Error> {
        if let Some(
            configuration @ LoRaConfiguration {
                bandwidth,
                spreading_factor,
                coding_rate,
                frequency,
                output_power,
            },
        ) = self.configuration
        {
            let modulation_params = self.lora.create_modulation_params(
                spreading_factor,
                bandwidth,
                coding_rate,
                frequency as u32,
            )?;
            let rx_packet_params =
                self.lora
                    .create_rx_packet_params(4, false, 140, true, true, &modulation_params)?;

            self.lora
                .prepare_for_rx(RxMode::Continuous, &modulation_params, &rx_packet_params)
                .await?;

            send_response(sender, self.destination, command_id, Response::Ack).await?;
            Ok(CommandResult::Receiving(LoraReceivingManager {
                configuration,
                lora: self.lora,
                destination: self.destination,
                rx_packet_params,
            }))
        } else {
            let response = Response::Error(ErrorCode::NoConfiguration);
            send_response(sender, self.destination, command_id, response).await?;
            Ok(CommandResult::Configuration(self))
        }
    }

    async fn process_command(
        mut self,
        sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
        command: Command,
    ) -> Result<CommandResult<RK, DLY>, Error> {
        match command.payload {
            rq2_telemetry_parser::Payload::Request {
                command_id,
                request,
            } => match request {
                Request::Ping => {
                    debug!("ping for {:#?}", defmt::Debug2Format(&self.destination));
                    send_response(sender, self.destination, command_id, Response::Pong).await?;
                    Ok(CommandResult::Configuration(self))
                }
                Request::Status => {
                    send_response(
                        sender,
                        self.destination,
                        command_id,
                        Response::Status(self.status()),
                    )
                    .await?;
                    Ok(CommandResult::Configuration(self))
                }
                Request::Configure { .. } => {
                    send_response(
                        sender,
                        self.destination,
                        command_id,
                        match self.set_configuration(request) {
                            Ok(_) => Response::Ack,
                            Err(_) => Response::Error(ErrorCode::CantSetConfiguration),
                        },
                    )
                    .await?;
                    Ok(CommandResult::Configuration(self))
                }
                Request::Stop => {
                    send_response(
                        sender,
                        self.destination,
                        command_id,
                        Response::Error(ErrorCode::InvalidCommand),
                    )
                    .await?;
                    Ok(CommandResult::Configuration(self))
                }
                Request::Send => self.start_sending(sender, command_id).await,
                Request::Receive => self.start_receiving(sender, command_id).await,
            },
            _ => {
                error!(
                    "Received unexpected command: {:#?}",
                    defmt::Debug2Format(&command)
                );
                Ok(CommandResult::Configuration(self))
            }
        }
    }
}

trait HasStatus {
    fn status(&self) -> Mode;
}

impl<RK, DLY> HasStatus for LoraConfigurationManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn status(&self) -> Mode {
        Mode::Idle
    }
}

impl<RK, DLY> HasStatus for LoraSendingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn status(&self) -> Mode {
        Mode::Tx
    }
}

impl<RK, DLY> HasStatus for LoraReceivingManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn status(&self) -> Mode {
        Mode::RxContinuous
    }
}

async fn send_response(
    sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
    destination: rq2_telemetry_parser::Module,
    command_id: u8,
    response: Response,
) -> Result<(), Error> {
    let response = Command {
        destination,
        payload: rq2_telemetry_parser::Payload::Response {
            command_id,
            response,
        },
    };
    sender.send(response).await;
    Ok(())
}

async fn lora_work<RK: RadioKind, DLY: DelayNs>(
    mut manager: LoraConfigurationManager<RK, DLY>,
    mut receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    mut sender: Sender<'static, NoopRawMutex, Command, 4>,
    mut payload_receiver: Receiver<'static, NoopRawMutex, MessageType, 4>,
    mut payload_sender: Sender<'static, NoopRawMutex, MessageType, 4>,
) -> Result<(), Error> {
    loop {
        let command = receiver.receive().await;
        manager = match manager.process_command(&mut sender, command).await? {
            CommandResult::Configuration(lora_configuration_manager) => lora_configuration_manager,
            CommandResult::Sending(lora_sending_manager) => {
                lora_sending_manager
                    .send(&mut receiver, &mut sender, &mut payload_receiver)
                    .await?
            }
            CommandResult::Receiving(lora_receiving_manager) => {
                lora_receiving_manager
                    .receive(&mut receiver, &mut sender, &mut payload_sender)
                    .await?
            }
        };
    }
}

#[embassy_executor::task]
pub async fn lora1_task(
    lora1: LoRa<
        Sx128x<
            ExclusiveDevice<Spi<'static, SPI0, Async>, Output<'static>, Delay>,
            GenericSx128xInterfaceVariant<Output<'static>, Input<'static>>,
            Sx1281,
        >,
        Delay,
    >,
    receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    sender: Sender<'static, NoopRawMutex, Command, 4>,
    payload_receiver: Receiver<'static, NoopRawMutex, MessageType, 4>,
    payload_sender: Sender<'static, NoopRawMutex, MessageType, 4>,
) {
    let manager = LoraConfigurationManager {
        lora: lora1,
        configuration: None,
        destination: Module::Lora1,
    };

    match lora_work(manager, receiver, sender, payload_receiver, payload_sender).await {
        Ok(_) => {}
        Err(e) => {
            error!(
                "error working with module, halting: {:#?}",
                defmt::Debug2Format(&e)
            );
        }
    }
}

#[embassy_executor::task]
pub async fn lora2_task(
    lora2: LoRa<
        Sx128x<
            ExclusiveDevice<Spi<'static, SPI1, Async>, Output<'static>, Delay>,
            GenericSx128xInterfaceVariant<Output<'static>, Input<'static>>,
            Sx1281,
        >,
        Delay,
    >,
    receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    sender: Sender<'static, NoopRawMutex, Command, 4>,
    payload_receiver: Receiver<'static, NoopRawMutex, MessageType, 4>,
    payload_sender: Sender<'static, NoopRawMutex, MessageType, 4>,
) {
    let manager = LoraConfigurationManager {
        lora: lora2,
        configuration: None,
        destination: Module::Lora2,
    };
    match lora_work(manager, receiver, sender, payload_receiver, payload_sender).await {
        Ok(_) => {}
        Err(e) => {
            error!(
                "error working with module, halting: {:#?}",
                defmt::Debug2Format(&e)
            );
        }
    }
}

fn sf_convert(spreading_factor: Rq2SpreadingFactor) -> SpreadingFactor {
    match spreading_factor {
        Rq2SpreadingFactor::_6 => SpreadingFactor::_6,
        Rq2SpreadingFactor::_7 => SpreadingFactor::_7,
        Rq2SpreadingFactor::_8 => SpreadingFactor::_8,
        Rq2SpreadingFactor::_9 => SpreadingFactor::_9,
        Rq2SpreadingFactor::_10 => SpreadingFactor::_10,
        Rq2SpreadingFactor::_11 => SpreadingFactor::_11,
        Rq2SpreadingFactor::_12 => SpreadingFactor::_12,
    }
}
fn bw_convert(bandwidth: Rq2Bandwidth) -> Bandwidth {
    match bandwidth {
        Rq2Bandwidth::_200KHz => Bandwidth::_200KHz,
        Rq2Bandwidth::_400KHz => Bandwidth::_400KHz,
        Rq2Bandwidth::_800KHz => Bandwidth::_800KHz,
        Rq2Bandwidth::_1600KHz => Bandwidth::_1600KHz,
    }
}

fn cr_convert(coding_rate: Rq2CodingRate) -> CodingRate {
    match coding_rate {
        Rq2CodingRate::_4_5 => CodingRate::_4_5,
        Rq2CodingRate::_4_6 => CodingRate::_4_6,
        Rq2CodingRate::_4_7 => CodingRate::_4_7,
        Rq2CodingRate::_4_8 => CodingRate::_4_8,
    }
}

pub type MessageType = heapless::Vec<u8, 128>;
pub static LORA1_SEND_CHANNEL: StaticCell<Channel<NoopRawMutex, Command, 4>> = StaticCell::new();
pub static LORA1_RECEIVE_CHANNEL: StaticCell<Channel<NoopRawMutex, Command, 4>> = StaticCell::new();
pub static LORA2_SEND_CHANNEL: StaticCell<Channel<NoopRawMutex, Command, 4>> = StaticCell::new();
pub static LORA2_RECEIVE_CHANNEL: StaticCell<Channel<NoopRawMutex, Command, 4>> = StaticCell::new();

pub static LORA1_PAYLOAD_SEND_CHANNEL: StaticCell<Channel<NoopRawMutex, MessageType, 4>> =
    StaticCell::new();
pub static LORA1_PAYLOAD_RECEIVE_CHANNEL: StaticCell<Channel<NoopRawMutex, MessageType, 4>> =
    StaticCell::new();
pub static LORA2_PAYLOAD_SEND_CHANNEL: StaticCell<Channel<NoopRawMutex, MessageType, 4>> =
    StaticCell::new();
pub static LORA2_PAYLOAD_RECEIVE_CHANNEL: StaticCell<Channel<NoopRawMutex, MessageType, 4>> =
    StaticCell::new();
