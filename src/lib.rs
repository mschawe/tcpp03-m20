#![no_std]

mod control;
mod i2c;
mod register;

pub use control::*;
pub use register::*;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<I2CE, GPIOE> {
    Device(i2c::DeviceError<I2CE, GPIOE>),
    InvalidCommand,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PdRole {
    #[default]
    None,
    Sink,
    Source,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CcState {
    #[default]
    None,
    CC1,
    CC2,
}