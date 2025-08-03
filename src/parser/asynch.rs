use embedded_io_async::Read;

use crate::{
    error::SbusError,
    packet::SbusPacket,
    parser::{Parser, SBUS_FRAME_LENGTH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Async {}
impl super::Mode for Async {}
impl super::Sealed for Async {}

impl<R, M> Parser<R, M>
where
    M: super::Mode,
{
    pub fn new<R1: Read>(reader: R1) -> Parser<R1, Async> {
        Parser {
            reader,
            _mode: Default::default(),
        }
    }
}

impl<R: Read> Parser<R, Async> {
    /// Asynchronously reads the next complete SBUS frame
    ///
    /// # Returns
    ///
    /// * `Ok(SbusPacket)` if a valid frame was read
    /// * `Err(SbusError)` if an error occurred or the frame was invalid
    pub async fn read_frame(&mut self) -> Result<SbusPacket, SbusError> {
        let mut buffer = [0u8; SBUS_FRAME_LENGTH];
        self.reader
            .read_exact(&mut buffer)
            .await
            .map_err(|_| SbusError::ReadError)?;

        SbusPacket::from_array(&buffer)
    }
}

pub struct SbusParserAsync<R>
where
    R: Read,
{
    reader: R,
}

impl<R> SbusParserAsync<R>
where
    R: Read,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Asynchronously reads the next complete SBUS frame
    ///
    /// # Returns
    ///
    /// * `Ok(SbusPacket)` if a valid frame was read
    /// * `Err(SbusError)` if an error occurred or the frame was invalid
    pub async fn read_frame(&mut self) -> Result<SbusPacket, SbusError> {
        let mut buffer = [0u8; SBUS_FRAME_LENGTH];
        self.reader
            .read_exact(&mut buffer)
            .await
            .map_err(|_| SbusError::ReadError)?;

        SbusPacket::from_array(&buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::parser::asynch::SbusParserAsync;
    use embedded_io_adapters::tokio_1::FromTokio;

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

    #[tokio::test]
    async fn test_valid_sbus_frame_async() {
        // Simulate a valid SBUS frame
        let data = [
            0x0F, // Header
            0x00, 0x00, // Channel 1 (bits 0-10)
            0x00, 0x00, // Channel 2 (bits 0-10)
            // Ensure to simulate all 16 channels and the flags byte
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Flags
            0x00, // Footer
        ];
        let cursor = Cursor::new(data);
        let mut parser = SbusParserAsync::new(FromTokio::new(cursor));

        let packet = parser.read_frame().await.expect("Should be a valid frame");

        assert_eq!(packet.channels[0], 0);
        assert_eq!(packet.channels[15], 0);
        assert!(!packet.flags.d1);
        assert!(!packet.flags.d2);
        assert!(!packet.flags.frame_lost);
        assert!(!packet.flags.failsafe);
    }

    #[tokio::test]
    async fn test_invalid_footer_async() {
        // Simulate a frame with an invalid header
        let mut data = TEST_PACKET;
        data[24] = 0x50; // Invalid footer

        let cursor = Cursor::new(data);
        let mut parser = SbusParserAsync::new(FromTokio::new(cursor));

        let result = parser.read_frame().await;
        assert!(matches!(result, Err(SbusError::InvalidFooter(0x50))));
    }

    #[tokio::test]
    async fn test_invalid_header_async() {
        // Simulate a frame with an invalid header
        let mut data = TEST_PACKET;
        data[0] = 0x00; // Invalid header

        let cursor = Cursor::new(data);
        let mut parser = SbusParserAsync::new(FromTokio::new(cursor));

        let result = parser.read_frame().await;
        assert!(matches!(result, Err(SbusError::InvalidHeader(0x00))));
    }
}
