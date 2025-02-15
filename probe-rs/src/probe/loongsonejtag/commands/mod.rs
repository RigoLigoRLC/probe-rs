pub mod probe_memory_rw;
pub mod get_firmware_version;

use bitfield::bitfield;
use zerocopy::IntoBytes;
use crate::probe::loongsonejtag::{LoongsonEjtagDevice, SendError};
use crate::probe::ProbeError;

#[derive(Debug, thiserror::Error, docsplay::Display)]
pub enum LoongsonEjtagError {
    /// Error handling Loongson EJTAG command
    Send {
        command_id: CommandId,
        source: SendError
    }
}

impl ProbeError for LoongsonEjtagError {}

/// Command ID for Loongson EJTAG commands.
///
/// Command ID takes up 6 bits at the MSB side in the first u16 of a command.
/// Some commands have become obsolete and are no longer used by host/no longer implemented by
/// newer probes.
#[repr(u16)]
#[derive(Clone, Debug)]
#[allow(unused)]
pub enum CommandId {
    ProbeMemoryRW = 0x01,
    IoManipulation = 0x03,
    IrRW = 0x04,
    DrRW = 0x05,
    LoopbackTest = 0x08,
    FastWriteWithFastdata = 0x0c,
    FastWrite = 0x0d,
    FastReadWithFastdata = 0x0e,
    FastRead = 0x0f,
    GetFirmwareVersion = 0x1f,
}

bitfield! {
    pub(crate) struct CommandHeader(u16);
    impl Debug;
    core_count, set_core_count: 6, 0;
    mem_rw_is_read, set_mem_rw_is_read: 0;
    is_64_bit, set_is_64_bit: 7;
    immediate_read, set_immediate_read: 8;
    read_tdo, set_read_tdo: 9;
    command_id, set_command_id: 15, 10;
}

pub(crate) trait Command {
    const COMMAND_ID: CommandId;

    type Response;

    /// Convert the request to bytes, which can be sent to the probe.
    /// Returns the amount of bytes written to the buffer.
    fn to_bytes(&self, buffer: &mut [u8]) -> Result<usize, SendError>;

    /// Return the total length in bytes when this command is converted to a byte sequence with
    /// `to_bytes`.
    fn length(&self) -> usize;

    /// Return a tuple which tells how many bytes of data this command queues in the probe memory,
    /// and whether the queued data should all be read out.
    fn read_length(&self) -> (usize, bool);

    /// Parse the response bytes into the intended response type
    fn parse_response(&self, buffer: Option<&[u8]>) -> Result<Self::Response, SendError>;
}

pub(crate) fn send_command<Cmd: Command>(
    device: &mut LoongsonEjtagDevice,
    command: &Cmd
) -> Result<Cmd::Response, LoongsonEjtagError> {
    send_command_inner(device, command).map_err(|e| LoongsonEjtagError::Send {
        command_id: Cmd::COMMAND_ID,
        source: e
    })
}

fn send_command_inner<Cmd: Command>(
    device: &mut LoongsonEjtagDevice,
    command: &Cmd
) -> Result<Cmd::Response, SendError> {
    // Loongson EJTAG can accept multiple commands at once. In fact, it doesn't care about the
    // number of commands sent at once - its official implementation simply reads USB buffer like
    // a stream and simply decodes commands when there is data. And therefore we can send multiple
    // commands at once (may cross packet boundary) to increase throughput, since EJTAG debug
    // scheme really requires many, many JTAG operations.

    // Reserve some buffer
    let mut buffer = vec![0u8; command.length()];

    // Convert all commands to bytes, and determine how many bytes to read
    let mut read_len = 0usize;
    command.to_bytes(buffer[0..].as_mut())?;

    let (queue_len, read_immediately) = command.read_length();
    device.queued_read_length += queue_len;
    if read_immediately {
        read_len += device.queued_read_length;
        device.queued_read_length = 0;
    }

    // Send buffer to device.
    // nusb uses WinUSB and does not enable RAW_IO mode so we can simply feed the entire buffer
    // to the device, and let WinUSB process the packets
    let mut buffer_offset = 0usize;
    while buffer_offset < buffer.len() {
        buffer_offset += device.write(buffer[buffer_offset..].as_bytes())?;
    }

    if read_len != 0 {
        // Reuse this buffer as read buffer
        buffer.resize(read_len, 0u8);

        let mut bytes_read = 0usize;
        while bytes_read < read_len {
            bytes_read += device.read(buffer.as_mut())?;
        }

        command.parse_response(Some(&buffer[0..bytes_read]))
    } else {
        command.parse_response(None)
    }
}
