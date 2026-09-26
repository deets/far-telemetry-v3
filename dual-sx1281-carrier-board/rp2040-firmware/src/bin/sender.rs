//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::spi::{Config, Spi};
use embassy_time::{Delay, Timer};
use embedded_hal_1::spi::SpiDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::sx128x::{Sx1281, Sx128x,};
use lora_phy::LoRa;
use lora_phy::{mod_params::*, sx128x};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 2_400_000_000; // warning: set this appropriately for the region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let nss = Output::new(p.PIN_13.degrade(), Level::High);
    let busy = Input::new(p.PIN_17.degrade(), Pull::None);
    let mut reset = Output::new(p.PIN_18.degrade(), Level::High);
    let irq = Input::new(p.PIN_16.degrade(), Pull::None);
    let rx_en = Output::new(p.PIN_22.degrade(), Level::Low);
    let tx_en = Output::new(p.PIN_15.degrade(), Level::Low);

    
    let spi = Spi::new(
        p.SPI1,
        p.PIN_14,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );

    reset.set_high();
    Timer::after_millis(10).await;    
    reset.set_low();
    Timer::after_millis(10).await;
    reset.set_high();
    
    let mut spi = ExclusiveDevice::new(spi, nss, Delay);
    // loop {
    //     // read register, address, nop, 1 byte payload
    //     let command = [0x19, 0x08, 0x91, 0x00, 0x00];
    //     let mut buf = [0; 5];
    //     let _ = spi.transfer(&mut buf, &command);
    //     info!("firmware version: {:?}", buf);
    //     Timer::after_millis(100).await;
    // }
    info!("Created SPI, waiting 5s ");
    Timer::after_millis(5000).await;
    info!("running");    
    let config = sx128x::Config {
        chip: Sx1281,
        tcxo_used: true,
        tx_boost: true,
        rx_boost: false,
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
            CodingRate::_4_8,
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
        match lora.create_tx_packet_params(4, false, true, false, &mdltn_params) {
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
        match lora
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
        match lora.tx().await {
            Ok(()) => {
                info!("TX DONE");
            }
            Err(err) => {
                info!("Radio error = {}", err);
                return;
            }
        };

        match lora.sleep(false).await {
            Ok(()) => info!("Sleep successful"),
            Err(err) => info!("Sleep unsuccessful = {}", err),
        }
        Timer::after_millis(10).await;
    }
}
