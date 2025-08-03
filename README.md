# sbus-rs

[![Crates.io](https://img.shields.io/crates/v/sbus-rs.svg)](https://crates.io/crates/sbus-rs)
[![Documentation](https://docs.rs/sbus-rs/badge.svg)](https://docs.rs/sbus-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A no_std compatible Rust implementation of the SBUS (Serial Bus) protocol parser, commonly used in RC (Radio Control) applications. Part of the [AeroRust](https://github.com/AeroRust) organization, dedicated to aerospace-related software in Rust.

## Features

- 🦀 Pure Rust implementation
- 🚫 `no_std` compatible for embedded systems
- ⚡ Async and blocking IO support (mutually exclusive)
- 🔄 Streaming parser for incremental data processing
- 🔍 Robust error handling and validation
- 🧪 Thoroughly tested with unit tests, property-based tests, and fuzzing
- 🛠️ Zero-copy parsing for efficient memory usage

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
sbus-rs = "0.1.3"  # Uses blocking IO by default
```

For async support (disables blocking):
```toml
[dependencies]
sbus-rs = { version = "0.1.3", default-features = false, features = ["async"] }
```

For std support (enables additional adapters):
```toml
[dependencies]
sbus-rs = { version = "0.1.3", features = ["std"] }
```

## Usage

### Blocking Example

```rust
use sbus_rs::{SbusParser, SbusPacket, SbusError};
use embedded_io_adapters::std::FromStd;
use std::io::Cursor;

fn main() -> Result<(), SbusError> {
    // Example with cursor (replace with your serial port)
    let data = [0x0F, /* ... SBUS frame data ... */, 0x00];
    let cursor = Cursor::new(&data[..]);
    let mut parser = SbusParser::new(FromStd::new(cursor));
    
    // Read a single SBUS frame
    let packet = parser.read_frame()?;
    
    // Access channel values (0-2047)
    println!("Channel 1: {}", packet.channels[0]);
    
    // Check flags
    if packet.flags.failsafe {
        println!("Failsafe active!");
    }
    if packet.flags.frame_lost {
        println!("Frame lost!");
    }
    
    Ok(())
}
```

### Async Example

```rust
use sbus_rs::{SbusParser, SbusPacket, SbusError};
use embedded_io_adapters::tokio_1::FromTokio;

async fn read_sbus() -> Result<(), SbusError> {
    let serial = /* your async serial port */;
    let mut parser = SbusParser::new(FromTokio::new(serial));
    
    // Read frames asynchronously
    let packet = parser.read_frame().await?;
    
    println!("Channels: {:?}", packet.channels);
    println!("Digital channel 1: {}", packet.flags.d1);
    println!("Digital channel 2: {}", packet.flags.d2);
    println!("Frame lost: {}", packet.flags.frame_lost);
    println!("Failsafe: {}", packet.flags.failsafe);
    
    Ok(())
}
```

### Streaming Parser Example

For scenarios where data arrives incrementally (e.g., serial ports):

```rust
use sbus_rs::StreamingParser;

fn main() {
    let mut parser = StreamingParser::new();
    
    // Process data as it arrives
    let incoming_data = [0x0F, 0x01, 0x02, /* ... more bytes ... */];
    
    for byte in incoming_data {
        if let Ok(Some(packet)) = parser.push_byte(byte) {
            println!("Complete packet received!");
            println!("Channel 1: {}", packet.channels[0]);
        }
    }
    
    // Or process chunks
    let chunk = [0x03, 0x04, 0x05, /* ... */];
    for result in parser.push_bytes(&chunk) {
        if let Ok(packet) = result {
            println!("Packet: {:?}", packet);
        }
    }
}
```

## Protocol Details

SBUS frames are 25 bytes long with the following structure:
- Start byte (0x0F)
- 22 bytes of channel data (16 channels, 11 bits each)
- 1 byte of flags
- End byte (0x00)

### Channel Data
- 16 channels, each 11 bits (values 0-2047)
- Channels are tightly packed across the 22 data bytes

### Flag Bits
The flag byte contains:
- `d1`: Digital channel 1 state
- `d2`: Digital channel 2 state  
- `frame_lost`: Indicates if frames have been lost
- `failsafe`: Indicates failsafe mode is active

## API Overview

### Core Types

- `SbusParser<R>`: Main parser for blocking or async I/O
- `SbusPacket`: Represents a decoded SBUS frame
- `StreamingParser`: Processes incremental data
- `SbusError`: Error types for parsing failures
- `Flags`: Status flags from SBUS frames

### Feature Flags

- `blocking`: Blocking I/O support (default)
- `async`: Async I/O support (mutually exclusive with blocking)
- `std`: Standard library support for additional adapters
- `defmt`: Logging support for embedded systems

## Performance

The library is optimized for performance with:
- Zero-copy parsing where possible
- Efficient bit manipulation for channel extraction
- Minimal allocations (no-std compatible)
- Streaming support for real-time processing

## Contributing

Contributions are welcome! Please ensure:

1. Tests pass: `cargo test --features blocking,std` and `cargo test --no-default-features --features async,std`
2. Code is formatted: `cargo fmt`
3. Clippy passes: `cargo clippy`
4. No-std compatibility: `cargo check --no-default-features`

## Safety

This crate uses only safe Rust and includes comprehensive testing:
- Unit tests for all parsing logic
- Property-based testing for edge cases
- Integration tests with real SBUS data
- CI testing on multiple platforms and Rust versions

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE.txt) file for details.

## Acknowledgments

Part of the [AeroRust](https://github.com/AeroRust) organization, promoting the use of Rust in aerospace applications.

Special thanks to:
- The AeroRust community
- Contributors and maintainers
- The Rust embedded community