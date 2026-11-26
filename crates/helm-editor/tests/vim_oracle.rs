//! T-108 acceptance tests: Vim's motions, `d` ranges and visual ends
//! checked against a plain `String` oracle written from the rules in the
//! `vim` module docs. The texts have lines longer than 512 bytes and run
//! across several of the rope's chunks, so motions that read the buffer a
//! chunk or a window at a time get checked where those pieces meet.

use helm_editor::{
    CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeyState, Keystroke, Modifiers, Selection,
    Selections, vim,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};
use proptest::prelude::*;
use proptest::sample::Index;

// ---- The oracle -------------------------------------------------------

/// A text with its lines: each line's start and the end of its text.
struct Text<'a> {
    s: &'a str,
    lines: Vec<(usize, usize)>,
}

impl<'a> Text<'a> {
    fn new(s: &'a str) -> Self {
        let b = s.as_bytes();
        let mut lines = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\r' if b.get(i + 1) == Some(&b'\n') => {
                    lines.push((start, i));
                    i += 2;
                    start = i;
                }
                b'\n' | b'\r' => {
                    lines.push((start, i));
                    i += 1;
                    start = i;
                }
                _ => i += 1,
            }
        }
        lines.push((start, b.len()));
        Self { s, lines }
    }

    fn len(&self) -> usize {
        self.s.len()
    }

    fn last(&self) -> usize {
        self.lines.len() - 1
    }

    fn line(&self, p: usize) -> usize {
        let p = if self.inside_crlf(p) { p - 1 } else { p };
        self.lines.partition_point(|&(start, _)| start <= p) - 1
    }

    fn start(&self, l: usize) -> usize {
        self.lines[l].0
    }

    fn end(&self, l: usize) -> usize {
        self.lines[l].1
    }

    fn empty(&self, l: usize) -> bool {
        self.start(l) == self.end(l)
    }

    fn inside_crlf(&self, p: usize) -> bool {
        p > 0 && self.s[..p].ends_with('\r') && self.s[p..].starts_with('\n')
    }

    fn char_at(&self, p: usize) -> Option<char> {
        self.s[p..].chars().next()
    }

    fn next(&self, p: usize) -> usize {
        if self.s[p..].starts_with("\r\n") {
            p + 2
        } else {
            self.char_at(p).map_or(p, |c| p + c.len_utf8())
        }
    }

    fn prev(&self, p: usize) -> usize {
        if self.s[..p].ends_with("\r\n") {
            p - 2
        } else {
            self.s[..p]
                .chars()
                .next_back()
                .map_or(p, |c| p - c.len_utf8())
        }
    }

    fn is_line_start(&self, p: usize) -> bool {
        p == self.start(self.line(p))
    }

    fn first_non_blank(&self, l: usize) -> usize {
        let (start, end) = self.lines[l];
        start
            + self.s[start..end]
                .chars()
                .take_while(|&c| blank(c))
                .map(char::len_utf8)
                .sum::<usize>()
    }

    fn column(&self, p: usize) -> usize {
        self.s[self.start(self.line(p))..p].chars().count()
    }

    fn at_column(&self, l: usize, column: usize) -> usize {
        let (start, end) = self.lines[l];
        start
            + self.s[start..end]
                .chars()
                .take(column)
                .map(char::len_utf8)
                .sum::<usize>()
    }

    /// A normal-mode position.
    fn settle(&self, p: usize) -> usize {
        let p = if self.inside_crlf(p) { p - 1 } else { p };
        let l = self.line(p);
        if p >= self.end(l) && !self.empty(l) {
            self.prev(self.end(l))
        } else {
            p
        }
    }

    /// A visual-mode position: never the empty last line.
    fn vsettle(&self, p: usize) -> usize {
        let p = self.settle(p);
        if p == self.len() && self.len() > 0 && self.empty(self.last()) {
            self.settle(self.prev(p))
        } else {
            p
        }
    }

    /// 0 for white space and the end of the text, 1 for punctuation (or
    /// any non-blank, for WORDs), 2 for word chars.
    fn class(&self, p: usize, big: bool) -> u8 {
        match self.char_at(p) {
            None => 0,
            Some(c) if c == '\n' || c == '\r' || blank(c) => 0,
            Some(_) if big => 1,
            Some(c) if c.is_alphanumeric() || c == '_' => 2,
            Some(_) => 1,
        }
    }

    fn empty_line_start(&self, p: usize) -> bool {
        self.is_line_start(p) && self.empty(self.line(p))
    }

    fn w(&self, p: usize, big: bool) -> usize {
        if p >= self.len() {
            return self.len();
        }
        let c = self.class(p, big);
        let mut q = p;
        if c != 0 {
            while q < self.len() && self.class(q, big) == c {
                q = self.next(q);
            }
        }
        while q < self.len() {
            if q != p && self.empty_line_start(q) || self.class(q, big) != 0 {
                return q;
            }
            q = self.next(q);
        }
        self.len()
    }

    fn b(&self, p: usize, big: bool) -> usize {
        if p == 0 {
            return 0;
        }
        let mut q = self.prev(p);
        while self.class(q, big) == 0 {
            if self.empty_line_start(q) || q == 0 {
                return q;
            }
            q = self.prev(q);
        }
        let c = self.class(q, big);
        while q > 0 && self.class(self.prev(q), big) == c {
            q = self.prev(q);
        }
        q
    }

    fn run_end(&self, mut q: usize, big: bool) -> usize {
        let c = self.class(q, big);
        while self.next(q) < self.len() && self.class(self.next(q), big) == c {
            q = self.next(q);
        }
        q
    }

    fn e(&self, p: usize, big: bool) -> usize {
        if p >= self.len() {
            return p;
        }
        let c = self.class(p, big);
        let mut q = self.next(p);
        if q < self.len() && c != 0 && self.class(q, big) == c {
            return self.run_end(q, big);
        }
        while q < self.len() && self.class(q, big) == 0 {
            q = self.next(q);
        }
        if q >= self.len() {
            p
        } else {
            self.run_end(q, big)
        }
    }

    fn repeat(&self, p: usize, n: usize, step: impl Fn(usize) -> usize) -> usize {
        let mut p = p;
        for _ in 0..n {
            let q = step(p);
            if q == p {
                break;
            }
            p = q;
        }
        p
    }

    /// Where `key` takes `p`, unsettled, or `None` for `j` and `k` that
    /// can't move.
    fn target(&self, p: usize, key: &str, count: Option<usize>) -> Option<usize> {
        let n = count.unwrap_or(1);
        let l = self.line(p);
        let last = self.last();
        Some(match key {
            "h" => self.at_column(l, self.column(p).saturating_sub(n)),
            "l" => self.at_column(l, self.column(p) + n),
            "j" if l < last => self.at_column((l + n).min(last), self.column(p)),
            "k" if l > 0 => self.at_column(l.saturating_sub(n), self.column(p)),
            "j" | "k" => return None,
            "w" | "W" => self.repeat(p, n, |q| self.w(q, key == "W")),
            "b" | "B" => self.repeat(p, n, |q| self.b(q, key == "B")),
            "e" | "E" => self.repeat(p, n, |q| self.e(q, key == "E")),
            "0" => self.start(l),
            "^" => self.first_non_blank(l),
            "$" => self.end((l + n - 1).min(last)),
            "gg" => self.first_non_blank(count.map_or(0, |c| c - 1).min(last)),
            "G" => self.first_non_blank(count.map_or(last, |c| c - 1).min(last)),
            "d" => self.first_non_blank((l + n - 1).min(last)),
            other => panic!("no motion {other}"),
        })
    }

    /// What `d{key}` deletes from `p`: a byte range and whether it was
    /// linewise. `None` when nothing is deleted. `Err` for `de` and `dE`
    /// where `e` can't move: the spec leaves that to Khakha (finding 2 of
    /// the council round), so the property doesn't check it.
    fn delete(
        &self,
        p: usize,
        key: &str,
        count: Option<usize>,
    ) -> Result<Option<(usize, usize, bool)>, ()> {
        let n = count.unwrap_or(1);
        let lines = |a: usize, b: usize| {
            let (a, b) = (a.min(b), a.max(b));
            let range = if b < self.last() {
                (self.start(a), self.start(b + 1))
            } else if a > 0 {
                (self.end(a - 1), self.len())
            } else {
                (0, self.len())
            };
            Some((range.0, range.1, true))
        };
        let (a, b) = match key {
            "j" | "k" | "gg" | "G" | "d" => {
                return Ok(self
                    .target(p, key, count)
                    .and_then(|t| lines(self.line(p), self.line(t))));
            }
            "e" | "E" => {
                let t = self.target(p, key, count).unwrap();
                if t == p {
                    return Err(());
                }
                return Ok(Some((p, self.next(t), false)));
            }
            "$" => {
                let t = self.target(p, key, count).unwrap();
                let (lp, lt) = (self.line(p), self.line(t));
                if lt > lp && self.empty(lt) && p <= self.first_non_blank(lp) {
                    return Ok(lines(lp, lt));
                }
                return Ok((p < t).then_some((p, t, false)));
            }
            "w" | "W" => {
                let big = key == "W";
                let s = self.repeat(p, n - 1, |q| self.w(q, big));
                let l = self.line(s);
                let t = if s >= self.len() {
                    self.len()
                } else if s == self.end(l) {
                    if l < self.last() {
                        self.start(l + 1)
                    } else {
                        self.len()
                    }
                } else {
                    let t = self.w(s, big);
                    if self.line(t) > l { self.end(l) } else { t }
                };
                (p, t)
            }
            _ => {
                let t = self.target(p, key, count).unwrap();
                (p.min(t), p.max(t))
            }
        };
        if a >= b {
            return Ok(None);
        }
        if self.is_line_start(b) && self.line(b) > self.line(a) {
            let la = self.line(a);
            if a <= self.first_non_blank(la) {
                return Ok(lines(la, self.line(b) - 1));
            }
            let b = self.end(self.line(b) - 1);
            return Ok((a < b).then_some((a, b, false)));
        }
        Ok(Some((a, b, false)))
    }
}

fn blank(c: char) -> bool {
    c.is_whitespace() && c != '\n' && c != '\r'
}

// ---- Driving the editor -----------------------------------------------

fn events(keys: &str) -> Vec<KeyEvent> {
    keys.chars()
        .map(|c| {
            let key = if c == '\u{1b}' {
                Keystroke::new(Key::Escape, Modifiers::NONE)
            } else {
                Keystroke::new(Key::Char(c), Modifiers::NONE)
            };
            KeyEvent::new(key)
        })
        .collect()
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

// ---- Strategies -------------------------------------------------------

/// Pieces of text, some long enough that a line passes 512 bytes and the
/// text spans several rope chunks.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-c_]{1,4}",
        2 => "[ \t]{1,3}",
        2 => "[.(){}]{1,2}",
        1 => "[a-c_ .]{500,900}",
        1 => "[ab ]{1,3}".prop_map(|s| s.repeat(200)),
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

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..30).prop_map(|p| p.concat())
}

const KEYS: [&str; 15] = [
    "h", "l", "j", "k", "w", "b", "e", "W", "B", "E", "0", "^", "$", "gg", "G",
];

/// A motion key and a count: `None` or 1 to 4. `0` takes no count.
fn motion() -> impl Strategy<Value = (&'static str, Option<usize>)> {
    (prop::sample::select(KEYS.to_vec()), 0..5usize)
        .prop_map(|(k, n)| (k, if n == 0 || k == "0" { None } else { Some(n) }))
}

fn keys_for(key: &str, count: Option<usize>) -> String {
    count.map_or(key.to_owned(), |n| format!("{n}{key}"))
}

fn pick(text: &str, index: &Index) -> usize {
    let t = Text::new(text);
    let all: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .filter(|&at| !t.inside_crlf(at))
        .collect();
    all[index.index(all.len())]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn motions_match_the_oracle_on_long_lines(
        text in text(),
        start in any::<Index>(),
        (key, count) in motion(),
    ) {
        let t = Text::new(&text);
        let at = pick(&text, &start);
        let from = t.settle(at);
        let expected = t.target(from, key, count).map_or(from, |to| t.settle(to));
        let mut v = Vim::new(&text, &[at]);
        v.keys(&keys_for(key, count));
        prop_assert_eq!(v.heads(), vec![expected], "{:?}{} from {}", count, key, at);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn d_deletes_the_range_the_oracle_gives(
        text in text(),
        start in any::<Index>(),
        (key, count) in prop_oneof![motion(), (Just("d"), prop::option::of(1..4usize))],
    ) {
        let t = Text::new(&text);
        let at = pick(&text, &start);
        let from = t.settle(at);
        let Ok(range) = t.delete(from, key, count) else {
            return Ok(());
        };
        let mut v = Vim::new(&text, &[at]);
        v.keys(&format!("d{}", keys_for(key, count)));
        let (expected, cursor) = match range {
            None => (text.clone(), from),
            Some((a, b, linewise)) => {
                let mut rest = text.clone();
                rest.replace_range(a..b, "");
                let r = Text::new(&rest);
                let cursor = if linewise {
                    r.settle(r.first_non_blank(r.line(a)))
                } else {
                    r.settle(a)
                };
                (rest, cursor)
            }
        };
        prop_assert_eq!(v.text(), expected, "d{:?}{} from {}", count, key, from);
        prop_assert_eq!(v.heads(), vec![cursor]);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn visual_ends_match_the_oracle(
        text in text(),
        start in any::<Index>(),
        (key, count) in motion(),
    ) {
        let t = Text::new(&text);
        let at = pick(&text, &start);
        let from = t.settle(at);
        let to = t.target(from, key, count).map_or(from, |to| t.vsettle(to));
        let empty = from == text.len();
        // `v` on the empty last line selects nothing, and the first move
        // re-anchors on the line before.
        let anchor = if empty && to != from { t.vsettle(from) } else { from };

        let mut v = Vim::new(&text, &[at]);
        v.keys(&format!("v{}", keys_for(key, count)));
        let selection = v.editor.selections().primary();
        let expected = if empty && to == from {
            from..from
        } else {
            anchor.min(to)..t.next(anchor.max(to))
        };
        prop_assert_eq!(selection.range(), expected.clone());
        prop_assert_eq!(selection.is_reversed(), to < anchor);
        v.keys("\u{1b}");
        prop_assert_eq!(v.heads(), vec![t.settle(to)]);

        let mut v = Vim::new(&text, &[at]);
        v.keys(&format!("v{}o\u{1b}", keys_for(key, count)));
        prop_assert_eq!(v.heads(), vec![t.settle(anchor)]);
    }

    #[test]
    #[ignore = "T-108: not implemented yet"]
    fn dd_at_cursors_on_touching_lines_is_one_counted_dd(
        lines in prop::collection::vec("[a-c \t中]{0,4}", 1..8),
        crlf in any::<bool>(),
        first in any::<Index>(),
        n in 1..4usize,
    ) {
        let newline = if crlf { "\r\n" } else { "\n" };
        let text = lines.join(newline);
        let t = Text::new(&text);
        let l = first.index(lines.len());
        let n = n.min(lines.len() - l);
        let cursors: Vec<usize> = (l..l + n).map(|i| t.start(i)).collect();
        let mut many = Vim::new(&text, &cursors);
        many.keys("dd");
        let mut one = Vim::new(&text, &[t.start(l)]);
        one.keys(&format!("{n}dd"));
        prop_assert_eq!(many.text(), one.text());
        prop_assert_eq!(many.heads(), one.heads());
    }
}

/// About 5 KB of text: lines of 3,000, 1,200 and 1,000 bytes, short, empty
/// lines, CRLF and wide chars, so it spans several rope chunks.
fn long_text() -> String {
    let mut s = String::new();
    s.push_str(&"ab_c d.e  ".repeat(300));
    s.push('\n');
    s.push_str("  short line\r\n\r\n\n");
    s.push_str(&"é中😀 x(y) ".repeat(80));
    s.push_str("\r\n");
    s.push_str(&"word ".repeat(200));
    s.push_str("\n\n  end");
    s
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn every_motion_matches_the_oracle_across_chunks() {
    let text = long_text();
    assert!(text.len() > 5000);
    let t = Text::new(&text);
    let positions: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .filter(|&at| !t.inside_crlf(at))
        .step_by(7)
        .collect();
    for &at in &positions {
        let from = t.settle(at);
        for key in KEYS {
            for count in [None, Some(3)] {
                if key == "0" && count.is_some() {
                    continue;
                }
                let expected = t.target(from, key, count).map_or(from, |to| t.settle(to));
                let mut v = Vim::new(&text, &[at]);
                v.keys(&keys_for(key, count));
                assert_eq!(v.heads(), vec![expected], "{count:?}{key} from {at}");
                if let Ok(range) = t.delete(from, key, count) {
                    let mut v = Vim::new(&text, &[at]);
                    v.keys(&format!("d{}", keys_for(key, count)));
                    let mut rest = text.clone();
                    if let Some((a, b, _)) = range {
                        rest.replace_range(a..b, "");
                    }
                    assert_eq!(v.text(), rest, "d{count:?}{key} from {at}");
                }
            }
        }
    }
}
