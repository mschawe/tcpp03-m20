use device_driver::FieldSet;

use crate::CcState;

device_driver::create_device!(
    device_name: Tcpp03M20,
    dsl: {
        config {
            type DefaultRegisterAccess = RW;
            type DefaultFieldAccess = RW;
            type DefaultByteOrder = LE;
            type DefaultBitOrder = LSB0;
            type RegisterAddressType = u8;
            type DefmtFeature = "defmt";
        }
        /// Control state changes write register
        register Control {
            const ADDRESS = 0x00;
            const SIZE_BITS = 8;
            type Access = WO;

            vconn_discharge: bool = 7,
            vbus_discharge: bool = 6,
            power_mode: uint as enum PowerMode {
                Hibernate = 0,
                Normal = 1,
                LowPower = 2,
                Unused = 3,
            } = 4..=5,
            gate_driver_consumer: bool = 3,
            gate_driver_provider: bool = 2,
            vconn_switch: uint as enum VconnSwitch {
                /// V1 and V2 Open
                BothOpen = 0,
                /// V1 Closed
                V1 = 1,
                /// V2 Closed
                V2 = 2,
                /// V1 and V2 Open
                BothOpen2 = 3
            } = 0..=1,
        },
        /// Current status of the system
        register Ack {
            const ADDRESS = 0x01;
            const SIZE_BITS = 8;
            type Access = RO;

            vconn_discharge: bool = 7,
            vbus_discharge: bool = 6,
            power_mode: uint as PowerMode = 4..=5,
            gate_driver_consumer: bool = 3,
            gate_driver_provider: bool = 2,
            vconn_switch: uint as VconnSwitch = 0..=1,
        },
        /// Flags are `true` when active
        register Flag {
            const ADDRESS = 0x02;
            const SIZE_BITS = 8;
            type Access = RO;

            id: bool = 7,
            vbus_ok: bool = 5,
            ovp_cc: bool = 4,
            otp: bool = 3,
            ovp_vbus: bool = 2,
            ocp_vbus: bool = 1,
            ocp_vconn: bool = 0,
        }
    }
);

#[expect(clippy::derivable_impls, reason = "Macro nonsense")]
impl Default for PowerMode {
    fn default() -> Self {
        PowerMode::Hibernate
    }
}

impl From<field_sets::Ack> for field_sets::Control {
    fn from(value: field_sets::Ack) -> Self {
        let inner = value.get_inner_buffer()[0];
        field_sets::Control::from([inner])
    }
}

impl From<CcState> for VconnSwitch {
    fn from(value: CcState) -> Self {
        match value {
            CcState::None => VconnSwitch::BothOpen,
            CcState::CC1 => VconnSwitch::V2,
            CcState::CC2 => VconnSwitch::V1,
        }
    }
}
