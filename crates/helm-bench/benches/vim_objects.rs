//! Vim text object, register, `.` and counted-insert benchmarks for
//! `helm-editor` (T-109), gated by the `bench` CI job against
//! `budgets.toml` with the default budget only. NF-1 gives a keypress 5 ms
//! from input to pixel, and a Vim command at many cursors is on that path.
//! Each benchmark times only the command, from the key event or invocation
//! to the changed buffer, selections and session, not drawing or input
//! delay. None has a target: T-109's task file sets none.
//!
//! `di_paren_no_pair_10mb` measures the worst case behind open question 6
//! of `tasks/109-vim-objects.md`: a bracket object with no enclosing pair
//! scans the text before the cursor to its start.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use helm_editor::{
    CommandId, CommandRegistry, Dispatcher, Editor, Invocation, Key, KeyEvent, KeyState, Keystroke,
    Modifiers, Selection, Selections, Session, vim, vim_commands,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings, TextBuffer};

/// Number of cursors per iteration.
const CURSORS: usize = 1000;

/// Size of the benchmark text: 10 MiB, cut down to a char boundary.
const TEXT_LEN: usize = 10 << 20;

/// Size of the clipboard text: 1 MiB, cut down to a char boundary.
const CLIPBOARD_LEN: usize = 1 << 20;

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

/// The key events for `keys`, each char one key, with `\u{1b}` for
/// Escape.
fn events(keys: &str) -> Vec<KeyEvent> {
    keys.chars()
        .map(|c| {
            let key = if c == '\u{1b}' {
                Key::Escape
            } else {
                Key::Char(c)
            };
            KeyEvent::new(Keystroke::new(key, Modifiers::NONE))
        })
        .collect()
}

/// `operator` with object or motion `motion`.
fn operator(operator: &str, motion: &str) -> Invocation {
    let mut invocation = Invocation::new(operator);
    invocation.args.motion = Some(CommandId::from(motion));
    invocation
}

/// One editor's input state, editor and session, typing through Vim's
/// keymap as `helm-ui` does.
struct Typist {
    state: KeyState,
    editor: Editor,
    session: Session,
}

impl Typist {
    fn keys(&mut self, dispatcher: &Dispatcher, set: &mut BufferSet, events: &[KeyEvent]) {
        for event in events {
            black_box(
                dispatcher
                    .dispatch_in(
                        &mut self.state,
                        black_box(event),
                        &[],
                        &mut self.editor,
                        set,
                        &mut self.session,
                    )
                    .expect("the command runs"),
            );
        }
    }
}

fn vim_objects(c: &mut Criterion) {
    let text = build_text();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(&text),
    };
    let buffer = set.get(id).expect("just added");
    // One cursor on each of 1,000 lines of one kind, spread evenly through
    // the text, as `benches/vim.rs` places them. `kind` is the line's index
    // in `LINES`, and `starts` its first bytes.
    let per_block = LINES.lines().count();
    let repeats = buffer.len_lines() / per_block - 1;
    let lines_of = |kind: usize, starts: &str| -> Selections {
        let cursors = (0..CURSORS)
            .map(|i| {
                let line = i * repeats / CURSORS * per_block + kind;
                let at = buffer.line_to_byte(line).expect("line in range");
                let head = buffer.slice(at..at + starts.len());
                assert_eq!(head.as_deref(), Ok(starts));
                Selection::cursor(at)
            })
            .collect();
        Selections::new(cursors, 0).expect("non-empty, primary in range")
    };
    let fn_lines = lines_of(0, "fn main() {");
    let let_lines = lines_of(1, "    let café");
    let println_lines = lines_of(3, "    println!");
    let middle_comment = {
        let line = repeats / 2 * per_block + 2;
        let at = buffer.line_to_byte(line).expect("line in range");
        assert_eq!(buffer.slice(at..at + 6).as_deref(), Ok("    //"));
        Selections::single(Selection::cursor(at))
    };
    let middle_fn = Selections::single(fn_lines.as_slice()[CURSORS / 2]);
    let clipboard = {
        let mut clipboard = text[..floor_boundary(&text, CLIPBOARD_LEN)].to_owned();
        clipboard.shrink_to_fit();
        clipboard
    };

    let registry = CommandRegistry::with_vim_commands();
    let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
    let editor_at = |selections: &Selections| {
        let mut editor = Editor::new(id, format);
        editor.set_selections(selections.clone());
        editor
    };
    let typist = |selections: &Selections| Typist {
        state: dispatcher.key_state(),
        editor: editor_at(selections),
        session: Session::new(),
    };
    let run = |editor: &mut Editor, set: &mut BufferSet, session: &mut Session, i: &Invocation| {
        black_box(
            registry
                .run_in(black_box(i), editor, set, session)
                .expect("the command runs"),
        );
    };

    let undo = Invocation::new(vim_commands::UNDO);
    let diw = operator(vim_commands::DELETE, vim_commands::INNER_WORD);
    let di_brace = operator(vim_commands::DELETE, vim_commands::INNER_BRACE);
    let di_paren = operator(vim_commands::DELETE, vim_commands::INNER_PAREN);
    let dap = operator(vim_commands::DELETE, vim_commands::A_PARAGRAPH);
    let yy = operator(vim_commands::YANK, vim_commands::LINE);
    let p = Invocation::new(vim_commands::PUT_AFTER);
    let repeat = Invocation::new(vim_commands::REPEAT);

    let mut group = c.benchmark_group("vim_objects");

    let mut editor = editor_at(&fn_lines);
    let mut session = Session::new();
    group.bench_function("diw_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &mut session, &diw);
            run(&mut editor, &mut set, &mut session, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &fn_lines,
        "diw_undo ends where it began"
    );

    // `u` after an object edit puts each cursor at its range's start, so
    // one untimed iteration first moves the cursors there, inside the
    // quotes, and every timed iteration starts and ends there.
    let mut v = typist(&let_lines);
    let ci_quote = events("ci\"x\u{1b}u");
    v.keys(&dispatcher, &mut set, &ci_quote);
    let settled = v.editor.selections().clone();
    assert_ne!(settled, let_lines, "the cursors moved into the quotes");
    group.bench_function("ci_quote_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| v.keys(&dispatcher, &mut set, &ci_quote))
    });
    assert_eq!(
        v.editor.selections(),
        &settled,
        "ci_quote_undo ends where it began"
    );

    // The same holds for `di{`: `u` puts each cursor at the start of the
    // block's first inner line.
    let mut editor = editor_at(&println_lines);
    run(&mut editor, &mut set, &mut session, &di_brace);
    assert_eq!(
        set.get(id).expect("in the set").len_lines(),
        buffer_lines(&text) - 4 * CURSORS,
        "di{{ empties a block of four lines at every cursor"
    );
    run(&mut editor, &mut set, &mut session, &undo);
    let settled = editor.selections().clone();
    group.bench_function("di_brace_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &mut session, &di_brace);
            run(&mut editor, &mut set, &mut session, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &settled,
        "di_brace_undo ends where it began"
    );

    let mut editor = editor_at(&middle_comment);
    let version = set.version(id);
    group.bench_function("di_paren_no_pair_10mb", |bench| {
        bench.iter(|| run(&mut editor, &mut set, &mut session, &di_paren))
    });
    assert_eq!(
        set.version(id),
        version,
        "di( finds no pair and edits nothing"
    );
    assert_eq!(editor.selections(), &middle_comment);

    let mut editor = editor_at(&fn_lines);
    group.bench_function("dap_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &mut session, &dap);
            run(&mut editor, &mut set, &mut session, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &fn_lines,
        "dap_undo ends where it began"
    );

    let mut editor = editor_at(&fn_lines);
    group.bench_function("yy_p_undo_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut editor, &mut set, &mut session, &yy);
            run(&mut editor, &mut set, &mut session, &p);
            run(&mut editor, &mut set, &mut session, &undo);
        })
    });
    assert_eq!(
        editor.selections(),
        &fn_lines,
        "yy_p_undo ends where it began"
    );
    assert_eq!(
        session.register('"').map(|r| r.pieces.len()),
        Some(CURSORS),
        "each cursor yanks its own piece"
    );

    // `ciwhelm<Esc>` is recorded once, outside the timed code. Each `.`
    // then changes `helm` to `helm`, and `u` puts the cursors back at the
    // word's start.
    let mut v = typist(&fn_lines);
    v.keys(&dispatcher, &mut set, &events("ciwhelm\u{1b}"));
    run(&mut v.editor, &mut set, &mut v.session, &repeat);
    run(&mut v.editor, &mut set, &mut v.session, &undo);
    let settled = v.editor.selections().clone();
    group.bench_function("repeat_ciw_1000_cursors_10mb", |bench| {
        bench.iter(|| {
            run(&mut v.editor, &mut set, &mut v.session, &repeat);
            run(&mut v.editor, &mut set, &mut v.session, &undo);
        })
    });
    assert_eq!(
        v.editor.selections(),
        &settled,
        "repeat_ciw ends where it began"
    );
    // Undo the recorded change, so the text is the 10 MiB it began as.
    run(&mut v.editor, &mut set, &mut v.session, &undo);

    let mut v = typist(&middle_fn);
    let insert = events("100ix\u{1b}u");
    group.bench_function("insert_count_100_10mb", |bench| {
        bench.iter(|| v.keys(&dispatcher, &mut set, &insert))
    });
    assert_eq!(
        v.editor.selections(),
        &middle_fn,
        "insert_count_100 ends where it began"
    );

    let mut v = typist(&middle_fn);
    let paste = events("\"+Pu");
    group.bench_function("clipboard_paste_1mb", |bench| {
        bench.iter_batched(
            || clipboard.clone(),
            |clipboard| {
                v.session.set_clipboard(Some(black_box(clipboard)));
                v.keys(&dispatcher, &mut set, &paste);
            },
            BatchSize::LargeInput,
        )
    });
    assert_eq!(
        v.editor.selections(),
        &middle_fn,
        "clipboard_paste ends where it began"
    );

    assert_eq!(set.get(id).map(|b| b.to_string()), Some(text));
    group.finish();
}

/// The number of lines in `text`, as the buffer counts them.
fn buffer_lines(text: &str) -> usize {
    Buffer::from(text).len_lines()
}

criterion_group!(benches, vim_objects);
criterion_main!(benches);
