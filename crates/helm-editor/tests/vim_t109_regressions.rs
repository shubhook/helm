//! Regression tests for the findings of PR #42's first review round
//! (T-109): oversized put counts, the quote scan on long lines, two editors
//! on one buffer sharing a session, counted `aw` from white space, and an
//! empty line inside a block.
//!
//! Markers and key notation are as in `vim_harness`.

mod vim_harness;

use helm_editor::{
    Dispatcher, Editor, KeyState, Selection, Selections, Session, vim, vim_commands,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};
use vim_harness::{Vim, events, keymap, registry, table};

#[test]
fn a_put_too_large_to_build_puts_nothing() {
    // The count saturates at `usize::MAX`, and even a count that fits
    // would make more text than a put may insert.
    for count in ["99999999999999999999", "2000000000"] {
        for (start, keys, expected) in [
            ("|ab", format!("yl{count}p"), "|ab"),
            ("|ab", format!("yl{count}P"), "|ab"),
            ("|ab\ncd", format!("yy{count}p"), "|ab\ncd"),
            ("|ab cd", format!("yiwwv{count}p"), "ab |cd"),
            ("|ab cd", format!("yiwwv{count}P"), "ab |cd"),
            ("|ab\ncd", format!("yyjV{count}p"), "ab\n|cd"),
        ] {
            let mut v = Vim::new(start);
            v.keys(&keys);
            assert_eq!(v.show(), expected, "{start:?} then {keys:?}");
            assert_eq!(v.mode(), vim::NORMAL, "{start:?} then {keys:?}");
            assert!(v.editor.undo(&mut v.set).unwrap().is_none(), "{keys:?}");
        }
    }
    // The registers stay as the yank left them.
    let mut v = Vim::new("|ab cd");
    v.keys("yiwwv99999999999999999999p");
    assert_eq!(v.reg1('"'), Some(("ab".to_owned(), false)));
}

#[test]
fn normal_put_counts_still_put() {
    table(&[
        ("|ab", "yl3p", "aaa|ab"),
        ("|ab", "yy2p", "ab\n|ab\nab"),
        ("|ab cd", "yiwwv2p", "ab aba|bd"),
    ]);
}

#[test]
fn quote_objects_on_a_long_line_of_quotes() {
    // Twenty thousand pairs on one line, the cursor inside the last.
    let pairs = "\"ab\" ".repeat(20_000);
    let start = format!("{pairs}\"x|y\"");
    let mut v = Vim::new(&start);
    v.keys("di\"");
    assert_eq!(v.show(), format!("{pairs}\"|\""));
    // Between two pairs, the nearest quote before and after are used.
    let start = format!("{pairs}|z \"q\"");
    let mut v = Vim::new(&start);
    v.keys("di\"");
    assert_eq!(v.show(), format!("{}|\"q\"", &pairs[..pairs.len() - 1]));
}

/// Two editors showing one buffer, typing through one dispatcher and one
/// session, as two views of one file in `helm-ui`.
struct TwoViews {
    dispatcher: Dispatcher,
    set: BufferSet,
    session: Session,
    views: [(KeyState, Editor); 2],
}

impl TwoViews {
    fn new(text: &str, cursors: [usize; 2]) -> Self {
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(text),
        };
        let dispatcher = Dispatcher::new(registry(), keymap());
        let view = |at: usize| {
            let mut editor = Editor::new(id, format);
            editor.set_selections(Selections::single(Selection::cursor(at)));
            (dispatcher.key_state(), editor)
        };
        let views = [view(cursors[0]), view(cursors[1])];
        Self {
            dispatcher,
            set,
            session: Session::new(),
            views,
        }
    }

    fn keys(&mut self, view: usize, keys: &str) {
        let (state, editor) = &mut self.views[view];
        for event in events(keys) {
            self.dispatcher
                .dispatch_in(state, &event, &[], editor, &mut self.set, &mut self.session)
                .expect("the command runs");
        }
    }

    fn text(&self) -> String {
        let id = self.views[0].1.buffer();
        self.set.get(id).expect("in the set").to_string()
    }
}

#[test]
fn another_view_of_the_buffer_leaves_the_record_alone() {
    let mut v = TwoViews::new("ab\ncd", [0, 3]);
    v.keys(0, "ix");
    v.keys(1, "l");
    v.keys(0, "y<esc>");
    let ids: Vec<&str> = v
        .session
        .last_change()
        .expect("a change")
        .iter()
        .map(|i| i.command.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            vim_commands::INSERT,
            "editor.insert",
            "editor.insert",
            vim_commands::NORMAL
        ]
    );
    // `.` in the second view repeats all of it. That view's cursor doesn't
    // follow the first view's edit, so it moves to the second line first.
    v.keys(1, "j0.");
    assert_eq!(v.text(), "xyab\nxycd");
}

#[test]
fn another_view_of_the_buffer_keeps_a_counted_insert() {
    let mut v = TwoViews::new("ab\ncd", [0, 3]);
    v.keys(0, "3ix");
    v.keys(1, "l");
    v.keys(0, "y<esc>");
    assert_eq!(v.text(), "xyxyxyab\ncd");
    // A change in the other view while the first is still typing records
    // on its own, and the first view's record still ends with its Escape.
    let mut v = TwoViews::new("ab\ncd", [0, 3]);
    v.keys(0, "iQ");
    v.keys(1, "x");
    assert_eq!(
        v.session.last_change().expect("a change")[0]
            .command
            .as_str(),
        vim_commands::DELETE_CHAR
    );
    v.keys(0, "<esc>");
    assert_eq!(
        v.session.last_change().expect("a change")[0]
            .command
            .as_str(),
        vim_commands::INSERT
    );
}

#[test]
fn counted_aw_from_white_space_takes_words_without_white_space_between() {
    table(&[
        ("| foo.bar", "d2aw", "|bar"),
        ("| foo.bar baz", "d3aw", "| baz"),
        ("| foo bar", "d2aw", "|"),
        ("| foo.bar baz", "d2aW", "|"),
        ("| foo.bar baz", "daW", "| baz"),
    ]);
}

#[test]
fn an_empty_line_inside_a_block_is_its_inner_object() {
    table(&[
        ("|{\n\n}", "di{", "{\n|}"),
        ("x{\n|\n}y", "di{", "x{\n|}y"),
        ("|{\n}", "di{", "|{\n}"),
        ("|(\n  \n)", "di(", "(\n|)"),
    ]);
}
