//! SBus Frame parser
use core::marker::PhantomData;

#[cfg(feature = "async")]
mod asynch;

#[cfg(feature = "async")]
pub use asynch::SbusParserAsync;

#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "blocking")]
pub use blocking::SbusParser;

pub struct Parser<R, M: Mode> {
    #[allow(dead_code)]
    reader: R,
    _mode: PhantomData<M>,
}

#[allow(private_bounds)]
pub trait Mode: Sealed {}

trait Sealed {}

/// The SBus Frame header should start with `0x0F` byte (15 decimal).
pub const SBUS_HEADER: u8 = 0x0F;
/// The SBus Frame footer should end with a zero byte `0x00` (0 decimal).
pub const SBUS_FOOTER: u8 = 0x00;
pub const SBUS_FOOTER_2: u8 = 0x04;
/// The SBus Frame length
pub const SBUS_FRAME_LENGTH: usize = 25;
/// The number of channels in a SBus Frame.
pub const CHANNEL_COUNT: usize = 16;
/// The maximum value of a channel.
pub const CHANNEL_MAX: u16 = 0x07FF; // 11 bits max = 2047
