#![no_std]
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Spi};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::mod_params::*;
use lora_phy::mod_traits::RadioKind;
use lora_phy::sx128x::{Sx1281, Sx128x};
use lora_phy::DelayNs;
use lora_phy::LoRa;
use rq2_telemetry_parser::{
    Bandwidth, CodingRate, Command, ErrorCode, Mode, Module, Request, Response, SpreadingFactor,
};
use {defmt_rtt as _, panic_probe as _};

pub mod host_interface;

const LORA_FREQUENCY_IN_HZ: u32 = 2_450_000_000; // warning: set this appropriately for the region

#[derive(Debug, Copy, Clone, PartialEq)]
enum Error {
    ConfigurationError,
}

struct LoRaConfiguration {
    bandwidth: Bandwidth,
    spreading_factor: SpreadingFactor,
    coding_rate: CodingRate,
    frequency: usize,
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
            } => Ok(Self {
                bandwidth,
                spreading_factor,
                coding_rate,
                frequency,
            }),
            _ => Err(Error::ConfigurationError),
        }
    }
}

struct LoraManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora: LoRa<RK, DLY>,
    configuration: Option<LoRaConfiguration>,
}

impl<RK, DLY> LoraManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    // fn create_modulation_params(
    //     &mut self,
    //     spreading_factor: SpreadingFactor,
    //     bandwidth: Bandwidth,
    //     coding_rate: CodingRate,
    //     frequency_in_hz: u32,
    // ) -> Result<ModulationParams, RadioError> {
    //     self.lora.create_modulation_params(
    //         spreading_factor,
    //         bandwidth,
    //         coding_rate,
    //         frequency_in_hz,
    //     )
    // }

    fn set_configuration(&mut self, request: Request) -> Result<(), Error> {
        //self.configuration = Some(request.into()?);
        Ok(())
    }
}

trait HasStatus {
    fn status(&self) -> Mode;
}

impl<RK, DLY> HasStatus for LoraManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    fn status(&self) -> Mode {
        Mode::Idle
    }
}

async fn send_response(
    sender: &mut Sender<'static, NoopRawMutex, Command, 4>,
    destination: rq2_telemetry_parser::Module,
    command_id: u8,
    response: Response,
) {
    let response = Command {
        destination,
        payload: rq2_telemetry_parser::Payload::Response {
            command_id,
            response,
        },
    };
    sender.send(response).await;
}

#[embassy_executor::task]
pub async fn receiver_task(
    mut lora2: LoRa<
        Sx128x<
            ExclusiveDevice<Spi<'static, SPI1, Async>, Output<'static>, Delay>,
            GenericSx128xInterfaceVariant<Output<'static>, Input<'static>>,
            Sx1281,
        >,
        Delay,
    >,
    receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    mut sender: Sender<'static, NoopRawMutex, Command, 4>,
) {
    let mut manager = LoraManager {
        lora: lora2,
        configuration: None,
    };

    loop {
        let command = receiver.receive().await;
        match command.payload {
            rq2_telemetry_parser::Payload::Data(items) => todo!(),
            rq2_telemetry_parser::Payload::Request {
                command_id,
                request,
            } => match request {
                Request::Ping => {
                    send_response(&mut sender, Module::Lora2, command_id, Response::Pong).await;
                }
                Request::Status => {
                    send_response(
                        &mut sender,
                        Module::Lora2,
                        command_id,
                        Response::Status(manager.status()),
                    )
                    .await;
                }
                Request::Configure { .. } => {
                    send_response(
                        &mut sender,
                        Module::Lora2,
                        command_id,
                        match manager.set_configuration(request) {
                            Ok(_) => Response::Ack,
                            Err(_) => Response::Error(ErrorCode::CantSetConfiguration),
                        },
                    )
                    .await;
                }
            },
            rq2_telemetry_parser::Payload::Response {
                command_id,
                response,
            } => todo!(),
        }
    }
    // let mdltn_params = {
    //     match manager.create_modulation_params(
    //         SpreadingFactor::_12,
    //         Bandwidth::_400KHz,
    //         CodingRate::_4_7,
    //         LORA_FREQUENCY_IN_HZ,
    //     ) {
    //         Ok(mp) => mp,
    //         Err(err) => {
    //             info!("Radio error = {}", err);
    //             return;
    //         }
    //     }
    // };
    // info!("modulation params set");
    // let rx_pkt_params = {
    //     match lora2.create_rx_packet_params(4, false, 255, true, false, &mdltn_params) {
    //         Ok(pp) => pp,
    //         Err(err) => {
    //             info!("Radio error = {}", err);
    //             return;
    //         }
    //     }
    // };
    // info!("transmission params set");

    // let mut receiving_buffer = [00u8; 100];

    // match lora2
    //     .prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
    //     .await
    // {
    //     Ok(()) => {}
    //     Err(err) => {
    //         info!("Radio error = {}", err);
    //         return;
    //     }
    // };
    // loop {
    //     match lora2.rx(&rx_pkt_params, &mut receiving_buffer).await {
    //         Ok((received_len, _rx_pkt_status)) => {
    //             error!("packet:{:?}", &receiving_buffer[0..received_len as usize]);
    //         }
    //         Err(err) => info!("rx unsuccessful = {}", err),
    //     }
    // }
}
