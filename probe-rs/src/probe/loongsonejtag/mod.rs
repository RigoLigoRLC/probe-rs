//! Loongson EJTAG probe implementation

mod commands;
mod tools;

use std::cmp::min;
use std::fmt::{Debug, Formatter};
use std::time::Duration;
use probe_rs_target::ScanChainElement;
use crate::Error;
use crate::probe::{DebugProbe, DebugProbeError, DebugProbeInfo, DebugProbeSelector, ProbeCreationError, ProbeFactory, WireProtocol};
use crate::probe::loongsonejtag::commands::get_firmware_version::GetFirmwareVersionCommand;
use crate::probe::loongsonejtag::commands::probe_memory_rw::{ProbeMemoryRWCommand, LSEJTAG_ADDR_REG_CLOCK_DIV};
use crate::probe::loongsonejtag::tools::{list_loongsonejtag_devices, LSEJTAG_IN_EP, LSEJTAG_OUT_EP};
use crate::probe::usb_util::InterfaceExt;

const USB_TIMEOUT: Duration = Duration::from_millis(1000);

/// Errors occured when sending command to the probe
#[derive(Debug, thiserror::Error, docsplay::Display)]
pub enum SendError {
    /// Error in the USB access.
    UsbError(std::io::Error),

    /// Timeout in USB communication.
    Timeout,

    /// Unexpected response to command.
    UnexpectedResponse,
}

impl From<std::io::Error> for SendError {
    fn from(error: std::io::Error) -> SendError {
        match error.kind() {
            std::io::ErrorKind::TimedOut => SendError::Timeout,
            _ => SendError::UsbError(error),
        }
    }
}

/// A factory for creating ['LoongsonEjtag'] probes.
#[derive(Debug)]
pub struct LoongsonEjtagFactory;

impl std::fmt::Display for LoongsonEjtagFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("Loongson EJTAG")
    }
}

impl ProbeFactory for LoongsonEjtagFactory {
    fn open(&self, selector: &DebugProbeSelector) -> Result<Box<dyn DebugProbe>, DebugProbeError> {
        Ok(Box::new(LoongsonEjtag::new_from_device(
            tools::open_device_from_selector(selector)?,
        )?))
    }

    fn list_probes(&self) -> Vec<DebugProbeInfo> {
        list_loongsonejtag_devices()
    }
}

/// A Loongson EJTAG probe.
pub struct LoongsonEjtag {
    device: LoongsonEjtagDevice,

    firmware_version: u32,

    speed_khz: u32,
    scan_chain: Option<Vec<ScanChainElement>>,
}

impl LoongsonEjtag {
    /// Try creating a new Loongson EJTAG probe from an open device
    pub fn new_from_device(mut device: LoongsonEjtagDevice) -> Result<Self, DebugProbeError> {
        // Drain all remaining data from the device
        device.drain();

        // Try fetch firmware version code, we assume it should be at least be a valid BCD number
        let firmware_ver = commands::send_command(&mut device, &GetFirmwareVersionCommand {})?;
        let firmware_ver_str = format!("{:08X}", firmware_ver);
        if let Some(_) = firmware_ver_str.as_bytes().iter().find(|c| { !c.is_ascii_digit() }) {
            return Err(DebugProbeError::ProbeCouldNotBeCreated(ProbeCreationError::Other("Invalid version number")));
        }

        // Set probe speed to ~2000kHz (8 divisions)
        commands::send_command(&mut device, &ProbeMemoryRWCommand {
            address: LSEJTAG_ADDR_REG_CLOCK_DIV,
            write_data: None,
        })?;

        // Create the probe
        tracing::debug!("Opened a Loongson EJTAG, version {}", firmware_ver_str);
        Ok(Self {
            device,
            firmware_version: firmware_ver,
            speed_khz: 2000,
            scan_chain: None,
        })
    }
}

impl Debug for LoongsonEjtag {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("LoongsonEjtag")
            .field("firmware_version", &self.firmware_version)
            .field("speed_khz", &self.speed_khz)
            .finish()
    }
}

impl DebugProbe for LoongsonEjtag {
    fn get_name(&self) -> &str {
        todo!()
    }

    fn speed_khz(&self) -> u32 {
        todo!()
    }

    fn set_speed(&mut self, speed_khz: u32) -> Result<u32, DebugProbeError> {
        todo!()
    }

    fn set_scan_chain(&mut self, scan_chain: Vec<ScanChainElement>) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn scan_chain(&self) -> Result<&[ScanChainElement], DebugProbeError> {
        todo!()
    }

    fn attach(&mut self) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn select_jtag_tap(&mut self, index: usize) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn detach(&mut self) -> Result<(), Error> {
        todo!()
    }

    fn target_reset(&mut self) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn target_reset_assert(&mut self) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn target_reset_deassert(&mut self) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn select_protocol(&mut self, protocol: WireProtocol) -> Result<(), DebugProbeError> {
        todo!()
    }

    fn active_protocol(&self) -> Option<WireProtocol> {
        todo!()
    }

    fn has_arm_interface(&self) -> bool {
        todo!()
    }

    fn has_riscv_interface(&self) -> bool {
        todo!()
    }

    fn has_xtensa_interface(&self) -> bool {
        todo!()
    }

    fn into_probe(self: Box<Self>) -> Box<dyn DebugProbe> {
        todo!()
    }
}

pub struct LoongsonEjtagDevice {
    handle: nusb::Interface,

    max_packet_size: usize,
    /// Loongson EJTAG allow IR/DR read values be queued and not sent back immediately until demanded
    /// In order to know how many bytes we should read, we need this counter
    queued_read_length: usize,
}

impl LoongsonEjtagDevice {
    /// Read from the probe into `buf`, returning the number of bytes read on success.
    fn read(&self, buf: &mut [u8]) -> Result<usize, SendError> {
        Ok(self.handle.read_bulk(LSEJTAG_IN_EP, buf, USB_TIMEOUT)?)
    }

    /// Write `buf` to the probe, returning the number of bytes written on success.
    fn write(&self, buf: &[u8]) -> Result<usize, SendError> {
        Ok(self.handle.write_bulk(LSEJTAG_OUT_EP, &buf, USB_TIMEOUT)?)
    }

    /// Drain any pending data from the probe, ensuring future responses are
    /// synchronised to requests.
    pub(super) fn drain(&self) {
        let timeout = Duration::from_millis(1);
        let mut discard = vec![0u8; self.max_packet_size];
        loop {
            match self.handle.read_bulk(LSEJTAG_IN_EP, &mut discard, timeout) {
                Ok(n) if n != 0 => continue,
                _ => break,
            }
        }
    }

    /// Set the packet size to use for this device (Bulk transfer size)
    fn set_packet_size(&mut self, size: usize) {
        self.max_packet_size = size;
    }

    /// Attempts to determine the correct packet size for this device.
    fn find_packet_size(&self) -> usize {
        let mut packet_size: usize = 512; // Use maximum packet size of HS device as starting point
        for descriptor in self.handle.descriptors() {
            packet_size = min(packet_size, descriptor.endpoints().find_map(|ep| {
                if ep.address() == LSEJTAG_IN_EP || ep.address() == LSEJTAG_OUT_EP {
                    Some(ep.max_packet_size())
                } else {
                    None
                }
            }).unwrap_or(packet_size))
        }
        packet_size
    }
}
