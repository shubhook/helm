//! Multi-cursor editing benchmarks for `helm-editor`, gated by the `bench`
//! CI job against `budgets.toml`. NF-7: typing at many cursors is a rope
//! edit per cursor, and no rope edit gets more than 5% slower than its base
//! commit. This is the headless part of NF-1 (keypress to pixel).

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use helm_editor::{Editor, Selection, Selections};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// Number of cursors typed at per iteration.
const CURSORS: usize = 1000;

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

fn editor(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(&text),
    };
    let mut editor = Editor::new(id, format);
    let cursors = (0..CURSORS)
        .map(|i| Selection::cursor(floor_boundary(&text, i * text.len() / CURSORS)))
        .collect();
    editor.set_selections(Selections::new(cursors, 0).expect("non-empty, primary in range"));

    let mut group = c.benchmark_group("editor");

    // The undo puts back the text and the cursors, and the next insert drops
    // the redo entry, so every iteration starts from the same state.
    group.bench_function("type_undo_1000_cursors_10mb", |b| {
        b.iter(|| {
            let typed = editor
                .insert(&mut set, black_box("x"))
                .expect("buffer is in the set");
            let undone = editor.undo(&mut set).expect("nothing blocks the undo");
            black_box((typed, undone));
        })
    });

    group.finish();
}

criterion_group!(benches, editor);
criterion_main!(benches);
