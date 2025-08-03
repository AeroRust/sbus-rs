use crate::{error::SbusError, packet::SbusPacket, parser::SBUS_FRAME_LENGTH, Parser};
use embedded_io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Blocking {}
impl super::Mode for Blocking {}
impl super::Sealed for Blocking {}

impl<R, M> Parser<R, M>
where
    M: super::Mode,
{
    pub fn new_blocking<R1: Read>(reader: R1) -> Parser<R1, Blocking> {
        Parser {
            reader,
            _mode: Default::default(),
        }
    }
}

impl<R: Read> Parser<R, Blocking> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            _mode: Default::default(),
        }
    }

    /// Reads the next complete SBUS frame
    ///
    /// # Returns
    ///
    /// * `Ok(SbusPacket)` if a valid frame was read
    /// * `Err(SbusError)` if an error occurred or the frame was invalid
    pub fn read_frame(&mut self) -> Result<SbusPacket, SbusError> {
        let mut buffer = [0u8; SBUS_FRAME_LENGTH];
        self.reader
            .read_exact(&mut buffer)
            .map_err(|_| SbusError::ReadError)?;

        SbusPacket::from_array(&buffer)
    }
}

pub type SbusParser<R> = Parser<R, Blocking>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CHANNEL_MAX;
    use embedded_io_adapters::std::FromStd;
    use std::io::Cursor;

    const TEST_PACKET: [u8; 25] = [
        0x0F, // HEAD_BYTE
        (1024 & 0x07FF) as u8,
        (((1024 & 0x07FF) >> 8) | ((1024 & 0x07FF) << 3)) as u8,
        (((1024 & 0x07FF) >> 5) | ((1024 & 0x07FF) << 6)) as u8,
        ((1024 & 0x07FF) >> 2) as u8,
        (((1024 & 0x07FF) >> 10) | ((1024 & 0x07FF) << 1)) as u8,
        (((1024 & 0x07FF) >> 7) | ((1024 & 0x07FF) << 4)) as u8,
        ((1024 & 0x07FF) >> 4) as u8,
        ((1024 & 0x07FF) << 2) as u8,
        (((1024 & 0x07FF) >> 8) | ((1024 & 0x07FF) << 5)) as u8,
        ((1024 & 0x07FF) >> 1) as u8,
        (((1024 & 0x07FF) >> 9) | ((1024 & 0x07FF) << 6)) as u8,
        ((1024 & 0x07FF) >> 3) as u8,
        (((1024 & 0x07FF) >> 10) | ((1024 & 0x07FF) << 1)) as u8,
        (((1024 & 0x07FF) >> 7) | ((1024 & 0x07FF) << 4)) as u8,
        ((1024 & 0x07FF) >> 4) as u8,
        ((1024 & 0x07FF) << 2) as u8,
        (((1024 & 0x07FF) >> 8) | ((1024 & 0x07FF) << 5)) as u8,
        ((1024 & 0x07FF) >> 1) as u8,
        (((1024 & 0x07FF) >> 9) | ((1024 & 0x07FF) << 6)) as u8,
        ((1024 & 0x07FF) >> 3) as u8,
        (((1024 & 0x07FF) >> 10) | ((1024 & 0x07FF) << 1)) as u8,
        (((1024 & 0x07FF) >> 7) | ((1024 & 0x07FF) << 4)) as u8,
        0x00, // FLAGS_BYTE, no flags set
        0x00, // FOOT_BYTE
    ];

    #[test]
    fn test_valid_sbus_frame() {
        // Simulate a valid SBUS frame
        let data = [
            0x0F, // Header
            0x00, 0x00, // Channel 1 (bits 0-10)
            0x00, 0x00, // Channel 2 (bits 0-10)
            // Remaining channels omitted for brevity, but should be similar
            // Ensure to simulate all 16 channels and the flags byte
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Flags
            0x00, // Footer
        ];
        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let packet = parser.read_frame().expect("Should be a valid frame");

        assert_eq!(packet.channels[0], 0);
        assert_eq!(packet.channels[15], 0);
        assert!(!packet.flags.d1);
        assert!(!packet.flags.d2);
        assert!(!packet.flags.frame_lost);
        assert!(!packet.flags.failsafe);
    }

    #[test]
    fn test_invalid_header() {
        // Simulate a frame with an invalid header
        let mut data = TEST_PACKET;
        data[0] = 0x00; // Invalid header

        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let result = parser.read_frame();
        assert!(matches!(result, Err(SbusError::InvalidHeader(0x00))));
    }

    #[test]
    fn test_invalid_footer() {
        let mut data = TEST_PACKET;
        data[24] = 0xFF; // Invalid footer

        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let result = parser.read_frame();
        assert!(matches!(result, Err(SbusError::InvalidFooter(0xFF))));
    }

    #[test]
    fn test_flag_bytes() {
        let mut data = TEST_PACKET;
        data[23] = 0b00001111; // All flags set

        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let result = parser.read_frame();
        assert!(result.is_ok());
        let packet = result.unwrap();
        assert!(packet.flags.d1);
        assert!(packet.flags.d2);
        assert!(packet.flags.frame_lost);
        assert!(packet.flags.failsafe);
    }

    #[test]
    fn test_partial_frame() {
        let data = &TEST_PACKET[..20]; // Cut off the last few bytes

        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let result = parser.read_frame();
        assert!(matches!(result, Err(SbusError::ReadError)));
    }

    #[test]
    fn test_channel_decoding() {
        let mut data = [0u8; 25];
        data[0] = 0x0F; // Header
                        // Channel 1 set to 0
        data[1] = 0;
        data[2] = 0;
        // Channel 2 set to 2047, needs to correctly span bytes 2, 3, and 4
        data[2] |= (CHANNEL_MAX << 3) as u8; // Start from bit 3 of byte 2
        data[3] = ((CHANNEL_MAX >> 5) & 0xFF) as u8; // Next full byte
        data[4] = ((CHANNEL_MAX >> 5) & 0x07) as u8; // Last few bits that fit into byte 4
        data[24] = 0x00; // Footer

        let cursor = Cursor::new(data);
        let mut parser = SbusParser::new(FromStd::new(cursor));

        let result = parser.read_frame();
        assert!(result.is_ok());
        let packet = result.unwrap();
        assert_eq!(packet.channels[0], 0); // Channel 1 should be 0
        assert_eq!(packet.channels[1], CHANNEL_MAX); // Channel 2 should be 2047
    }
}
