// SPDX-License-Identifier: GPL-3.0-or-later
//! A TV on the Pi's HDMI0 port over HDMI-CEC (ADR 0012), through the kernel's CEC device
//! (`/dev/cec0`) and the `cec_linux` crate. The device's calls block, so each runs on tokio's
//! blocking pool; the device is opened (and our logical address claimed) on first use and again
//! after an error that suggests it went away.
//!
//! - **On**: "Image View On" to the TV, then "Active Source" with our physical address, so the TV
//!   also switches to this input. Without a physical address (the TV was off and its EDID not
//!   read; likely with the forced HDMI mode) the kernel still lets an unconfigured adapter send
//!   "Image View On" from Unregistered, which wakes most TVs.
//! - **Standby**: "Standby" to the TV.
//! - **State**: "Give Device Power Status", answered by "Report Power Status".

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cec_linux::{
    CecDevice, CecLogAddrType, CecLogAddrs, CecLogicalAddress, CecModeFollower, CecModeInitiator, CecOpcode,
    CecPhysicalAddress, CecPrimDevType, VendorID, Version,
};
use sursum_core::display_power::cec_power_status;
use sursum_protocol::{DisplayPowerState, PowerAction};

use super::{DisplayPower, PowerError};

/// The CEC device of the Pi's HDMI0 port.
pub const DEFAULT_DEVICE: &str = "/dev/cec0";

/// The name the TV may show for this source.
const OSD_NAME: &str = "sursum";

/// A TV reached over HDMI-CEC.
pub struct CecDisplay {
    path: PathBuf,
    device: Arc<Mutex<Option<CecDevice>>>,
}

impl CecDisplay {
    /// `path` is the CEC device, normally [`DEFAULT_DEVICE`].
    pub fn new(path: PathBuf) -> Self {
        CecDisplay { path, device: Arc::default() }
    }

    /// Runs `call` with the (opened and configured) device on the blocking pool.
    async fn with<T: Send + 'static>(
        &self,
        call: impl FnOnce(&CecDevice) -> Result<T, PowerError> + Send + 'static,
    ) -> Result<T, PowerError> {
        let path = self.path.clone();
        let device = self.device.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = device.lock().map_err(|_| PowerError::Failure("CEC lock poisoned".into()))?;
            if guard.is_none() {
                *guard = Some(open(&path)?);
            }
            let result = call(guard.as_ref().expect("opened above"));
            if matches!(result, Err(PowerError::Unreachable(_))) {
                // Open it afresh next time (the adapter may have been reset or replugged).
                *guard = None;
            }
            result
        })
        .await
        .map_err(|error| PowerError::Failure(error.to_string()))?
    }
}

impl DisplayPower for CecDisplay {
    async fn set(&self, action: PowerAction) -> Result<(), PowerError> {
        self.with(move |device| match action {
            PowerAction::On => switch_on(device),
            PowerAction::Standby => {
                let from = configured_address(device)?;
                device.transmit(from, CecLogicalAddress::Tv, CecOpcode::Standby).map_err(no_answer)
            }
        })
        .await
    }

    async fn state(&self) -> Result<DisplayPowerState, PowerError> {
        self.with(|device| {
            let from = configured_address(device)?;
            let reply = device
                .request_data(
                    from,
                    CecLogicalAddress::Tv,
                    CecOpcode::GiveDevicePowerStatus,
                    &[],
                    CecOpcode::ReportPowerStatus,
                )
                .map_err(no_answer)?;
            let operand =
                reply.first().copied().ok_or_else(|| PowerError::Failure("empty power status".into()))?;
            cec_power_status(operand).ok_or_else(|| PowerError::Failure(format!("power status {operand}")))
        })
        .await
    }
}

/// Opens the device as a sender and claims a playback-device logical address named "sursum"
/// (unless the adapter already has one).
fn open(path: &std::path::Path) -> Result<CecDevice, PowerError> {
    let unreachable = |error: std::io::Error| PowerError::Unreachable(format!("{}: {error}", path.display()));
    let device = CecDevice::open(path).map_err(unreachable)?;
    device.set_mode(CecModeInitiator::Send, CecModeFollower::RepliesOnly).map_err(unreachable)?;
    if device.get_log().map_err(unreachable)?.addresses().is_empty() {
        let name = OSD_NAME.to_owned().try_into().map_err(|_| PowerError::Failure("OSD name".into()))?;
        let request = CecLogAddrs::new(
            VendorID::NONE,
            Version::V1_4,
            name,
            &[CecPrimDevType::PLAYBACK],
            &[CecLogAddrType::PLAYBACK],
        );
        device.set_log(request).map_err(unreachable)?;
    }
    Ok(device)
}

/// Wakes the TV and makes this the active source where the physical address is known.
fn switch_on(device: &CecDevice) -> Result<(), PowerError> {
    let own = own_address(device)?;
    device.transmit(own, CecLogicalAddress::Tv, CecOpcode::ImageViewOn).map_err(no_answer)?;
    let physical = device.get_phys().map_err(no_answer)?;
    if own != CecLogicalAddress::UnregisteredBroadcast && physical != CecPhysicalAddress::INVALID {
        device
            .transmit_data(
                own,
                CecLogicalAddress::UnregisteredBroadcast,
                CecOpcode::ActiveSource,
                &physical.to_bytes(),
            )
            .map_err(no_answer)?;
    }
    Ok(())
}

/// Our claimed logical address, or Unregistered while the adapter has none.
fn own_address(device: &CecDevice) -> Result<CecLogicalAddress, PowerError> {
    let addresses = device.get_log().map_err(no_answer)?;
    Ok(addresses.addresses().first().copied().unwrap_or(CecLogicalAddress::UnregisteredBroadcast))
}

/// Our claimed logical address; without one only "Image View On" may be sent.
fn configured_address(device: &CecDevice) -> Result<CecLogicalAddress, PowerError> {
    match own_address(device)? {
        CecLogicalAddress::UnregisteredBroadcast => {
            Err(PowerError::Unreachable("no CEC address yet: the TV is off or not connected".into()))
        }
        address => Ok(address),
    }
}

/// A failed exchange: the TV did not acknowledge or answer.
#[allow(clippy::needless_pass_by_value, reason = "shaped for map_err")]
fn no_answer(error: std::io::Error) -> PowerError {
    PowerError::Unreachable(error.to_string())
}
