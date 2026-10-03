use embassy_rp::{peripherals::UART0, uart::{Async, Uart}};

pub async fn uart_task(mut uart: Uart<'static, UART0, Async>) {
    loop {
        uart.write(b"HALLO").await.expect("Can't write to uart");
    }
}
