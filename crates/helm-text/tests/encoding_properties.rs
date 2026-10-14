//! T-104 acceptance tests: property tests for decoding and encoding. Valid
//! UTF-8 with or without a BOM, with any mix of line breaks, decodes and
//! encodes back to the same bytes. Any bytes at all decode to exactly what
//! the rules in `FileFormat::decode`'s docs say, checked against an oracle
//! written straight from those rules.

use helm_text::{
    Buffer, ByteOrder, DecodeError, FileFormat, LineEnding, LineEndingStyle, LineEndings,
    TextBuffer, UTF16_SAMPLE_LEN,
};
use proptest::prelude::*;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// One piece of random text: ASCII runs, each character width, each line
/// break, U+FEFF, Unicode breaks that aren't line breaks, and any char
/// except NUL.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => "[a-z ]{1,8}",
        3 => Just("\n".to_owned()),
        3 => Just("\r".to_owned()),
        3 => Just("\r\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => Just("\u{FEFF}".to_owned()),
        1 => Just("\u{2028}".to_owned()),
        1 => Just("\u{85}".to_owned()),
        2 => any::<char>()
            .prop_filter("no NUL", |&c| c != '\0')
            .prop_map(String::from),
    ]
}

/// Random text of up to `max_pieces` pieces, with no NUL characters.
fn text(max_pieces: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..max_pieces).prop_map(|pieces| pieces.concat())
}

/// Random text that may contain NUL, including runs that look like UTF-16
/// code units.
fn text_with_nuls() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        4 => piece(),
        2 => Just("\0".to_owned()),
        2 => "[a-z]\0",
        2 => "\0[a-z]",
        2 => any::<String>(),
    ];
    prop::collection::vec(piece, 0..60).prop_map(|pieces| pieces.concat())
}

/// Random bytes: valid text, raw bytes, and pieces that look like UTF-16,
/// after a prefix that may be a BOM, a UTF-16 BOM or part of a BOM.
fn mixed_bytes() -> impl Strategy<Value = Vec<u8>> {
    let prefix = prop_oneof![
        4 => Just(Vec::new()),
        2 => Just(BOM.to_vec()),
        1 => Just(b"\xFF\xFE".to_vec()),
        1 => Just(b"\xFE\xFF".to_vec()),
        1 => Just(b"\xEF\xBB".to_vec()),
        1 => Just(b"\xEF".to_vec()),
    ];
    let piece = prop_oneof![
        4 => piece().prop_map(String::into_bytes),
        2 => any::<u8>().prop_map(|b| vec![b]),
        2 => prop::collection::vec(any::<u8>(), 0..8),
        2 => "[a-z]\0".prop_map(String::into_bytes),
        1 => "\0[a-z]".prop_map(String::into_bytes),
    ];
    (prefix, prop::collection::vec(piece, 0..100)).prop_map(|(prefix, pieces)| {
        let mut bytes = prefix;
        bytes.extend(pieces.concat());
        bytes
    })
}

/// Bytes that are close to UTF-16 without a BOM: code units with one zero
/// byte on the same side, around the sample's size, with sometimes a unit
/// that breaks the pattern and sometimes an odd number of bytes.
fn near_utf16_bytes() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::collection::vec(any::<u8>(), 0..UTF16_SAMPLE_LEN / 2 + 40),
        any::<bool>(),
        prop::option::of((any::<prop::sample::Index>(), any::<[u8; 2]>())),
        prop::collection::vec(any::<u8>(), 0..3),
    )
        .prop_map(|(units, big_endian, broken, tail)| {
            let mut bytes: Vec<u8> = units
                .iter()
                .flat_map(|&unit| if big_endian { [0, unit] } else { [unit, 0] })
                .collect();
            if let Some((index, unit)) = broken
                && !units.is_empty()
            {
                let at = index.index(units.len()) * 2;
                bytes[at..at + 2].copy_from_slice(&unit);
            }
            bytes.extend(tail);
            bytes
        })
}

/// Any of the byte strategies above.
fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![mixed_bytes(), near_utf16_bytes()]
}

/// The byte order of `bytes` if they look like UTF-16 without a BOM, by
/// rule 3 of `FileFormat::decode`.
fn looks_like_utf16(bytes: &[u8]) -> Option<ByteOrder> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let sample = &bytes[..bytes.len().min(UTF16_SAMPLE_LEN)];
    if sample.len() < 4 {
        return None;
    }
    let (units, _) = sample.as_chunks::<2>();
    if units.iter().all(|&[low, high]| low != 0 && high == 0) {
        Some(ByteOrder::LittleEndian)
    } else if units.iter().all(|&[high, low]| high == 0 && low != 0) {
        Some(ByteOrder::BigEndian)
    } else {
        None
    }
}

/// What `FileFormat::decode` must return for `bytes`: the text's start
/// offset and the BOM flag, or the error. Written from the rules in its docs.
fn oracle(bytes: &[u8]) -> Result<(usize, bool), DecodeError> {
    let utf8 = |start: usize| {
        std::str::from_utf8(&bytes[start..])
            .map(|_| start)
            .map_err(|e| DecodeError::InvalidUtf8 {
                offset: start + e.valid_up_to(),
            })
    };
    if bytes.starts_with(BOM) {
        return utf8(BOM.len()).map(|start| (start, true));
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return Err(DecodeError::Utf16 {
            byte_order: ByteOrder::LittleEndian,
            bom: true,
        });
    }
    if bytes.starts_with(b"\xFE\xFF") {
        return Err(DecodeError::Utf16 {
            byte_order: ByteOrder::BigEndian,
            bom: true,
        });
    }
    if let Some(byte_order) = looks_like_utf16(bytes) {
        return Err(DecodeError::Utf16 {
            byte_order,
            bom: false,
        });
    }
    utf8(0).map(|start| (start, false))
}

/// Line breaks counted one byte at a time.
fn count(text: &str) -> LineEndings {
    let bytes = text.as_bytes();
    let mut counts = LineEndings::default();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                counts.crlf += 1;
                i += 1;
            }
            b'\r' => counts.cr += 1,
            b'\n' => counts.lf += 1,
            _ => {}
        }
        i += 1;
    }
    counts
}

/// The style and typed newline `counts` must report, from their docs.
fn style_and_newline(counts: LineEndings) -> (LineEndingStyle, LineEnding) {
    let kinds = [
        (counts.lf, LineEnding::Lf),
        (counts.crlf, LineEnding::Crlf),
        (counts.cr, LineEnding::Cr),
    ];
    let present: Vec<LineEnding> = kinds
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|&(_, kind)| kind)
        .collect();
    let style = match present.as_slice() {
        [] => LineEndingStyle::None,
        [kind] => LineEndingStyle::Uniform(*kind),
        _ => LineEndingStyle::Mixed,
    };
    // The first kind with the highest count, in the tie order LF, CRLF, CR.
    let most = kinds.iter().map(|&(n, _)| n).max().unwrap_or(0);
    let newline = kinds
        .iter()
        .find(|&&(n, _)| n == most)
        .map_or(LineEnding::Lf, |&(_, kind)| kind);
    (style, newline)
}

/// Checks a successful decode of `bytes` against the oracle and the round
/// trip.
fn check_decoded(bytes: &[u8], start: usize, bom: bool) -> Result<(), TestCaseError> {
    let decoded = FileFormat::decode(bytes).expect("oracle says this decodes");
    let text = std::str::from_utf8(&bytes[start..]).unwrap();
    prop_assert_eq!(decoded.text, text);
    prop_assert_eq!(decoded.format.bom, bom);
    let counts = count(text);
    prop_assert_eq!(decoded.format.line_endings, counts);
    prop_assert_eq!(LineEndings::count(text), counts);
    let (style, newline) = style_and_newline(counts);
    prop_assert_eq!(counts.style(), style);
    prop_assert_eq!(counts.newline(), newline);
    prop_assert_eq!(decoded.format.newline(), newline);
    prop_assert_eq!(decoded.format.encode(decoded.text), bytes);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// ED-7: valid UTF-8 with or without a BOM and with any mix of line
    /// breaks decodes, and encoding the text gives back the same bytes.
    #[test]
    fn valid_utf8_round_trips(text in text(400), bom in any::<bool>()) {
        let mut bytes = if bom { BOM.to_vec() } else { Vec::new() };
        bytes.extend_from_slice(text.as_bytes());
        // Text that starts with U+FEFF and no BOM before it reads as a BOM,
        // so the BOM flag comes from the bytes, not from `bom`.
        let has_bom = bytes.starts_with(BOM);
        prop_assert_eq!(oracle(&bytes), Ok((if has_bom { 3 } else { 0 }, has_bom)));
        check_decoded(&bytes, if has_bom { 3 } else { 0 }, has_bom)?;
        // The counts agree with the buffer's own line count.
        let decoded = FileFormat::decode(&bytes).unwrap();
        let LineEndings { lf, crlf, cr } = decoded.format.line_endings;
        prop_assert_eq!(lf + crlf + cr + 1, Buffer::from(decoded.text).len_lines());
    }

    /// Any valid UTF-8, NULs included, round-trips unless it matches the
    /// UTF-16 rule, in which case it is rejected as UTF-16.
    #[test]
    fn any_valid_utf8_round_trips_or_is_utf16(text in text_with_nuls(), bom in any::<bool>()) {
        let mut bytes = if bom { BOM.to_vec() } else { Vec::new() };
        bytes.extend_from_slice(text.as_bytes());
        match oracle(&bytes) {
            Ok((start, bom)) => check_decoded(&bytes, start, bom)?,
            Err(error) => {
                let utf16 = matches!(error, DecodeError::Utf16 { bom: false, .. });
                prop_assert!(utf16, "valid UTF-8 can only fail as UTF-16: {:?}", error);
                prop_assert_eq!(FileFormat::decode(&bytes), Err(error));
            }
        }
    }

    /// Any bytes decode to exactly what the rules say: the right text and
    /// format, the first invalid UTF-8 offset, or UTF-16. Never a panic.
    #[test]
    fn any_bytes_follow_the_rules(bytes in bytes()) {
        match oracle(&bytes) {
            Ok((start, bom)) => check_decoded(&bytes, start, bom)?,
            Err(error) => prop_assert_eq!(FileFormat::decode(&bytes), Err(error)),
        }
    }

    /// Latin-1 text written as UTF-16, with or without a BOM and followed
    /// by any bytes past the sample, is rejected as UTF-16 in the right
    /// byte order.
    #[test]
    fn latin1_as_utf16_is_rejected(
        units in prop::collection::vec(1u8..=0xFF, 2..700),
        big_endian in any::<bool>(),
        bom in any::<bool>(),
        tail in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let (byte_order, bom_bytes) = if big_endian {
            (ByteOrder::BigEndian, [0xFE, 0xFF])
        } else {
            (ByteOrder::LittleEndian, [0xFF, 0xFE])
        };
        let mut bytes = if bom { bom_bytes.to_vec() } else { Vec::new() };
        for unit in units {
            if big_endian {
                bytes.extend_from_slice(&[0, unit]);
            } else {
                bytes.extend_from_slice(&[unit, 0]);
            }
        }
        // A tail can only follow a full sample, and must keep the length
        // even, or the file no longer looks like UTF-16 without a BOM.
        if bytes.len() >= UTF16_SAMPLE_LEN {
            let even = tail.len() & !1;
            bytes.extend_from_slice(&tail[..even]);
        }
        prop_assert_eq!(
            FileFormat::decode(&bytes),
            Err(DecodeError::Utf16 { byte_order, bom })
        );
    }
}
