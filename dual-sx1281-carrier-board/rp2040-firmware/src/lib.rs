#![no_std]
use defmt::*;
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Spi};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::mod_params::*;
use lora_phy::mod_traits::RadioKind;
use lora_phy::sx128x::{Sx1281, Sx128x};
use lora_phy::DelayNs;
use lora_phy::LoRa;
use {defmt_rtt as _, panic_probe as _};

pub mod host_interface;

const LORA_FREQUENCY_IN_HZ: u32 = 2_450_000_000; // warning: set this appropriately for the region

struct LoraManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora: LoRa<RK, DLY>,
}

impl<RK, DLY> LoraManager<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    pub fn create_modulation_params(
        &mut self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency_in_hz: u32,
    ) -> Result<ModulationParams, RadioError> {
        self.lora.create_modulation_params(
            spreading_factor,
            bandwidth,
            coding_rate,
            frequency_in_hz,
        )
    }
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
) {
    let mut manager = LoraManager { lora: lora2 };
    let mdltn_params = {
        match manager.create_modulation_params(
            SpreadingFactor::_12,
            Bandwidth::_400KHz,
            CodingRate::_4_7,
            LORA_FREQUENCY_IN_HZ,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };
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
