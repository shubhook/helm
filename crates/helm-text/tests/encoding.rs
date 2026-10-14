//! T-104 acceptance tests: hand-written examples of decoding file bytes,
//! detecting the BOM and line endings, rejecting bytes that aren't UTF-8,
//! and encoding the text back to the same bytes.

use helm_text::{
    Buffer, ByteOrder, DecodeError, FileFormat, LineEnding, LineEndingStyle, LineEndings,
    TextBuffer, UTF16_SAMPLE_LEN,
};

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// `BOM` followed by `rest`.
fn with_bom(rest: &[u8]) -> Vec<u8> {
    [BOM, rest].concat()
}

/// Line ending counts, in the order LF, CRLF, CR.
fn counts(lf: usize, crlf: usize, cr: usize) -> LineEndings {
    LineEndings { lf, crlf, cr }
}

/// Decodes `bytes`, checks the text, BOM flag and line ending counts, and
/// checks that encoding the text with the format gives back `bytes`.
#[track_caller]
fn assert_decodes(bytes: &[u8], text: &str, bom: bool, line_endings: LineEndings) -> FileFormat {
    let decoded = FileFormat::decode(bytes).expect("bytes are valid UTF-8");
    assert_eq!(decoded.text, text, "text");
    assert_eq!(decoded.format.bom, bom, "bom");
    assert_eq!(decoded.format.line_endings, line_endings, "line endings");
    assert_eq!(
        decoded.format.encode(decoded.text),
        bytes,
        "encode(decode(bytes)) == bytes"
    );
    decoded.format
}

#[track_caller]
fn assert_invalid_utf8(bytes: &[u8], offset: usize) {
    assert_eq!(
        FileFormat::decode(bytes),
        Err(DecodeError::InvalidUtf8 { offset }),
        "{bytes:?}"
    );
}

#[track_caller]
fn assert_utf16(bytes: &[u8], byte_order: ByteOrder, bom: bool) {
    assert_eq!(
        FileFormat::decode(bytes),
        Err(DecodeError::Utf16 { byte_order, bom }),
        "{bytes:?}"
    );
}

// Empty files and the BOM.

#[test]
fn empty_file() {
    let format = assert_decodes(b"", "", false, counts(0, 0, 0));
    assert_eq!(format, FileFormat::default());
    assert_eq!(format.line_endings.style(), LineEndingStyle::None);
    assert_eq!(format.newline(), LineEnding::Lf);
    assert_eq!(format.encode(""), b"");
}

#[test]
fn bom_only() {
    let format = assert_decodes(BOM, "", true, counts(0, 0, 0));
    assert_eq!(format.newline(), LineEnding::Lf);
    assert_eq!(format.encode(""), BOM);
}

#[test]
fn bom_with_content() {
    let bytes = with_bom("fn main() {}\n// café 中 😀\n".as_bytes());
    let format = assert_decodes(
        &bytes,
        "fn main() {}\n// café 中 😀\n",
        true,
        counts(2, 0, 0),
    );
    assert_eq!(
        format.line_endings.style(),
        LineEndingStyle::Uniform(LineEnding::Lf)
    );
}

#[test]
fn bom_with_crlf_content() {
    let bytes = with_bom(b"a\r\nb\r\n");
    let format = assert_decodes(&bytes, "a\r\nb\r\n", true, counts(0, 2, 0));
    assert_eq!(format.newline(), LineEnding::Crlf);
}

#[test]
fn only_the_first_bom_is_removed() {
    let bytes = with_bom(&with_bom(b"a"));
    assert_decodes(&bytes, "\u{FEFF}a", true, counts(0, 0, 0));
}

#[test]
fn bom_character_after_the_start_is_text() {
    assert_decodes(
        "a\u{FEFF}b".as_bytes(),
        "a\u{FEFF}b",
        false,
        counts(0, 0, 0),
    );
}

#[test]
fn no_bom() {
    let format = assert_decodes(b"hello", "hello", false, counts(0, 0, 0));
    assert_eq!(format.line_endings.style(), LineEndingStyle::None);
    assert_eq!(format.newline(), LineEnding::Lf);
}

// Line endings.

#[test]
fn lf() {
    let format = assert_decodes(b"a\nb\nc\n", "a\nb\nc\n", false, counts(3, 0, 0));
    assert_eq!(
        format.line_endings.style(),
        LineEndingStyle::Uniform(LineEnding::Lf)
    );
    assert_eq!(format.newline(), LineEnding::Lf);
}

#[test]
fn crlf_is_kept_in_the_text() {
    let format = assert_decodes(
        b"a\r\nb\r\nc\r\n",
        "a\r\nb\r\nc\r\n",
        false,
        counts(0, 3, 0),
    );
    assert_eq!(
        format.line_endings.style(),
        LineEndingStyle::Uniform(LineEnding::Crlf)
    );
    assert_eq!(format.newline(), LineEnding::Crlf);
}

#[test]
fn lone_cr() {
    let format = assert_decodes(b"a\rb\rc", "a\rb\rc", false, counts(0, 0, 2));
    assert_eq!(
        format.line_endings.style(),
        LineEndingStyle::Uniform(LineEnding::Cr)
    );
    assert_eq!(format.newline(), LineEnding::Cr);
}

#[test]
fn mixed_endings_round_trip() {
    let text = "a\r\nb\nc\r\nd\re\r\n";
    let format = assert_decodes(text.as_bytes(), text, false, counts(1, 3, 1));
    assert_eq!(format.line_endings.style(), LineEndingStyle::Mixed);
    // The most common line break wins.
    assert_eq!(format.newline(), LineEnding::Crlf);
}

#[test]
fn mixed_endings_with_bom_round_trip() {
    let bytes = with_bom(b"one\ntwo\r\nthree\rfour\n");
    let format = assert_decodes(&bytes, "one\ntwo\r\nthree\rfour\n", true, counts(2, 1, 1));
    assert_eq!(format.line_endings.style(), LineEndingStyle::Mixed);
    assert_eq!(format.newline(), LineEnding::Lf);
}

#[test]
fn newline_is_the_most_common_with_ties_to_lf_then_crlf() {
    let newline = |lf, crlf, cr| counts(lf, crlf, cr).newline();
    assert_eq!(newline(1, 1, 0), LineEnding::Lf);
    assert_eq!(newline(1, 0, 1), LineEnding::Lf);
    assert_eq!(newline(0, 1, 1), LineEnding::Crlf);
    assert_eq!(newline(2, 2, 2), LineEnding::Lf);
    assert_eq!(newline(1, 2, 2), LineEnding::Crlf);
    assert_eq!(newline(0, 0, 0), LineEnding::Lf);
    assert_eq!(newline(0, 1, 2), LineEnding::Cr);
}

#[test]
fn style_from_counts() {
    assert_eq!(counts(0, 0, 0).style(), LineEndingStyle::None);
    assert_eq!(
        counts(5, 0, 0).style(),
        LineEndingStyle::Uniform(LineEnding::Lf)
    );
    assert_eq!(
        counts(0, 5, 0).style(),
        LineEndingStyle::Uniform(LineEnding::Crlf)
    );
    assert_eq!(
        counts(0, 0, 5).style(),
        LineEndingStyle::Uniform(LineEnding::Cr)
    );
    assert_eq!(counts(1, 1, 0).style(), LineEndingStyle::Mixed);
    assert_eq!(counts(0, 1, 1).style(), LineEndingStyle::Mixed);
    assert_eq!(counts(1, 0, 1).style(), LineEndingStyle::Mixed);
}

#[test]
fn cr_and_lf_next_to_each_other() {
    // CR then CRLF: a lone CR, then one CRLF.
    assert_decodes(b"\r\r\n", "\r\r\n", false, counts(0, 1, 1));
    // LF then CR: two separate breaks, not a reversed CRLF.
    assert_decodes(b"\n\r", "\n\r", false, counts(1, 0, 1));
    // CRLF then LF.
    assert_decodes(b"\r\n\n", "\r\n\n", false, counts(1, 1, 0));
    // A CR at the very end is a lone CR.
    assert_decodes(b"a\r\nb\r", "a\r\nb\r", false, counts(0, 1, 1));
}

#[test]
fn unicode_breaks_are_not_line_endings() {
    let text = "a\u{2028}b\u{2029}c\u{85}d\u{b}e\u{c}f";
    let format = assert_decodes(text.as_bytes(), text, false, counts(0, 0, 0));
    assert_eq!(format.line_endings.style(), LineEndingStyle::None);
}

#[test]
fn counts_match_the_buffer_line_count() {
    for text in ["", "a", "a\nb", "a\r\nb\rc\n", "\r\r\n\n\r", "x\u{2028}\n"] {
        let endings = LineEndings::count(text);
        assert_eq!(
            endings.lf + endings.crlf + endings.cr + 1,
            Buffer::from(text).len_lines(),
            "{text:?}"
        );
    }
}

// Final newline.

#[test]
fn final_newline_is_kept() {
    assert_decodes(b"a\nb\n", "a\nb\n", false, counts(2, 0, 0));
    assert_decodes(b"a\r\nb\r\n", "a\r\nb\r\n", false, counts(0, 2, 0));
    assert_decodes(b"a\rb\r", "a\rb\r", false, counts(0, 0, 2));
}

#[test]
fn missing_final_newline_is_not_added() {
    assert_decodes(b"a\nb", "a\nb", false, counts(1, 0, 0));
    assert_decodes(b"a\r\nb", "a\r\nb", false, counts(0, 1, 0));
    assert_decodes(b"a\rb", "a\rb", false, counts(0, 0, 1));
    assert_decodes(&with_bom(b"a\r\nb"), "a\r\nb", true, counts(0, 1, 0));
}

// Encoding.

#[test]
fn encode_writes_the_bom_and_the_text() {
    let plain = FileFormat::default();
    let bom = FileFormat {
        bom: true,
        ..FileFormat::default()
    };
    assert_eq!(plain.encode("a\nb"), b"a\nb");
    assert_eq!(bom.encode("a\nb"), with_bom(b"a\nb"));
    assert_eq!(bom.encode(""), BOM);
}

#[test]
fn encode_never_converts_line_endings() {
    // A CRLF file that gained an LF line, say from a paste, saves both as
    // they are. Converting is the editor's job when the text goes in.
    let format = FileFormat::decode(b"a\r\nb\r\n").unwrap().format;
    assert_eq!(format.encode("a\r\nnew\nb\r\n"), b"a\r\nnew\nb\r\n");
    let format = FileFormat::decode(b"a\nb\n").unwrap().format;
    assert_eq!(format.encode("a\r\nb\r"), b"a\r\nb\r");
}

#[test]
fn typed_newline_goes_back_out_unchanged() {
    // Open a CRLF file, type a line with the file's newline, save.
    let decoded = FileFormat::decode(b"\xEF\xBB\xBFone\r\ntwo\r\n").unwrap();
    let mut buffer = Buffer::from(decoded.text);
    let newline = decoded.format.newline().as_str();
    assert_eq!(newline, "\r\n");
    buffer
        .insert(buffer.len_bytes(), &format!("three{newline}"))
        .unwrap();
    assert_eq!(
        decoded.format.encode(&buffer.to_string()),
        b"\xEF\xBB\xBFone\r\ntwo\r\nthree\r\n"
    );
}

#[test]
fn new_file_defaults() {
    let format = FileFormat::default();
    assert!(!format.bom);
    assert_eq!(format.line_endings, counts(0, 0, 0));
    assert_eq!(format.newline(), LineEnding::Lf);
    assert_eq!(format.newline().as_str(), "\n");
    assert_eq!(format.encode("x\n"), b"x\n");
}

// Invalid UTF-8.

#[test]
fn invalid_utf8_at_the_start() {
    // A stray continuation byte.
    assert_invalid_utf8(b"\x80abc", 0);
    // 0xFF never appears in UTF-8, and isn't a UTF-16 BOM without 0xFE.
    assert_invalid_utf8(b"\xFFabc", 0);
    // An overlong encoding of '/'.
    assert_invalid_utf8(b"\xC0\xAFabc", 0);
    // A BOM cut short is invalid at its first byte.
    assert_invalid_utf8(b"\xEF\xBB", 0);
    assert_invalid_utf8(b"\xEF\xBBa", 0);
}

#[test]
fn invalid_utf8_in_the_middle() {
    // A lead byte followed by a non-continuation byte.
    assert_invalid_utf8(b"ab\xC3(cd", 2);
    // A UTF-16 surrogate encoded in UTF-8 (CESU-8), after valid é and 中.
    assert_invalid_utf8(&["é中".as_bytes(), b"\xED\xA0\x80x"].concat(), 5);
    // A byte after a complete 4-byte character.
    assert_invalid_utf8(b"\xF0\x9F\x98\x80\xBFz", 4);
    // Past U+10FFFF.
    assert_invalid_utf8(b"line\n\xF4\x90\x80\x80\n", 5);
    // Invalid bytes on a CRLF line, after the CR.
    assert_invalid_utf8(b"a\r\x80\n", 2);
}

#[test]
fn invalid_utf8_at_the_end() {
    // 中 cut after two of its three bytes.
    assert_invalid_utf8(b"abc\xE4\xB8", 3);
    // 😀 cut after one byte.
    assert_invalid_utf8(b"abc\n\xF0", 4);
    // A lone continuation byte as the last byte.
    assert_invalid_utf8(b"abc\x80", 3);
}

#[test]
fn invalid_utf8_offset_counts_the_bom() {
    assert_invalid_utf8(&with_bom(b"\x80"), 3);
    assert_invalid_utf8(&with_bom(b"ab\xFF"), 5);
    assert_invalid_utf8(&with_bom(b"ab\xE4\xB8"), 5);
}

// UTF-16.

#[test]
fn utf16_bom() {
    // "hi\r\n" in UTF-16 with a BOM.
    assert_utf16(b"\xFF\xFEh\0i\0\r\0\n\0", ByteOrder::LittleEndian, true);
    assert_utf16(b"\xFE\xFF\0h\0i\0\r\0\n", ByteOrder::BigEndian, true);
    // The BOM alone, and the BOM before text that isn't ASCII.
    assert_utf16(b"\xFF\xFE", ByteOrder::LittleEndian, true);
    assert_utf16(b"\xFE\xFF", ByteOrder::BigEndian, true);
    assert_utf16(b"\xFF\xFE\x2D\x4E", ByteOrder::LittleEndian, true);
    // A UTF-32 little-endian BOM starts with the UTF-16 one.
    assert_utf16(b"\xFF\xFE\0\0h\0\0\0", ByteOrder::LittleEndian, true);
}

#[test]
fn utf16_without_bom() {
    // "hi\n" and "café\r\n" in UTF-16 without a BOM: valid UTF-8 full of
    // NULs, which must not decode as text.
    assert_utf16(b"h\0i\0\n\0", ByteOrder::LittleEndian, false);
    assert_utf16(b"\0h\0i\0\n", ByteOrder::BigEndian, false);
    assert_utf16(b"c\0a\0f\0\xE9\0\r\0\n\0", ByteOrder::LittleEndian, false);
    // The shortest match: two code units.
    assert_utf16(b"h\0i\0", ByteOrder::LittleEndian, false);
    assert_utf16(b"\0h\0i", ByteOrder::BigEndian, false);
}

#[test]
fn not_utf16() {
    // One code unit is too little to tell.
    assert_decodes(b"h\0", "h\0", false, counts(0, 0, 0));
    // Odd length.
    assert_decodes(b"h\0i\0j", "h\0i\0j", false, counts(0, 0, 0));
    // A code unit that is all zeros, or has no zero.
    assert_decodes(b"h\0\0\0", "h\0\0\0", false, counts(0, 0, 0));
    assert_decodes(b"h\0ij", "h\0ij", false, counts(0, 0, 0));
    // Zeros switching sides.
    assert_decodes(b"h\0\0i", "h\0\0i", false, counts(0, 0, 0));
    // A NUL in ordinary text.
    assert_decodes(b"abc\0def\n", "abc\0def\n", false, counts(1, 0, 0));
    // After a UTF-8 BOM the file says it is UTF-8, so the NULs are text.
    assert_decodes(&with_bom(b"h\0i\0"), "h\0i\0", true, counts(0, 0, 0));
}

#[test]
fn utf16_check_looks_only_at_the_sample() {
    // UTF-16-looking start, ASCII after the sample: still UTF-16.
    let mut bytes = b"a\0".repeat(UTF16_SAMPLE_LEN / 2);
    bytes.extend_from_slice(b"plain ascii!");
    assert_utf16(&bytes, ByteOrder::LittleEndian, false);
    // ASCII start, UTF-16-looking bytes after the sample: valid UTF-8.
    let mut bytes = b"ab".repeat(UTF16_SAMPLE_LEN / 2);
    bytes.extend_from_slice(b"c\0d\0");
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert_decodes(&bytes, &text, false, counts(0, 0, 0));
    // One code unit inside the sample that breaks the pattern: valid UTF-8.
    let mut bytes = b"a\0".repeat(UTF16_SAMPLE_LEN / 2);
    bytes[UTF16_SAMPLE_LEN - 1] = b'b';
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert_decodes(&bytes, &text, false, counts(0, 0, 0));
}
