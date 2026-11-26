//! T-108 acceptance tests: property tests of Vim's keymap on random text
//! with every line break and character width. Normal mode keeps every
//! cursor on a char, a count moves like repeating its motion, an insert
//! session undoes in one step, every edit undoes to the text before it,
//! `dd` matches a line oracle, and a visual `d` deletes what was selected.

use helm_editor::{
    CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeyState, Keystroke, Modifiers, Selection,
    Selections, vim,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};
use proptest::prelude::*;
use proptest::sample::Index;

/// One piece of random text: words, blanks, punctuation, each line break
/// and each character width.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-c_]{1,4}",
        2 => "[ \t]{1,3}",
        2 => "[.(){}]{1,2}",
        1 => Just("\n".to_owned()),
        1 => Just("\r".to_owned()),
        1 => Just("\r\n".to_owned()),
        1 => Just("\n\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => any::<char>().prop_map(String::from),
    ]
}

fn text(max_pieces: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..max_pieces).prop_map(|pieces| pieces.concat())
}

fn is_break(c: char) -> bool {
    c == '\n' || c == '\r'
}

fn inside_crlf(text: &str, at: usize) -> bool {
    at > 0 && text[..at].ends_with('\r') && text[at..].starts_with('\n')
}

/// Every char boundary of `text` outside a CRLF, its length included.
fn positions(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .filter(|&at| !inside_crlf(text, at))
        .collect()
}

/// The start and end of the text of the line holding `at`.
fn line_bounds(text: &str, at: usize) -> (usize, usize) {
    let start = text[..at].rfind(is_break).map_or(0, |i| i + 1);
    let end = text[at..].find(is_break).map_or(text.len(), |i| at + i);
    (start, end)
}

/// Whether `at` is where Vim's normal-mode cursor may be: on a char, so
/// never at the end of a line that has text, and never inside a CRLF.
fn on_a_char(text: &str, at: usize) -> bool {
    if at > text.len() || !text.is_char_boundary(at) || inside_crlf(text, at) {
        return false;
    }
    let (start, end) = line_bounds(text, at);
    at < end || start == end
}

/// The key events for keys in Vim notation, as in `vim_normal.rs`.
fn events(keys: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut chars = keys.chars();
    while let Some(c) = chars.next() {
        let key = if c == '<' {
            let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
            match name.as_str() {
                "esc" => Keystroke::new(Key::Escape, Modifiers::NONE),
                "cr" => Keystroke::new(Key::Enter, Modifiers::NONE),
                "bs" => Keystroke::new(Key::Backspace, Modifiers::NONE),
                "del" => Keystroke::new(Key::Delete, Modifiers::NONE),
                other => panic!("unknown key <{other}>"),
            }
        } else {
            Keystroke::new(Key::Char(c), Modifiers::NONE)
        };
        out.push(KeyEvent::new(key));
    }
    out
}

struct Vim {
    dispatcher: Dispatcher,
    state: KeyState,
    set: BufferSet,
    editor: Editor,
}

impl Vim {
    fn new(text: &str, cursors: &[usize]) -> Self {
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(text),
        };
        let mut editor = Editor::new(id, format);
        let list = cursors.iter().map(|&at| Selection::cursor(at)).collect();
        editor.set_selections(Selections::new(list, 0).expect("non-empty"));
        let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
        let state = dispatcher.key_state();
        Self {
            dispatcher,
            state,
            set,
            editor,
        }
    }

    fn keys(&mut self, keys: &str) {
        for event in events(keys) {
            self.dispatcher
                .dispatch(
                    &mut self.state,
                    &event,
                    &[],
                    &mut self.editor,
                    &mut self.set,
                )
                .expect("the command runs");
        }
    }

    fn text(&self) -> String {
        self.set.get(self.editor.buffer()).unwrap().to_string()
    }

    fn heads(&self) -> Vec<usize> {
        self.editor
            .selections()
            .as_slice()
            .iter()
            .map(|s| s.head)
            .collect()
    }
}

const MOTION_KEYS: [&str; 15] = [
    "h", "l", "j", "k", "w", "b", "e", "W", "B", "E", "0", "^", "$", "gg", "G",
];

/// A motion key, sometimes with a count.
fn motion_keys() -> impl Strategy<Value = String> {
    (prop::sample::select(MOTION_KEYS.to_vec()), 0..4usize).prop_map(|(m, n)| {
        if n == 0 || m == "0" {
            m.to_owned()
        } else {
            format!("{n}{m}")
        }
    })
}

/// A position of `text` picked by `index`.
fn pick(text: &str, index: &Index) -> usize {
    let all = positions(text);
    all[index.index(all.len())]
}

/// One normal-mode edit, as keys.
fn edit() -> impl Strategy<Value = String> {
    let simple = prop::sample::select(vec![
        "x", "X", "D", "dd", "2dd", "3x", "2D", "S<esc>", "C<esc>", "s<esc>", "cc<esc>",
    ])
    .prop_map(str::to_owned);
    let operator = (
        prop::sample::select(vec!["d", "c"]),
        motion_keys(),
        "[a-c\u{e9}]{0,3}",
    )
        .prop_map(|(op, motion, typed)| {
            if op == "c" {
                format!("c{motion}{typed}<esc>")
            } else {
                format!("d{motion}")
            }
        });
    let visual = (
        prop::sample::select(vec!["v", "V"]),
        prop::collection::vec(motion_keys(), 0..4),
        prop::sample::select(vec!["d", "x", "cX<esc>", "s<esc>"]),
    )
        .prop_map(|(v, motions, end)| format!("{v}{}{end}", motions.concat()));
    prop_oneof![simple, operator, visual]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn normal_mode_keeps_every_cursor_on_a_char(
        text in text(40),
        starts in prop::collection::vec(any::<Index>(), 1..4),
        moves in prop::collection::vec(motion_keys(), 1..12),
    ) {
        let mut cursors: Vec<usize> = starts.iter().map(|i| pick(&text, i)).collect();
        cursors.sort_unstable();
        cursors.dedup();
        let mut v = Vim::new(&text, &cursors);
        for keys in &moves {
            v.keys(keys);
            prop_assert_eq!(v.state.mode().as_str(), "normal");
            for head in v.heads() {
                prop_assert!(on_a_char(&text, head), "{:?} after {:?}: {}", text, keys, head);
            }
            for s in v.editor.selections().as_slice() {
                prop_assert!(s.is_empty());
            }
        }
        prop_assert_eq!(v.text(), text);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn a_count_moves_like_repeating_the_motion(
        text in text(40),
        start in any::<Index>(),
        motion in prop::sample::select(vec!["h", "l", "j", "k", "w", "b", "e", "W", "B", "E"]),
        n in 1..6usize,
    ) {
        let at = pick(&text, &start);
        let mut counted = Vim::new(&text, &[at]);
        counted.keys(&format!("{n}{motion}"));
        let mut repeated = Vim::new(&text, &[at]);
        repeated.keys(&motion.repeat(n));
        prop_assert_eq!(counted.heads(), repeated.heads(), "{:?} from {}", text, at);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn an_insert_session_undoes_in_one_step(
        text in text(30),
        start in any::<Index>(),
        enter in prop::sample::select(vec!["i", "a", "I", "A", "o", "O"]),
        typed in prop::collection::vec(
            prop::sample::select(vec!["x", "é", "😀", " ", "<cr>", "<bs>", "<del>", "0", "d"]),
            0..20,
        ),
    ) {
        let at = pick(&text, &start);
        let mut v = Vim::new(&text, &[at]);
        v.keys(&format!("{enter}{}<esc>", typed.concat()));
        prop_assert_eq!(v.state.mode().as_str(), "normal");
        v.keys("u");
        prop_assert_eq!(v.text(), text.clone());
        // Nothing older to undo.
        v.keys("u");
        prop_assert_eq!(v.text(), text);
        for head in v.heads() {
            prop_assert!(on_a_char(&v.text(), head));
        }
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn every_edit_undoes_to_the_text_before_it(
        text in text(40),
        starts in prop::collection::vec(any::<Index>(), 1..3),
        edits in prop::collection::vec(edit(), 1..4),
    ) {
        let mut cursors: Vec<usize> = starts.iter().map(|i| pick(&text, i)).collect();
        cursors.sort_unstable();
        cursors.dedup();
        let mut v = Vim::new(&text, &cursors);
        for keys in &edits {
            v.keys(keys);
            prop_assert_eq!(v.state.mode().as_str(), "normal", "{}", keys);
            let now = v.text();
            for head in v.heads() {
                prop_assert!(on_a_char(&now, head), "{:?} after {:?}: {}", now, keys, head);
            }
        }
        // Each edit is at most one undo step, so as many undos as edits
        // get back to the start, and the history has nothing older.
        for _ in 0..edits.len() {
            v.keys("u");
        }
        prop_assert_eq!(v.text(), text.clone());
        v.keys("u");
        prop_assert_eq!(v.text(), text);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn dd_matches_a_line_oracle(
        lines in prop::collection::vec("[a-c \t中😀]{0,5}", 1..8),
        crlf in any::<bool>(),
        line in any::<Index>(),
        n in 1..5usize,
    ) {
        let newline = if crlf { "\r\n" } else { "\n" };
        let text = lines.join(newline);
        let l = line.index(lines.len());
        let start: usize = lines[..l].iter().map(|s| s.len() + newline.len()).sum();
        let mut v = Vim::new(&text, &[start]);
        v.keys(&format!("{n}dd"));
        let mut kept = lines.clone();
        kept.drain(l..(l + n).min(lines.len()));
        prop_assert_eq!(v.text(), kept.join(newline));
        // The cursor is on the first non-blank of the line now at `l`, or
        // of the last line.
        let at_line = l.min(kept.len().saturating_sub(1));
        let line_start: usize = kept[..at_line].iter().map(|s| s.len() + newline.len()).sum();
        let line_text = kept.get(at_line).map_or("", String::as_str);
        let blanks: usize = line_text
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .map(char::len_utf8)
            .sum();
        let mut expected = line_start + blanks;
        if blanks == line_text.len() && !line_text.is_empty() {
            // An all-blank line: its last char.
            expected = line_start + line_text.len() - 1;
        }
        prop_assert_eq!(v.heads(), vec![expected]);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn a_visual_delete_deletes_what_was_selected(
        text in text(40),
        start in any::<Index>(),
        moves in prop::collection::vec(motion_keys(), 0..5),
    ) {
        let at = pick(&text, &start);
        let mut v = Vim::new(&text, &[at]);
        v.keys(&format!("v{}", moves.concat()));
        prop_assert_eq!(v.state.mode().as_str(), "visual");
        let selected = v.editor.selections().primary().range();
        prop_assert!(!selected.is_empty() || text.is_empty() || at == text.len());
        v.keys("d");
        let mut expected = text.clone();
        expected.replace_range(selected, "");
        prop_assert_eq!(v.text(), expected);
    }
}
