//! T-101 acceptance tests: hand-written examples of lengths, reads,
//! conversions and edits, one character class or line break at a time.

use helm_text::{Buffer, Point, TextBuffer, Utf16Point};

/// Asserts the four lengths of `buf`: bytes, chars, UTF-16 code units, lines.
fn assert_lens(buf: &Buffer, bytes: usize, chars: usize, utf16: usize, lines: usize) {
    assert_eq!(buf.len_bytes(), bytes, "len_bytes");
    assert_eq!(buf.len_chars(), chars, "len_chars");
    assert_eq!(buf.len_utf16(), utf16, "len_utf16");
    assert_eq!(buf.len_lines(), lines, "len_lines");
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn empty_buffer() {
    for buf in [Buffer::new(), Buffer::from(""), Buffer::default()] {
        assert_lens(&buf, 0, 0, 0, 1);
        assert_eq!(buf.to_string(), "");
        assert_eq!(buf.line(0).unwrap(), "");
        assert_eq!(buf.slice(0..0).unwrap(), "");
        assert_eq!(buf.byte_to_char(0), Ok(0));
        assert_eq!(buf.char_to_byte(0), Ok(0));
        assert_eq!(buf.byte_to_utf16(0), Ok(0));
        assert_eq!(buf.utf16_to_byte(0), Ok(0));
        assert_eq!(buf.byte_to_line(0), Ok(0));
        assert_eq!(buf.line_to_byte(0), Ok(0));
        assert_eq!(buf.byte_to_point(0), Ok(Point::new(0, 0)));
        assert_eq!(buf.point_to_byte(Point::new(0, 0)), Ok(0));
        assert_eq!(buf.byte_to_utf16_point(0), Ok(Utf16Point::new(0, 0)));
        assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 0)), Ok(0));
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn ascii() {
    let buf = Buffer::from("hello\nworld");
    assert_lens(&buf, 11, 11, 11, 2);
    assert_eq!(buf.to_string(), "hello\nworld");
    assert_eq!(buf.line(0).unwrap(), "hello\n");
    assert_eq!(buf.line(1).unwrap(), "world");
    assert_eq!(buf.slice(2..8).unwrap(), "llo\nwo");
    assert_eq!(buf.byte_to_char(7), Ok(7));
    assert_eq!(buf.char_to_byte(7), Ok(7));
    assert_eq!(buf.byte_to_utf16(7), Ok(7));
    assert_eq!(buf.utf16_to_byte(7), Ok(7));
    // The line break belongs to the line it ends.
    assert_eq!(buf.byte_to_line(5), Ok(0));
    assert_eq!(buf.byte_to_line(6), Ok(1));
    assert_eq!(buf.byte_to_line(11), Ok(1));
    assert_eq!(buf.line_to_byte(1), Ok(6));
    assert_eq!(buf.byte_to_point(7), Ok(Point::new(1, 1)));
    assert_eq!(buf.point_to_byte(Point::new(1, 5)), Ok(11));
    assert_eq!(buf.byte_to_utf16_point(5), Ok(Utf16Point::new(0, 5)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 5)), Ok(5));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn two_byte_character() {
    // c a f é(2 bytes) \n
    let buf = Buffer::from("café\n");
    assert_lens(&buf, 6, 5, 5, 2);
    assert_eq!(buf.slice(3..5).unwrap(), "é");
    assert_eq!(buf.byte_to_char(5), Ok(4));
    assert_eq!(buf.char_to_byte(4), Ok(5));
    assert_eq!(buf.byte_to_utf16(5), Ok(4));
    assert_eq!(buf.utf16_to_byte(4), Ok(5));
    assert_eq!(buf.utf16_to_byte(3), Ok(3));
    assert_eq!(buf.byte_to_point(5), Ok(Point::new(0, 5)));
    assert_eq!(buf.byte_to_utf16_point(5), Ok(Utf16Point::new(0, 4)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 4)), Ok(5));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn three_byte_character() {
    // a 中(3 bytes) b
    let buf = Buffer::from("a中b");
    assert_lens(&buf, 5, 3, 3, 1);
    assert_eq!(buf.slice(1..4).unwrap(), "中");
    assert_eq!(buf.byte_to_char(4), Ok(2));
    assert_eq!(buf.char_to_byte(2), Ok(4));
    assert_eq!(buf.byte_to_utf16(4), Ok(2));
    assert_eq!(buf.utf16_to_byte(2), Ok(4));
    assert_eq!(buf.byte_to_utf16_point(4), Ok(Utf16Point::new(0, 2)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 2)), Ok(4));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn surrogate_pair_character() {
    // a 😀(4 bytes, 2 UTF-16 code units) b
    let buf = Buffer::from("a😀b");
    assert_lens(&buf, 6, 3, 4, 1);
    assert_eq!(buf.slice(1..5).unwrap(), "😀");
    assert_eq!(buf.byte_to_char(5), Ok(2));
    assert_eq!(buf.char_to_byte(2), Ok(5));
    assert_eq!(buf.byte_to_utf16(1), Ok(1));
    assert_eq!(buf.byte_to_utf16(5), Ok(3));
    assert_eq!(buf.utf16_to_byte(1), Ok(1));
    assert_eq!(buf.utf16_to_byte(3), Ok(5));
    assert_eq!(buf.utf16_to_byte(4), Ok(6));
    assert_eq!(buf.byte_to_utf16_point(5), Ok(Utf16Point::new(0, 3)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 3)), Ok(5));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn mixed_widths_across_lines() {
    // é(0..2) 中(2..5) 😀(5..9) \n(9) x(10)
    let buf = Buffer::from("é中😀\nx");
    assert_lens(&buf, 11, 5, 6, 2);
    assert_eq!(buf.line(0).unwrap(), "é中😀\n");
    assert_eq!(buf.byte_to_utf16(10), Ok(5));
    assert_eq!(buf.utf16_to_byte(5), Ok(10));
    assert_eq!(buf.byte_to_point(9), Ok(Point::new(0, 9)));
    assert_eq!(buf.byte_to_utf16_point(9), Ok(Utf16Point::new(0, 4)));
    assert_eq!(buf.byte_to_utf16_point(10), Ok(Utf16Point::new(1, 0)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 4)), Ok(9));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 1)), Ok(11));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn lf_line_breaks() {
    let buf = Buffer::from("a\nb\n\nc");
    assert_lens(&buf, 6, 6, 6, 4);
    assert_eq!(buf.line(0).unwrap(), "a\n");
    assert_eq!(buf.line(1).unwrap(), "b\n");
    assert_eq!(buf.line(2).unwrap(), "\n");
    assert_eq!(buf.line(3).unwrap(), "c");
    assert_eq!(buf.line_to_byte(2), Ok(4));
    assert_eq!(buf.line_to_byte(3), Ok(5));
    assert_eq!(buf.byte_to_line(4), Ok(2));
    assert_eq!(buf.byte_to_point(4), Ok(Point::new(2, 0)));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn crlf_is_one_line_break() {
    let buf = Buffer::from("a\r\nb");
    assert_lens(&buf, 4, 4, 4, 2);
    assert_eq!(buf.line(0).unwrap(), "a\r\n");
    assert_eq!(buf.line(1).unwrap(), "b");
    assert_eq!(buf.line_to_byte(1), Ok(3));
    assert_eq!(buf.byte_to_line(1), Ok(0));
    // Between the CR and the LF is still line 0.
    assert_eq!(buf.byte_to_line(2), Ok(0));
    assert_eq!(buf.byte_to_point(2), Ok(Point::new(0, 2)));
    assert_eq!(buf.point_to_byte(Point::new(0, 2)), Ok(2));
    assert_eq!(buf.byte_to_utf16_point(2), Ok(Utf16Point::new(0, 2)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 2)), Ok(2));
    assert_eq!(buf.byte_to_line(3), Ok(1));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 0)), Ok(3));

    let buf = Buffer::from("\r\n\r\n");
    assert_eq!(buf.len_lines(), 3);
    assert_eq!(buf.line(0).unwrap(), "\r\n");
    assert_eq!(buf.line(1).unwrap(), "\r\n");
    assert_eq!(buf.line(2).unwrap(), "");
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn lone_cr_is_a_line_break() {
    let buf = Buffer::from("a\rb\rc");
    assert_lens(&buf, 5, 5, 5, 3);
    assert_eq!(buf.line(0).unwrap(), "a\r");
    assert_eq!(buf.line(1).unwrap(), "b\r");
    assert_eq!(buf.line(2).unwrap(), "c");
    assert_eq!(buf.line_to_byte(2), Ok(4));
    assert_eq!(buf.byte_to_point(2), Ok(Point::new(1, 0)));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn mixed_line_breaks() {
    // LF then CR is two breaks, not a CRLF.
    let buf = Buffer::from("\n\r");
    assert_eq!(buf.len_lines(), 3);
    assert_eq!(buf.line(0).unwrap(), "\n");
    assert_eq!(buf.line(1).unwrap(), "\r");

    // CR, then CRLF: two breaks.
    let buf = Buffer::from("\r\r\n");
    assert_eq!(buf.len_lines(), 3);
    assert_eq!(buf.line(0).unwrap(), "\r");
    assert_eq!(buf.line(1).unwrap(), "\r\n");

    // CRLF, lone CR, LF.
    let buf = Buffer::from("a\r\n\rb\n");
    assert_eq!(buf.len_lines(), 4);
    assert_eq!(buf.line_to_byte(1), Ok(3));
    assert_eq!(buf.line_to_byte(2), Ok(4));
    assert_eq!(buf.line_to_byte(3), Ok(6));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn trailing_newline_makes_an_empty_last_line() {
    let buf = Buffer::from("a\n");
    assert_lens(&buf, 2, 2, 2, 2);
    assert_eq!(buf.line(1).unwrap(), "");
    assert_eq!(buf.line_to_byte(1), Ok(2));
    assert_eq!(buf.byte_to_line(2), Ok(1));
    assert_eq!(buf.byte_to_point(2), Ok(Point::new(1, 0)));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 0)), Ok(2));

    let buf = Buffer::from("a\r\n");
    assert_eq!(buf.len_lines(), 2);
    assert_eq!(buf.byte_to_point(3), Ok(Point::new(1, 0)));

    let buf = Buffer::from("a\r");
    assert_eq!(buf.len_lines(), 2);
    assert_eq!(buf.byte_to_point(2), Ok(Point::new(1, 0)));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn unicode_line_breaks_are_not_line_breaks() {
    // U+2028 LINE SEPARATOR is 3 bytes and 1 UTF-16 code unit.
    let buf = Buffer::from("a\u{2028}b");
    assert_lens(&buf, 5, 3, 3, 1);
    assert_eq!(buf.line(0).unwrap(), "a\u{2028}b");
    assert_eq!(buf.byte_to_point(4), Ok(Point::new(0, 4)));
    assert_eq!(buf.byte_to_utf16_point(4), Ok(Utf16Point::new(0, 2)));

    // Nor are U+2029 PARAGRAPH SEPARATOR, U+0085 NEL, VT or FF.
    for text in ["a\u{2029}b", "a\u{85}b", "a\u{b}b", "a\u{c}b"] {
        let buf = Buffer::from(text);
        assert_eq!(buf.len_lines(), 1, "{text:?}");
        assert_eq!(buf.line(0).unwrap(), text);
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn insert_and_delete() {
    let mut buf = Buffer::new();
    buf.insert(0, "ac").unwrap();
    buf.insert(1, "b").unwrap();
    buf.insert(3, "😀").unwrap();
    assert_eq!(buf.to_string(), "abc😀");
    assert_lens(&buf, 7, 4, 5, 1);

    buf.delete(1..2).unwrap();
    assert_eq!(buf.to_string(), "ac😀");
    buf.delete(2..6).unwrap();
    assert_eq!(buf.to_string(), "ac");
    buf.delete(0..0).unwrap();
    assert_eq!(buf.to_string(), "ac");
    buf.delete(0..2).unwrap();
    assert_lens(&buf, 0, 0, 0, 1);
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn edits_that_split_or_join_a_crlf() {
    // Inserting between CR and LF splits one break into two.
    let mut buf = Buffer::from("a\r\nb");
    buf.insert(2, "x").unwrap();
    assert_eq!(buf.to_string(), "a\rx\nb");
    assert_eq!(buf.len_lines(), 3);
    assert_eq!(buf.line(1).unwrap(), "x\n");

    // Deleting what sat between them joins the two back into one CRLF.
    buf.delete(2..3).unwrap();
    assert_eq!(buf.to_string(), "a\r\nb");
    assert_eq!(buf.len_lines(), 2);
    assert_eq!(buf.line(0).unwrap(), "a\r\n");

    // Deleting the LF of a CRLF leaves a lone CR, still one break.
    buf.delete(2..3).unwrap();
    assert_eq!(buf.to_string(), "a\rb");
    assert_eq!(buf.len_lines(), 2);

    // Appending an LF after a trailing CR makes a CRLF, not a second break.
    let mut buf = Buffer::from("a\r");
    buf.insert(2, "\n").unwrap();
    assert_eq!(buf.len_lines(), 2);
    assert_eq!(buf.line(0).unwrap(), "a\r\n");
    assert_eq!(buf.line(1).unwrap(), "");
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn clones_are_independent() {
    let mut buf = Buffer::from("abc");
    let copy = buf.clone();
    buf.insert(3, "d").unwrap();
    assert_eq!(buf.to_string(), "abcd");
    assert_eq!(copy.to_string(), "abc");
}
