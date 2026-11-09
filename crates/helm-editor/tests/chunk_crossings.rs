//! Moves that read text from two chunks. A NEW file next to
//! `chunk_motions.rs`, added for council round 3 (PR #34, T-107): that file
//! counts a move as crossing a chunk edge when the edge lies anywhere in the
//! moved range, ends included, which also counts a head sitting on an edge.
//! Here a move counts only when an edge lies strictly inside the bytes it
//! scans, so its readers need bytes from both chunks, and every such move is
//! checked against the reference.
//!
//! The reference and the texts are copied from `chunk_motions.rs`, which
//! stays unchanged. Like there, the reference works on the whole text as one
//! `String`, and the test calls `chunk_at` only to find where chunks start.

use helm_editor::{Editor, Motion, Selection, Selections};
use helm_text::{Buffer, BufferId, BufferSet, FileFormat, LineEndings, TextBuffer};

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

/// Whether an edge lies strictly between `a` and `b`, so the bytes from
/// one to the other come from at least two chunks.
fn strictly_crosses(edges: &[usize], a: usize, b: usize) -> bool {
    let (lo, hi) = (a.min(b), a.max(b));
    let i = edges.partition_point(|&e| e <= lo);
    edges.get(i).is_some_and(|&e| e < hi)
}

// The tests.

#[test]
fn moves_that_read_two_chunks_match_the_reference() {
    // Left and Right move one char, and chunks start and end on char
    // boundaries, so a single char never spans an edge; they aren't here.
    let motions = [
        Motion::Up,
        Motion::Down,
        Motion::WordLeft,
        Motion::WordRight,
        Motion::LineStart,
        Motion::LineEnd,
        Motion::TextStart,
        Motion::TextEnd,
    ];
    let mut totals = vec![0usize; motions.len()];
    for (name, text) in texts() {
        let reference = Reference::new(&text);
        let (set, id, format) = setup(&text);
        let edges = chunk_edges(set.get(id).expect("in the set"));
        let mut counts = vec![0usize; motions.len()];
        for at in positions(&text) {
            for (m, &motion) in motions.iter().enumerate() {
                let (to, _) = reference.target(at, motion, None);
                if !strictly_crosses(&edges, at, to) {
                    continue;
                }
                counts[m] += 1;
                let mut editor = one(id, format, Selection::cursor(at));
                editor.move_cursors(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::cursor(
                        reference.moved(Selection::cursor(at), motion)
                    )],
                    "{name}: {motion:?} from {at} to {to} across {edges:?}"
                );
                let mut editor = one(id, format, Selection::cursor(at));
                editor.extend_selections(&set, motion).expect("in the set");
                assert_eq!(
                    editor.selections().as_slice(),
                    [Selection::new(at, to)],
                    "{name}: extend {motion:?} from {at}"
                );
            }
        }
        // Every text has moves of every kind that read two chunks, except
        // where its lines or words are all shorter than any chunk span.
        for (m, &motion) in motions.iter().enumerate() {
            totals[m] += counts[m];
            if matches!(
                motion,
                Motion::Up | Motion::Down | Motion::TextStart | Motion::TextEnd
            ) {
                assert!(counts[m] > 0, "{name}: no {motion:?} read two chunks");
            }
        }
    }
    for (m, &motion) in motions.iter().enumerate() {
        assert!(totals[m] > 0, "no {motion:?} read two chunks in any text");
    }
}

#[test]
fn a_long_word_across_many_chunks_moves_in_one_step() {
    // The long run spans several chunks: WordRight from its start and
    // WordLeft from its end each read every chunk in between.
    let text = long_run();
    let reference = Reference::new(&text);
    let (set, id, format) = setup(&text);
    let edges = chunk_edges(set.get(id).expect("in the set"));
    let start = text.find('"').expect("quote") + 1;
    let end = text.rfind('"').expect("quote");
    let inside = edges.iter().filter(|&&e| start < e && e < end).count();
    assert!(inside >= 3, "{inside} chunk edges inside the run");
    for (at, motion, to) in [
        (start, Motion::WordRight, end),
        (end, Motion::WordLeft, start),
    ] {
        assert_eq!(reference.target(at, motion, None).0, to);
        let mut editor = one(id, format, Selection::cursor(at));
        editor.move_cursors(&set, motion).expect("in the set");
        assert_eq!(
            editor.selections().as_slice(),
            [Selection::cursor(to)],
            "{motion:?}"
        );
    }
}
