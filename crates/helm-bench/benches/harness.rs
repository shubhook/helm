//! Benchmarks gated by the `bench` CI job against `budgets.toml`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

/// Size of the string the placeholder works on: 1 MiB of ASCII.
const STRING_LEN: usize = 1 << 20;

/// Number of insert-and-slice steps per iteration.
const EDITS: usize = 128;

/// PLACEHOLDER, replaced by the rope benchmarks in T-101. It exists so the
/// harness and the CI comparison have something real to measure: each
/// iteration inserts into a 1 MiB `String` at spread-out offsets and slices
/// it, which moves about 32 MiB of memory and takes a millisecond or two.
fn placeholder_string_edit(c: &mut Criterion) {
    let base = "abcdefghijklmnopqrstuvwxyz0123456789\n".repeat(STRING_LEN / 37 + 1);
    let base = &base[..STRING_LEN];

    c.bench_function("placeholder_string_edit", |b| {
        b.iter(|| {
            let mut text = String::from(black_box(base));
            let mut checksum = 0usize;
            for i in 0..EDITS {
                let at = (i * 7919) % text.len();
                text.insert_str(at, "helm");
                let end = (at + 4096).min(text.len());
                checksum += black_box(&text[at..end]).len();
            }
            black_box(checksum)
        })
    });
}

criterion_group!(benches, placeholder_string_edit);
criterion_main!(benches);
