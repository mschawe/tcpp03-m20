//! TCPP03-M20 USB-C PD Dual Role Gate Controller Driver
#![no_std]

pub mod control;
pub use control::Device;
pub mod error;
pub use error::*;

mod device;
mod interface;

use core::fmt::Display;

/// Power delivery role for the device to assume
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PdRole {
    #[default]
    None,
    Sink,
    Source,
}

/// Which CC line is being used for PD communication
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CcState {
    #[default]
    None,
    CC1,
    CC2,
}

bitfield::bitfield! {
    /// Errors thrown from device protection failsafes
    #[derive(Clone, Copy, thiserror::Error)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct DeviceProtectionFlags(u8);
    impl Debug;

    vbus_bad, _: 5;
    over_voltage_cc, _: 4;
    over_temperature, _: 3;
    over_voltage_vbus, _: 2;
    over_current_vbus, _: 1;
    over_current_vconn, _: 0;
}

impl Display for DeviceProtectionFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<device::field_sets::Flag> for DeviceProtectionFlags {
    fn from(value: device::field_sets::Flag) -> Self {
        use device_driver::FieldSet;
        let inner = value.get_inner_buffer()[0] ^ (1 << 5);
        Self(inner)
    }
}
