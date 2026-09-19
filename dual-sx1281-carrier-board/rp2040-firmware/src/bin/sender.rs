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
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::sx126x::{Sx1262, Sx126x, TcxoCtrlVoltage};
use lora_phy::LoRa;
use lora_phy::{mod_params::*, sx126x};
use {defmt_rtt as _, panic_probe as _};

const LORA_FREQUENCY_IN_HZ: u32 = 433_000_000; // warning: set this appropriately for the region

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut rf_sw = Output::new(p.PIN_4.degrade(), Level::Low);
    let nss = Output::new(p.PIN_3.degrade(), Level::High);
    let reset = Output::new(p.PIN_15.degrade(), Level::High);
    let dio1 = Input::new(p.PIN_20.degrade(), Pull::None);
    let busy = Input::new(p.PIN_2.degrade(), Pull::None);

    let spi = Spi::new(
        p.SPI1,
        p.PIN_10,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config::default(),
    );
    rf_sw.set_high();
    let spi = ExclusiveDevice::new(spi, nss, Delay);
    info!("Created SPI");
    let config = sx126x::Config {
        chip: Sx1262,
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
        use_dcdc: true,
        rx_boost: false,
    };
    let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();
    let mut lora = LoRa::new(Sx126x::new(spi, iv, config), true, Delay).await.unwrap();
    info!("Created lora");
    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_12,
            Bandwidth::_500KHz,
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
        rf_sw.toggle();
        info!("prepare send packet");
        for b in buffer.iter_mut() {
            *b = packet_count as u8;
        }
        packet_count += 1;
        match lora
            .prepare_for_tx(&mdltn_params, &mut tx_pkt_params, 20, &buffer)
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
        Timer::after_millis(100).await;
    }
}
