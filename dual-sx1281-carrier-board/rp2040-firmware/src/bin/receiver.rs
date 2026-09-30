//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::spi::{Config, Spi};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::{GenericSx128xInterfaceVariant};
use lora_phy::sx128x::{self, Sx128x, Sx1281};
use lora_phy::{LoRa};
use lora_phy::{mod_params::*};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 2_450_000_000; // warning: set this appropriately for the region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let nss = Output::new(p.PIN_13.degrade(), Level::High);
    let busy = Input::new(p.PIN_17.degrade(), Pull::None);
    let reset = Output::new(p.PIN_18.degrade(), Level::High);
    let irq = Input::new(p.PIN_16.degrade(), Pull::None);
    let rx_en = Output::new(p.PIN_15.degrade(), Level::Low);
    let tx_en = Output::new(p.PIN_19.degrade(), Level::Low);

    
    let spi = Spi::new(
        p.SPI1,
        p.PIN_14,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );

    let spi = ExclusiveDevice::new(spi, nss, Delay);
    let config = sx128x::Config {
        chip: Sx1281,
        tcxo_used: true,
        tx_boost: true,
        rx_boost: true,
    };
    let iv = GenericSx128xInterfaceVariant::new(reset, irq, busy, Some(rx_en), Some(tx_en)).unwrap();
    info!("Created GenericSx128xInterfaceVariant");
    
    let mut lora = LoRa::new(Sx128x::new(spi, iv, config), false, Delay)
        .await
        .unwrap();
    info!("Created lora");

    let mdltn_params = {
        match lora.create_modulation_params(
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
    let mut rx_pkt_params = {
        match lora.create_rx_packet_params(4, false, 255, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };
    info!("transmission params set");
    
    let mut receiving_buffer = [00u8; 100];

    match lora
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
        receiving_buffer = [00u8; 100];
        match lora.rx(&rx_pkt_params, &mut receiving_buffer).await {
            Ok((received_len, _rx_pkt_status)) => {
                info!("packet:{:?}", &receiving_buffer[0..received_len as usize]);
            }
            Err(err) => info!("rx unsuccessful = {}", err),
        }
    }
}
