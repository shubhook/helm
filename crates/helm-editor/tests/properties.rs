//! T-105 acceptance tests: property tests. ED-3's acceptance is that N
//! cursors typing give the same text as N single edits. Deleting matches a
//! plain `String` oracle, any input normalizes to the documented form, and
//! undoing and redoing a random session restores every text and selection.

use std::ops::Range;

use helm_editor::{Editor, Selection, Selections};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};
use proptest::prelude::*;
use proptest::sample::Index;

/// One piece of random text: ASCII runs, each character width, each line
/// break, and any char.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => "[a-c ]{1,4}",
        1 => Just("\n".to_owned()),
        1 => Just("\r".to_owned()),
        1 => Just("\r\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => any::<char>().prop_map(String::from),
    ]
}

/// Random text of up to `max_pieces` pieces.
fn text(max_pieces: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..max_pieces).prop_map(|pieces| pieces.concat())
}

/// One selection, as indexes into the text's char boundaries. No head
/// makes a cursor.
type Pick = (Index, Option<Index>);

/// Up to 16 selections and the index of the primary.
fn picks() -> impl Strategy<Value = (Vec<Pick>, Index)> {
    (
        prop::collection::vec(
            (any::<Index>(), prop::option::weighted(0.4, any::<Index>())),
            1..=16,
        ),
        any::<Index>(),
    )
}

/// Every char boundary of `text`, including its length.
fn boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .collect()
}

/// Selections on char boundaries of `text`, made from `picks`.
fn pick_selections(text: &str, picks: &[Pick], primary: &Index) -> Selections {
    let bounds = boundaries(text);
    let list = picks
        .iter()
        .map(|(anchor, head)| {
            let anchor = *anchor.get(&bounds);
            let head = head.as_ref().map_or(anchor, |head| *head.get(&bounds));
            Selection::new(anchor, head)
        })
        .collect();
    Selections::new(list, primary.index(picks.len())).expect("non-empty, primary in range")
}

/// The format of a file whose text is `text`, without a BOM.
fn format_of(text: &str) -> FileFormat {
    FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    }
}

/// A set with one buffer holding `text`, and an editor on it with the
/// file's format and `selections`.
fn setup(text: &str, selections: &Selections) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let mut editor = Editor::new(id, format_of(text));
    editor.set_selections(selections.clone());
    (set, editor)
}

/// The text of the editor's buffer.
fn buffer_text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer())
        .expect("buffer is in the set")
        .to_string()
}

/// The oracle for typing: every selection in `text` replaced by `with`,
/// and a cursor after each copy, in normal form.
fn replaced(text: &str, selections: &Selections, with: &str) -> (String, Selections) {
    let mut out = String::new();
    let mut last = 0;
    let mut cursors = Vec::new();
    for selection in selections.as_slice() {
        out.push_str(&text[last..selection.start()]);
        out.push_str(with);
        cursors.push(Selection::cursor(out.len()));
        last = selection.end();
    }
    out.push_str(&text[last..]);
    let cursors = Selections::new(cursors, selections.primary_index()).expect("same count");
    (out, cursors)
}

/// The range one selection deletes on its own, as the `Editor` docs
/// describe, or `None` if it deletes nothing.
fn delete_range(text: &str, selection: Selection, backward: bool) -> Option<Range<usize>> {
    if !selection.is_empty() {
        return Some(selection.range());
    }
    let at = selection.head;
    if backward {
        if text[..at].ends_with("\r\n") {
            Some(at - 2..at)
        } else {
            let len = text[..at].chars().next_back()?.len_utf8();
            Some(at - len..at)
        }
    } else if text[at..].starts_with("\r\n") {
        Some(at..at + 2)
    } else {
        let len = text[at..].chars().next()?.len_utf8();
        Some(at..at + len)
    }
}

/// The oracle for deleting: each selection's range, overlapping ranges
/// joined, removed from `text`, and the cursors where the docs put them.
/// `None` when nothing is deleted.
fn deleted(text: &str, selections: &Selections, backward: bool) -> Option<(String, Selections)> {
    let ranges: Vec<Option<Range<usize>>> = selections
        .as_slice()
        .iter()
        .map(|&selection| delete_range(text, selection, backward))
        .collect();
    let mut sorted: Vec<Range<usize>> = ranges.iter().flatten().cloned().collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by_key(|range| (range.start, range.end));
    let mut joined: Vec<Range<usize>> = Vec::new();
    for range in sorted {
        match joined.last_mut() {
            Some(last) if range.start < last.end => last.end = last.end.max(range.end),
            _ => joined.push(range),
        }
    }

    // Bytes removed by joined ranges that end at or before `at`.
    let removed_before = |at: usize| -> usize {
        joined
            .iter()
            .filter(|range| range.end <= at)
            .map(|range| range.len())
            .sum()
    };
    let cursors = selections
        .as_slice()
        .iter()
        .zip(&ranges)
        .map(|(selection, range)| {
            let at = match range {
                Some(range) => {
                    joined
                        .iter()
                        .find(|joined| joined.start <= range.start && range.end <= joined.end)
                        .expect("every range is inside a joined one")
                        .start
                }
                None => selection.head,
            };
            Selection::cursor(at - removed_before(at))
        })
        .collect();

    let mut out = String::new();
    let mut last = 0;
    for range in &joined {
        out.push_str(&text[last..range.start]);
        last = range.end;
    }
    out.push_str(&text[last..]);
    let cursors = Selections::new(cursors, selections.primary_index()).expect("same count");
    Some((out, cursors))
}

/// Applies `with` at every selection as N single-cursor edits, one editor
/// per selection, from the last selection to the first, so no edit moves
/// one still to come. Returns the text.
fn single_edits(text: &str, selections: &Selections, with: &str) -> String {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    for &selection in selections.as_slice().iter().rev() {
        let mut editor = Editor::new(id, format_of(text));
        editor.set_selections(Selections::single(selection));
        editor.insert(&mut set, with).expect("buffer is in the set");
    }
    set.get(id).expect("buffer is in the set").to_string()
}

/// One step of a random editing session.
#[derive(Clone, Debug)]
enum Op {
    Type(String),
    Newline,
    DeleteBackward,
    DeleteForward,
    Select(Vec<Pick>, Index),
    AddAbove,
    AddBelow,
    AddNextMatch,
    SelectAllMatches,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => text(3).prop_map(Op::Type),
        1 => Just(Op::Newline),
        2 => Just(Op::DeleteBackward),
        2 => Just(Op::DeleteForward),
        2 => picks().prop_map(|(picks, primary)| Op::Select(picks, primary)),
        1 => Just(Op::AddAbove),
        1 => Just(Op::AddBelow),
        1 => Just(Op::AddNextMatch),
        1 => Just(Op::SelectAllMatches),
    ]
}

/// What one editing command did: its transaction, and the text and
/// selections before and after it.
struct Step {
    id: helm_text::TransactionId,
    before: (String, Selections),
    after: (String, Selections),
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// ED-3: N cursors typing give the same text as N single edits.
    #[test]
    fn typing_at_n_cursors_matches_n_single_edits(
        start in text(24),
        (picks, primary) in picks(),
        typed in text(3),
    ) {
        let selections = pick_selections(&start, &picks, &primary);
        let (mut set, mut editor) = setup(&start, &selections);
        let result = editor.insert(&mut set, &typed).expect("buffer is in the set");

        let (expected, cursors) = replaced(&start, &selections, &typed);
        prop_assert_eq!(buffer_text(&set, &editor), single_edits(&start, &selections, &typed));
        prop_assert_eq!(buffer_text(&set, &editor), expected);
        prop_assert_eq!(editor.selections(), &cursors);
        let nothing_to_do =
            typed.is_empty() && selections.as_slice().iter().all(Selection::is_empty);
        prop_assert_eq!(result.is_none(), nothing_to_do);
    }

    /// Newline at N cursors is the file's newline typed at each, and gives
    /// the same text as N single newlines.
    #[test]
    fn newline_at_n_cursors_matches_n_single_edits(
        start in text(24),
        (picks, primary) in picks(),
    ) {
        let selections = pick_selections(&start, &picks, &primary);
        let (mut set, mut editor) = setup(&start, &selections);
        editor.newline(&mut set).expect("buffer is in the set");

        let newline = format_of(&start).newline().as_str();
        let (expected, cursors) = replaced(&start, &selections, newline);
        prop_assert_eq!(buffer_text(&set, &editor), single_edits(&start, &selections, newline));
        prop_assert_eq!(buffer_text(&set, &editor), expected);
        prop_assert_eq!(editor.selections(), &cursors);
    }

    /// Backspace at N cursors matches the oracle, text and cursors.
    #[test]
    fn delete_backward_matches_the_oracle(
        start in text(24),
        (picks, primary) in picks(),
    ) {
        let selections = pick_selections(&start, &picks, &primary);
        let (mut set, mut editor) = setup(&start, &selections);
        let result = editor.delete_backward(&mut set).expect("buffer is in the set");
        match deleted(&start, &selections, true) {
            Some((expected, cursors)) => {
                prop_assert!(result.is_some());
                prop_assert_eq!(buffer_text(&set, &editor), expected);
                prop_assert_eq!(editor.selections(), &cursors);
            }
            None => {
                prop_assert!(result.is_none());
                prop_assert_eq!(buffer_text(&set, &editor), start);
                prop_assert_eq!(editor.selections(), &selections);
            }
        }
    }

    /// Delete at N cursors matches the oracle, text and cursors.
    #[test]
    fn delete_forward_matches_the_oracle(
        start in text(24),
        (picks, primary) in picks(),
    ) {
        let selections = pick_selections(&start, &picks, &primary);
        let (mut set, mut editor) = setup(&start, &selections);
        let result = editor.delete_forward(&mut set).expect("buffer is in the set");
        match deleted(&start, &selections, false) {
            Some((expected, cursors)) => {
                prop_assert!(result.is_some());
                prop_assert_eq!(buffer_text(&set, &editor), expected);
                prop_assert_eq!(editor.selections(), &cursors);
            }
            None => {
                prop_assert!(result.is_none());
                prop_assert_eq!(buffer_text(&set, &editor), start);
                prop_assert_eq!(editor.selections(), &selections);
            }
        }
    }

    /// Any list of selections, in any order and overlapping, comes out in
    /// the normal form `Selections` documents.
    #[test]
    fn any_selections_normalize(
        raw in prop::collection::vec((0usize..40, 0usize..40, prop::bool::weighted(0.4)), 1..12),
        primary in any::<Index>(),
    ) {
        let input: Vec<Selection> = raw
            .iter()
            .map(|&(anchor, head, cursor)| Selection::new(anchor, if cursor { anchor } else { head }))
            .collect();
        let primary = primary.index(input.len());
        let set = Selections::new(input.clone(), primary).expect("non-empty, primary in range");
        let out = set.as_slice();

        // Sorted, not overlapping, and touching only when both are
        // non-empty.
        for pair in out.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            prop_assert!(a.end() <= b.start(), "{a:?} and {b:?} overlap");
            if a.end() == b.start() {
                prop_assert!(!a.is_empty() && !b.is_empty(), "{a:?} and {b:?} should merge");
            }
        }

        // Which output each input went into.
        let home = |selection: &Selection| {
            out.iter()
                .position(|o| o.start() <= selection.start() && selection.end() <= o.end())
        };
        for selection in &input {
            prop_assert!(home(selection).is_some(), "{selection:?} is lost");
            // A cursor touches only the selection it joined: one where two
            // selections touch would have joined them.
            if selection.is_empty() {
                let touching = out
                    .iter()
                    .filter(|o| o.start() <= selection.start() && selection.start() <= o.end())
                    .count();
                prop_assert_eq!(touching, 1, "{:?} touches more than one", selection);
            }
        }
        prop_assert_eq!(home(&input[primary]), Some(set.primary_index()));
        prop_assert_eq!(set.primary(), out[set.primary_index()]);

        for (index, o) in out.iter().enumerate() {
            let group: Vec<Selection> =
                input.iter().copied().filter(|s| home(s) == Some(index)).collect();
            prop_assert!(!group.is_empty());
            prop_assert_eq!(group.iter().map(Selection::start).min(), Some(o.start()));
            prop_assert_eq!(group.iter().map(Selection::end).max(), Some(o.end()));

            // The group is connected by overlaps and by touches with a
            // cursor, so it really is one selection. Cursors sort first at
            // the same start, so a cursor joining two touching selections
            // is seen before the second one.
            let mut sorted = group.clone();
            sorted.sort_by_key(|s| (s.start(), !s.is_empty()));
            let mut end = sorted[0].end();
            let mut cursor_at_end = sorted[0].is_empty();
            for s in &sorted[1..] {
                prop_assert!(
                    s.start() < end || (s.start() == end && (s.is_empty() || cursor_at_end)),
                    "{s:?} was merged into {o:?} without touching it"
                );
                if s.end() > end {
                    end = s.end();
                    cursor_at_end = false;
                } else if s.is_empty() && s.start() == end {
                    cursor_at_end = true;
                }
            }

            // Direction: the primary's if it is here and not empty, else
            // the first non-empty one given.
            if o.is_empty() {
                prop_assert!(group.iter().all(|s| *s == *o));
            } else {
                let p = input[primary];
                let leader = if home(&p) == Some(index) && !p.is_empty() {
                    p
                } else {
                    *group.iter().find(|s| !s.is_empty()).expect("o is not empty")
                };
                prop_assert_eq!(o.is_reversed(), leader.is_reversed());
            }
        }
    }

    /// Undoing a random session puts back the text and the selections from
    /// before each command, in reverse order; redoing puts back the ones
    /// from after.
    #[test]
    fn undo_and_redo_restore_text_and_selections(
        start in text(16),
        ops in prop::collection::vec(op(), 1..24),
    ) {
        let (mut set, mut editor) = setup(&start, &Selections::single(Selection::cursor(0)));
        let mut steps = Vec::new();
        for op in &ops {
            let before = (buffer_text(&set, &editor), editor.selections().clone());
            let id = match op {
                Op::Type(typed) => editor.insert(&mut set, typed).unwrap(),
                Op::Newline => editor.newline(&mut set).unwrap(),
                Op::DeleteBackward => editor.delete_backward(&mut set).unwrap(),
                Op::DeleteForward => editor.delete_forward(&mut set).unwrap(),
                Op::Select(picks, primary) => {
                    editor.set_selections(pick_selections(&before.0, picks, primary));
                    None
                }
                Op::AddAbove => {
                    editor.add_cursor_above(&set).unwrap();
                    None
                }
                Op::AddBelow => {
                    editor.add_cursor_below(&set).unwrap();
                    None
                }
                Op::AddNextMatch => {
                    editor.add_next_match(&set).unwrap();
                    None
                }
                Op::SelectAllMatches => {
                    editor.select_all_matches(&set).unwrap();
                    None
                }
            };
            if let Some(id) = id {
                let after = (buffer_text(&set, &editor), editor.selections().clone());
                steps.push(Step { id, before, after });
            }
        }

        for step in steps.iter().rev() {
            prop_assert_eq!(editor.undo(&mut set), Ok(Some(step.id)));
            prop_assert_eq!(&buffer_text(&set, &editor), &step.before.0);
            prop_assert_eq!(editor.selections(), &step.before.1);
        }
        prop_assert_eq!(editor.undo(&mut set), Ok(None));
        prop_assert_eq!(&buffer_text(&set, &editor), &start);

        for step in &steps {
            prop_assert_eq!(editor.redo(&mut set), Ok(Some(step.id)));
            prop_assert_eq!(&buffer_text(&set, &editor), &step.after.0);
            prop_assert_eq!(editor.selections(), &step.after.1);
        }
        prop_assert_eq!(editor.redo(&mut set), Ok(None));
    }
}
