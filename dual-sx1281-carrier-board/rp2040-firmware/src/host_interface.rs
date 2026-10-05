use embassy_rp::{peripherals::UART0, uart::{Async, Uart}};
use rq2_telemetry_parser::process_uart_data;

pub async fn uart_task(mut uart: Uart<'static, UART0, Async>) {
    loop {
        uart.write(b"HALLO").await.expect("Can't write to uart");
       process_uart_data(100);
    }
}


