//! Amend benchmarks for `helm-text`, gated by the `bench` CI job against
//! `budgets.toml`. NF-7: no rope edit gets more than 5% slower than its base
//! commit. This one also guards T-110's cost rule: an amend that copied the
//! buffer, or an undo that replayed every amend, would show up here.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_text::{Buffer, BufferSet, Edit, Transaction};

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// Number of amends after the first transaction.
const AMENDS: usize = 100;

/// The characters typed, in turn: 1-, 2-, 3- and 4-byte ones.
const TYPED: &str = "helm 😀中é";

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

fn amend(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));

    // Character `i` lands right after character `i - 1`, starting at the
    // char boundary at or below the middle.
    let mut at = floor_boundary(&text, text.len() / 2);
    let mut typed = TYPED.chars().cycle().map(|c| {
        let edit = Edit::insert(id, at, c.to_string());
        at += c.len_utf8();
        Transaction::from(vec![edit])
    });
    let first = typed.next().expect("cycle never ends");
    let amends: Vec<Transaction> = typed.take(AMENDS).collect();

    let mut group = c.benchmark_group("amend");

    group.bench_function("type_100_chars_undo_10mb", |b| {
        b.iter_batched(
            || (first.clone(), amends.clone()),
            |(first, amends)| {
                let transaction = set.apply(black_box(first)).expect("valid edit");
                for amend in amends {
                    set.amend(transaction, black_box(amend))
                        .expect("valid amend");
                }
                black_box(set.undo(id).expect("known buffer"));
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, amend);
criterion_main!(benches);
