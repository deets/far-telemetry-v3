use defmt::{debug, error, info};
use embassy_rp::{
    peripherals::UART0,
    uart::{Async, Uart, UartTx},
};
use heapless::Vec;
use postcard::to_vec;
use rq2_telemetry_parser::{
    encode_frame, seven_bit_encode, Command, FeedIncomingData, FrameError, FrameProcessor,
    FrameResult, IncomingDataProcessor, Payload, Request, Response, SelfContainedFrameParser,
};

#[derive(Debug, Copy, Clone, PartialEq)]
enum TestResponseProcessorError {
    NothingWorks,
}

struct TestResponseProcessor {}

async fn process_incoming(result: Result<FrameResult<'_>, FrameError>) {
    match result {
        Ok(result) => match result {
            FrameResult::Nothing => {}
            FrameResult::Frame(data) => {
                debug!("got data: {:?}", data);
            }
        },
        Err(FrameError::ShortFrame) => debug!("ignoring short frame"),
        Err(e) => error!("Got error: {:?}", defmt::Debug2Format(&e)),
    }
}

async fn send_response<'a>(uart: &mut UartTx<'static, UART0, Async>, response: Command<'a>) {
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

pub async fn uart_task(uart: Uart<'static, UART0, Async>) {
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
                    Ok(Some(command)) => match command.payload {
                        Payload::Data(items) => todo!(),
                        Payload::Request {
                            command_id,
                            request,
                        } => match request {
                            Request::Ping => {
                                info!("got ping, responding with pong");
                                let response = Command {
                                    destination: command.destination,
                                    payload: Payload::Response {
                                        command_id,
                                        response: Response::Pong,
                                    },
                                };
                                send_response(&mut outgoing, response).await;
                            }
                            Request::Status => todo!(),
                            Request::Configure { .. } => todo!(),
                        },
                        Payload::Response {
                            command_id,
                            response,
                        } => todo!(),
                    },
                    Ok(None) => {}
                    Err(e) => error!("Got error: {:?}", defmt::Debug2Format(&e)),
                }
            }
            Err(e) => error!("Got error: {:?}", e),
        }
    }
}
