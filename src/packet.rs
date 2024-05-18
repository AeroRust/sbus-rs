use crate::{channels_parsing, SbusError, SBUS_FOOTER, SBUS_FRAME_LENGTH, SBUS_HEADER};

#[derive(Debug, Clone, Copy)]
pub struct SbusPacket {
    pub channels: [u16; 16],
    pub flags: Flags,
}

impl SbusPacket {
    pub fn from_array(buffer: &[u8; SBUS_FRAME_LENGTH]) -> Result<Self, SbusError> {
        SbusPacket::validate_frame(buffer)?;

        // Parse channels and flags
        let channels = channels_parsing(buffer);
        let flags = Flags::from_byte(buffer[23]);

        Ok(Self { channels, flags })
    }
    /// Validates if header and footer and set correctly
    pub fn validate_frame(frame_buf: &[u8; SBUS_FRAME_LENGTH]) -> Result<(), SbusError> {
        let header = frame_buf[0];
        let footer = frame_buf[SBUS_FRAME_LENGTH - 1];

        // Check header and footer
        if header != SBUS_HEADER {
            Err(SbusError::InvalidHeader(header))
        } else if frame_buf[SBUS_FRAME_LENGTH - 1] != SBUS_FOOTER {
            Err(SbusError::InvalidFooter(footer))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Flags {
    pub d1: bool,
    pub d2: bool,
    pub failsafe: bool,
    pub frame_lost: bool,
}

impl Flags {
    pub fn from_byte(flag_byte: u8) -> Self {
        Flags::from(flag_byte)
    }
}

impl From<u8> for Flags {
    fn from(flag_byte: u8) -> Self {
        Self {
            d1: (flag_byte & (1 << 0)) != 0,
            d2: (flag_byte & (1 << 1)) != 0,
            frame_lost: (flag_byte & (1 << 2)) != 0,
            failsafe: (flag_byte & (1 << 3)) != 0,
        }
    }
}
