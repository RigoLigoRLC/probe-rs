use std::collections::HashMap;
use nusb::{DeviceInfo};
use nusb::transfer::{Direction, EndpointType};
use crate::probe::{DebugProbeInfo, DebugProbeSelector, ProbeCreationError};
use crate::probe::loongsonejtag::{LoongsonEjtagDevice, LoongsonEjtagFactory};

const LSEJTAG_VID: u16 = 0x2961;
const LSEJTAG_PID: u16 = 0x6688;
pub(super) const LSEJTAG_OUT_EP: u8 = 2 | 0x80;
pub(super) const LSEJTAG_IN_EP: u8 = 6;

/// Finds all Loongson EJTAG probes.
///
/// Loongson EJTAG probes (until 2025/02/13) are identified using a fixed VID/PID pair. They use
/// two Bulk mode endpoints (2 for OUT and 6 for IN) to communicate with the host.
#[tracing::instrument(skip_all)]
pub fn list_loongsonejtag_devices() -> Vec<DebugProbeInfo> {
    tracing::debug!("Searching for Loongson EJTAG probes using nusb");

    let probes = match nusb::list_devices() {
        Ok(devices) => devices
            .filter_map(|device| get_loongson_ejtag_info(&device))
            .collect(),
        Err(e) => {
            tracing::error!("Error while listing Loongson EJTAG devices: {}", e);
            vec![]
        }
    };

    tracing::debug!(
        "Found {} Loongson EJTAG probes using nusb",
        probes.len()
    );

    probes
}

/// Attempt to open the given device
pub fn open_device(device_info: &DeviceInfo) -> Option<LoongsonEjtagDevice> {
    // Open device handle and read basic information
    let vid = device_info.vendor_id();
    let pid = device_info.product_id();

    let device = device_info.open().ok()?;

    // Iterate through interfaces and open an interface that has specified endpoints
    let c_desc = device.configurations().next()?;
    for interface in c_desc.interfaces() {
        let Some(interface_handle) = device.claim_interface(interface.interface_number()).ok() else {
            continue;
        };

        for descriptor in interface_handle.descriptors() {
            let eps = descriptor.endpoints().fold(HashMap::new(), |mut map, ep| {
                map.insert(ep.address(), ep);
                map
            });

            if eps.contains_key(&LSEJTAG_IN_EP) && eps.contains_key(&LSEJTAG_OUT_EP) &&
                eps[&LSEJTAG_IN_EP].direction() == Direction::In &&
                eps[&LSEJTAG_OUT_EP].direction() == Direction::Out {
                return Some(LoongsonEjtagDevice {
                    handle: interface_handle.clone(),
                    max_packet_size: eps[&LSEJTAG_IN_EP].max_packet_size(),
                    queued_read_length: 0,
                })
            }
        }
    }

    tracing::debug!(
        "Could not open {:04x}:{:04x} as a Loongson EJTAG device",
        vid,
        pid
    );
    None
}

/// Attempt to open the given DebugProbeInfo.
pub fn open_device_from_selector(
    selector: &DebugProbeSelector,
) -> Result<LoongsonEjtagDevice, ProbeCreationError> {
    tracing::trace!("Attempting to open device matching {}", selector);

    let mut device_info = None;
    if let Ok(devices) = nusb::list_devices() {
        for device in devices {
            tracing::trace!("Trying device {:?}", device);

            if selector.matches(&device) {
                device_info = get_loongson_ejtag_info(&device);

                if device_info.is_some() {
                    if let Some(device) = open_device(&device) {
                        return Ok(device)
                    }
                }
            }
        }
    } else {
        tracing::debug!("No devices matched using nusb");
    }

    Err(ProbeCreationError::NotFound)
}

fn get_loongson_ejtag_info(device: &DeviceInfo) -> Option<DebugProbeInfo> {
    let prod_str = device.product_string().unwrap_or("");
    let sn_str = device.serial_number();

    // Match three things: VID, PID, One Bulk Interface with specified Endpoints
    if !is_loongson_ejtag_vidpid(device) {
        return None;
    }

    // Check the interface
    let mut found_endpoints = false;
    let device_handle = device.open().ok()?;
    for interface in device.interfaces() {
        let Some(interface_handle) = device_handle.claim_interface(interface.interface_number()).ok() else {
            continue;
        };

        for descriptor in interface_handle.descriptors() {
            let mut in_ep_count = 0;
            let mut out_ep_count = 0;
            descriptor.endpoints().for_each(|ep| {
                match (ep.address(), ep.transfer_type()) {
                    (LSEJTAG_IN_EP, EndpointType::Bulk) => { in_ep_count += 1; },
                    (LSEJTAG_OUT_EP, EndpointType::Bulk) => { out_ep_count += 1; },
                    (_, _) => (),
                }
            });

            if in_ep_count == 1 && out_ep_count == 1 {
                found_endpoints = true;
                break;
            }
        }
    }

    if !found_endpoints {
        Some(DebugProbeInfo::new(
            prod_str,
            device.vendor_id(),
            device.product_id(),
            sn_str.map(|s| s.to_string()),
            &LoongsonEjtagFactory,
            None
        ))
    } else {
        None
    }
}

fn is_loongson_ejtag_vidpid(device: &DeviceInfo) -> bool {
    device.vendor_id() == LSEJTAG_VID && device.product_id() == LSEJTAG_PID
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_scan_lsejtag() {
        for device in list_loongsonejtag_devices() {
            println!("{:?}", device);
        }
    }
}
