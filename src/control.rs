use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::i2c::I2c;

use crate::register::*;
use crate::i2c::I2cDevice;
use crate::Error;
use crate::PowerRole;
use crate::CcConfig;

// ""Shorthand""
type DeviceResult<T, I2CE, GPIOE> = Result<T, Error<I2CE, GPIOE>>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Private enum over `ControlReg[PM2, PM1]`
/// - `[Table 11]` Power modes versus power supply
/// - `[Table 12]` TCPP03-M20 states versus power modes
enum PowerMode {
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

impl From<CcConfig> for VconnSwitch {
    fn from(value: CcConfig) -> Self {
        // VC is **OPPOSITE** of CC
        match value {
            CcConfig::CC1 => Self::VC2,
            CcConfig::CC2 => Self::VC1,
            CcConfig::None => Self::None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Private enum over `ControlReg[GDC, GDP]`
enum GateDriverState {
    #[default]
    None = 0,
    Provider = 1,
    Consumer = 2,
}

impl From<PowerRole> for GateDriverState {
    fn from(value: PowerRole) -> Self {
        match value {
            PowerRole::Sink => Self::Consumer,
            PowerRole::Source => Self::Provider,
            PowerRole::None => Self::None,
        }
    }
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
    role: PowerRole,
    cc_config: CcConfig,
    power_mode: PowerMode,
    vconn: VconnSwitch,
    gate_drive: GateDriverState,
}

impl <DELAY, I2C, I2CE, GPO, GPI, GPIOE> Device<DELAY, I2C, I2CE, GPO, GPI, GPIOE>
where
    DELAY: DelayNs,
    I2C: I2c<Error = I2CE>,
    GPO: OutputPin<Error = GPIOE>,
    GPI: InputPin<Error = GPIOE>,
{
    const RESET_MS: u32 = 10;

    /// Construct a new PD controller `Device` with owned peripherals
    pub fn new(delay: DELAY, i2c: I2C, enable: GPO, flgn: GPI, i2c_add_vddio: bool) -> Self {
        Self {
            inner: I2cDevice::new_i2c(delay, i2c, enable, flgn, i2c_add_vddio),
            role: Default::default(),
            cc_config: Default::default(),
            power_mode: Default::default(),
            vconn: Default::default(),
            gate_drive: Default::default(),
        }
    }

    /// Initialize the controller as `role` with `cc_config`
    pub async fn init(&mut self, role: PowerRole, cc_config: CcConfig) -> DeviceResult<(), I2CE, GPIOE> {
        self.reset().await?;
        self.set_power_mode(PowerMode::Normal).await?;
        self.set_role(role).await?;
        self.set_cc(cc_config).await?;
        Ok(())
    }

    /// `[Table 11]` Power modes versus supply.
    /// Enable/disable PD communication by toggling CC line high impedance.
    /// 
    /// **UNSAFE** Only disable this if you know what you're doing. You can get your PD loop stuck in
    /// unsafe states doing this. (especially with EPR capable devices)
    pub async unsafe fn set_pd_communication(&mut self, enable: bool) -> DeviceResult<(), I2CE, GPIOE> {
        match enable {
            true => self.set_power_mode(PowerMode::Normal).await?,
            false => self.set_power_mode(PowerMode::LowPower).await?,
        }

        Ok(())
    }

    /// Set the controller's power role. Immediately returns if power role is already `role`.
    pub async fn set_role(&mut self, role: PowerRole) -> DeviceResult<(), I2CE, GPIOE> {
        if self.role == role {
            return Ok(())
        }

        self.set_gate_drive(role.into()).await?;
        self.role = role;
        Ok(())
    }

    /// Set the controller's cc configuration. Immediately returns if the configuration is already `cc_config`
    pub async fn set_cc(&mut self, cc_config: CcConfig) -> DeviceResult<(), I2CE, GPIOE> {
        if self.cc_config == cc_config {
            return Ok(())
        }

        self.discharge_vbus().await?;

        self.set_vconn(cc_config.into()).await?;
        self.set_gate_drive(self.gate_drive).await?;

        self.cc_config = cc_config;
        Ok(())
    }

    /// Discharge `VBUS`
    pub async fn discharge_vbus(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        // TODO
        Ok(())
    }

    /// Discharge `VCONN`
    pub async fn discharge_vconn(&mut self) -> DeviceResult<(), I2CE, GPIOE> {
        
        self.set_vconn(VconnSwitch::None).await?;
        // TODO
        // initiate discharge
        // wait
        self.set_vconn(self.vconn).await?;

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

    async fn set_power_mode(&mut self, power_mode: PowerMode) -> DeviceResult<(), I2CE, GPIOE> {
        if self.power_mode == power_mode {
            return Ok(());
        }

        self.control_read_write(|c| c.power_mode(power_mode as u8)).await?;

        self.power_mode = power_mode;
        Ok(())
    }

    /// Set the gate driver to `consume`, `produce`, or `none`
    async fn set_gate_drive(&mut self, gate_drive: GateDriverState) -> DeviceResult<(), I2CE, GPIOE> {
        if self.gate_drive == gate_drive {
            return Ok(());
        }

        self.control_read_write(|c| match gate_drive {
            GateDriverState::None => {
                c.gate_driver_consumer(false);
                c.gate_driver_provider(false);
            },
            GateDriverState::Provider => {
                c.gate_driver_consumer(false);
                c.gate_driver_provider(true);
            },
            GateDriverState::Consumer => {
                c.gate_driver_consumer(true);
                c.gate_driver_provider(false);
            },
        }).await?;

        self.gate_drive = gate_drive;
        Ok(())
    }

    /// Set `VCONN` to `VC1 (CC1)`, `VC2 (CC2)`, or `None`
    async fn set_vconn(&mut self, vconn: VconnSwitch) -> DeviceResult<(), I2CE, GPIOE> {
        if self.vconn == vconn {
            return Ok(());
        }

        self.control_read_write(|c| c.vconn_switch(vconn as u8)).await?;

        self.vconn = vconn;
        Ok(())
    }

    async fn control_read_write(&mut self, f: impl FnOnce(&mut ControlReg)) -> DeviceResult<(), I2CE, GPIOE> {
        let ack_reg = self.inner.read::<AckReg>().await.map_err(Error::Device)?;
        let mut control_reg = ControlReg(ack_reg.0);
        f(&mut control_reg);
        self.inner.write(control_reg).await.map_err(Error::Device)
    }

}