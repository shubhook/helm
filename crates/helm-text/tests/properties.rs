//! T-101 acceptance tests: property tests with a plain `String` as the
//! oracle. For random text, every length, read and conversion matches what
//! the `String` computes, every conversion round-trips, every invalid offset
//! errors, and random inserts and deletes leave the buffer equal to the same
//! edits applied to the `String`.

use helm_text::{Buffer, Point, TextBuffer, TextError, Utf16Point};
use proptest::prelude::*;
use proptest::sample::Index;

/// One piece of random text: ASCII runs, each character width, each line
/// break, Unicode breaks that must not count as line breaks, and any char.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => "[a-z ]{1,8}",
        2 => Just("\n".to_owned()),
        2 => Just("\r".to_owned()),
        2 => Just("\r\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => Just("\u{2028}".to_owned()),
        1 => Just("\u{85}".to_owned()),
        1 => Just("\u{b}\u{c}".to_owned()),
        2 => any::<char>().prop_map(String::from),
    ]
}

/// Random text of up to `max_pieces` pieces.
fn text(max_pieces: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..max_pieces).prop_map(|pieces| pieces.concat())
}

/// Texts of up to about 1.5 KB, so the larger ones span more than one rope
/// chunk and chunk boundaries can land inside characters and CRLFs.
fn large_text() -> impl Strategy<Value = String> {
    text(400)
}

/// How many random offsets each case checks, on top of the text's start and
/// end. Checking every offset of every text is too slow in a debug build.
const SAMPLES: usize = 32;

/// What the buffer must agree with, computed from a plain `String`.
struct Oracle<'a> {
    text: &'a str,
    /// Byte offset of every char boundary, including the end. Entry `i` is
    /// the byte offset of char offset `i`.
    boundaries: Vec<usize>,
    /// UTF-16 offset of every entry in `boundaries`.
    utf16: Vec<usize>,
    /// Byte offset where each line starts. Always starts with 0.
    line_starts: Vec<usize>,
}

impl<'a> Oracle<'a> {
    fn new(text: &'a str) -> Self {
        let mut boundaries = Vec::with_capacity(text.len() + 1);
        let mut utf16 = Vec::with_capacity(text.len() + 1);
        let mut units = 0;
        for (byte, c) in text.char_indices() {
            boundaries.push(byte);
            utf16.push(units);
            units += c.len_utf16();
        }
        boundaries.push(text.len());
        utf16.push(units);

        let bytes = text.as_bytes();
        let mut line_starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    i += 2;
                    line_starts.push(i);
                }
                b'\r' | b'\n' => {
                    i += 1;
                    line_starts.push(i);
                }
                _ => i += 1,
            }
        }

        Self {
            text,
            boundaries,
            utf16,
            line_starts,
        }
    }

    fn len_chars(&self) -> usize {
        self.boundaries.len() - 1
    }

    fn len_utf16(&self) -> usize {
        self.utf16[self.len_chars()]
    }

    /// A valid byte offset picked by `index`.
    fn pick(&self, index: Index) -> usize {
        *index.get(&self.boundaries)
    }

    /// The char offset of valid byte offset `byte`.
    fn char_of(&self, byte: usize) -> usize {
        self.boundaries
            .binary_search(&byte)
            .expect("byte offset is a char boundary")
    }

    fn utf16_of(&self, byte: usize) -> usize {
        self.utf16[self.char_of(byte)]
    }

    fn line_of(&self, byte: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= byte) - 1
    }

    fn line_end(&self, line: usize) -> usize {
        self.line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len())
    }

    fn point(&self, byte: usize) -> Point {
        let line = self.line_of(byte);
        Point::new(line, byte - self.line_starts[line])
    }

    fn utf16_point(&self, byte: usize) -> Utf16Point {
        let line = self.line_of(byte);
        let start = self.utf16_of(self.line_starts[line]);
        Utf16Point::new(line, self.utf16_of(byte) - start)
    }

    /// The byte offsets a case checks: the start, the end, and one per
    /// sample.
    fn offsets(&self, samples: &[Index]) -> Vec<usize> {
        let mut offsets = vec![0, self.text.len()];
        offsets.extend(samples.iter().map(|&index| self.pick(index)));
        offsets
    }
}

fn samples() -> impl Strategy<Value = Vec<Index>> {
    prop::collection::vec(any::<Index>(), SAMPLES)
}

/// Checks every length and line of `buf`, and every conversion at the
/// sampled offsets, against the oracle for `text`.
fn check_against_oracle(buf: &Buffer, text: &str, samples: &[Index]) -> Result<(), TestCaseError> {
    let oracle = Oracle::new(text);

    prop_assert_eq!(buf.to_string(), text);
    prop_assert_eq!(buf.len_bytes(), text.len());
    prop_assert_eq!(buf.len_chars(), oracle.len_chars());
    prop_assert_eq!(buf.len_utf16(), oracle.len_utf16());
    prop_assert_eq!(buf.len_lines(), oracle.line_starts.len());

    for (line, &start) in oracle.line_starts.iter().enumerate() {
        let end = oracle.line_end(line);
        prop_assert_eq!(buf.line(line), Ok(text[start..end].to_owned()));
        prop_assert_eq!(buf.line_to_byte(line), Ok(start));
    }

    for byte in oracle.offsets(samples) {
        let char = oracle.char_of(byte);
        let utf16 = oracle.utf16_of(byte);
        let point = oracle.point(byte);
        let utf16_point = oracle.utf16_point(byte);

        prop_assert_eq!(buf.byte_to_char(byte), Ok(char));
        prop_assert_eq!(buf.char_to_byte(char), Ok(byte));
        prop_assert_eq!(buf.byte_to_utf16(byte), Ok(utf16));
        prop_assert_eq!(buf.utf16_to_byte(utf16), Ok(byte));
        prop_assert_eq!(buf.byte_to_line(byte), Ok(point.line));
        prop_assert_eq!(buf.byte_to_point(byte), Ok(point));
        prop_assert_eq!(buf.point_to_byte(point), Ok(byte));
        prop_assert_eq!(buf.byte_to_utf16_point(byte), Ok(utf16_point));
        prop_assert_eq!(buf.utf16_point_to_byte(utf16_point), Ok(byte));
    }
    Ok(())
}

proptest! {
    // Each buffer call scans part of a rope chunk, which is slow in a debug
    // build. 128 cases keep this file to a few seconds there.
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn conversions_match_the_oracle(text in large_text(), samples in samples()) {
        let buf = Buffer::from(text.as_str());
        check_against_oracle(&buf, &text, &samples)?;
    }

    #[test]
    fn conversions_round_trip(text in large_text(), samples in samples()) {
        let buf = Buffer::from(text.as_str());
        let oracle = Oracle::new(&text);

        for byte in oracle.offsets(&samples) {
            let char = buf.byte_to_char(byte)?;
            prop_assert_eq!(buf.char_to_byte(char), Ok(byte));
            let utf16 = buf.byte_to_utf16(byte)?;
            prop_assert_eq!(buf.utf16_to_byte(utf16), Ok(byte));
            let point = buf.byte_to_point(byte)?;
            prop_assert_eq!(buf.point_to_byte(point), Ok(byte));
            let utf16_point = buf.byte_to_utf16_point(byte)?;
            prop_assert_eq!(buf.utf16_point_to_byte(utf16_point), Ok(byte));
        }
        for index in &samples {
            let char = index.index(oracle.len_chars() + 1);
            let byte = buf.char_to_byte(char)?;
            prop_assert_eq!(buf.byte_to_char(byte), Ok(char));

            // Offsets inside a surrogate pair are covered by
            // `invalid_offsets_error`.
            let utf16 = index.index(oracle.len_utf16() + 1);
            if let Ok(byte) = buf.utf16_to_byte(utf16) {
                prop_assert_eq!(buf.byte_to_utf16(byte), Ok(utf16));
            }
        }
        for line in 0..buf.len_lines() {
            let byte = buf.line_to_byte(line)?;
            prop_assert_eq!(buf.byte_to_line(byte), Ok(line));
        }
    }

    #[test]
    fn invalid_offsets_error(text in large_text(), samples in samples()) {
        let buf = Buffer::from(text.as_str());
        let oracle = Oracle::new(&text);
        let len = text.len();

        // Any byte offset up to two past the end, valid or not.
        let bytes = samples.iter().map(|index| index.index(len + 3));
        for byte in bytes.chain([len + 1, len + 2]) {
            let want = if byte > len {
                Err(TextError::ByteOutOfBounds { offset: byte, len })
            } else if !text.is_char_boundary(byte) {
                Err(TextError::NotCharBoundary { offset: byte })
            } else {
                Ok(())
            };
            prop_assert_eq!(buf.byte_to_char(byte).map(drop), want);
            prop_assert_eq!(buf.byte_to_utf16(byte).map(drop), want);
            prop_assert_eq!(buf.byte_to_line(byte).map(drop), want);
            prop_assert_eq!(buf.byte_to_point(byte).map(drop), want);
            prop_assert_eq!(buf.byte_to_utf16_point(byte).map(drop), want);
            prop_assert_eq!(buf.slice(byte..byte).map(drop), want);
        }

        let len_chars = oracle.len_chars();
        for char in [len_chars + 1, len_chars + 2] {
            prop_assert_eq!(
                buf.char_to_byte(char),
                Err(TextError::CharOutOfBounds { offset: char, len: len_chars })
            );
        }

        // Any UTF-16 offset up to two past the end, valid or not.
        let len16 = oracle.len_utf16();
        let utf16s = samples.iter().map(|index| index.index(len16 + 3));
        for utf16 in utf16s.chain([len16 + 1, len16 + 2]) {
            let want = if utf16 > len16 {
                Err(TextError::Utf16OutOfBounds { offset: utf16, len: len16 })
            } else if oracle.utf16.binary_search(&utf16).is_err() {
                Err(TextError::InsideSurrogatePair { offset: utf16 })
            } else {
                Ok(())
            };
            prop_assert_eq!(buf.utf16_to_byte(utf16).map(drop), want);
        }

        let lines = buf.len_lines();
        prop_assert_eq!(
            buf.line(lines),
            Err(TextError::LineOutOfBounds { line: lines, len: lines })
        );
    }

    #[test]
    fn slices_match_the_oracle(text in large_text(), a in any::<Index>(), b in any::<Index>()) {
        let buf = Buffer::from(text.as_str());
        let oracle = Oracle::new(&text);
        let (a, b) = (oracle.pick(a), oracle.pick(b));
        let (start, end) = (a.min(b), a.max(b));
        prop_assert_eq!(buf.slice(start..end), Ok(text[start..end].to_owned()));
    }

    #[test]
    fn edits_match_the_oracle(
        text in large_text(),
        edits in prop::collection::vec(edit(), 1..40),
        samples in samples(),
    ) {
        let mut buf = Buffer::from(text.as_str());
        let mut expected = text;

        for edit in edits {
            let oracle = Oracle::new(&expected);
            match edit {
                Edit::Insert(at, inserted) => {
                    let at = oracle.pick(at);
                    buf.insert(at, &inserted)?;
                    expected.insert_str(at, &inserted);
                }
                Edit::Delete(a, b) => {
                    let (a, b) = (oracle.pick(a), oracle.pick(b));
                    let range = a.min(b)..a.max(b);
                    buf.delete(range.clone())?;
                    expected.replace_range(range, "");
                }
            }
            prop_assert_eq!(buf.to_string(), expected.as_str());
            prop_assert_eq!(buf.len_lines(), Oracle::new(&expected).line_starts.len());
        }
        check_against_oracle(&buf, &expected, &samples)?;
    }
}

/// One random edit. Offsets are picked from the text's char boundaries when
/// the edit is applied, since the text changes as edits run.
#[derive(Clone, Debug)]
enum Edit {
    Insert(Index, String),
    Delete(Index, Index),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (any::<Index>(), text(8)).prop_map(|(at, text)| Edit::Insert(at, text)),
        (any::<Index>(), any::<Index>()).prop_map(|(a, b)| Edit::Delete(a, b)),
    ]
}
