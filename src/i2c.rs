
use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c;

use crate::register::ReadReg;
use crate::register::WriteReg;

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
pub enum DeviceError<I2CE, GPIOE> {
    I2c(I2CE),
    Gpio(GPIOE),
}

pub type DeviceResult<T, I2CE, GPIOE> = Result<T, DeviceError<I2CE, GPIOE>>;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct I2cDevice<DELAY, I2C, I2CE, GPO, GPI, GPIOE>
where
    DELAY: DelayNs,
    I2C: I2c<Error = I2CE>,
    GPO: OutputPin<Error = GPIOE>,
    GPI: InputPin<Error = GPIOE>,
{
    i2c: I2C,
    enable: GPO,
    flgn: GPI,
    delay: DELAY,
    addr: u8,
}

impl <DELAY, I2C, I2CE, GPO, GPI, GPIOE> I2cDevice<DELAY, I2C, I2CE, GPO, GPI, GPIOE>
where
    DELAY: DelayNs,
    I2C: I2c<Error = I2CE>,
    GPO: OutputPin<Error = GPIOE>,
    GPI: InputPin<Error = GPIOE>,
{

    pub fn new_i2c(delay: DELAY, i2c: I2C, enable: GPO, flgn: GPI, i2c_add_vddio: bool) -> Self {
        let addr = match i2c_add_vddio {
            false => I2C_ADDR_GND,
            true => I2C_ADDR_VDDIIO
        };

        Self { i2c, enable, flgn, delay, addr }
    }

    pub fn enable(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        self.enable.set_high().map_err(DeviceError::Gpio)
    }

    pub fn disable(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        self.enable.set_low().map_err(DeviceError::Gpio)
    }

    pub fn flgn(&mut self) -> DeviceResult<bool, I2CE, GPIOE> {
        self.flgn.is_high().map_err(DeviceError::Gpio)
    }

    pub async fn read<R: ReadReg>(&mut self) -> DeviceResult<R, I2CE, GPIOE> {
        let mut read = [0u8; 1];
        self.i2c.write_read(self.addr, &[R::ADDR], &mut read)
            .await
            .map_err(DeviceError::I2c)?;

        Ok(R::from(read[0]))
    }

    pub async fn write<R: WriteReg>(&mut self, write: R) -> DeviceResult<(), I2CE, GPIOE> {
        self.i2c.write(self.addr, &[R::ADDR, write.value()])
            .await
            .map_err(DeviceError::I2c)
    }

    pub async fn delay_ms(&mut self, ms: u32) {
        self.delay.delay_ms(ms).await;
    }

}

