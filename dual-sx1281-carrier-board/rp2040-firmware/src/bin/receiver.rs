//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Config, Spi};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::{GenericSx128xInterfaceVariant};
use lora_phy::sx128x::{self, Sx128x, Sx1281};
use lora_phy::{LoRa};
use lora_phy::{mod_params::*};
use rf_range_test::receiver_task;
use {defmt_rtt as _, panic_probe as _};



#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let lora2_nss = Output::new(p.PIN_13.degrade(), Level::High);
    let lora2_busy = Input::new(p.PIN_17.degrade(), Pull::None);
    let lora2_reset = Output::new(p.PIN_18.degrade(), Level::High);
    let lora2_irq = Input::new(p.PIN_16.degrade(), Pull::None);
    let lora2_rx_en = Output::new(p.PIN_15.degrade(), Level::Low);
    let lora2_tx_en = Output::new(p.PIN_19.degrade(), Level::Low);

    
    let lora2_spi = Spi::new(
        p.SPI1,
        p.PIN_14,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH2,
        p.DMA_CH3,
        Config::default(),
    );

    let lora2_spi = ExclusiveDevice::new(lora2_spi, lora2_nss, Delay);
    let lora2_config = sx128x::Config {
        chip: Sx1281,
        tcxo_used: true,
        tx_boost: true,
        rx_boost: true,
    };
    let lora2_iv = GenericSx128xInterfaceVariant::new(lora2_reset, lora2_irq, lora2_busy, Some(lora2_rx_en), Some(lora2_tx_en)).unwrap();
    info!("Created GenericSx128xInterfaceVariant");
    
    let lora2 = LoRa::new(Sx128x::new(lora2_spi, lora2_iv, lora2_config), false, Delay)
        .await
        .unwrap();
    info!("Created lora");
    spawner.spawn(receiver_task(lora2)).expect("Can't spawn receiver task");
}




