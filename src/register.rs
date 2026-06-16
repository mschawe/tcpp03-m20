use bitfield::bitfield;

pub trait Reg {
    const ADDR: u8;

    fn value(&self) -> u8;
    fn from(v: u8) -> Self;
}

pub trait ReadReg: Reg {}

pub trait WriteReg: Reg {}

/// Declares a read-only register.
#[macro_export]
macro_rules! read_reg {
    ($name:ident, $addr:expr) => {
 
        impl Reg for $name {
            const ADDR: u8 = $addr;

            fn value(&self) -> u8 { self.0 }
            fn from(v: u8) -> Self { Self(v) }
        }
 
        impl ReadReg for $name {}
    };
}
 
/// Declares a write-only register.
#[macro_export]
macro_rules! write_reg {
    ($name:ident, $addr:expr) => {
 
        impl Reg for $name {
            const ADDR: u8 = $addr;

            fn value(&self) -> u8 { self.0 }
            fn from(v: u8) -> Self { Self(v) }
        }
 
        impl WriteReg for $name {}
    };
}
 
/// Declares a read-write register.
#[macro_export]
macro_rules! rw_reg {
    ($name:ident, $addr:expr) => {

        impl Reg for $name {
            const ADDR: u8 = $addr;

            fn value(&self) -> u8 { self.0 }
            fn from(v: u8) -> Self { Self(v) }
        }
 
        impl ReadReg for $name {}

        impl WriteReg for $name {}
    };
}

bitfield! {
    /// Writing register (address = 0)
    pub struct ControlReg(u8);
    impl Debug;

    /// **VCONND**
    /// - 1: Discharge ON
    /// - 0: Discharge OFF
    pub _, vconn_discharge: 7;
    /// **VBUSD**
    /// - 1: Discharge ON
    /// - 0: Discharge OFF
    pub _, vbus_discharge: 6;
    /// **Power Mode** (PM2, 1)
    /// - 0b00: Hibernate
    /// - 0b10: Low Power
    /// - 0b01: Normal
    /// - 0b11: Not Used
    pub _, power_mode: 5, 4;
    /// **GDC**
    /// - 1: Switch load opened
    /// - 0: Switch load closed
    pub _, gate_driver_consumer: 3;
    /// **GDP**
    /// - 1: Switch load closed
    /// - 1: Switch load opened
    pub _, gate_driver_provider: 2;
    /// **VCONN** (V2, V1)
    /// - 0b00: (Open, Open)
    /// - 0b10: (Close, Open)
    /// - 0b01: (Open, Close)
    /// - 0b11: (Open, Open)
    pub _, vconn_switch: 1, 0;

}
write_reg!(ControlReg, 0x00);

bitfield! {
    /// Flags are set to `1` when active
    pub struct AckReg(u8);
    impl Debug;

    pub vconn_discharge_ack, _: 7;
    pub vbus_discharge_ack, _: 6;
    pub power_mode_ack, _: 5, 4;
    pub gate_driver_consumer_ack, _: 3;
    pub gate_driver_provider_ack, _: 2;
    pub vconn_switch_ack, _: 1, 0;
}
read_reg!(AckReg, 0x01);

bitfield! {
    /// Flags are set to `1` when active
    pub struct FlagReg(u8);
    impl Debug;

    pub id, _: 7;
    pub flgn_vbus_ok, _: 5;
    pub flgn_ovp_cc, _: 4;
    pub flgn_otp, _: 3;
    pub flgn_ovp_vbus, _: 2;
    pub flgn_ocp_vbus, _: 1;
    pub flgn_ocp_vconn, _: 0;
}
read_reg!(FlagReg, 0x02);