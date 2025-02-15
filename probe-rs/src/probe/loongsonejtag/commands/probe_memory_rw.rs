use crate::probe::loongsonejtag::commands::{Command, CommandHeader, CommandId};
use crate::probe::loongsonejtag::SendError;
use crate::probe::loongsonejtag::SendError::UnexpectedResponse;

/// JTAG port clock division register.
pub const LSEJTAG_ADDR_REG_CLOCK_DIV: u32 = 0x81000070;

pub struct ProbeMemoryRWCommand {
    pub address: u32,
    pub write_data: Option<u32>,
}

impl Command for ProbeMemoryRWCommand {
    const COMMAND_ID: CommandId = CommandId::ProbeMemoryRW;
    type Response = Option<u32>;

    fn to_bytes(&self, buffer: &mut [u8]) -> Result<usize, SendError> {
        let mut header = CommandHeader(0);
        header.set_command_id(Self::COMMAND_ID as u16);
        header.set_mem_rw_is_read(self.write_data.is_none());

        buffer[0..2].copy_from_slice(header.0.to_le_bytes().as_ref());
        buffer[2..6].copy_from_slice(self.address.to_le_bytes().as_ref());
        if let Some(data) = self.write_data {
            buffer[6..10].copy_from_slice(&data.to_le_bytes());
        }

        Ok(self.length())
    }

    fn length(&self) -> usize { if self.write_data.is_some() { 10 } else { 6 } }

    fn read_length(&self) -> (usize, bool) { if self.write_data.is_some() { (0, false) } else { (4, true) } }

    fn parse_response(&self, buffer: Option<&[u8]>) -> Result<Self::Response, SendError> {
        if self.write_data.is_none() {
            // Write command: should not receive any return data
            if buffer.is_some() { Err(UnexpectedResponse) } else { Ok(None) }
        } else {
            // Read command: must receive 4 bytes
            if let Some(buf) = buffer {
                if buf.len() == 4 {
                    Ok(Some(u32::from_le_bytes(buf[0..5].try_into().unwrap())))
                } else {
                    Err(UnexpectedResponse)
                }
            } else {
                Err(UnexpectedResponse)
            }
        }
    }
}
