//! T-114's tests: Khakha's answers to PR #42's open questions on the bracket
//! scan's 1 MiB cap, repeated visual bracket objects and escaped quotes,
//! and a CRLF row for normal linewise `p` below a last line with no line
//! break.
//!
//! Every row that ends in normal mode with one cursor and no CR was run
//! through Vim 9.1 (`vim -u NONE -N -i NONE -n`, `binary`, `noeol`), and the
//! expected text and cursor are Vim's, except where a comment says
//! otherwise. Markers and key notation are as in `vim_harness`.

mod vim_harness;

use vim_harness::{Vim, table};

/// How far a bracket object reads each way from where its scan starts.
const MIB: usize = 1 << 20;

#[test]
fn repeated_visual_bracket_objects_grow_to_the_next_pair_out() {
    table(&[
        ("f(a(b|c)d)e", "va(a(d", "f|e"),
        ("f(a(b|c)d)e", "vi(i(d", "f(|)e"),
        ("f(a(b|c)d)e", "Va(a(d", "f|e"),
        ("x {a {b|c} d} y", "va{a{d", "x | y"),
        ("x [a [b|c] d] y", "vi[i[d", "x [|] y"),
        ("x <a <b|c> d> y", "vi<lt>i<lt>d", "x <|> y"),
        ("f(a\n  (b|c)\n  d)e", "vi(i(d", "f(|)e"),
        // Three levels, and a count on the second object.
        ("((((|bc))))", "vi(i(i(d", "((|))"),
        ("((((|bc))))", "vi(2i(d", "(((|)))"),
        ("(((|b)))", "va(2a(d", "|"),
        ("f(a(b|c)d)e", "vi(2i(d", "f(|)e"),
    ]);
}

#[test]
fn a_bracket_object_after_another_object_grows_past_it() {
    table(&[
        // `a(` after `i(`: the pair around the inside.
        ("f(a(b|c)d)e", "vi(a(d", "f(a|d)e"),
        // `i(` after `a(`: the inside of the next pair out.
        ("f(a(b|c)d)e", "va(i(d", "f(|)e"),
        // With the visual cursor at the start.
        ("f(a(b|c)d)e", "va(oa(d", "f|e"),
        ("f(a(b|)c)", "vla(a(d", "|f"),
    ]);
}

#[test]
fn a_bracket_object_from_a_longer_selection_starts_at_its_first_char() {
    table(&[
        ("f(a(b|cd)e)", "vla(d", "f(a|e)"),
        ("f(a(bc|d)e)", "vhi(d", "f(a(|)e)"),
        // The inside of `(bc)` doesn't cover `)`, so `i(` moves out.
        ("f(a(|bc)d)", "vlli(d", "f(|)"),
        // A closing bracket at the first char doesn't count.
        ("f(a(b|)c)", "vli(d", "f(a(|)c)"),
        // A selection that crosses a pair takes the pair around its start.
        ("f(a(|b)c(d)e)", "v4la(d", "f(a|c(d)e)"),
        // An inside of one char doesn't grow, even inside the selection.
        ("f(a(|b)c(d)e)", "v4li(d", "f(a(|)c(d)e)"),
        // A one-char selection is a fresh object.
        ("((|b))", "vi(i(d", "((|))"),
        ("f|(a(b)c)", "vi(i(d", "f(|)"),
    ]);
}

#[test]
fn a_grown_bracket_selection_runs_forward() {
    // Vim always puts the visual cursor on the pair's last char.
    let mut v = Vim::new("f(a(bc|d)e)");
    v.keys("vha(");
    assert_eq!(v.show(), "f(a^(bcd)|e)");
    let mut v = Vim::new("f(a(b|c)d)e");
    v.keys("va(oa(");
    assert_eq!(v.show(), "f^(a(bc)d)|e");
}

#[test]
fn a_grown_inner_block_of_lines_stops_before_the_line_break() {
    // Vim selects through the line break after `  }`. A visual end here is
    // never the line break of a line with text (T-108), so the selection
    // stops on `}`.
    let mut v = Vim::new("{\n  {\n    |x\n  }\n}");
    v.keys("vi{i{");
    assert_eq!(v.show(), "{\n^  {\n    x\n  }|\n}");
}

#[test]
fn a_bracket_object_with_no_pair_out_keeps_the_selection() {
    // Helm's rule: where the object finds nothing, the selection stays.
    // Vim 9.1 instead leaves `(bc)` selected after the third `a(`, which
    // `vim.rs` lists as a difference.
    table(&[
        ("f(a(b|c)d)e", "va(a(a(d", "f|e"),
        ("f(a(b|c)d)e", "vi(i(i(d", "f(|)e"),
    ]);
}

#[test]
fn a_quote_is_escaped_only_after_an_odd_number_of_backslashes() {
    table(&[
        // Two backslashes escape each other, so the quote after them closes.
        ("\"|a\\\\\"b\"", "di\"", "\"|\"b\""),
        ("\"a\\\\|\"b\"", "di\"", "\"|\"b\""),
        ("\"|a\\\\\\\\\"b\"", "di\"", "\"|\"b\""),
        ("x\\\\\"|a\"b\"", "di\"", "x\\\\\"|\"b\""),
        ("|x \\\\\"a\" b", "di\"", "x \\\\\"|\" b"),
        ("\"a\\\\\" |x \"c\"", "di\"", "\"a\\\\\"|\"c\""),
        ("\"|a\\\\\"", "di\"", "\"|\""),
        ("\\\\\"|a\"", "di\"", "\\\\\"|\""),
        ("'|a\\\\'b'", "di'", "'|'b'"),
        ("`|a\\\\`b`", "di`", "`|`b`"),
        ("\"|a\\\\\" b", "da\"", "|b"),
        ("\"|a\\\\\"", "vi\"d", "\"|\""),
        ("\"|a\\\\\"b\"", "ci\"X<esc>", "\"|X\"b\""),
        // The cursor's quote is the third, so it opens a pair that never
        // closes.
        ("\"a\\\\\"|\"", "di\"", "\"a\\\\\"|\""),
        ("\"a\\\\\"b|\"", "da\"", "\"a\\\\\"b|\""),
        // One, three and five backslashes escape the quote.
        ("\"|a\\\\\\\"b\"", "di\"", "\"|\""),
        ("\"|a\\\\\\\\\\\"b\"", "di\"", "\"|\""),
        ("s = \"a\\\"|b\";", "di\"", "s = \"|\";"),
    ]);
}

#[test]
fn an_escaped_quote_never_opens_a_pair() {
    // Vim 9.1 leaves `x\""b"`: with no unescaped quote before the cursor,
    // it takes the escaped one as the opening quote. `vim.rs` lists this
    // as a difference.
    table(&[("x\\\"|a\"b\"", "di\"", "x\\\"a\"|\"")]);
}

#[test]
fn a_bracket_scan_reads_at_most_1_mib_before_the_cursor() {
    // `(` at offset 0 and the cursor 1 MiB after it: just inside.
    let text = format!("({}|b)", "a".repeat(MIB - 1));
    let mut v = Vim::new(&text);
    v.keys("di(");
    assert_eq!(v.text(), "()");
    // One byte further: just outside, so no pair.
    let text = format!("({}|b)", "a".repeat(MIB));
    let mut v = Vim::new(&text);
    v.keys("di(");
    assert_eq!(v.text().len(), MIB + 3);
}

#[test]
fn a_bracket_scan_reads_at_most_1_mib_from_the_cursor_on() {
    // The cursor at offset 1 and `)` at 1 MiB: the last byte it may read.
    let text = format!("(|b{})", "a".repeat(MIB - 2));
    let mut v = Vim::new(&text);
    v.keys("di(");
    assert_eq!(v.text(), "()");
    // One byte further: just outside, so no pair.
    let text = format!("(|b{})", "a".repeat(MIB - 1));
    let mut v = Vim::new(&text);
    v.keys("di(");
    assert_eq!(v.text().len(), MIB + 2);
}

#[test]
fn a_capped_scan_finds_no_outer_pair_for_a_count() {
    // The inner pair is near, the outer one's `(` is past the cap.
    let text = format!("({}(|b))", "a".repeat(MIB));
    let mut v = Vim::new(&text);
    v.keys("d2i(");
    assert_eq!(v.text().len(), MIB + 5);
    v.keys("di(");
    assert_eq!(v.text().len(), MIB + 4);
}

#[test]
fn a_capped_scan_keeps_the_visual_selection() {
    let text = format!("({}(|b))", "a".repeat(MIB));
    let mut v = Vim::new(&text);
    v.keys("va(a(d");
    // `va(` selects `(b)`; the second `a(` finds no pair within the cap.
    assert_eq!(v.text().len(), MIB + 2);
}

#[test]
fn linewise_p_below_a_last_line_with_no_crlf_puts_one() {
    // The register's CRLF moves in front of the put lines.
    table(&[
        ("|a\r\nb", "yyjp", "a\r\nb\r\n|a"),
        ("|a\r\nb", "yyj2p", "a\r\nb\r\n|a\r\na"),
        ("a\r\n|b", "yyp", "a\r\nb\r\n|b"),
    ]);
}

/// Whether `di(` from `marked` changes the text.
fn di_paren_deletes(marked: &str) -> bool {
    let mut v = Vim::new(marked);
    let before = v.text().len();
    v.keys("di(");
    v.text().len() != before
}

// The `k` ranges in the next three tests (940..1030 and 330..420) are
// tuned to where ropey puts chunk boundaries in these texts today. If
// helm-text's chunking changes, the tests still pass but may no longer
// put a chunk boundary 1 to 3 bytes inside the window. The `é` and `中`
// tests after them don't depend on chunks.

#[test]
fn a_bracket_just_before_the_window_never_counts() {
    // The `(` is one byte before the window's first byte. The `x`s move
    // the rope's chunk boundaries past the window's start, 1 to 3 bytes
    // in for some `k`, where a reader's 4-byte minimum read used to reach
    // the `(`.
    for k in 940..1030 {
        let text = format!("{}({}|b)", "x".repeat(k), "a".repeat(MIB));
        assert!(!di_paren_deletes(&text), "k = {k}");
    }
}

#[test]
fn a_bracket_at_the_windows_end_never_counts() {
    // The `)` is at the window's end, the first byte it excludes.
    for k in 330..420 {
        let text = format!(
            "{}(|b{}){}",
            "x".repeat(k),
            "a".repeat(MIB - 1),
            "y".repeat(2000)
        );
        assert!(!di_paren_deletes(&text), "k = {k}");
    }
}

#[test]
fn a_visual_bracket_object_never_grows_past_the_window() {
    for k in 940..1030 {
        let text = format!("{}({}(|b))", "x".repeat(k), "a".repeat(MIB));
        let mut v = Vim::new(&text);
        v.keys("va(a(d");
        // `va(` selects `(b)`, and `a(` finds no pair within the window.
        assert_eq!(v.text().len(), k + MIB + 2, "k = {k}");
    }
}

#[test]
fn a_char_across_the_windows_end_doesnt_let_the_scan_past_it() {
    // The window ends inside `é` or `中`, and `)` comes right after it.
    for wide in ["é", "中"] {
        let text = format!("(|b{}{wide})", "a".repeat(MIB - 2));
        assert!(!di_paren_deletes(&text), "{wide}");
        let text = format!("(|b{}{wide})", "a".repeat(MIB - 3));
        assert!(!di_paren_deletes(&text), "{wide}, one byte in");
    }
    // A pair that fits stays a pair.
    let text = format!("(|b{}é)", "a".repeat(MIB - 4));
    assert!(di_paren_deletes(&text));
}

#[test]
fn a_char_across_the_windows_start_doesnt_let_the_scan_past_it() {
    // The window starts inside `é` or `中`, and `(` comes right before it.
    for wide in ["é", "中"] {
        for a in [MIB - 2, MIB - 1] {
            let text = format!("({wide}{}|b)", "a".repeat(a));
            assert!(!di_paren_deletes(&text), "{wide}, {a}");
        }
    }
    // A pair that fits stays a pair.
    let text = format!("(é{}|b)", "a".repeat(MIB - 3));
    assert!(di_paren_deletes(&text));
}

#[test]
fn a_backslash_ending_the_line_before_escapes_nothing() {
    table(&[("a\\\n\"|b\"", "di\"", "a\\\n\"|\"")]);
}
