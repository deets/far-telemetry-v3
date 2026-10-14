use defmt::{debug, error, info};
use embassy_rp::{
    peripherals::UART0,
    uart::{Async, Uart},
};
use rq2_telemetry_parser::{
    Command, FrameError, FrameProcessor, FrameResult, IncomingDataDispatcher, Payload, Request,
    ResponseProcessor, ResponseProcessorResult, SelfContainedFrameParser,
};

#[derive(Debug, Copy, Clone, PartialEq)]
enum TestResponseProcessorError {
    NothingWorks,
}

struct TestResponseProcessor {}

impl ResponseProcessor for TestResponseProcessor {
    type Error = TestResponseProcessorError;

    fn process_response_or_timeout<'a>(
        &mut self,
        command: Option<Command<'a>>,
    ) -> Result<ResponseProcessorResult, Self::Error> {
        let command = command.unwrap();
        match command.payload {
            Payload::Request {
                command_id,
                request,
            } => match request {
                Request::Ping => {
                    info!("got ping!");
                    return Ok(ResponseProcessorResult::Response(
                        rq2_telemetry_parser::Response::Pong,
                    ));
                }
                _ => todo!(),
            },
            _ => todo!(),
        };
        Err(TestResponseProcessorError::NothingWorks)
    }
}

async fn process_incoming(result: Result<FrameResult<'_>, FrameError>) {
    match result {
        Ok(result) => match result {
            FrameResult::Nothing => {}
            FrameResult::Frame(data) => {
                info!("got data: {:?}", data);
            }
        },
        Err(FrameError::ShortFrame) => debug!("ignoring short frame"),
        Err(e) => error!("Got error: {:?}", defmt::Debug2Format(&e)),
    }
}
pub async fn uart_task(uart: Uart<'static, UART0, Async>) {
    let mut parser = SelfContainedFrameParser::new();
    let mut processor = TestResponseProcessor {};
    let mut dispatcher = IncomingDataDispatcher::new(&mut processor, &mut parser);
    let (mut outgoing, mut incoming) = uart.split();
    loop {
        info!("test");
        outgoing.write(b"TEST");
    }
    loop {
        let mut buffer = [0; 1];
        info!("wait for incoming uart data");
        match incoming.read(&mut buffer).await {
            Ok(_) => dispatcher
                .feed(buffer[0])
                .await
                .expect("Can't process data"),
            Err(e) => error!("Got error: {:?}", e),
        }
    }
}
