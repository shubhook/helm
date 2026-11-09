//! Cursor motion benchmarks for `helm-editor`, gated by the `bench` CI job
//! against `budgets.toml`. NF-1 gives a keypress 5 ms from input to pixel,
//! and a motion at many cursors is on that path. A motion that copied the
//! whole text would show up here.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use helm_editor::{Editor, Motion, Selection, Selections};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings, TextBuffer};

/// Number of cursors moved per iteration.
const CURSORS: usize = 1000;

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// The same lines `benches/editor.rs` repeats: ASCII code mixed with é,
/// 中 and 😀, ending in both LF and CRLF.
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

fn cursor(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(&text),
    };
    let buffer = set.get(id).expect("just added");
    // One cursor at the start of each of 1,000 `fn main() {` lines spread
    // evenly across the text. That line has no indentation, so WordRight
    // then WordLeft, and Down then Up, end where they started, and every
    // iteration does the same work.
    let repeats = buffer.len_lines() / LINES.lines().count() - 1;
    let cursors: Vec<Selection> = (0..CURSORS)
        .map(|i| {
            let line = i * repeats / CURSORS * LINES.lines().count();
            let at = buffer.line_to_byte(line).expect("line in range");
            assert_eq!(buffer.slice(at..at + 2).as_deref(), Ok("fn"));
            Selection::cursor(at)
        })
        .collect();
    let selections = Selections::new(cursors, 0).expect("non-empty, primary in range");

    let mut group = c.benchmark_group("cursor");

    let mut editor = Editor::new(id, format);
    editor.set_selections(selections.clone());
    group.bench_function("word_right_1000_cursors_10mb", |b| {
        b.iter(|| {
            let right = editor
                .move_cursors(&set, black_box(Motion::WordRight))
                .expect("buffer is in the set");
            let left = editor
                .move_cursors(&set, black_box(Motion::WordLeft))
                .expect("buffer is in the set");
            black_box((right, left));
        })
    });

    let mut editor = Editor::new(id, format);
    editor.set_selections(selections);
    group.bench_function("down_up_1000_cursors_10mb", |b| {
        b.iter(|| {
            let down = editor
                .move_cursors(&set, black_box(Motion::Down))
                .expect("buffer is in the set");
            let up = editor
                .move_cursors(&set, black_box(Motion::Up))
                .expect("buffer is in the set");
            black_box((down, up));
        })
    });

    group.finish();
}

criterion_group!(benches, cursor);
criterion_main!(benches);
