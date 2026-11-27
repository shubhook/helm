//! Vim command benchmarks for `helm-editor` (T-108), gated by the `bench`
//! CI job against `budgets.toml`. NF-1 gives a keypress 5 ms from input to
//! pixel, and a Vim command at many cursors is on that path. Each
//! benchmark times only the command, from the key event or invocation to
//! the changed buffer and selections, not drawing or input delay.
//!
//! The first three have targets in T-108's task file, which the gate
//! doesn't check: `w_b` under 500 µs, `d3w_undo` under 2.5 ms and
//! `insert_100_chars_undo` under 250 µs. The other five cover the paths
//! the task's cost rules name, gated by the default budget only.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_editor::{
    CommandId, CommandRegistry, Dispatcher, Editor, Invocation, Key, KeyEvent, Keystroke,
    Modifiers, Selection, Selections, vim, vim_commands,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings, TextBuffer};

/// Number of cursors per iteration.
const CURSORS: usize = 1000;

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// The characters typed in insert mode, in turn: 1-, 2-, 3- and 4-byte
/// ones.
const TYPED: &str = "helm 😀中é";

/// Number of characters typed in one insert session.
const TYPED_CHARS: usize = 100;

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

/// A key event for a plain char, which types it in insert mode.
fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE))
}

/// `command` with `count` in its arguments.
fn counted(command: &str, count: usize) -> Invocation {
    let mut invocation = Invocation::new(command);
    invocation.args.count = Some(count);
    invocation
}

fn vim(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(&text),
    };
    let buffer = set.get(id).expect("just added");
    // One cursor at the start of each of 1,000 `fn main() {` lines spread
    // evenly across the text, as `benches/cursor.rs` places them. That line
    // has no indentation, so `w` then `b` ends where it started, and `u`
    // puts back the cursors from before each edit.
    let repeats = buffer.len_lines() / LINES.lines().count() - 1;
    let starts: Vec<usize> = (0..CURSORS)
        .map(|i| {
            let line = i * repeats / CURSORS * LINES.lines().count();
            let at = buffer.line_to_byte(line).expect("line in range");
            assert_eq!(buffer.slice(at..at + 2).as_deref(), Ok("fn"));
            at
        })
        .collect();
    let selections = Selections::new(starts.iter().map(|&at| Selection::cursor(at)).collect(), 0)
        .expect("non-empty, primary in range");
    let middle = Selections::single(Selection::cursor(starts[CURSORS / 2]));

    let registry = CommandRegistry::with_vim_commands();
    let editor_at = |selections: &Selections| {
        let mut editor = Editor::new(id, format);
        editor.set_selections(selections.clone());
        editor
    };
    let run = |editor: &mut Editor, set: &mut BufferSet, invocation: &Invocation| {
        registry
            .run(black_box(invocation), editor, set)
            .expect("the command runs");
    };

    let w = Invocation::new(vim_commands::NEXT_WORD);
    let b = Invocation::new(vim_commands::PREV_WORD);
    let undo = Invocation::new(vim_commands::UNDO);
    let mut d3w = counted(vim_commands::DELETE, 3);
    d3w.args.motion = Some(CommandId::from(vim_commands::NEXT_WORD));
    let down = counted(vim_commands::DOWN, 999_999);
    let up = counted(vim_commands::UP, 999_999);
    let first = Invocation::new(vim_commands::FIRST_LINE);
    let last = Invocation::new(vim_commands::LAST_LINE);
    let dollar = counted(vim_commands::LINE_END, 3);
    let mut dd = Invocation::new(vim_commands::DELETE);
    dd.args.motion = Some(CommandId::from(vim_commands::LINE));
    let visual = Invocation::new(vim_commands::VISUAL);
    let select_w = Invocation::new(vim_commands::SELECT_NEXT_WORD);
    let delete_selection = Invocation::new(vim_commands::DELETE_SELECTION);

    let mut group = c.benchmark_group("vim");

    let mut editor = editor_at(&selections);
    group.bench_function("w_b_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &w);
            run(&mut editor, &mut set, &b);
        })
    });
    assert_eq!(
        editor.selections(),
        &selections,
        "w_b_1000_cursors_10mb ends where it began"
    );

    let mut editor = editor_at(&selections);
    group.bench_function("d3w_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &d3w);
            run(&mut editor, &mut set, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &selections,
        "d3w_undo_1000_cursors_10mb ends where it began"
    );

    let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
    let mut state = dispatcher.key_state();
    let mut events = vec![char_key('i')];
    events.extend(TYPED.chars().cycle().take(TYPED_CHARS).map(char_key));
    events.push(KeyEvent::new(Keystroke::new(Key::Escape, Modifiers::NONE)));
    events.push(char_key('u'));
    let mut editor = editor_at(&middle);
    group.bench_function("insert_100_chars_undo_10mb", |bench| {
        bench.iter(|| {
            for event in &events {
                black_box(
                    dispatcher
                        .dispatch(&mut state, black_box(event), &[], &mut editor, &mut set)
                        .expect("the command runs"),
                );
            }
        })
    });
    assert_eq!(
        editor.selections(),
        &middle,
        "insert_100_chars_undo_10mb ends where it began"
    );

    // `999999j` takes every cursor to the last line, where they merge, so
    // each iteration starts from a new editor with the 1,000 cursors, built
    // and dropped outside the timed code. So do the next two.
    group.bench_function("counted_j_k_1000_cursors_10mb", |bench| {
        bench.iter_batched(
            || editor_at(&selections),
            |mut editor| {
                run(&mut editor, &mut set, &down);
                run(&mut editor, &mut set, &up);
                editor
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("gg_G_1000_cursors_10mb", |bench| {
        bench.iter_batched(
            || editor_at(&selections),
            |mut editor| {
                run(&mut editor, &mut set, &first);
                run(&mut editor, &mut set, &last);
                editor
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("counted_dollar_1000_cursors_10mb", |bench| {
        bench.iter_batched(
            || editor_at(&selections),
            |mut editor| {
                run(&mut editor, &mut set, &dollar);
                editor
            },
            BatchSize::SmallInput,
        )
    });

    let mut editor = editor_at(&selections);
    group.bench_function("dd_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &dd);
            run(&mut editor, &mut set, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &selections,
        "dd_undo_1000_cursors_10mb ends where it began"
    );

    // `u` puts back the visual selections from before `d`, and turns each
    // into a cursor at its start: where the cursors began.
    let mut editor = editor_at(&selections);
    group.bench_function("visual_w_d_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &visual);
            run(&mut editor, &mut set, &select_w);
            run(&mut editor, &mut set, &delete_selection);
            run(&mut editor, &mut set, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &selections,
        "visual_w_d_undo_1000_cursors_10mb ends where it began"
    );

    assert_eq!(set.get(id).map(|b| b.to_string()), Some(text));
    group.finish();
}

criterion_group!(benches, vim);
criterion_main!(benches);
