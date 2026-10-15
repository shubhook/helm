//! T-110 acceptance tests: property tests for `BufferSet::amend`.
//!
//! A chain of amends must leave every buffer as applying the same edits one
//! transaction at a time would, and then undo in one step where the separate
//! transactions take one undo each. Random histories that mix applies,
//! amends, undos and redos must match an oracle that keeps each buffer's
//! text, each transaction's texts before and after it, and the undo and
//! redo histories, written straight from the `BufferSet` docs.

use std::collections::HashSet;
use std::ops::Range;

use helm_text::{
    AmendError, Buffer, BufferId, BufferSet, Edit, HistoryError, TextError, Transaction,
    TransactionError, TransactionId,
};
use proptest::prelude::*;
use proptest::sample::Index;

/// How many buffers each case edits.
const BUFFERS: usize = 3;

/// One piece of random text: ASCII runs, each character width, each line
/// break, and any char.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => "[a-z ]{1,4}",
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

/// One end of a random range, picked when the edit is made, since the text
/// changes as the case runs.
#[derive(Clone, Debug)]
enum Offset {
    /// A char boundary of the buffer's current text.
    Boundary(Index),
    /// Any byte offset up to two past the end, valid or not.
    Byte(Index),
}

/// One random edit, before its offsets are picked.
#[derive(Clone, Debug)]
struct EditSpec {
    buffer: usize,
    start: Offset,
    end: Offset,
    text: String,
}

fn offset() -> impl Strategy<Value = Offset> {
    prop_oneof![
        12 => any::<Index>().prop_map(Offset::Boundary),
        1 => any::<Index>().prop_map(Offset::Byte),
    ]
}

fn edit_spec() -> impl Strategy<Value = EditSpec> {
    (0..BUFFERS, offset(), offset(), text(3)).prop_map(|(buffer, start, end, text)| EditSpec {
        buffer,
        start,
        end,
        text,
    })
}

/// Up to four edits. An empty list tests the `Empty` error.
fn edit_specs() -> impl Strategy<Value = Vec<EditSpec>> {
    prop::collection::vec(edit_spec(), 0..5)
}

fn starts() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(text(12), BUFFERS)
}

/// Picks a byte range in `text`. Two boundary offsets are put in order, so
/// most ranges are valid; a raw byte offset keeps the order it came in, so
/// reversed, out-of-bounds and mid-character ranges come up too.
fn pick_range(text: &str, start: &Offset, end: &Offset) -> Range<usize> {
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .collect();
    let pick = |offset: &Offset| match offset {
        Offset::Boundary(index) => *index.get(&boundaries),
        Offset::Byte(index) => index.index(text.len() + 3),
    };
    let (a, b) = (pick(start), pick(end));
    match (start, end) {
        (Offset::Boundary(_), Offset::Boundary(_)) => a.min(b)..a.max(b),
        _ => a..b,
    }
}

/// What `TextBuffer::slice` returns for `range` of `text`, if it errors.
fn range_error(text: &str, range: &Range<usize>) -> Option<TextError> {
    let len = text.len();
    if range.start > range.end {
        Some(TextError::InvalidRange {
            start: range.start,
            end: range.end,
        })
    } else if range.end > len {
        Some(TextError::ByteOutOfBounds {
            offset: range.end,
            len,
        })
    } else if !text.is_char_boundary(range.start) {
        Some(TextError::NotCharBoundary {
            offset: range.start,
        })
    } else if !text.is_char_boundary(range.end) {
        Some(TextError::NotCharBoundary { offset: range.end })
    } else {
        None
    }
}

/// An edit by buffer index, with its range picked.
type PlainEdit = (usize, Range<usize>, String);

/// Picks every spec's range against `texts`.
fn pick_edits(texts: &[String], specs: &[EditSpec]) -> Vec<PlainEdit> {
    specs
        .iter()
        .map(|spec| {
            let range = pick_range(&texts[spec.buffer], &spec.start, &spec.end);
            (spec.buffer, range, spec.text.clone())
        })
        .collect()
}

/// `edits` as a transaction on the buffers `ids`.
fn transaction(ids: &[BufferId], edits: &[PlainEdit]) -> Transaction {
    edits
        .iter()
        .map(|(buffer, range, text)| Edit::replace(ids[*buffer], range.clone(), text.as_str()))
        .collect()
}

/// The range checks `apply` makes on edit `i`: a range valid in `text`,
/// then no overlap with an earlier edit on the same buffer.
fn check_range(texts: &[String], edits: &[PlainEdit], i: usize) -> Result<(), TransactionError> {
    let (buffer, range, _) = &edits[i];
    if let Some(error) = range_error(&texts[*buffer], range) {
        return Err(TransactionError::InvalidRange { edit: i, error });
    }
    let earlier = edits[..i].iter().position(|(other, earlier, _)| {
        other == buffer && range.start < earlier.end && earlier.start < range.end
    });
    match earlier {
        Some(other) => Err(TransactionError::Overlap { edit: i, other }),
        None => Ok(()),
    }
}

/// The new text of every buffer `edits` touch, by buffer index, lowest
/// first. The edits must have passed every check.
fn place(texts: &[String], edits: &[PlainEdit]) -> Vec<(usize, String)> {
    let mut touched: Vec<usize> = edits.iter().map(|(buffer, ..)| *buffer).collect();
    touched.sort_unstable();
    touched.dedup();
    touched
        .into_iter()
        .map(|buffer| {
            // Place edits by start, inserts before the non-empty range at
            // the same start, then by transaction order.
            let mut mine: Vec<_> = edits
                .iter()
                .enumerate()
                .filter(|(_, (b, ..))| *b == buffer)
                .map(|(i, (_, range, text))| (range.start, range.end, i, text))
                .collect();
            mine.sort_unstable_by_key(|&(start, end, i, _)| (start, end, i));
            let old = &texts[buffer];
            let mut out = String::new();
            let mut at = 0;
            for (start, end, _, text) in mine {
                out.push_str(&old[at..start]);
                out.push_str(text);
                at = end;
            }
            out.push_str(&old[at..]);
            (buffer, out)
        })
        .collect()
}

/// One transaction in the model: each touched buffer's text before it was
/// applied and after its last amend, by buffer index, lowest first.
struct Recorded {
    id: TransactionId,
    texts: Vec<(usize, String, String)>,
}

/// The oracle: a `String` and a version per buffer, plus the undo and redo
/// histories the `BufferSet` docs describe, as stacks of indexes into
/// `transactions`.
struct Model {
    texts: Vec<String>,
    versions: Vec<u64>,
    undo: Vec<Vec<usize>>,
    redo: Vec<Vec<usize>>,
    transactions: Vec<Recorded>,
}

impl Model {
    fn new(starts: &[String]) -> Self {
        Self {
            texts: starts.to_vec(),
            versions: vec![0; starts.len()],
            undo: vec![Vec::new(); starts.len()],
            redo: vec![Vec::new(); starts.len()],
            transactions: Vec::new(),
        }
    }

    /// The new texts `apply` gives, or the error it returns.
    fn check_apply(&self, edits: &[PlainEdit]) -> Result<Vec<(usize, String)>, TransactionError> {
        if edits.is_empty() {
            return Err(TransactionError::Empty);
        }
        for i in 0..edits.len() {
            check_range(&self.texts, edits, i)?;
        }
        Ok(place(&self.texts, edits))
    }

    /// Records a transaction `apply` accepted.
    fn commit(&mut self, id: TransactionId, new: Vec<(usize, String)>) {
        let t = self.transactions.len();
        let touched: Vec<usize> = new.iter().map(|(buffer, _)| *buffer).collect();
        self.drop_redo(&touched);
        let texts = new
            .into_iter()
            .map(|(buffer, after)| {
                let before = std::mem::replace(&mut self.texts[buffer], after.clone());
                self.undo[buffer].push(t);
                self.versions[buffer] += 1;
                (buffer, before, after)
            })
            .collect();
        self.transactions.push(Recorded { id, texts });
    }

    /// The new texts amending transaction `t` with `edits` gives, or the
    /// error `amend` returns, following the order in the `amend` docs.
    /// `ids` are the set's buffer ids, by index.
    fn check_amend(
        &self,
        t: usize,
        edits: &[PlainEdit],
        ids: &[BufferId],
    ) -> Result<Vec<(usize, String)>, AmendError> {
        let transaction = self.transactions[t].id;
        let applied = self.undo.iter().any(|stack| stack.contains(&t));
        let undone = self.redo.iter().any(|stack| stack.contains(&t));
        if !applied && !undone {
            return Err(AmendError::UnknownTransaction { transaction });
        }
        if undone {
            return Err(AmendError::Undone { transaction });
        }
        if edits.is_empty() {
            return Err(AmendError::Transaction(TransactionError::Empty));
        }
        for (i, (buffer, ..)) in edits.iter().enumerate() {
            let buffer_id = ids[*buffer];
            if !self.transactions[t].texts.iter().any(|(b, ..)| b == buffer) {
                return Err(AmendError::Untouched {
                    edit: i,
                    buffer: buffer_id,
                });
            }
            if self.undo[*buffer].last() != Some(&t) {
                return Err(AmendError::NotOnTop {
                    edit: i,
                    buffer: buffer_id,
                });
            }
            check_range(&self.texts, edits, i).map_err(AmendError::Transaction)?;
        }
        Ok(place(&self.texts, edits))
    }

    /// Records an amend of transaction `t` that `amend` accepted.
    fn amend(&mut self, t: usize, new: Vec<(usize, String)>) {
        let touched: Vec<usize> = new.iter().map(|(buffer, _)| *buffer).collect();
        self.drop_redo(&touched);
        for (buffer, after) in new {
            let recorded = self.transactions[t]
                .texts
                .iter_mut()
                .find(|(b, ..)| *b == buffer)
                .expect("checked: `t` touched the buffer");
            recorded.2 = after.clone();
            self.texts[buffer] = after;
            self.versions[buffer] += 1;
        }
    }

    /// Drops the redo histories of `touched`, then everything below a
    /// dropped transaction in any redo history, until nothing more drops.
    fn drop_redo(&mut self, touched: &[usize]) {
        let mut dropped: HashSet<usize> = touched
            .iter()
            .flat_map(|&buffer| self.redo[buffer].iter().copied())
            .collect();
        loop {
            let before = dropped.len();
            for stack in &self.redo {
                if let Some(top) = stack.iter().rposition(|t| dropped.contains(t)) {
                    dropped.extend(&stack[..=top]);
                }
            }
            if dropped.len() == before {
                break;
            }
        }
        for stack in &mut self.redo {
            stack.retain(|t| !dropped.contains(t));
        }
    }

    /// Undoes (`undo == true`) or redoes the top of `buffer`'s history.
    /// Returns the transaction's index, or the transaction and the buffer
    /// that block it.
    fn step(&mut self, buffer: usize, undo: bool) -> Result<Option<usize>, (usize, usize)> {
        let (from, to) = if undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(&t) = from[buffer].last() else {
            return Ok(None);
        };
        let recorded = &self.transactions[t];
        for (touched, ..) in &recorded.texts {
            if from[*touched].last() != Some(&t) {
                return Err((t, *touched));
            }
        }
        for (touched, before, after) in &recorded.texts {
            from[*touched].pop();
            to[*touched].push(t);
            self.texts[*touched] = if undo { before } else { after }.clone();
            self.versions[*touched] += 1;
        }
        Ok(Some(t))
    }
}

/// Every buffer's text in `set`.
fn texts_of(set: &BufferSet, ids: &[BufferId]) -> Vec<String> {
    ids.iter()
        .map(|&id| set.get(id).expect("buffer is in the set").to_string())
        .collect()
}

/// Every buffer's version number in `set`.
fn versions_of(set: &BufferSet, ids: &[BufferId]) -> Vec<u64> {
    ids.iter()
        .map(|&id| set.version(id).expect("buffer is in the set").get())
        .collect()
}

/// A set holding `starts`, and its buffer ids.
fn set_of(starts: &[String]) -> (BufferSet, Vec<BufferId>) {
    let mut set = BufferSet::new();
    let ids = starts
        .iter()
        .map(|text| set.add(Buffer::from(text.as_str())))
        .collect();
    (set, ids)
}

// Property 1: a chain of amends against the same edits applied one at a
// time.

/// A first transaction that always applies: one to four inserts or
/// replaces at char boundaries, spread over random buffers, made valid by
/// dropping any edit that overlaps an earlier one.
fn first_specs() -> impl Strategy<Value = Vec<(usize, Index, Index, String)>> {
    prop::collection::vec((0..BUFFERS, any::<Index>(), any::<Index>(), text(3)), 1..5)
}

fn chain() -> impl Strategy<Value = Vec<Vec<EditSpec>>> {
    prop::collection::vec(edit_specs(), 1..16)
}

/// Builds the first transaction's edits from `specs` against `texts`.
fn first_edits(texts: &[String], specs: &[(usize, Index, Index, String)]) -> Vec<PlainEdit> {
    let mut edits: Vec<PlainEdit> = Vec::new();
    for (buffer, start, end, text) in specs {
        let range = pick_range(
            &texts[*buffer],
            &Offset::Boundary(*start),
            &Offset::Boundary(*end),
        );
        let overlaps = edits.iter().any(|(other, earlier, _)| {
            other == buffer && range.start < earlier.end && earlier.start < range.end
        });
        if !overlaps {
            edits.push((*buffer, range, text.clone()));
        }
    }
    edits
}

// Property 2 and 3: random histories with amends.

/// One random step.
#[derive(Clone, Debug)]
enum Op {
    Apply(Vec<EditSpec>),
    /// Amends a transaction the set returned earlier: the last one, or a
    /// random one, which may since be undone, dropped or covered.
    Amend {
        last: bool,
        pick: Index,
        specs: Vec<EditSpec>,
    },
    Undo(usize),
    Redo(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => edit_specs().prop_map(Op::Apply),
        5 => (any::<bool>(), any::<Index>(), edit_specs())
            .prop_map(|(last, pick, specs)| Op::Amend { last, pick, specs }),
        2 => (0..BUFFERS).prop_map(Op::Undo),
        2 => (0..BUFFERS).prop_map(Op::Redo),
    ]
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op(), 1..40)
}

/// The set under test, its buffer ids and the oracle, side by side.
struct Case {
    set: BufferSet,
    ids: Vec<BufferId>,
    model: Model,
}

impl Case {
    fn new(starts: &[String]) -> Self {
        let (set, ids) = set_of(starts);
        Self {
            set,
            ids,
            model: Model::new(starts),
        }
    }

    /// Runs `op` on both and checks they agree.
    fn run(&mut self, op: &Op) -> Result<(), TestCaseError> {
        match op {
            Op::Apply(specs) => {
                let edits = pick_edits(&self.model.texts, specs);
                let got = self.set.apply(transaction(&self.ids, &edits));
                match (self.model.check_apply(&edits), got) {
                    (Ok(new), Ok(id)) => self.model.commit(id, new),
                    (want, got) => prop_assert_eq!(got.err(), want.err()),
                }
            }
            Op::Amend { last, pick, specs } => {
                if self.model.transactions.is_empty() {
                    return Ok(());
                }
                let t = if *last {
                    self.model.transactions.len() - 1
                } else {
                    pick.index(self.model.transactions.len())
                };
                let edits = pick_edits(&self.model.texts, specs);
                let id = self.model.transactions[t].id;
                let got = self.set.amend(id, transaction(&self.ids, &edits));
                match self.model.check_amend(t, &edits, &self.ids) {
                    Ok(new) => {
                        prop_assert_eq!(got, Ok(()));
                        self.model.amend(t, new);
                    }
                    Err(error) => prop_assert_eq!(got, Err(error)),
                }
            }
            Op::Undo(buffer) => {
                let got = self.set.undo(self.ids[*buffer]);
                let want = self.model.step(*buffer, true);
                prop_assert_eq!(got, self.history_result(want));
            }
            Op::Redo(buffer) => {
                let got = self.set.redo(self.ids[*buffer]);
                let want = self.model.step(*buffer, false);
                prop_assert_eq!(got, self.history_result(want));
            }
        }
        self.check_state()
    }

    /// The model's undo or redo result, in the set's types.
    fn history_result(
        &self,
        want: Result<Option<usize>, (usize, usize)>,
    ) -> Result<Option<TransactionId>, HistoryError> {
        match want {
            Ok(t) => Ok(t.map(|t| self.model.transactions[t].id)),
            Err((t, buffer)) => Err(HistoryError::Blocked {
                transaction: self.model.transactions[t].id,
                buffer: self.ids[buffer],
            }),
        }
    }

    fn check_state(&self) -> Result<(), TestCaseError> {
        prop_assert_eq!(texts_of(&self.set, &self.ids), self.model.texts.clone());
        prop_assert_eq!(
            versions_of(&self.set, &self.ids),
            self.model.versions.clone()
        );
        Ok(())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Applies a first transaction to two sets, then the same edits to
    /// both: as amends of the first transaction in `amended`, and as
    /// separate transactions in `separate`. Amends that touch a buffer the
    /// first transaction didn't must fail with `Untouched`; other failures
    /// must be the `apply` error, wrapped. After every step both sets hold
    /// the same texts and versions. Then one undo returns `amended` to the
    /// starting texts, as undoing every separate transaction does, and one
    /// redo brings back the final texts.
    #[test]
    #[ignore = "T-110: not implemented yet"]
    fn a_chain_of_amends_matches_separate_transactions(
        starts in starts(),
        first in first_specs(),
        chain in chain(),
        undo_from in any::<Index>(),
        redo_from in any::<Index>(),
    ) {
        let (mut amended, ids) = set_of(&starts);
        let (mut separate, _) = set_of(&starts);

        let edits = first_edits(&starts, &first);
        let id = amended.apply(transaction(&ids, &edits)).expect("valid first edits");
        let first_id = separate.apply(transaction(&ids, &edits)).expect("valid first edits");
        let mut touched: Vec<usize> = edits.iter().map(|(buffer, ..)| *buffer).collect();
        touched.sort_unstable();
        touched.dedup();
        // Every separate transaction, with a buffer to undo it from.
        let mut applied = vec![(first_id, edits[0].0)];

        for specs in &chain {
            let texts = texts_of(&separate, &ids);
            let edits = pick_edits(&texts, specs);
            let got = amended.amend(id, transaction(&ids, &edits));

            let untouched = edits
                .iter()
                .enumerate()
                .map(|(i, (buffer, ..))| (i, *buffer))
                .take_while(|&(i, _)| check_range_before(&texts, &edits, i))
                .find(|(_, buffer)| !touched.contains(buffer));
            if let Some((edit, buffer)) = untouched {
                prop_assert_eq!(got, Err(AmendError::Untouched { edit, buffer: ids[buffer] }));
            } else {
                match separate.apply(transaction(&ids, &edits)) {
                    Ok(next) => {
                        prop_assert_eq!(got, Ok(()));
                        applied.push((next, edits[0].0));
                    }
                    Err(error) => prop_assert_eq!(got, Err(AmendError::Transaction(error))),
                }
            }
            prop_assert_eq!(texts_of(&amended, &ids), texts_of(&separate, &ids));
            prop_assert_eq!(versions_of(&amended, &ids), versions_of(&separate, &ids));
        }

        let finals = texts_of(&amended, &ids);
        let versions = versions_of(&amended, &ids);

        // One undo, from any buffer the first transaction touched.
        let from = ids[*undo_from.get(&touched)];
        prop_assert_eq!(amended.undo(from), Ok(Some(id)));
        prop_assert_eq!(texts_of(&amended, &ids), starts.clone());
        for (i, &buffer) in ids.iter().enumerate() {
            let bump = u64::from(touched.contains(&i));
            prop_assert_eq!(amended.version(buffer).unwrap().get(), versions[i] + bump);
            prop_assert_eq!(amended.undo(buffer), Ok(None));
        }

        // The separate transactions take one undo each, latest first.
        for &(next, buffer) in applied.iter().rev() {
            prop_assert_eq!(separate.undo(ids[buffer]), Ok(Some(next)));
        }
        prop_assert_eq!(texts_of(&separate, &ids), starts.clone());

        // One redo brings back the final texts.
        let from = ids[*redo_from.get(&touched)];
        prop_assert_eq!(amended.redo(from), Ok(Some(id)));
        prop_assert_eq!(texts_of(&amended, &ids), finals.clone());
        for &buffer in &ids {
            prop_assert_eq!(amended.redo(buffer), Ok(None));
        }
        for &(next, buffer) in &applied {
            prop_assert_eq!(separate.redo(ids[buffer]), Ok(Some(next)));
        }
        prop_assert_eq!(texts_of(&separate, &ids), finals);
    }

    /// Random applies, amends of the latest or any earlier transaction,
    /// undos and redos: every result, text and version matches the oracle.
    #[test]
    #[ignore = "T-110: not implemented yet"]
    fn random_histories_with_amends_match_the_oracle(starts in starts(), ops in ops()) {
        let mut case = Case::new(&starts);
        for op in &ops {
            case.run(op)?;
        }
    }

    /// After random histories with amends, undoing whatever isn't blocked,
    /// over and over, returns every buffer to its starting text.
    #[test]
    #[ignore = "T-110: not implemented yet"]
    fn undoing_everything_after_amends_restores_every_buffer(
        starts in starts(),
        ops in ops(),
    ) {
        let mut case = Case::new(&starts);
        for op in &ops {
            case.run(op)?;
        }

        loop {
            let mut undone = false;
            for buffer in 0..BUFFERS {
                let got = case.set.undo(case.ids[buffer]);
                let want = case.model.step(buffer, true);
                undone |= matches!(want, Ok(Some(_)));
                prop_assert_eq!(got, case.history_result(want));
            }
            case.check_state()?;
            if !undone {
                break;
            }
        }
        for (i, &id) in case.ids.iter().enumerate() {
            prop_assert_eq!(case.set.undo(id), Ok(None));
            prop_assert_eq!(case.set.get(id).unwrap().to_string(), starts[i].as_str());
        }
    }
}

/// Whether every edit before `i` passes its range checks, so `amend` gets
/// as far as edit `i`'s buffer checks.
fn check_range_before(texts: &[String], edits: &[PlainEdit], i: usize) -> bool {
    (0..i).all(|j| check_range(texts, edits, j).is_ok())
}
