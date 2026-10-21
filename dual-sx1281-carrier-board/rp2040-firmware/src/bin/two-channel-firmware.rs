//! This example runs on the Raspberry Pi Pico with a Waveshare board containing a Semtech Sx1262 radio.
//! It demonstrates LORA P2P send functionality.

#![no_std]
#![no_main]

use defmt::*;
use dual_sx1281_carrier_board::{
    host_interface::uart_task, lora1_task, lora2_task, LORA1_RECEIVE_CHANNEL, LORA1_SEND_CHANNEL,
    LORA2_RECEIVE_CHANNEL, LORA2_SEND_CHANNEL,
};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::UART0;
use embassy_rp::spi::{Config, Spi};
use embassy_rp::uart::{Config as UartConfig, InterruptHandler, Uart};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::iv::GenericSx128xInterfaceVariant;
use lora_phy::sx128x::{self, Sx1281, Sx128x};
use lora_phy::LoRa;
use rq2_telemetry_parser::Command;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UART0_IRQ => InterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let lora1_nss = Output::new(p.PIN_5.degrade(), Level::High);
    let lora1_busy = Input::new(p.PIN_7.degrade(), Pull::None);
    let lora1_reset = Output::new(p.PIN_8.degrade(), Level::High);
    let lora1_irq = Input::new(p.PIN_6.degrade(), Pull::None);
    let lora1_rx_en = Output::new(p.PIN_10.degrade(), Level::Low);
    let lora1_tx_en = Output::new(p.PIN_9.degrade(), Level::Low);

    let (tx_pin, rx_pin, uart) = (p.PIN_0, p.PIN_1, p.UART0);
    let uart = Uart::new(
        uart,
        tx_pin,
        rx_pin,
        Irqs,
        p.DMA_CH4,
        p.DMA_CH5,
        UartConfig::default(),
    );

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
    let lora1_iv = GenericSx128xInterfaceVariant::new(
        lora1_reset,
        lora1_irq,
        lora1_busy,
        Some(lora1_rx_en),
        Some(lora1_tx_en),
    )
    .unwrap();
    info!("Created GenericSx128xInterfaceVariant");

    let lora1 = LoRa::new(Sx128x::new(lora1_spi, lora1_iv, lora1_config), false, Delay)
        .await
        .unwrap();
    info!("Created lora1");

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
    let lora2_iv = GenericSx128xInterfaceVariant::new(
        lora2_reset,
        lora2_irq,
        lora2_busy,
        Some(lora2_rx_en),
        Some(lora2_tx_en),
    )
    .unwrap();
    info!("Created GenericSx128xInterfaceVariant");

    let lora1_send_channel = Channel::<NoopRawMutex, Command, 4>::new();
    let static_lora1_send_channel = LORA1_SEND_CHANNEL.init(lora1_send_channel);
    let lora1_receive_channel = Channel::<NoopRawMutex, Command, 4>::new();
    let static_lora1_receive_channel = LORA1_RECEIVE_CHANNEL.init(lora1_receive_channel);

    let lora2 = LoRa::new(Sx128x::new(lora2_spi, lora2_iv, lora2_config), false, Delay)
        .await
        .unwrap();
    let lora2_send_channel = Channel::<NoopRawMutex, Command, 4>::new();
    let static_lora2_send_channel = LORA2_SEND_CHANNEL.init(lora2_send_channel);
    let lora2_receive_channel = Channel::<NoopRawMutex, Command, 4>::new();
    let static_lora2_receive_channel = LORA2_RECEIVE_CHANNEL.init(lora2_receive_channel);

    spawner
        .spawn(lora1_task(
            lora1,
            static_lora1_send_channel.receiver(),
            static_lora1_receive_channel.sender(),
        ))
        .expect("Can't spawn receiver task");

    spawner
        .spawn(lora2_task(
            lora2,
            static_lora2_send_channel.receiver(),
            static_lora2_receive_channel.sender(),
        ))
        .expect("Can't spawn receiver task");

    uart_task(
        uart,
        static_lora1_send_channel.sender(),
        static_lora1_receive_channel.receiver(),
        static_lora2_send_channel.sender(),
        static_lora2_receive_channel.receiver(),
    )
    .await;
    loop {
        Timer::after_millis(10).await;
    }
}
