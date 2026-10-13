//! T-101 acceptance tests: every kind of bad offset returns the right error
//! value, nothing panics, and a rejected edit leaves the buffer unchanged.

use helm_text::{Buffer, Point, TextBuffer, TextError, Utf16Point};

/// a(0) é(1..3) 中(3..6) 😀(6..10). 10 bytes, 4 chars, 5 UTF-16 code units:
/// a=0, é=1, 中=2, 😀=3..5.
const TEXT: &str = "aé中😀";

/// Byte offsets inside the 2-, 3- and 4-byte characters of `TEXT`.
const INSIDE_CHAR: [usize; 6] = [2, 4, 5, 7, 8, 9];

/// A range that starts after it ends. Bound to names so clippy's
/// `reversed_empty_ranges` doesn't reject the literal.
const REVERSED: (usize, usize) = (3, 1);

/// Byte offsets past the end of `TEXT`.
const PAST_END: [usize; 3] = [11, 12, usize::MAX];

/// Every `TextBuffer` method that takes one byte offset, by name.
fn byte_queries(buf: &Buffer, byte: usize) -> Vec<(&'static str, Option<TextError>)> {
    vec![
        ("byte_to_char", buf.byte_to_char(byte).err()),
        ("byte_to_utf16", buf.byte_to_utf16(byte).err()),
        ("byte_to_line", buf.byte_to_line(byte).err()),
        ("byte_to_point", buf.byte_to_point(byte).err()),
        ("byte_to_utf16_point", buf.byte_to_utf16_point(byte).err()),
    ]
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn byte_offset_past_end() {
    let buf = Buffer::from(TEXT);
    for byte in PAST_END {
        let want = Some(TextError::ByteOutOfBounds {
            offset: byte,
            len: 10,
        });
        for (name, got) in byte_queries(&buf, byte) {
            assert_eq!(got, want, "{name}({byte})");
        }
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn byte_offset_at_end_is_valid() {
    let buf = Buffer::from(TEXT);
    for (name, got) in byte_queries(&buf, 10) {
        assert_eq!(got, None, "{name}(10)");
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn byte_offset_inside_a_character() {
    let buf = Buffer::from(TEXT);
    for byte in INSIDE_CHAR {
        let want = Some(TextError::NotCharBoundary { offset: byte });
        for (name, got) in byte_queries(&buf, byte) {
            assert_eq!(got, want, "{name}({byte})");
        }
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn byte_offset_on_empty_buffer() {
    let buf = Buffer::new();
    let want = Some(TextError::ByteOutOfBounds { offset: 1, len: 0 });
    for (name, got) in byte_queries(&buf, 1) {
        assert_eq!(got, want, "{name}(1)");
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn char_offset_past_end() {
    let buf = Buffer::from(TEXT);
    assert_eq!(buf.char_to_byte(4), Ok(10));
    for char in [5, usize::MAX] {
        assert_eq!(
            buf.char_to_byte(char),
            Err(TextError::CharOutOfBounds {
                offset: char,
                len: 4
            })
        );
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn utf16_offset_past_end() {
    let buf = Buffer::from(TEXT);
    assert_eq!(buf.utf16_to_byte(5), Ok(10));
    for utf16 in [6, usize::MAX] {
        assert_eq!(
            buf.utf16_to_byte(utf16),
            Err(TextError::Utf16OutOfBounds {
                offset: utf16,
                len: 5
            })
        );
    }
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn utf16_offset_inside_surrogate_pair() {
    let buf = Buffer::from(TEXT);
    assert_eq!(buf.utf16_to_byte(3), Ok(6));
    assert_eq!(
        buf.utf16_to_byte(4),
        Err(TextError::InsideSurrogatePair { offset: 4 })
    );
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn line_past_end() {
    let buf = Buffer::from("ab\ncd");
    for line in [2, usize::MAX] {
        let want = TextError::LineOutOfBounds { line, len: 2 };
        assert_eq!(buf.line(line), Err(want), "line({line})");
        assert_eq!(buf.line_to_byte(line), Err(want), "line_to_byte({line})");
        assert_eq!(buf.point_to_byte(Point::new(line, 0)), Err(want));
        assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(line, 0)), Err(want));
    }

    let buf = Buffer::new();
    let want = TextError::LineOutOfBounds { line: 1, len: 1 };
    assert_eq!(buf.line(1), Err(want));
    assert_eq!(buf.line_to_byte(1), Err(want));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn column_past_end_of_line() {
    let buf = Buffer::from("ab\ncd");
    // Line 0's last column is 2, the position of its LF. Column 3 would be
    // the start of line 1.
    assert_eq!(buf.point_to_byte(Point::new(0, 2)), Ok(2));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 2)), Ok(2));
    // The last line has no line break, so its last column is its length.
    assert_eq!(buf.point_to_byte(Point::new(1, 2)), Ok(5));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 2)), Ok(5));

    for (line, column) in [(0, 3), (0, usize::MAX), (1, 3), (1, usize::MAX)] {
        let want = Err(TextError::ColumnOutOfBounds { line, column });
        assert_eq!(buf.point_to_byte(Point::new(line, column)), want);
        assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(line, column)), want);
    }

    let buf = Buffer::new();
    let want = Err(TextError::ColumnOutOfBounds { line: 0, column: 1 });
    assert_eq!(buf.point_to_byte(Point::new(0, 1)), want);
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 1)), want);
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn column_past_end_of_crlf_line() {
    let buf = Buffer::from("ab\r\ncd");
    // Column 3 sits between the CR and the LF, which is a valid offset.
    assert_eq!(buf.point_to_byte(Point::new(0, 3)), Ok(3));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 3)), Ok(3));
    let want = Err(TextError::ColumnOutOfBounds { line: 0, column: 4 });
    assert_eq!(buf.point_to_byte(Point::new(0, 4)), want);
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 4)), want);
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn column_past_end_of_line_with_wide_characters() {
    // Line 0 is 9 bytes but 4 UTF-16 code units before its LF.
    let buf = Buffer::from("é中😀\nx");
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(0, 4)), Ok(9));
    assert_eq!(
        buf.utf16_point_to_byte(Utf16Point::new(0, 5)),
        Err(TextError::ColumnOutOfBounds { line: 0, column: 5 })
    );
    assert_eq!(buf.point_to_byte(Point::new(0, 9)), Ok(9));
    assert_eq!(
        buf.point_to_byte(Point::new(0, 10)),
        Err(TextError::ColumnOutOfBounds {
            line: 0,
            column: 10
        })
    );
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn byte_column_inside_a_character() {
    // é(0..2) 中(2..5) 😀(5..9) \n(9) é(10..12)
    let buf = Buffer::from("é中😀\né");
    for column in [1, 3, 4, 6, 7, 8] {
        assert_eq!(
            buf.point_to_byte(Point::new(0, column)),
            Err(TextError::NotCharBoundary { offset: column }),
            "column {column}"
        );
    }
    // The error names the absolute byte offset, not the column.
    assert_eq!(
        buf.point_to_byte(Point::new(1, 1)),
        Err(TextError::NotCharBoundary { offset: 11 })
    );
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn utf16_column_inside_surrogate_pair() {
    let buf = Buffer::from("😀\nx😀");
    assert_eq!(
        buf.utf16_point_to_byte(Utf16Point::new(0, 1)),
        Err(TextError::InsideSurrogatePair { offset: 1 })
    );
    // In UTF-16, line 1 starts at 3 and holds x(3) 😀(4..6); in bytes, x(5)
    // 😀(6..10). The error names the absolute UTF-16 offset, not the column.
    assert_eq!(
        buf.utf16_point_to_byte(Utf16Point::new(1, 2)),
        Err(TextError::InsideSurrogatePair { offset: 5 })
    );
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 1)), Ok(6));
    assert_eq!(buf.utf16_point_to_byte(Utf16Point::new(1, 3)), Ok(10));
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn bad_slice_ranges() {
    let buf = Buffer::from(TEXT);
    assert_eq!(
        buf.slice(REVERSED.0..REVERSED.1),
        Err(TextError::InvalidRange { start: 3, end: 1 })
    );
    assert_eq!(
        buf.slice(0..11),
        Err(TextError::ByteOutOfBounds {
            offset: 11,
            len: 10
        })
    );
    assert_eq!(
        buf.slice(11..12),
        Err(TextError::ByteOutOfBounds {
            offset: 12,
            len: 10
        })
    );
    for byte in INSIDE_CHAR {
        let want = Err(TextError::NotCharBoundary { offset: byte });
        assert_eq!(buf.slice(byte..10), want, "slice({byte}..10)");
        assert_eq!(buf.slice(0..byte), want, "slice(0..{byte})");
    }
    // The start is checked before the end.
    assert_eq!(
        buf.slice(2..4),
        Err(TextError::NotCharBoundary { offset: 2 })
    );
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn rejected_insert_leaves_buffer_unchanged() {
    let mut buf = Buffer::from(TEXT);
    for byte in PAST_END {
        assert_eq!(
            buf.insert(byte, "x"),
            Err(TextError::ByteOutOfBounds {
                offset: byte,
                len: 10
            })
        );
    }
    for byte in INSIDE_CHAR {
        assert_eq!(
            buf.insert(byte, "x"),
            Err(TextError::NotCharBoundary { offset: byte })
        );
    }
    assert_eq!(buf.to_string(), TEXT);
    assert_eq!(buf.len_bytes(), 10);
}

#[test]
#[ignore = "T-101: not implemented yet"]
fn rejected_delete_leaves_buffer_unchanged() {
    let mut buf = Buffer::from(TEXT);
    assert_eq!(
        buf.delete(REVERSED.0..REVERSED.1),
        Err(TextError::InvalidRange { start: 3, end: 1 })
    );
    assert_eq!(
        buf.delete(0..11),
        Err(TextError::ByteOutOfBounds {
            offset: 11,
            len: 10
        })
    );
    assert_eq!(
        buf.delete(0..usize::MAX),
        Err(TextError::ByteOutOfBounds {
            offset: usize::MAX,
            len: 10
        })
    );
    for byte in INSIDE_CHAR {
        let want = Err(TextError::NotCharBoundary { offset: byte });
        assert_eq!(buf.delete(byte..10), want, "delete({byte}..10)");
        assert_eq!(buf.delete(0..byte), want, "delete(0..{byte})");
    }
    assert_eq!(buf.to_string(), TEXT);
    assert_eq!(buf.len_bytes(), 10);
}
