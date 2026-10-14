//! Rope benchmarks for `helm-text`, gated by the `bench` CI job against
//! `budgets.toml`. NF-7: no rope edit or offset lookup gets more than 5%
//! slower than its base commit.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_text::{Buffer, TextBuffer};

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// Number of offsets each lookup benchmark converts per iteration.
const LOOKUPS: usize = 1024;

/// The lines the text repeats: ASCII code mixed with é (2 bytes), 中
/// (3 bytes) and 😀 (4 bytes, a surrogate pair in UTF-16), ending in both LF
/// and CRLF. Fixed, so base and head measure the same text.
const LINES: &str = concat!(
    "fn main() {\n",
    "    let café = \"crème brûlée\";\r\n",
    "    // 中文注释: 编辑器\n",
    "    println!(\"{café} 😀 done\");\r\n",
    "    let emoji = ['😀', '中', 'é'];\n",
    "}\r\n",
    "\n",
);

/// The largest char boundary of `text` at or below `byte`.
fn floor_boundary(text: &str, mut byte: usize) -> usize {
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// 10 MiB of `LINES` repeated, cut at the last char boundary at or below
/// `TEXT_LEN`.
fn build_text() -> String {
    let mut text = LINES.repeat(TEXT_LEN / LINES.len() + 1);
    text.truncate(floor_boundary(&text, TEXT_LEN));
    text
}

/// `LOOKUPS` valid byte offsets spread evenly across `text`.
fn spread_offsets(text: &str) -> Vec<usize> {
    (0..LOOKUPS)
        .map(|i| floor_boundary(text, i * text.len() / LOOKUPS))
        .collect()
}

fn text(c: &mut Criterion) {
    let text = build_text();
    let buffer = Buffer::from(text.as_str());
    let middle = floor_boundary(&text, text.len() / 2);
    let bytes = spread_offsets(&text);
    // Converted from char boundaries, so none falls inside a surrogate pair.
    let utf16s: Vec<usize> = bytes
        .iter()
        .map(|&byte| buffer.byte_to_utf16(byte).expect("valid byte offset"))
        .collect();

    let mut group = c.benchmark_group("text");

    group.bench_function("insert_middle_10mb", |b| {
        b.iter_batched(
            || buffer.clone(),
            |mut buffer| {
                buffer
                    .insert(black_box(middle), black_box("helm 😀"))
                    .expect("valid byte offset");
                buffer
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("byte_to_line_10mb", |b| {
        b.iter(|| {
            for &byte in &bytes {
                black_box(
                    buffer
                        .byte_to_line(black_box(byte))
                        .expect("valid byte offset"),
                );
            }
        })
    });

    group.bench_function("utf16_to_byte_10mb", |b| {
        b.iter(|| {
            for &utf16 in &utf16s {
                black_box(
                    buffer
                        .utf16_to_byte(black_box(utf16))
                        .expect("valid UTF-16 offset"),
                );
            }
        })
    });

    group.finish();
}

criterion_group!(benches, text);
criterion_main!(benches);
