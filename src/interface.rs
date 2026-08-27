use core::marker::PhantomData;

use device_driver::AsyncRegisterInterface;
use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::digital::Wait as DigitalWait;
use embedded_hal_async::i2c::I2c;

use crate::error::GpioError;
use crate::error::I2cError;

/// [6.3.1]
/// The LSB bit of the I2C address is set when
/// connecting pin I2C_ADD to GND (0) or VddIIO
const I2C_ADDR_GND: u8 = 0b011_0100;

/// [6.3.1]
/// The LSB bit of the I2C address is set when
/// connecting pin I2C_ADD to GND (0) or VddIIO
const I2C_ADDR_VDDIIO: u8 = 0b011_0101;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct GpioInterace<Gpi, Gpo> {
    enable: Gpo,
    flgn: Gpi,
}

impl<Gpi, Gpo> GpioInterace<Gpi, Gpo> {
    pub const fn new(enable: Gpo, flgn: Gpi) -> Self {
        Self { enable, flgn }
    }
}

impl<Gpi, Gpo> GpioInterace<Gpi, Gpo>
where
    Gpo: OutputPin,
{
    pub fn enable(&mut self) -> Result<(), GpioError> {
        self.enable.set_high().map_err(|_| GpioError)
    }

    pub fn disable(&mut self) -> Result<(), GpioError> {
        self.enable.set_low().map_err(|_| GpioError)
    }
}

impl<Gpi, Gpo> GpioInterace<Gpi, Gpo>
where
    Gpi: InputPin,
{
    // FIXME: make a `logical-pin` crate to not have to hardcode this inversion
    pub fn flgn(&mut self) -> Result<bool, GpioError> {
        self.flgn.is_low().map_err(|_| GpioError)
    }
}

impl<Gpi, Gpo> GpioInterace<Gpi, Gpo>
where
    Gpi: DigitalWait,
{
    pub async fn wait_flgn(&mut self) -> Result<(), GpioError> {
        self.flgn.wait_for_low().await.map_err(|_| GpioError)
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct I2cInterface<Bus, Error> {
    addr: u8,
    bus: Bus,
    err: PhantomData<Error>,
}

impl<Bus, Error> I2cInterface<Bus, Error> {
    pub const fn new(bus: Bus, i2c_addr_vdd: bool) -> Self {
        let addr = match i2c_addr_vdd {
            false => I2C_ADDR_GND,
            true => I2C_ADDR_VDDIIO,
        };

        Self {
            addr,
            bus,
            err: PhantomData,
        }
    }
}

impl<Bus, Error> AsyncRegisterInterface for I2cInterface<Bus, Error>
where
    Bus: I2c<Error = Error>,
{
    type Error = I2cError<Error>;
    type AddressType = u8;

    async fn write_register(
        &mut self,
        address: Self::AddressType,
        size_bits: u32,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        debug_assert!(size_bits == 8);
        debug_assert!(data.len() == 1);

        self.bus
            .write(self.addr, &[address, data[0]])
            .await
            .map_err(I2cError)
    }

    async fn read_register(
        &mut self,
        address: Self::AddressType,
        size_bits: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        debug_assert!(size_bits == 8);
        debug_assert!(data.len() == 1);

        self.bus
            .write_read(self.addr, &[address], data)
            .await
            .map_err(I2cError)
    }
}
