use tokio_serial::{available_ports, SerialPortInfo, SerialPortType::UsbPort};

fn default_port() -> eyre::Result<Option<SerialPortInfo>> {
    let mut res = vec![];
    for port in available_ports()? {
        if let UsbPort(_) = port.port_type {
            res.push(port);
        }
    }
    if res.len() == 1 {
        return Ok(res.get(0).cloned());
    }
    Ok(None)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    if let Some(port) = default_port()? {
        println!("could open {port:?}");
    }
    Ok(())
}
