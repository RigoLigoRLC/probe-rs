use crate::probe::loongsonejtag::commands::{Command, CommandHeader, CommandId};
use crate::probe::loongsonejtag::SendError;

pub struct GetFirmwareVersionCommand {}

impl Command for GetFirmwareVersionCommand {
    const COMMAND_ID: CommandId = CommandId::GetFirmwareVersion;
    type Response = u32;

    fn to_bytes(&self, buffer: &mut [u8]) -> Result<usize, SendError> {
        let mut header = CommandHeader(0);
        header.set_command_id(Self::COMMAND_ID as u16);
        buffer[0..2].copy_from_slice(&header.0.to_le_bytes());
        Ok(self.length())
    }

    fn length(&self) -> usize { 2 }

    fn read_length(&self) -> (usize, bool) { (4, true) }

    fn parse_response(&self, buffer: Option<&[u8]>) -> Result<Self::Response, SendError> {
        if let Some(buffer) = buffer {
            if buffer.len() != 4 {
                return Err(SendError::UnexpectedResponse);
            }
            return Ok(u32::from_le_bytes(buffer.try_into().unwrap()));
        }

        Err(SendError::UnexpectedResponse)
    }
}
