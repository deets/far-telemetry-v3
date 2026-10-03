#![no_std]
use defmt::*;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Config, Spi};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::{GenericSx128xInterfaceVariant};
use lora_phy::sx128x::{self, Sx128x, Sx1281};
use lora_phy::{LoRa};
use lora_phy::{mod_params::*};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 2_450_000_000; // warning: set this appropriately for the region

 #[embassy_executor::task]
pub async fn receiver_task(mut lora2: LoRa<Sx128x<ExclusiveDevice<Spi<'static, SPI1, Async>, Output<'static>, Delay>, GenericSx128xInterfaceVariant<Output<'static>, Input<'static>>, Sx1281>, Delay>) {

    let mdltn_params = {
        match lora2.create_modulation_params(
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
    info!("modulation params set");
    let rx_pkt_params = {
        match lora2.create_rx_packet_params(4, false, 255, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };
    info!("transmission params set");
    
    let mut receiving_buffer = [00u8; 100];

    match lora2
        .prepare_for_rx(RxMode::Continuous, &mdltn_params, &rx_pkt_params)
        .await
    {
        Ok(()) => {}
        Err(err) => {
            info!("Radio error = {}", err);
            return;
        }
    };
    loop {
        match lora2.rx(&rx_pkt_params, &mut receiving_buffer).await {
            Ok((received_len, _rx_pkt_status)) => {
                error!("packet:{:?}", &receiving_buffer[0..received_len as usize]);
            }
            Err(err) => info!("rx unsuccessful = {}", err),
        }
    }
}
