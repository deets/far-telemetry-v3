use defmt::{debug, error, info};
use embassy_rp::{peripherals::UART0, uart::{Async, Uart}};
use rq2_telemetry_parser::{FrameError, FrameProcessor, FrameResult, SelfContainedFrameParser};


async fn process_incoming(result: Result<FrameResult<'_>, FrameError>) {
    match result {
        Ok(result) => match result {
            FrameResult::Nothing => {},
            FrameResult::Frame(data) => {
                info!("got data: {:?}", data);
            }
        },
        Err(FrameError::ShortFrame) => debug!("ignoring short frame"),
        Err(e) => error!("Got error: {:?}", defmt::Debug2Format(&e)),
    }
}
pub async fn uart_task(uart: Uart<'static, UART0, Async>) {
    let mut incoming_parser = SelfContainedFrameParser::new();
    let (_outgoing, mut incoming) = uart.split();
    //outgoing.write(buffer)
    loop {
        let mut buffer = [0; 1];
        match incoming.read(&mut buffer).await {
            Ok(_) => {
                process_incoming(incoming_parser.feed(buffer[0])).await;
            }
            Err(_) => todo!(),
        }
    }
}


