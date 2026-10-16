#![no_std]
mod commands;
mod crc;
mod framing;
use core::future::Future;

pub use commands::*;
pub use framing::*;

pub trait Write {
    type Error;
    fn write(&mut self, buffer: &[u8]) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait Read {
    type Error;
    fn read_all(
        &mut self,
        buffer: &mut [u8],
    ) -> impl Future<Output = Result<usize, Self::Error>> + Send;
}
