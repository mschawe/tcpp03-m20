use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait as DigitalWait;
use embedded_hal_async::i2c::I2c;

use crate::CcState;
use crate::DeviceProtectionFlags;
use crate::Error;
use crate::PdRole;
use crate::device::PowerMode;
use crate::device::Tcpp03M20;
use crate::device::VconnSwitch;
use crate::device::field_sets;
use crate::interface::GpioInterace;
use crate::interface::I2cInterface;

/// Shorthand
type DeviceResult<T, I2cError> = Result<T, Error<I2cError>>;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct State {
    mode: PowerMode,
    pd: PdRole,
    cc: CcState,
    #[allow(unused)]
    vconn_drive: bool, // TODO: Vconn drive setting/unsetting
}

/// TCPP03-M20 Device Controller
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Device<Delay, I2C, I2CError, Gpo, Gpi>
where
    Delay: DelayNs,
    I2C: I2c<Error = I2CError>,
    Gpo: OutputPin,
    Gpi: InputPin,
{
    delay: Delay,
    gpio: GpioInterace<Gpi, Gpo>,
    regs: Tcpp03M20<I2cInterface<I2C, I2CError>>,
    default_pd: PdRole,
    state: State,
}

impl<Delay, I2C, I2CError, Gpo, Gpi> Device<Delay, I2C, I2CError, Gpo, Gpi>
where
    Delay: DelayNs,
    I2C: I2c<Error = I2CError>,
    Gpo: OutputPin,
    Gpi: InputPin,
{
    const RESET_MS: u32 = 5;
    /// t_DIS_VBUS
    const DISCHARGE_DELAY_MS: u32 = 250;

    /// Construct a new PD controller `Device` with owned peripherals
    pub fn new(
        delay: Delay,
        i2c: I2C,
        enable: Gpo,
        flgn: Gpi,
        i2c_address_vddio: bool,
        default_pd: PdRole,
    ) -> Self {
        let regs = Tcpp03M20::new(I2cInterface::new(i2c, i2c_address_vddio));
        let gpio = GpioInterace::new(enable, flgn);
        let state = State::default();

        Self {
            delay,
            gpio,
            regs,
            state,
            default_pd,
        }
    }

    /// Reset the controller by disabling and re-enabling it through the `EN` pin
    pub async fn reset(&mut self) -> DeviceResult<(), I2CError> {
        self.gpio.disable()?;
        self.delay.delay_ms(Self::RESET_MS).await;
        self.gpio.enable()?;

        self.state = State::default();
        Ok(())
    }

    /// Initialize the controller to `role`
    pub async fn init(&mut self) -> DeviceResult<(), I2CError> {
        self.reset().await?;

        self.control_read_write(|r| {
            r.set_power_mode(PowerMode::LowPower);
            r.set_vconn_switch(VconnSwitch::BothOpen);
        })
        .await?;

        self.state.mode = PowerMode::LowPower;
        self.state.cc = CcState::None;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Initialized!");

        Ok(())
    }

    /// Attach to a cable with a power role and CC line configuration
    pub async fn attach(&mut self, orientation: CcState) -> DeviceResult<(), I2CError> {
        if self.state.mode != PowerMode::LowPower {
            return Err(Error::InvalidCommand);
        }

        self.set_mode(PowerMode::Normal).await?;
        self.set_cc(orientation).await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Attached!");

        Ok(())
    }

    /// Detach the controller by putting in the `Unattached` state by:
    /// 1. Discharging Vconn and Vbus
    /// 2. Resetting CC & PD state
    ///
    /// This should only be called after a physical disconnection has been detected
    pub async fn detach(&mut self) -> DeviceResult<(), I2CError> {
        if self.state.mode != PowerMode::Normal {
            return Err(Error::InvalidCommand);
        }

        self.discharge().await?;

        self.set_pd(self.default_pd).await?;
        self.set_mode(PowerMode::LowPower).await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Detached!");

        Ok(())
    }

    /// Returns the current `DeviceProtectionFlags` that are raised.
    ///
    /// Will save the register read if `FLGn` is high
    pub async fn current_flags(&mut self) -> DeviceResult<DeviceProtectionFlags, I2CError> {
        Ok(match self.gpio.flgn()? {
            true => self.regs.flag().read_async().await?.into(),
            false => DeviceProtectionFlags(0x00),
        })
    }

    /// Set the controller mode
    async fn set_mode(&mut self, mode: PowerMode) -> DeviceResult<(), I2CError> {
        if self.state.mode == mode {
            return Ok(());
        }

        self.control_read_write(|r| {
            r.set_power_mode(mode);
        })
        .await?;
        self.state.mode = mode;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Set mode {:?}!", mode);

        Ok(())
    }

    /// Set controller V1 & V2 gates to adhere to `orientation`
    async fn set_cc(&mut self, orientation: CcState) -> DeviceResult<(), I2CError> {
        if self.state.cc == orientation {
            return Ok(());
        }

        self.control_read_write(|r| {
            r.set_vconn_switch(VconnSwitch::from(orientation));
        })
        .await?;
        self.state.cc = orientation;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Set orientation {:?}!", orientation);

        Ok(())
    }

    /// Set controller consumer and provider gates to adhere to `pd`
    async fn set_pd(&mut self, pd: PdRole) -> DeviceResult<(), I2CError> {
        if self.state.mode != PowerMode::Normal {
            return Err(Error::InvalidCommand);
        }

        if self.state.pd == pd {
            return Ok(());
        }

        self.control_read_write(|r| {
            match pd {
                PdRole::None => {
                    r.set_gate_driver_consumer(true); // GDC Open
                    r.set_gate_driver_provider(false); // GDP Open
                }
                PdRole::Sink => {
                    r.set_gate_driver_consumer(false); // GDC Close
                    r.set_gate_driver_provider(false); // GDP Open
                }
                PdRole::Source => {
                    r.set_gate_driver_consumer(true); // GDC Open
                    r.set_gate_driver_provider(true); // GDP Close
                }
            }
        })
        .await?;
        self.state.pd = pd;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: PD Role set to {:?}", pd);

        Ok(())
    }

    /// Discharge Vbus
    pub async fn discharge_vbus(&mut self) -> DeviceResult<(), I2CError> {
        self.control_read_write(|r| r.set_vbus_discharge(true))
            .await?;
        self.delay.delay_ms(Self::DISCHARGE_DELAY_MS).await;
        self.control_read_write(|r| r.set_vbus_discharge(false))
            .await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Discharged Vbus!");

        Ok(())
    }

    /// Discharge Vconn
    pub async fn discharge_vconn(&mut self) -> DeviceResult<(), I2CError> {
        // Disable vconn switch & start discharge
        self.control_read_write(|r| {
            r.set_vconn_discharge(true);
            r.set_vconn_switch(VconnSwitch::BothOpen);
        })
        .await?;

        self.delay.delay_ms(Self::DISCHARGE_DELAY_MS).await;

        // Re-enable vconn switch after discharge
        let vconn_switch = self.state.cc.into();
        self.control_read_write(|r| {
            r.set_vconn_discharge(false);
            r.set_vconn_switch(vconn_switch);
        })
        .await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Discharged Vconn!");

        Ok(())
    }

    /// Discharge Vbus and Vconn
    pub async fn discharge(&mut self) -> DeviceResult<(), I2CError> {
        self.discharge_vbus().await?;
        self.discharge_vconn().await?;
        Ok(())
    }

    /// Read the `Ack` register (current state), apply a modification to it,
    /// then write it to the `Control` register to update the device behavior
    async fn control_read_write(
        &mut self,
        f: impl FnOnce(&mut field_sets::Control),
    ) -> DeviceResult<(), I2CError> {
        let ack = self.regs.ack().read_async().await?;
        let mut control = field_sets::Control::from(ack);
        f(&mut control);
        self.regs.control().write_async(|_| control).await?;
        Ok(())
    }
}

impl<Delay, I2C, I2CError, Gpo, Gpi> Device<Delay, I2C, I2CError, Gpo, Gpi>
where
    Delay: DelayNs,
    I2C: I2c<Error = I2CError>,
    Gpo: OutputPin,
    Gpi: InputPin + DigitalWait,
{
    /// Wait for the device to raise an error flag
    pub async fn wait_flgn(&mut self) -> DeviceResult<DeviceProtectionFlags, I2CError> {
        self.gpio.wait_flgn().await?;
        let flags = self.regs.flag().read_async().await?;
        Ok(flags.into())
    }
}
