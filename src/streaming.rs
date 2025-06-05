//! Streaming parser for SBUS frames that can handle partial data
use crate::{SbusError, SbusPacket, SBUS_FOOTER, SBUS_FRAME_LENGTH, SBUS_HEADER};

/// A streaming parser that accumulates bytes until a complete SBUS frame is decoded
///
/// This parser is designed for real-world scenarios where data arrives incrementally
/// from serial ports. It handles:
/// - Partial frame data
/// - Synchronization recovery
/// - Mid-stream starts
///
/// # Example
/// ```
/// # use sbus_rs::{StreamingParser, SBUS_HEADER, SBUS_FOOTER, SBUS_FRAME_LENGTH};
/// let mut parser = StreamingParser::new();
///
/// // Feed bytes as they arrive
/// if let Some(packet) = parser.push_byte(0x0F).unwrap() {
///     // Won't return a packet yet
/// }
///
/// // Or feed chunks
/// let data = [0x0F, 0x00, 0x00, /* ... */];
/// for packet in parser.push_bytes(&data) {
///     println!("Got packet: {:?}", packet.unwrap());
/// }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct StreamingParser {
    /// Buffer to accumulate partial frame data
    buffer: [u8; SBUS_FRAME_LENGTH],
    /// Current position in the buffer
    pos: usize,
    /// Statistics for debugging
    stats: StreamingStats,
}

/// Statistics about the streaming parser's operation
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct StreamingStats {
    /// Total valid frames decoded
    pub frames_decoded: u32,
    /// Times synchronization was lost
    pub sync_losses: u32,
    /// Bytes discarded during resync
    pub bytes_discarded: u32,
}

impl Default for StreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingParser {
    /// Creates a new streaming parser
    pub const fn new() -> Self {
        Self {
            buffer: [0; SBUS_FRAME_LENGTH],
            pos: 0,
            stats: StreamingStats {
                frames_decoded: 0,
                sync_losses: 0,
                bytes_discarded: 0,
            },
        }
    }

    /// Get parser statistics
    pub const fn stats(&self) -> &StreamingStats {
        &self.stats
    }

    /// Reset the parser state
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// Push a single byte into the parser
    ///
    /// Returns `Some(packet)` if a complete frame was decoded, `None` otherwise
    pub fn push_byte(&mut self, byte: u8) -> Result<Option<SbusPacket>, SbusError> {
        // If we're at the start, only accept header
        if self.pos == 0 {
            if byte == SBUS_HEADER {
                self.buffer[0] = byte;
                self.pos = 1;
            } else {
                self.stats.bytes_discarded = self.stats.bytes_discarded.saturating_add(1);
            }
            return Ok(None);
        }

        // Add byte to buffer
        self.buffer[self.pos] = byte;
        self.pos += 1;

        // Check if we have a complete frame
        if self.pos == SBUS_FRAME_LENGTH {
            // Validate footer
            if self.buffer[SBUS_FRAME_LENGTH - 1] == SBUS_FOOTER {
                // Valid frame!
                match SbusPacket::from_array(&self.buffer) {
                    Ok(packet) => {
                        self.stats.frames_decoded = self.stats.frames_decoded.saturating_add(1);
                        self.pos = 0;
                        Ok(Some(packet))
                    }
                    Err(e) => {
                        self.resync();
                        Err(e)
                    }
                }
            } else {
                // Invalid frame, need to resync
                self.resync();
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Push multiple bytes into the parser
    ///
    /// Returns an iterator over successfully decoded packets
    pub fn push_bytes<'a>(&'a mut self, data: &'a [u8]) -> StreamingIterator<'a> {
        StreamingIterator {
            parser: self,
            data,
            index: 0,
        }
    }

    /// Try to resynchronize after frame error
    fn resync(&mut self) {
        self.stats.sync_losses = self.stats.sync_losses.saturating_add(1);

        // Look for next header in existing buffer
        let mut found = false;
        for i in 1..self.pos {
            if self.buffer[i] == SBUS_HEADER {
                // Found potential header, shift buffer
                let remaining = self.pos - i;
                self.stats.bytes_discarded = self.stats.bytes_discarded.saturating_add(i as u32);

                // Shift data to start of buffer
                for j in 0..remaining {
                    self.buffer[j] = self.buffer[i + j];
                }
                self.pos = remaining;
                found = true;
                break;
            }
        }

        if !found {
            // No header found, discard everything
            self.stats.bytes_discarded = self.stats.bytes_discarded.saturating_add(self.pos as u32);
            self.pos = 0;
        }
    }
}

/// Iterator returned by `push_bytes`
pub struct StreamingIterator<'a> {
    parser: &'a mut StreamingParser,
    data: &'a [u8],
    index: usize,
}

impl<'a> Iterator for StreamingIterator<'a> {
    type Item = Result<SbusPacket, SbusError>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.data.len() {
            let byte = self.data[self.index];
            self.index += 1;

            match self.parser.push_byte(byte) {
                Ok(Some(packet)) => return Some(Ok(packet)),
                Err(e) => return Some(Err(e)),
                Ok(None) => continue,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pack_channels, CHANNEL_COUNT, CHANNEL_MAX};
    use alloc::vec::Vec;

    fn create_test_frame(channels: &[u16; CHANNEL_COUNT], flags: u8) -> [u8; SBUS_FRAME_LENGTH] {
        let mut frame = [0u8; SBUS_FRAME_LENGTH];
        frame[0] = SBUS_HEADER;
        frame[SBUS_FRAME_LENGTH - 1] = SBUS_FOOTER;
        pack_channels(&mut frame, channels);
        frame[23] = flags;
        frame
    }

    #[test]
    fn test_single_complete_frame() {
        let mut parser = StreamingParser::new();
        let frame = create_test_frame(&[1000; CHANNEL_COUNT], 0);

        // Feed frame byte by byte
        for (i, &byte) in frame.iter().enumerate() {
            let result = parser.push_byte(byte).unwrap();
            if i < SBUS_FRAME_LENGTH - 1 {
                assert!(result.is_none());
            } else {
                assert!(result.is_some());
                let packet = result.unwrap();
                assert_eq!(packet.channels[0], 1000);
            }
        }

        assert_eq!(parser.stats().frames_decoded, 1);
        assert_eq!(parser.stats().sync_losses, 0);
    }

    #[test]
    fn test_multiple_frames_chunked() {
        let mut parser = StreamingParser::new();
        let frame1 = create_test_frame(&[100; CHANNEL_COUNT], 0);
        let frame2 = create_test_frame(&[200; CHANNEL_COUNT], 0);

        let mut data = [0u8; SBUS_FRAME_LENGTH * 2];
        data[..SBUS_FRAME_LENGTH].copy_from_slice(&frame1);
        data[SBUS_FRAME_LENGTH..].copy_from_slice(&frame2);

        let packets: Vec<_> = parser.push_bytes(&data).collect();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].as_ref().unwrap().channels[0], 100);
        assert_eq!(packets[1].as_ref().unwrap().channels[0], 200);

        assert_eq!(parser.stats().frames_decoded, 2);
    }

    #[test]
    fn test_resync_after_corruption() {
        let mut parser = StreamingParser::new();

        // Start with garbage
        let garbage = [0xFF, 0xAA, 0x55, 0x00];
        for &byte in &garbage {
            assert!(parser.push_byte(byte).unwrap().is_none());
        }

        // Then valid frame
        let frame = create_test_frame(&[500; CHANNEL_COUNT], 0);
        for &byte in &frame {
            parser.push_byte(byte).unwrap();
        }

        assert_eq!(parser.stats().frames_decoded, 1);
        assert_eq!(parser.stats().bytes_discarded, garbage.len() as u32);
    }

    #[test]
    fn test_corrupted_footer_recovery() {
        let mut parser = StreamingParser::new();
        let mut frame = create_test_frame(&[1500; CHANNEL_COUNT], 0);

        // Corrupt footer
        frame[SBUS_FRAME_LENGTH - 1] = 0xFF;

        // Feed corrupted frame
        for &byte in &frame {
            parser.push_byte(byte).unwrap();
        }

        assert_eq!(parser.stats().frames_decoded, 0);
        assert_eq!(parser.stats().sync_losses, 1);

        // Feed valid frame
        let good_frame = create_test_frame(&[2000; CHANNEL_COUNT], 0);
        let packets: Vec<_> = parser.push_bytes(&good_frame).collect();

        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].as_ref().unwrap().channels[0], 2000);
    }

    #[test]
    fn test_mid_stream_start() {
        let mut parser = StreamingParser::new();
        let frame = create_test_frame(&[1234; CHANNEL_COUNT], 0x0F); // All flags set

        // Start mid-frame
        let mid_start = &frame[10..];
        for &byte in mid_start {
            parser.push_byte(byte).unwrap();
        }

        // No frame should be decoded
        assert_eq!(parser.stats().frames_decoded, 0);

        // Now send complete frame
        let packets: Vec<_> = parser.push_bytes(&frame).collect();
        assert_eq!(packets.len(), 1);

        let packet = packets[0].as_ref().unwrap();
        assert_eq!(packet.channels[0], 1234);
        assert!(packet.flags.failsafe);
        assert!(packet.flags.frame_lost);
    }

    #[test]
    fn test_edge_cases() {
        let mut parser = StreamingParser::new();

        // Test double header - second header should start new frame attempt
        parser.push_byte(SBUS_HEADER).unwrap();
        assert_eq!(parser.pos, 1);

        // Feed 23 more bytes to complete a frame (even though it started with double header)
        for i in 0..23 {
            parser
                .push_byte(if i == 22 { SBUS_FOOTER } else { 0x00 })
                .unwrap();
        }
        // This will complete a frame (though it might be invalid data)

        // Reset and test all channel values
        parser.reset();
        let mut channels = [0u16; CHANNEL_COUNT];
        for i in 0..CHANNEL_COUNT {
            channels[i] = ((i as u16) * 100).min(CHANNEL_MAX);
        }
        let frame = create_test_frame(&channels, 0);

        let packets: Vec<_> = parser.push_bytes(&frame).collect();
        assert_eq!(packets.len(), 1);

        let packet = packets[0].as_ref().unwrap();
        for i in 0..CHANNEL_COUNT {
            assert_eq!(packet.channels[i], channels[i]);
        }
    }

    #[test]
    fn test_statistics_tracking() {
        let mut parser = StreamingParser::new();

        // Generate pattern: garbage, valid, corrupted, valid
        let garbage = [0xDE, 0xAD, 0xBE, 0xEF];
        let valid1 = create_test_frame(&[100; CHANNEL_COUNT], 0);
        let mut corrupted = create_test_frame(&[200; CHANNEL_COUNT], 0);
        corrupted[SBUS_FRAME_LENGTH - 1] = 0xFF; // Corrupt the footer instead
        let valid2 = create_test_frame(&[300; CHANNEL_COUNT], 0);

        // Feed all data
        for &b in &garbage {
            parser.push_byte(b).unwrap();
        }
        for &b in &valid1 {
            parser.push_byte(b).unwrap();
        }
        for &b in &corrupted {
            parser.push_byte(b).unwrap();
        }
        for &b in &valid2 {
            parser.push_byte(b).unwrap();
        }

        let stats = parser.stats();
        assert_eq!(stats.frames_decoded, 2); // valid1 and valid2
        assert!(stats.sync_losses >= 1); // At least one from corrupted frame
        assert!(stats.bytes_discarded >= garbage.len() as u32);
    }
}
