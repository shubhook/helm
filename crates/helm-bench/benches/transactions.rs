//! Transaction benchmarks for `helm-text`, gated by the `bench` CI job
//! against `budgets.toml`. NF-7: no rope edit gets more than 5% slower than
//! its base commit.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_text::{Buffer, BufferSet, Edit, Transaction};

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// Number of edits in the benchmark transaction.
const EDITS: usize = 100;

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

fn transactions(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    // Edit `i` sits at the char boundary at or below `i * len / EDITS`. Even
    // edits insert there; odd ones delete the one character that starts
    // there. The edits are far apart, so none overlap.
    let transaction: Transaction = (0..EDITS)
        .map(|i| {
            let at = floor_boundary(&text, i * text.len() / EDITS);
            if i % 2 == 0 {
                Edit::insert(id, at, "helm 😀")
            } else {
                let len = text[at..]
                    .chars()
                    .next()
                    .expect("not at the end")
                    .len_utf8();
                Edit::delete(id, at..at + len)
            }
        })
        .collect();

    let mut group = c.benchmark_group("transactions");

    group.bench_function("apply_undo_100_edits_10mb", |b| {
        b.iter_batched(
            || transaction.clone(),
            |transaction| {
                black_box(set.apply(black_box(transaction)).expect("valid edits"));
                black_box(set.undo(id).expect("known buffer"));
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, transactions);
criterion_main!(benches);
