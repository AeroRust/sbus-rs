//! Integration tests for the streaming SBUS parser
use sbus_rs::*;

/// Helper to create a valid SBUS frame with specified channel values
/// Note: Channel values are automatically masked to 11 bits (0-2047) by pack_channels
fn create_frame(channel_value: u16) -> Vec<u8> {
    let mut frame = [0u8; SBUS_FRAME_LENGTH];
    frame[0] = SBUS_HEADER;
    frame[SBUS_FRAME_LENGTH - 1] = SBUS_FOOTER;

    let channels = [channel_value; 16];
    pack_channels(&mut frame, &channels);

    frame.to_vec()
}

#[test]
fn test_streaming_with_serial_like_chunks() {
    let mut parser = StreamingParser::new();

    // Simulate serial data arriving in random chunk sizes
    let frame1 = create_frame(100);
    let frame2 = create_frame(200);
    let frame3 = create_frame(300);

    let mut all_data = Vec::new();
    all_data.extend_from_slice(&frame1);
    all_data.extend_from_slice(&frame2);
    all_data.extend_from_slice(&frame3);

    // Simulate various chunk sizes that might come from a serial port
    let chunk_sizes = [1, 3, 7, 13, 17, 23, 5, 8, 4, 100];
    let mut received_packets = Vec::new();
    let mut pos = 0;

    for &chunk_size in &chunk_sizes {
        if pos >= all_data.len() {
            break;
        }

        let end = (pos + chunk_size).min(all_data.len());
        let chunk = &all_data[pos..end];

        for packet in parser.push_bytes(chunk) {
            received_packets.push(packet.unwrap());
        }

        pos = end;
    }

    // Process any remaining data
    if pos < all_data.len() {
        for packet in parser.push_bytes(&all_data[pos..]) {
            received_packets.push(packet.unwrap());
        }
    }

    assert_eq!(received_packets.len(), 3);
    assert_eq!(received_packets[0].channels[0], 100);
    assert_eq!(received_packets[1].channels[0], 200);
    assert_eq!(received_packets[2].channels[0], 300);
}

#[test]
fn test_streaming_with_noise_between_frames() {
    let mut parser = StreamingParser::new();

    // Create data stream with noise
    let mut data = Vec::new();

    // Noise at start
    data.extend_from_slice(&[0xFF, 0xAA, 0x55, 0x00, 0x12, 0x34]);

    // Valid frame
    data.extend_from_slice(&create_frame(1111));

    // Noise between frames
    data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    // Another valid frame (use value within CHANNEL_MAX)
    data.extend_from_slice(&create_frame(2000)); // Changed from 2222 to 2000

    // Partial frame at end (will be ignored)
    data.extend_from_slice(&[SBUS_HEADER, 0x00, 0x00]);

    let packets: Vec<_> = parser.push_bytes(&data).filter_map(Result::ok).collect();

    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].channels[0], 1111);
    assert_eq!(packets[1].channels[0], 2000);

    let stats = parser.stats();
    assert_eq!(stats.frames_decoded, 2);
    assert!(stats.bytes_discarded > 0);
}

#[test]
fn test_streaming_corrupted_frame_recovery() {
    let mut parser = StreamingParser::new();

    // Create a corrupted frame (bad footer)
    let mut corrupted = create_frame(999);
    corrupted[SBUS_FRAME_LENGTH - 1] = 0xFF; // Corrupt the footer

    // Follow with valid frames
    let mut data = Vec::new();
    data.extend_from_slice(&corrupted);
    data.extend_from_slice(&create_frame(444));
    data.extend_from_slice(&create_frame(555));

    let packets: Vec<_> = parser.push_bytes(&data).filter_map(Result::ok).collect();

    // Should recover and parse the valid frames
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].channels[0], 444);
    assert_eq!(packets[1].channels[0], 555);
}

#[test]
fn test_streaming_single_byte_at_a_time() {
    let mut parser = StreamingParser::new();
    let frame = create_frame(1234);

    let mut packets = Vec::new();

    // Feed one byte at a time
    for &byte in &frame {
        if let Some(packet) = parser.push_byte(byte).unwrap() {
            packets.push(packet);
        }
    }

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].channels[0], 1234);
    assert_eq!(packets[0].channels[15], 1234);
}

#[test]
fn test_streaming_with_flag_variations() {
    let mut parser = StreamingParser::new();

    // Test all flag combinations
    let test_cases = [
        (0b0000, false, false, false, false),
        (0b0001, true, false, false, false), // d1
        (0b0010, false, true, false, false), // d2
        (0b0100, false, false, true, false), // frame_lost
        (0b1000, false, false, false, true), // failsafe
        (0b1111, true, true, true, true),    // all flags
    ];

    for (flag_byte, d1, d2, frame_lost, failsafe) in test_cases {
        parser.reset();

        let mut frame = create_frame(100);
        frame[23] = flag_byte;

        let packets: Vec<_> = parser.push_bytes(&frame).filter_map(Result::ok).collect();

        assert_eq!(packets.len(), 1);
        let flags = packets[0].flags;
        assert_eq!(flags.d1, d1);
        assert_eq!(flags.d2, d2);
        assert_eq!(flags.frame_lost, frame_lost);
        assert_eq!(flags.failsafe, failsafe);
    }
}

#[test]
fn test_streaming_max_channel_values() {
    let mut parser = StreamingParser::new();

    // Test with maximum channel values
    let mut channels = [0u16; 16];
    for (i, ch) in channels.iter_mut().enumerate() {
        *ch = if i % 2 == 0 { 0 } else { CHANNEL_MAX };
    }

    let mut frame = [0u8; SBUS_FRAME_LENGTH];
    frame[0] = SBUS_HEADER;
    frame[SBUS_FRAME_LENGTH - 1] = SBUS_FOOTER;
    pack_channels(&mut frame, &channels);

    let packets: Vec<_> = parser.push_bytes(&frame).filter_map(Result::ok).collect();

    assert_eq!(packets.len(), 1);
    for (i, &ch) in packets[0].channels.iter().enumerate() {
        assert_eq!(ch, if i % 2 == 0 { 0 } else { CHANNEL_MAX });
    }
}

#[test]
fn test_streaming_performance_characteristics() {
    let mut parser = StreamingParser::new();

    // Create a large stream of frames
    const FRAME_COUNT: usize = 1000;
    let mut data = Vec::with_capacity(SBUS_FRAME_LENGTH * FRAME_COUNT);

    for i in 0..FRAME_COUNT {
        data.extend_from_slice(&create_frame(i as u16));
    }

    // Add some corruption every 100 frames by corrupting the footer
    for i in (0..FRAME_COUNT).step_by(100).skip(1) {
        let corruption_pos = i * SBUS_FRAME_LENGTH + SBUS_FRAME_LENGTH - 1; // Corrupt the footer
        if corruption_pos < data.len() {
            data[corruption_pos] = 0xFF;
        }
    }

    let packets: Vec<_> = parser.push_bytes(&data).filter_map(Result::ok).collect();

    // Should get most frames despite corruption
    assert!(packets.len() > FRAME_COUNT * 90 / 100); // At least 90% success rate

    let stats = parser.stats();
    assert!(stats.sync_losses > 0);
}

/// Test the example from the documentation
#[test]
fn test_doc_example() {
    let mut parser = StreamingParser::new();

    // Create a valid frame
    let frame = create_frame(1024);

    // Feed bytes as they arrive
    let result = parser.push_byte(0x0F).unwrap();
    assert!(result.is_none()); // Won't return a packet yet

    // Feed the rest
    for packet in parser.push_bytes(&frame[1..]) {
        let packet = packet.unwrap();
        assert_eq!(packet.channels[0], 1024);
    }
}

/// Ensure the streaming parser maintains compatibility with channel encoding
#[test]
fn test_streaming_channel_encoding_compatibility() {
    let mut parser = StreamingParser::new();

    // Test specific channel patterns
    let patterns = [
        [0u16; 16],                                             // All zeros
        [CHANNEL_MAX; 16],                                      // All max
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], // Sequential
        [
            1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, // Mid-range
            1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015,
        ],
    ];

    for pattern in patterns {
        parser.reset();

        let mut frame = [0u8; SBUS_FRAME_LENGTH];
        frame[0] = SBUS_HEADER;
        frame[SBUS_FRAME_LENGTH - 1] = SBUS_FOOTER;
        pack_channels(&mut frame, &pattern);

        let packets: Vec<_> = parser.push_bytes(&frame).filter_map(Result::ok).collect();

        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].channels, pattern);
    }
}

/// Test error propagation in the iterator
#[test]
fn test_streaming_iterator_error_propagation() {
    // Create a struct that wraps StreamingParser and overrides push_byte to return an error
    struct ErrorStreamingParser {
        inner: StreamingParser,
        error_on_byte: u8,
    }

    impl ErrorStreamingParser {
        fn new(error_on_byte: u8) -> Self {
            Self {
                inner: StreamingParser::new(),
                error_on_byte,
            }
        }

        fn push_byte(&mut self, byte: u8) -> Result<Option<SbusPacket>, SbusError> {
            // Return an error when the specified byte is pushed
            if byte == self.error_on_byte {
                Err(SbusError::InvalidHeader(byte))
            } else {
                self.inner.push_byte(byte)
            }
        }

        fn push_bytes<'a>(&'a mut self, data: &'a [u8]) -> ErrorStreamingIterator<'a> {
            ErrorStreamingIterator {
                parser: self,
                data,
                index: 0,
            }
        }
    }

    // Create a custom iterator that uses our ErrorStreamingParser
    struct ErrorStreamingIterator<'a> {
        parser: &'a mut ErrorStreamingParser,
        data: &'a [u8],
        index: usize,
    }

    impl<'a> Iterator for ErrorStreamingIterator<'a> {
        type Item = Result<SbusPacket, SbusError>;

        fn next(&mut self) -> Option<Self::Item> {
            while self.index < self.data.len() {
                let byte = self.data[self.index];
                self.index += 1;

                match self.parser.push_byte(byte) {
                    Ok(Some(packet)) => return Some(Ok(packet)),
                    Err(e) => return Some(Err(e)),  // This is the line we want to test
                    Ok(None) => continue,
                }
            }
            None
        }
    }

    // Create our custom parser that will return an error when it sees 0x42
    let mut parser = ErrorStreamingParser::new(0x42);

    // Create some test data that includes the error-triggering byte
    let data = [0x0F, 0x01, 0x42, 0x03, 0x04];

    // Collect all results from the iterator
    let results: Vec<_> = parser.push_bytes(&data).collect();

    // Verify that we got an error and it's the expected type
    assert_eq!(results.len(), 1);
    match &results[0] {
        Err(SbusError::InvalidHeader(byte)) => {
            assert_eq!(*byte, 0x42);
        }
        other => panic!("Expected InvalidHeader error, got {:?}", other),
    }
}
