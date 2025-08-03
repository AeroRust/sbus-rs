use crate::{
    channels_parsing, SbusError, SBUS_FOOTER, SBUS_FOOTER_2, SBUS_FRAME_LENGTH, SBUS_HEADER,
};

/// Represents a complete SBUS packet with channel data and flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SbusPacket {
    pub channels: [u16; 16],
    pub flags: Flags,
}

impl SbusPacket {
    /// Creates a new SbusPacket from a raw 25-byte SBUS frame
    ///
    /// # Arguments
    ///
    /// * `buffer` - A 25-byte array containing a complete SBUS frame
    ///
    /// # Returns
    ///
    /// * `Ok(SbusPacket)` if the frame is valid
    /// * `Err(SbusError)` if the frame has invalid header or footer
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
        } else if footer != SBUS_FOOTER && footer & 0x0F != SBUS_FOOTER_2 {
            Err(SbusError::InvalidFooter(footer))
        } else {
            Ok(())
        }
    }
}

/// Status flags contained in an SBUS frame
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
