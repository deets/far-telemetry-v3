#![no_std]
mod commands;
mod crc;
mod framing;
pub use commands::*;
pub use framing::*;

pub trait Write {
    type Error;
    async fn write(&mut self, buffer: &[u8]) -> Result<(), Self::Error>;
}

pub trait Read {
    type Error;
    async fn read_all(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error>;
}
