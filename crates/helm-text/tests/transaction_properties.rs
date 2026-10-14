//! T-102 acceptance tests: property tests with a plain `String` per buffer
//! as the oracle. Any random sequence of transactions, undos and redos over
//! several buffers leaves every buffer equal to the oracle, returns the
//! results the oracle predicts, and undoing everything returns every buffer
//! to its starting text.

use std::collections::HashSet;
use std::ops::Range;

use helm_text::{
    Buffer, BufferId, BufferSet, Edit, HistoryError, TextError, Transaction, TransactionError,
    TransactionId,
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

/// One end of a random range, picked when the transaction is applied, since
/// the text changes as the case runs.
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

/// One random step.
#[derive(Clone, Debug)]
enum Op {
    Apply(Vec<EditSpec>),
    Undo(usize),
    Redo(usize),
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

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        // Up to four edits. An empty list tests the `Empty` error.
        4 => prop::collection::vec(edit_spec(), 0..5).prop_map(Op::Apply),
        3 => (0..BUFFERS).prop_map(Op::Undo),
        2 => (0..BUFFERS).prop_map(Op::Redo),
    ]
}

fn starts() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(text(12), BUFFERS)
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op(), 1..40)
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

/// One applied transaction in the model: each touched buffer's text before
/// and after it, by buffer index, lowest first.
struct Recorded {
    id: TransactionId,
    texts: Vec<(usize, String, String)>,
}

/// The oracle: a `String` per buffer, plus the undo and redo histories the
/// `BufferSet` docs describe, as stacks of indexes into `transactions`.
struct Model {
    texts: Vec<String>,
    undo: Vec<Vec<usize>>,
    redo: Vec<Vec<usize>>,
    transactions: Vec<Recorded>,
}

impl Model {
    fn new(starts: &[String]) -> Self {
        Self {
            texts: starts.to_vec(),
            undo: vec![Vec::new(); starts.len()],
            redo: vec![Vec::new(); starts.len()],
            transactions: Vec::new(),
        }
    }

    /// The new text of every buffer `edits` touch, by buffer index, or the
    /// error `apply` must return.
    fn check(
        &self,
        edits: &[(usize, Range<usize>, String)],
    ) -> Result<Vec<(usize, String)>, TransactionError> {
        if edits.is_empty() {
            return Err(TransactionError::Empty);
        }
        for (i, (buffer, range, _)) in edits.iter().enumerate() {
            if let Some(error) = range_error(&self.texts[*buffer], range) {
                return Err(TransactionError::InvalidRange { edit: i, error });
            }
            let earlier = edits[..i].iter().position(|(other, earlier, _)| {
                other == buffer && range.start < earlier.end && earlier.start < range.end
            });
            if let Some(other) = earlier {
                return Err(TransactionError::Overlap { edit: i, other });
            }
        }

        let mut touched: Vec<usize> = edits.iter().map(|(buffer, ..)| *buffer).collect();
        touched.sort_unstable();
        touched.dedup();
        let new = touched
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
                let old = &self.texts[buffer];
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
            .collect();
        Ok(new)
    }

    /// Records a transaction `apply` accepted, as `check` computed it.
    fn commit(&mut self, id: TransactionId, new: Vec<(usize, String)>) {
        let t = self.transactions.len();
        let touched: Vec<usize> = new.iter().map(|(buffer, _)| *buffer).collect();
        self.drop_redo(&touched);
        let texts = new
            .into_iter()
            .map(|(buffer, after)| {
                let before = std::mem::replace(&mut self.texts[buffer], after.clone());
                self.undo[buffer].push(t);
                (buffer, before, after)
            })
            .collect();
        self.transactions.push(Recorded { id, texts });
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
        }
        Ok(Some(t))
    }
}

/// The set under test, its buffer ids and the oracle, side by side.
struct Case {
    set: BufferSet,
    ids: Vec<BufferId>,
    model: Model,
    last_id: Option<TransactionId>,
}

impl Case {
    fn new(starts: &[String]) -> Self {
        let mut set = BufferSet::new();
        let ids = starts
            .iter()
            .map(|text| set.add(Buffer::from(text.as_str())))
            .collect();
        Self {
            set,
            ids,
            model: Model::new(starts),
            last_id: None,
        }
    }

    /// Runs `op` on both and checks they agree.
    fn run(&mut self, op: &Op) -> Result<(), TestCaseError> {
        match op {
            Op::Apply(specs) => {
                let edits: Vec<(usize, Range<usize>, String)> = specs
                    .iter()
                    .map(|spec| {
                        let text = &self.model.texts[spec.buffer];
                        let range = pick_range(text, &spec.start, &spec.end);
                        (spec.buffer, range, spec.text.clone())
                    })
                    .collect();
                let transaction: Transaction = edits
                    .iter()
                    .map(|(buffer, range, text)| {
                        Edit::replace(self.ids[*buffer], range.clone(), text.as_str())
                    })
                    .collect();
                let got = self.set.apply(transaction);
                match (self.model.check(&edits), got) {
                    (Ok(new), Ok(id)) => {
                        prop_assert!(self.last_id < Some(id), "ids increase");
                        self.last_id = Some(id);
                        self.model.commit(id, new);
                    }
                    (want, got) => prop_assert_eq!(got.err(), want.err()),
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
        self.check_texts()
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

    fn check_texts(&self) -> Result<(), TestCaseError> {
        for (i, &id) in self.ids.iter().enumerate() {
            let buffer = self.set.get(id).expect("buffer is in the set");
            prop_assert_eq!(
                buffer.to_string(),
                self.model.texts[i].as_str(),
                "buffer {}",
                i
            );
        }
        Ok(())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "T-102: not implemented yet"]
    fn random_histories_match_the_oracle(starts in starts(), ops in ops()) {
        let mut case = Case::new(&starts);
        for op in &ops {
            case.run(op)?;
        }
    }

    #[test]
    #[ignore = "T-102: not implemented yet"]
    fn undoing_everything_restores_every_buffer(starts in starts(), ops in ops()) {
        let mut case = Case::new(&starts);
        for op in &ops {
            case.run(op)?;
        }

        // The transaction applied last is never blocked, so undoing every
        // buffer that isn't blocked, over and over, empties every history.
        loop {
            let mut undone = false;
            for buffer in 0..BUFFERS {
                let got = case.set.undo(case.ids[buffer]);
                let want = case.model.step(buffer, true);
                undone |= matches!(want, Ok(Some(_)));
                prop_assert_eq!(got, case.history_result(want));
            }
            case.check_texts()?;
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
