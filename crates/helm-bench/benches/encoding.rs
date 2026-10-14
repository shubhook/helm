//! File format benchmarks for `helm-text`, gated by the `bench` CI job
//! against `budgets.toml`. NF-7: opening and saving a file sit on the same
//! path as rope edits, so they get the same 5% budget.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use helm_text::FileFormat;

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

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

fn encoding(c: &mut Criterion) {
    let text = build_text();
    let mut bytes = Vec::with_capacity(3 + text.len());
    bytes.extend_from_slice(b"\xEF\xBB\xBF");
    bytes.extend_from_slice(text.as_bytes());

    let mut group = c.benchmark_group("encoding");

    group.bench_function("decode_encode_bom_10mb", |b| {
        b.iter(|| {
            let decoded = black_box(FileFormat::decode(black_box(&bytes)).expect("valid UTF-8"));
            black_box(decoded.format.encode(decoded.text))
        })
    });

    group.finish();
}

criterion_group!(benches, encoding);
criterion_main!(benches);
