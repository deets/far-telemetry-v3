use defmt::{debug, error};
use embassy_futures::select::{select3, Either3};
use embassy_rp::{
    peripherals::UART0,
    uart::{Async, Uart, UartTx},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use heapless::Vec;
use postcard::to_vec;
use rq2_telemetry_parser::{
    encode_frame, seven_bit_encode, Command, FeedIncomingData, IncomingDataProcessor, Module,
    Payload, SelfContainedFrameParser,
};

use crate::MessageType;

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

async fn send_payload<'a>(
    uart: &mut UartTx<'static, UART0, Async>,
    destination: Module,
    payload: Vec<u8, 128>,
) {
    let command = Command {
        destination,
        payload: Payload::Data(payload),
    };
    send_response(uart, command).await;
}

pub async fn uart_task(
    uart: Uart<'static, UART0, Async>,
    lora1_sender: Sender<'static, NoopRawMutex, Command, 4>,
    lora1_receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    lora1_payload_sender: Sender<'static, NoopRawMutex, MessageType, 4>,
    lora1_payload_receiver: Receiver<'static, NoopRawMutex, MessageType, 4>,
    lora2_sender: Sender<'static, NoopRawMutex, Command, 4>,
    lora2_receiver: Receiver<'static, NoopRawMutex, Command, 4>,
    lora2_payload_sender: Sender<'static, NoopRawMutex, MessageType, 4>,
    lora2_payload_receiver: Receiver<'static, NoopRawMutex, MessageType, 4>,
) {
    let mut parser = SelfContainedFrameParser::new();
    let mut dispatcher = IncomingDataProcessor::new(&mut parser);
    let (mut outgoing, mut incoming) = uart.split();
    loop {
        let mut buffer = [0; 1];
        debug!("wait for incoming uart data");
        match select3(
            incoming.read(&mut buffer),
            lora1_payload_receiver.receive(),
            lora2_payload_receiver.receive(),
        )
        .await
        {
            Either3::First(uart_result) => match uart_result {
                Ok(_) => {
                    debug!("{}", buffer[0]);
                    match dispatcher.feed(buffer[0]).await {
                        Ok(Some(Command {
                            destination: Module::Lora1,
                            payload: Payload::Data(data),
                        })) => {
                            let _ = lora1_payload_sender.try_send(data);
                        }
                        Ok(Some(Command {
                            destination: Module::Lora2,
                            payload: Payload::Data(data),
                        })) => {
                            let _ = lora2_payload_sender.try_send(data);
                        }
                        Ok(Some(command)) => match command.destination {
                            Module::Lora1 => {
                                debug!("sending command: {:#?}", defmt::Debug2Format(&command));
                                lora1_sender.send(command).await;
                                let response = lora1_receiver.receive().await;
                                send_response(&mut outgoing, response).await;
                            }
                            Module::Lora2 => {
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
            },
            Either3::Second(message) => {
                send_payload(&mut outgoing, Module::Lora1, message).await;
            }
            Either3::Third(message) => {
                send_payload(&mut outgoing, Module::Lora2, message).await;
            }
        }
    }
}
