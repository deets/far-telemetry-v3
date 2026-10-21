use defmt::{debug, error};
use embassy_rp::{
    peripherals::UART0,
    uart::{Async, Uart, UartTx},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use heapless::Vec;
use postcard::to_vec;
use rq2_telemetry_parser::{
    encode_frame, seven_bit_encode, Command, FeedIncomingData, IncomingDataProcessor,
    SelfContainedFrameParser,
};

async fn send_response<'a>(uart: &mut UartTx<'static, UART0, Async>, response: Command) {
    debug!("send_response::start");
    let output: Vec<u8, 256> = to_vec(&response).expect("Can't transform to vec");
    let mut seven_bit_buffer = [0; 127];
    let seven_bit_encoded =
        seven_bit_encode(&mut seven_bit_buffer, &output).expect("Can't encode 7-bit");
    let mut encoded_buffer = [0; 127];
    let frame_encoded =
        encode_frame(&mut encoded_buffer, seven_bit_encoded).expect("Can't generate frame");
    uart.write(frame_encoded)
        .await
        .expect("Can't send response");
    debug!("send_response::end");
}

pub async fn uart_task(
    uart: Uart<'static, UART0, Async>,
    lora1_sender: Sender<'static, NoopRawMutex, Command, 4>,
    lora1_receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    lora2_sender: Sender<'static, NoopRawMutex, Command, 4>,
    lora2_receiver: Receiver<'static, NoopRawMutex, Command, 4>,
) {
    let mut parser = SelfContainedFrameParser::new();
    let mut dispatcher = IncomingDataProcessor::new(&mut parser);
    let (mut outgoing, mut incoming) = uart.split();
    loop {
        let mut buffer = [0; 1];
        debug!("wait for incoming uart data");
        match incoming.read(&mut buffer).await {
            Ok(_) => {
                debug!("{}", buffer[0]);
                match dispatcher.feed(buffer[0]).await {
                    Ok(Some(command)) => match command.destination {
                        rq2_telemetry_parser::Module::Lora1 => {
                            debug!("sending command: {:#?}", defmt::Debug2Format(&command));
                            lora1_sender.send(command).await;
                            let response = lora1_receiver.receive().await;
                            send_response(&mut outgoing, response).await;
                        }
                        rq2_telemetry_parser::Module::Lora2 => {
                            lora2_sender.send(command).await;
                            let response = lora2_receiver.receive().await;
                            send_response(&mut outgoing, response).await;
                        }
                    },
                    Ok(None) => {}
                    Err(e) => error!("Got error: {:?}", defmt::Debug2Format(&e)),
                }
            }
            Err(e) => error!("Got error: {:?}", e),
        }
    }
}
