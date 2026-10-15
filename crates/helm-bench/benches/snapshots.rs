//! Snapshot benchmarks for `helm-text`, gated by the `bench` CI job against
//! `budgets.toml`. NF-9: taking a snapshot costs the same on a 10 MiB buffer
//! as on an empty one, and holding one doesn't make edits much slower.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_text::{Buffer, BufferSet, Edit, Transaction};

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// Snapshots taken in one iteration of `snapshot_1024x_10mb`. One snapshot
/// is a few nanoseconds, too short to gate on its own.
const SNAPSHOTS: usize = 1024;

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

fn snapshots(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let middle = floor_boundary(&text, text.len() / 2);
    let transaction = Transaction::from(vec![Edit::insert(id, middle, "helm 😀")]);

    let mut group = c.benchmark_group("snapshots");

    group.bench_function("snapshot_1024x_10mb", |b| {
        b.iter(|| {
            for _ in 0..SNAPSHOTS {
                black_box(set.snapshot(black_box(id)).expect("known buffer"));
            }
        })
    });

    group.bench_function("apply_undo_holding_snapshot_10mb", |b| {
        b.iter_batched(
            || transaction.clone(),
            |transaction| {
                let snapshot = set.snapshot(black_box(id)).expect("known buffer");
                black_box(set.apply(black_box(transaction)).expect("valid edit"));
                black_box(set.undo(id).expect("known buffer"));
                drop(black_box(snapshot));
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, snapshots);
criterion_main!(benches);
