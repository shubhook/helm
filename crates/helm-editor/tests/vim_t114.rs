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
