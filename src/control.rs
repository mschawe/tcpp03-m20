use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::i2c::I2c;

use crate::register::*;
use crate::i2c::I2cDevice;
use crate::Error;
use crate::PdRole;
use crate::CcState;

// ""Shorthand""
type DeviceResult<T, I2CE, GPIOE> = Result<T, Error<I2CE, GPIOE>>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Private enum over `ControlReg[PM2, PM1]`
/// - `[Table 11]` Power modes versus power supply
/// - `[Table 12]` TCPP03-M20 states versus power modes
enum Mode {
    #[default]
    Hibernate = 0,
    LowPower = 1,
    Normal = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Private enum over `ControlReg[V2, V1]`
enum VconnSwitch {
    #[default]
    None = 0,
    VC1 = 1,
    VC2 = 2,
}

impl From<CcState> for VconnSwitch {
    fn from(value: CcState) -> Self {
        // VC is **OPPOSITE** of CC
        match value {
            CcState::CC1 => Self::VC2,
            CcState::CC2 => Self::VC1,
            CcState::None => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct State {
    mode: Mode,
    pd: PdRole,
    cc: CcState,
    vconn_drive: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Device<DELAY, I2C, I2CE, GPO, GPI, GPIOE>
where
    DELAY: DelayNs,
    I2C: I2c<Error = I2CE>,
    GPO: OutputPin<Error = GPIOE>,
    GPI: InputPin<Error = GPIOE>,
{
    inner: I2cDevice<DELAY, I2C, I2CE, GPO, GPI, GPIOE>,
    state: State,
}

impl <DELAY, I2C, I2CE, GPO, GPI, GPIOE> Device<DELAY, I2C, I2CE, GPO, GPI, GPIOE>
where
    DELAY: DelayNs,
    I2C: I2c<Error = I2CE>,
    GPO: OutputPin<Error = GPIOE>,
    GPI: InputPin<Error = GPIOE>,
{
    const DEFAULT_PD_ROLE: PdRole = PdRole::Sink;
    const RESET_MS: u32 = 5;
    /// t_DIS_VBUS
    const DISCHARGE_DELAY_MS: u32 = 250;

    /// Construct a new PD controller `Device` with owned peripherals
    pub fn new(delay: DELAY, i2c: I2C, enable: GPO, flgn: GPI, i2c_add_vddio: bool) -> Self {
        Self {
            inner: I2cDevice::new_i2c(delay, i2c, enable, flgn, i2c_add_vddio),
            state: State::default(),
        }
    }

    /// Initialize the controller to `role`
    pub async fn init(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        self.reset().await?;

        self.control_read_write(|r| {
            r.power_mode(Mode::LowPower as u8);
            r.vconn_switch(VconnSwitch::None as u8);
        }).await?;

        self.state.mode = Mode::LowPower;
        self.state.cc = CcState::None;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Initialized!");

        Ok(())
    }

    /// Attach to a cable with a power role and CC line configuration
    pub async fn attach(&mut self, orientation: CcState) -> DeviceResult<(), I2CE, GPIOE> {
        if self.state.mode != Mode::LowPower {
            return Err(Error::InvalidCommand);
        }

        self.set_mode(Mode::Normal).await?;
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
    pub async fn detach(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        if self.state.mode != Mode::Normal {
            return Err(Error::InvalidCommand);
        }

        self.discharge().await?;

        self.set_pd(Self::DEFAULT_PD_ROLE).await?;
        self.set_mode(Mode::LowPower).await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Detached!");

        Ok(())
    }

    /// Set the controller mode
    async fn set_mode(&mut self, mode: Mode) -> DeviceResult<(), I2CE, GPIOE> {
        if self.state.mode == mode {
            return Ok(());
        }

        self.control_read_write(|r| {
            r.power_mode(mode as u8);
        }).await?;
        self.state.mode = mode;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Set mode {:?}!", mode);

        Ok(())
    }

    /// Set controller V1 & V2 gates to adhere to `orientation`
    async fn set_cc(&mut self, orientation: CcState) -> DeviceResult<(), I2CE, GPIOE> {

        if self.state.cc == orientation {
            return Ok(());
        }

        self.control_read_write(|r| {
            r.vconn_switch(VconnSwitch::from(orientation) as u8);
        }).await?;
        self.state.cc = orientation;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Set orientation {:?}!", orientation);

        Ok(())
    }

    /// Set controller consumer and provider gates to adhere to `pd`
    async fn set_pd(&mut self, pd: PdRole) -> DeviceResult<(), I2CE, GPIOE> {
        if self.state.mode != Mode::Normal {
            return Err(Error::InvalidCommand);
        }

        if self.state.pd == pd {
            return Ok(());
        }

        self.control_read_write(|r| {
            match pd {
                PdRole::None => {
                    r.gate_driver_consumer(true);   // GDC Open
                    r.gate_driver_provider(false);  // GDP Open
                },
                PdRole::Sink => {
                    r.gate_driver_consumer(false);  // GDC Close
                    r.gate_driver_provider(false);  // GDP Open
                },
                PdRole::Source => {
                    r.gate_driver_consumer(true);   // GDC Open
                    r.gate_driver_provider(true);   // GDP Close
                },
            }
        }).await?;
        self.state.pd = pd;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: PD Role set to {:?}", pd);

        Ok(())
    }

    pub async fn discharge_vbus(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        self.control_read_write(|r| r.vbus_discharge(true)).await?;
        self.inner.delay_ms(Self::DISCHARGE_DELAY_MS).await;
        self.control_read_write(|r| r.vbus_discharge(false)).await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Discharged Vbus!");

        Ok(())
    }

    pub async fn discharge_vconn(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        let vconn = self.state.cc;

        // Disable vconn switch & start discharge
        self.control_read_write(|r| {
            r.vconn_discharge(true);
            r.vconn_switch(VconnSwitch::None as u8);
        }).await?;

        self.inner.delay_ms(Self::DISCHARGE_DELAY_MS).await;

        // Re-enable vconn switch after discharge
        self.control_read_write(|r| {
            r.vconn_discharge(false);
            r.vconn_switch(vconn as u8);
        }).await?;

        #[cfg(feature = "defmt")]
        defmt::trace!("TCPP03-M20: Discharged Vconn!");

        Ok(())
    }

    /// Discharge Vbus and Vconn
    pub async fn discharge(&mut self) -> DeviceResult<(), I2CE, GPIOE> {

        self.control_read_write(|r| {
            r.vbus_discharge(true);
            r.vconn_discharge(true);
        }).await?;

        self.inner.delay_ms(Self::DISCHARGE_DELAY_MS).await;

        self.control_read_write(|r| {
            r.vbus_discharge(false);
            r.vconn_discharge(false);
        }).await?;

        Ok(())
    }

    /// Wait for an error flag to be raised, then return the flags raised.
    pub async fn wait_flg(&mut self) -> DeviceResult<FlagReg, I2CE, GPIOE> {
        // TODO
        Ok(FlagReg(0x00))
    }

    /// Reset the controller by disabling and re-enabling it through the `EN` pin
    pub async fn reset(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        self.inner.disable().map_err(Error::Device)?;
        self.inner.delay_ms(Self::RESET_MS).await;
        self.inner.enable().map_err(Error::Device)?;
        Ok(())
    }

    async fn control_read_write(&mut self, f: impl FnOnce(&mut ControlReg)) -> DeviceResult<(), I2CE, GPIOE> {
        let ack_reg = self.inner.read::<AckReg>().await.map_err(Error::Device)?;
        let mut control_reg = ControlReg(ack_reg.0);
        f(&mut control_reg);
        self.inner.write(control_reg).await.map_err(Error::Device)
    }

}