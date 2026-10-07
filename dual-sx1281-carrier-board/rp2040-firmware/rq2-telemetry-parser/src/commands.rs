use core::ops::Deref;
use heapless::Vec;
use postcard::{from_bytes, to_vec};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Module {
    Lora1,
    Lora2,
}

#[derive(Serialize, Debug, Eq, PartialEq)]
pub struct Command<'a> {
    destination: Module,
    payload: Payload<'a>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Payload<'a> {
    Data(&'a [u8]),
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn test_serialization() {
        let data = b"DIEZ WAS HERE";
        let command = Command {
            destination: Module::Lora1,
            payload: Payload::Data(data.as_slice()),
        };
        let output: Vec<u8, 256> = to_vec(&command).unwrap();
        std::println!("{:?}", output);
    }
}
