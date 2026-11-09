//! Motions on texts of several KB, which the rope stores in many chunks,
//! against a reference written here from the `Motion` docs. Added to PR #34
//! (T-107) with the `TextBuffer::chunk_at` scope grant: the editor now reads
//! through `chunk_at`, and these tests check it still does what it did when
//! it read through `slice`.
//!
//! The reference doesn't share code with the editor. It works on the whole
//! text as one `String` (`to_string`, allowed in a test), finds lines and
//! chars with `str` methods, and never calls `chunk_at` or `slice`. The
//! tests call `chunk_at` only to find where chunks start, to check that the
//! moves they compare cross those places.

use std::ops::Range;

use helm_editor::{Editor, Motion, Selection, Selections};
use helm_text::{Buffer, BufferId, BufferSet, FileFormat, LineEndings, TextBuffer};

const MOTIONS: [Motion; 10] = [
    Motion::Left,
    Motion::Right,
    Motion::Up,
    Motion::Down,
    Motion::WordLeft,
    Motion::WordRight,
    Motion::LineStart,
    Motion::LineEnd,
    Motion::TextStart,
    Motion::TextEnd,
];

// The reference.

fn is_break(c: char) -> bool {
    c == '\n' || c == '\r'
}

fn is_blank(c: char) -> bool {
    c.is_whitespace() && !is_break(c)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 0 word, 1 blank, 2 punctuation.
fn class(c: char) -> u8 {
    if is_word(c) {
        0
    } else if is_blank(c) {
        1
    } else {
        2
    }
}

/// Lines of one text: where each starts and where its text ends, before its
/// line break. LF, CR and CRLF each end a line; CRLF counts once.
struct Lines {
    starts: Vec<usize>,
    ends: Vec<usize>,
}

impl Lines {
    fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let (mut starts, mut ends) = (vec![0], Vec::new());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    ends.push(i);
                    starts.push(i + 2);
                    i += 2;
                }
                b'\r' | b'\n' => {
                    ends.push(i);
                    starts.push(i + 1);
                    i += 1;
                }
                _ => i += 1,
            }
        }
        ends.push(text.len());
        Self { starts, ends }
    }

    /// The line `at` is on. Between the CR and LF of a CRLF is on the CR's.
    fn of(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at) - 1
    }
}

/// A text and what the reference needs to know about it.
struct Reference {
    text: String,
    lines: Lines,
}

impl Reference {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            lines: Lines::new(text),
        }
    }

    fn before(&self, at: usize) -> Option<char> {
        self.text[..at].chars().next_back()
    }

    fn after(&self, at: usize) -> Option<char> {
        self.text[at..].chars().next()
    }

    fn left(&self, at: usize) -> usize {
        if self.text[..at].ends_with("\r\n") {
            at - 2
        } else {
            at - self.before(at).map_or(0, char::len_utf8)
        }
    }

    fn right(&self, at: usize) -> usize {
        if self.text[at..].starts_with("\r\n") {
            at + 2
        } else {
            at + self.after(at).map_or(0, char::len_utf8)
        }
    }

    /// Chars from the line's start to `at`, capped at the line's text.
    fn column(&self, at: usize) -> usize {
        let line = self.lines.of(at);
        let end = at.min(self.lines.ends[line]);
        self.text[self.lines.starts[line]..end].chars().count()
    }

    fn vertical(&self, at: usize, down: bool, goal: usize) -> usize {
        let line = self.lines.of(at);
        let target = if down {
            if line + 1 == self.lines.starts.len() {
                return self.text.len();
            }
            line + 1
        } else {
            if line == 0 {
                return 0;
            }
            line - 1
        };
        let (start, end) = (self.lines.starts[target], self.lines.ends[target]);
        let body = &self.text[start..end];
        start + body.char_indices().nth(goal).map_or(body.len(), |(i, _)| i)
    }

    fn word_right(&self, mut at: usize) -> usize {
        match self.after(at) {
            None => return at,
            Some(c) if is_break(c) => return self.right(at),
            Some(_) => {}
        }
        while let Some(c) = self.after(at).filter(|&c| is_blank(c)) {
            at += c.len_utf8();
        }
        let Some(run) = self.after(at).filter(|&c| !is_break(c)).map(class) else {
            return at;
        };
        while let Some(c) = self.after(at).filter(|&c| !is_break(c) && class(c) == run) {
            at += c.len_utf8();
        }
        at
    }

    fn word_left(&self, mut at: usize) -> usize {
        match self.before(at) {
            None => return at,
            Some(c) if is_break(c) => return self.left(at),
            Some(_) => {}
        }
        while let Some(c) = self.before(at).filter(|&c| is_blank(c)) {
            at -= c.len_utf8();
        }
        let Some(run) = self.before(at).filter(|&c| !is_break(c)).map(class) else {
            return at;
        };
        while let Some(c) = self.before(at).filter(|&c| !is_break(c) && class(c) == run) {
            at -= c.len_utf8();
        }
        at
    }

    fn line_start(&self, at: usize) -> usize {
        let start = self.lines.starts[self.lines.of(at)];
        let blanks: usize = self.text[start..]
            .chars()
            .take_while(|&c| is_blank(c))
            .map(char::len_utf8)
            .sum();
        let first = start + blanks;
        if at == first { start } else { first }
    }

    /// Where `motion` takes a head at `at`, and the goal Up and Down use.
    fn target(&self, at: usize, motion: Motion, goal: Option<usize>) -> (usize, usize) {
        let goal = goal.unwrap_or_else(|| self.column(at));
        let to = match motion {
            Motion::Left => self.left(at),
            Motion::Right => self.right(at),
            Motion::Up => self.vertical(at, false, goal),
            Motion::Down => self.vertical(at, true, goal),
            Motion::WordLeft => self.word_left(at),
            Motion::WordRight => self.word_right(at),
            Motion::LineStart => self.line_start(at),
            Motion::LineEnd => self.lines.ends[self.lines.of(at)],
            Motion::TextStart => 0,
            Motion::TextEnd => self.text.len(),
        };
        (to, goal)
    }

    fn inside_crlf(&self, at: usize) -> bool {
        self.text[..at].ends_with('\r') && self.text[at..].starts_with('\n')
    }

    /// `move_cursors` on one selection.
    fn moved(&self, selection: Selection, motion: Motion) -> usize {
        match motion {
            Motion::Left if !selection.is_empty() => {
                let start = selection.start();
                if self.inside_crlf(start) {
                    start - 1
                } else {
                    start
                }
            }
            Motion::Right if !selection.is_empty() => {
                let end = selection.end();
                if self.inside_crlf(end) { end + 1 } else { end }
            }
            _ => self.target(selection.head, motion, None).0,
        }
    }

    /// `offset` clamped the way the editor documents: to the end, or back
    /// to the start of the char it falls inside.
    fn clamp(&self, offset: usize) -> usize {
        let mut at = offset.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }
}

// The texts, and where their chunks start.

/// Code with every line break, every char width, indented and blank lines,
/// and long lines: one longer than the 512 bytes Up and Down scan.
fn mixed() -> String {
    let mut text = String::new();
    for i in 0..40 {
        text.push_str(&format!("    let café_{i} = \"crème 中文 😀\"; // é\r\n"));
        text.push_str("\tif x.y(z) { return [1, 2]; }\n");
        text.push_str("\n  \t \nshort\rcr\r");
        if i % 9 == 0 {
            text.push_str(&"word_run".repeat(90));
            text.push_str(" (tail)\n");
        }
    }
    text
}

/// CRLF everywhere, with lines of very different lengths.
fn crlf() -> String {
    (0..120)
        .map(|i| {
            let width = (i * 37) % 160;
            format!("{}{}\r\n", " ".repeat(i % 7), "ab_é.".repeat(width / 5))
        })
        .collect()
}

/// Lone CRs only, with 中 and 😀 runs.
fn lone_cr() -> String {
    (0..200)
        .map(|i| format!("中文{}😀😀 x{i}\r", "字".repeat(i % 11)))
        .collect()
}

/// One long run of word chars across many chunks, between punctuation.
fn long_run() -> String {
    let run: String = (0..9000)
        .map(|i| (b'a' + (i * 7 % 26) as u8) as char)
        .collect();
    format!("x = \"{run}\";\nend\n")
}

/// The texts, each several KB.
fn texts() -> Vec<(&'static str, String)> {
    vec![
        ("mixed", mixed()),
        ("crlf", crlf()),
        ("lone cr", lone_cr()),
        ("long run", long_run()),
    ]
}

/// An editor's set on `text`, and the buffer's id.
fn setup(text: &str) -> (BufferSet, BufferId, FileFormat) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    (set, id, format)
}

/// Where each chunk after the first starts.
fn chunk_edges(buffer: &Buffer) -> Vec<usize> {
    let mut edges = Vec::new();
    let mut at = 0;
    while at < buffer.len_bytes() {
        let (chunk, start) = buffer.chunk_at(at).expect("in bounds");
        if start > 0 {
            edges.push(start);
        }
        at = start + chunk.len();
    }
    edges
}

/// Whether some edge lies in `range` (ends included).
fn crosses(edges: &[usize], range: Range<usize>) -> bool {
    let i = edges.partition_point(|&e| e < range.start);
    edges.get(i).is_some_and(|&e| e <= range.end)
}

/// Every offset a head can hold: each char boundary, and between the CR
/// and LF of each CRLF.
fn positions(text: &str) -> Vec<usize> {
    (0..=text.len())
        .filter(|&at| text.is_char_boundary(at))
        .collect()
}

/// An editor on `id` with one selection.
fn one(id: BufferId, format: FileFormat, selection: Selection) -> Editor {
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::single(selection));
    editor
}

// The tests.

#[test]
fn the_texts_are_stored_in_many_chunks() {
    for (name, text) in texts() {
        let (set, id, _) = setup(&text);
        let edges = chunk_edges(set.get(id).expect("in the set"));
        assert!(text.len() > 4 * 1024, "{name}: {} bytes", text.len());
        assert!(edges.len() >= 3, "{name}: {} chunk edges", edges.len());
    }
}

#[test]
fn every_motion_from_every_offset_matches_the_reference() {
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        let (set, id, format) = setup(&text);
        let edges = chunk_edges(set.get(id).expect("in the set"));
        let mut crossed = [0usize; 10];
        for at in positions(&text) {
            for (m, &motion) in MOTIONS.iter().enumerate() {
                let (to, _) = reference.target(at, motion, None);
                let mut editor = one(id, format, Selection::cursor(at));
                editor.move_cursors(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::cursor(to)],
                    "{name}: {motion:?} from {at}"
                );
                if to != at && crosses(&edges, at.min(to)..at.max(to)) {
                    crossed[m] += 1;
                }

                // Extending moves only the head.
                let mut editor = one(id, format, Selection::cursor(at));
                editor.extend_selections(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::new(at, to)],
                    "{name}: extend {motion:?} from {at}"
                );
            }
        }
        // Each motion that moves by more than a char crossed a chunk edge
        // somewhere in this text; Left and Right cross the edge itself.
        for (m, &motion) in MOTIONS.iter().enumerate() {
            assert!(
                crossed[m] > 0,
                "{name}: {motion:?} never crossed a chunk edge"
            );
        }
    }
}

#[test]
fn up_and_down_keep_their_goal_across_chunks() {
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        let (set, id, format) = setup(&text);
        let steps = [
            Motion::Down,
            Motion::Down,
            Motion::Up,
            Motion::Down,
            Motion::Down,
            Motion::Down,
            Motion::Up,
            Motion::Up,
        ];
        for at in positions(&text).into_iter().step_by(7) {
            let mut editor = one(id, format, Selection::cursor(at));
            let (mut head, mut goal) = (at, None);
            for &motion in &steps {
                let (to, used) = reference.target(head, motion, goal);
                (head, goal) = (to, Some(used));
                editor.move_cursors(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::cursor(head)],
                    "{name}: {motion:?} in the steps from {at}"
                );
            }
        }
    }
}

#[test]
fn many_cursors_at_once_match_the_reference() {
    // Cursors and selections close together, so one command reads a chunk
    // edge for several of them, and far apart. Some merge.
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        let (set, id, format) = setup(&text);
        let all = positions(&text);
        for (step, width) in [(3, 0), (5, 2), (11, 0), (97, 7), (401, 0)] {
            for first in [0, 1, 2] {
                let selections: Vec<Selection> = all
                    .iter()
                    .skip(first)
                    .step_by(step)
                    .map(|&at| Selection::new(reference.clamp(at + width), at))
                    .collect();
                let start = Selections::new(selections, 0).expect("non-empty");
                let primary = start.as_slice().len() / 2;
                let start = Selections::new(start.as_slice().to_vec(), primary).expect("non-empty");
                for &motion in &MOTIONS {
                    for extend in [false, true] {
                        let expected: Vec<Selection> = start
                            .as_slice()
                            .iter()
                            .map(|&s| {
                                if extend {
                                    Selection::new(
                                        s.anchor,
                                        reference.target(s.head, motion, None).0,
                                    )
                                } else {
                                    Selection::cursor(reference.moved(s, motion))
                                }
                            })
                            .collect();
                        let expected =
                            Selections::new(expected, start.primary_index()).expect("non-empty");
                        let mut editor = Editor::new(id, format);
                        editor.set_selections(start.clone());
                        if extend {
                            editor.extend_selections(&set, motion).expect("in the set");
                        } else {
                            editor.move_cursors(&set, motion).expect("in the set");
                        }
                        assert_eq!(
                            editor.selections(),
                            &expected,
                            "{name}: step {step} width {width} first {first} {motion:?} extend {extend}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn stale_offsets_clamp_then_move_like_the_reference() {
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        let (set, id, format) = setup(&text);
        // Offsets inside chars, and past the end.
        let stale: Vec<usize> = (0..text.len())
            .filter(|&at| !text.is_char_boundary(at))
            .step_by(3)
            .chain([text.len() + 1, text.len() + 50])
            .collect();
        // The ASCII text has only the two past the end.
        assert!(stale.len() >= 2, "{name}");
        for &at in &stale {
            for &motion in &MOTIONS {
                let head = reference.clamp(at);
                let (to, _) = reference.target(head, motion, None);
                let mut editor = one(id, format, Selection::cursor(at));
                editor.move_cursors(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::cursor(to)],
                    "{name}: {motion:?} from stale {at}"
                );
            }
        }
    }
}

#[test]
fn delete_to_matches_the_reference() {
    let deleting = [
        Motion::WordLeft,
        Motion::WordRight,
        Motion::LineStart,
        Motion::LineEnd,
        Motion::Left,
        Motion::Right,
    ];
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        for at in positions(&text).into_iter().step_by(13) {
            for &motion in &deleting {
                let (to, _) = reference.target(at, motion, None);
                let range = at.min(to)..at.max(to);
                let (mut set, id, format) = setup(&text);
                let mut editor = Editor::new(id, format);
                editor.set_selections(Selections::single(Selection::cursor(at)));
                let applied = editor.delete_to(&mut set, motion).expect("in the set");
                let mut expected = text.clone();
                expected.replace_range(range.clone(), "");
                assert_eq!(
                    applied.is_some(),
                    !range.is_empty(),
                    "{name}: {motion:?} from {at}"
                );
                assert_eq!(
                    set.get(id).expect("in the set").to_string(),
                    expected,
                    "{name}: {motion:?} from {at}"
                );
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::cursor(range.start)],
                    "{name}: {motion:?} from {at}"
                );
            }
        }
    }
}
