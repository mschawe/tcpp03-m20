use core::fmt::Display;

/// TCPP03-M20 Errors
#[derive(Debug, thiserror::Error)]
pub enum Error<I2C> {
    I2c(#[from] I2cError<I2C>),
    Gpio(#[from] GpioError),
    InvalidCommand,
}

/// I2C Error
#[derive(Debug, Clone, thiserror::Error)]
pub struct I2cError<Error>(pub Error);

impl<Error: embedded_hal_async::i2c::Error> Display for I2cError<Error> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "I2C Error: {:?}", self.0.kind())
    }
}

/// GPIO Pin Error
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error)]
pub struct GpioError;
