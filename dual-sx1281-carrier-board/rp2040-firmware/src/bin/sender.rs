//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Async, Config, Spi};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::sx128x::{Sx1281, Sx128x,};
use lora_phy::LoRa;
use lora_phy::{mod_params::*, sx128x};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 2_450_000_000; // warning: set this appropriately for the region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let lora1_nss = Output::new(p.PIN_5.degrade(), Level::High);
    let lora1_busy = Input::new(p.PIN_7.degrade(), Pull::None);
    let lora1_reset = Output::new(p.PIN_8.degrade(), Level::High);
    let lora1_irq = Input::new(p.PIN_6.degrade(), Pull::None);
    let lora1_rx_en = Output::new(p.PIN_10.degrade(), Level::Low);
    let lora1_tx_en = Output::new(p.PIN_9.degrade(), Level::Low);

    
    let lora1_spi = Spi::new(
        p.SPI0,
        p.PIN_2,
        p.PIN_3,
        p.PIN_4,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );

    
    let lora1_spi = ExclusiveDevice::new(lora1_spi, lora1_nss, Delay);
    info!("running");    
    let lora1_config = sx128x::Config {
        chip: Sx1281,
        tcxo_used: true,
        tx_boost: true,
        rx_boost: false,
    };
    let lora1_iv = GenericSx128xInterfaceVariant::new(lora1_reset, lora1_irq, lora1_busy, Some(lora1_rx_en), Some(lora1_tx_en)).unwrap();
    info!("Created GenericSx128xInterfaceVariant");
    
    let lora1 = LoRa::new(Sx128x::new(lora1_spi, lora1_iv, lora1_config), false, Delay)
        .await
        .unwrap();
    info!("Created lora");
    sender_task(lora1).await;
}

async fn sender_task(mut lora1: LoRa<Sx128x<ExclusiveDevice<Spi<'_, SPI0, Async>, Output<'_>, Delay>, GenericSx128xInterfaceVariant<Output<'_>, Input<'_>>, Sx1281>, Delay>) {
    let mdltn_params = {
        match lora1.create_modulation_params(
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
    let mut tx_pkt_params = {
        match lora1.create_tx_packet_params(4, false, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        }
    };
    info!("transmission params set");
    let mut buffer = [0x00; 64];
    let mut packet_count = 0xf0;
    loop {
        info!("prepare send packet");
        for b in buffer.iter_mut() {
            *b = packet_count as u8;
        }
        packet_count += 1;
        match lora1
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 10, &buffer)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };
        info!("transmit");
        match lora1.tx().await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora1.sleep(false).await {
            Ok(()) => info!("Sleep successful"),
            Err(err) => info!("Sleep unsuccessful = {}", err),
        }
        Timer::after_millis(10).await;
    }
}
