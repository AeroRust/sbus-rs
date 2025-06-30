use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use embedded_io_adapters::std::FromStd;
use sbus_rs::{
    pack_channels, SbusPacket, SbusParser, CHANNEL_COUNT, CHANNEL_MAX, SBUS_FOOTER,
    SBUS_FRAME_LENGTH, SBUS_HEADER,
};
use std::hint::black_box;
use std::io::Cursor;

const fn generate_alternating() -> [u16; CHANNEL_COUNT] {
    let mut arr = [0u16; 16];
    let mut i = 0;
    while i < 16 {
        arr[i] = if i % 2 == 0 { 0u16 } else { CHANNEL_MAX };
        i += 1;
    }
    arr
}

const fn generate_ascending() -> [u16; CHANNEL_COUNT] {
    let mut arr = [0u16; 16];
    let mut i = 0;
    while i < 16 {
        arr[i] = i as u16 * 128;
        i += 1;
    }
    arr
}

const SCENARIOS: &[(&str, [u16; CHANNEL_COUNT])] = &[
    ("all_min", [0u16; 16]),
    ("all_max", [CHANNEL_MAX; 16]),
    ("all_mid", [CHANNEL_MAX.div_ceil(2); 16]),
    ("alternating", generate_alternating()),
    ("ascending", generate_ascending()),
];

fn create_test_frame(channels: &[u16; 16], flags: u8) -> [u8; SBUS_FRAME_LENGTH] {
    let mut buffer = [0u8; SBUS_FRAME_LENGTH];
    buffer[0] = SBUS_HEADER;
    buffer[SBUS_FRAME_LENGTH - 1] = SBUS_FOOTER;

    // Pack channels using the same logic from tests
    pack_channels(&mut buffer, channels);
    buffer[23] = flags;

    buffer
}

fn create_streaming_buffer(frame_count: usize) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(SBUS_FRAME_LENGTH * frame_count);
    let channels = [1000u16; 16]; // Mid-range value for all channels
    let frame = create_test_frame(&channels, 0);

    for _ in 0..frame_count {
        buffer.extend_from_slice(&frame);
    }
    buffer
}

fn bench_sync_frame_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync/frame_parsing");

    for (name, channels) in SCENARIOS {
        let frame = create_test_frame(channels, 0);
        group.bench_with_input(
            BenchmarkId::new("sync/parse_frame", name),
            &frame,
            |b, frame| b.iter(|| black_box(SbusPacket::from_array(black_box(frame))).unwrap()),
        );
    }

    group.finish();
}

fn bench_sync_streaming_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync/streaming_parser");

    for frames in [1, 10, 100] {
        let buffer = create_streaming_buffer(frames);

        group.bench_with_input(
            BenchmarkId::new("sync/parse_stream", frames),
            &buffer,
            |b, data| {
                b.iter(|| {
                    let cursor = Cursor::new(data);
                    let mut parser = SbusParser::new(FromStd::new(cursor));
                    for _ in 0..frames {
                        black_box(parser.read_frame()).unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_sync_frame_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync/frame_validation");

    let valid_frame = create_test_frame(&[1000u16; 16], 0);
    let mut invalid_header = valid_frame;
    invalid_header[0] = 0x00;
    let mut invalid_footer = valid_frame;
    invalid_footer[SBUS_FRAME_LENGTH - 1] = 0xFF;

    group.bench_function("sync/validate/valid_frame", |b| {
        b.iter(|| black_box(SbusPacket::from_array(black_box(&valid_frame))).unwrap())
    });

    group.bench_function("sync/validate/invalid_header", |b| {
        b.iter(|| {
            let _ = black_box(SbusPacket::from_array(black_box(&invalid_header)));
        })
    });

    group.bench_function("sync/validate/invalid_footer", |b| {
        b.iter(|| {
            let _ = black_box(SbusPacket::from_array(black_box(&invalid_footer)));
        })
    });

    group.finish();
}

#[cfg(feature = "async")]
fn bench_async_parser(c: &mut Criterion) {
    use embedded_io_adapters::tokio_1::FromTokio;
    use sbus_rs::SbusParserAsync;
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("async/streaming_parser");

    for frames in [1, 10, 100] {
        let buffer = create_streaming_buffer(frames);

        group.bench_with_input(
            BenchmarkId::new("async/parse_stream", frames),
            &buffer,
            |b, data| {
                b.iter(|| {
                    rt.block_on(async {
                        let cursor = Cursor::new(data);
                        let mut parser = SbusParserAsync::new(FromTokio::new(cursor));
                        for _ in 0..frames {
                            black_box(parser.read_frame().await).unwrap();
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

#[cfg(not(feature = "async"))]
criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(200);
    targets = bench_sync_frame_parsing, bench_sync_streaming_parser, bench_sync_frame_validation
}

#[cfg(feature = "async")]
criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(200);
    targets = bench_sync_frame_parsing, bench_sync_streaming_parser, bench_sync_frame_validation, bench_async_parser
}

criterion_main!(benches);
