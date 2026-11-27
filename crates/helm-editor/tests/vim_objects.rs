//! T-109 acceptance tests: Vim's text objects under `d`, `c` and `y`, and
//! in both visual modes, typed key by key into an `Editor` (ED-5).
//!
//! Markers and key notation are as in `vim_harness`. Every row of a
//! `table` ends in normal mode.

mod vim_harness;

use vim_harness::{Vim, reg_table, table};

#[test]
#[ignore = "T-109: not implemented yet"]
fn word_objects_under_d() {
    table(&[
        ("foo b|ar baz", "diw", "foo | baz"),
        ("foo b|ar baz", "daw", "foo |baz"),
        ("foo bar b|az", "daw", "foo ba|r"),
        ("foo|   bar", "diw", "foo|bar"),
        ("foo|   bar", "daw", "fo|o"),
        ("f|oo.bar", "diw", "|.bar"),
        ("foo|.bar", "diw", "foo|bar"),
        ("foo.b|ar", "diW", "|"),
        ("a foo.b|ar b", "daW", "a |b"),
        ("a foo.b|ar b", "diW", "a | b"),
        ("|foo\nbar", "diw", "|\nbar"),
        ("fo|o\nbar", "daw", "|\nbar"),
        ("x fo|o\nbar", "daw", "|x\nbar"),
        ("中|文 x", "diw", "| x"),
        ("é|t 😀 x", "daw", "|😀 x"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn word_objects_with_counts() {
    table(&[
        ("foo b|ar baz qux", "d3iw", "foo | qux"),
        ("foo b|ar baz qux", "d2aw", "foo |qux"),
        ("foo b|ar baz qux", "2daw", "foo |qux"),
        ("|a b c d e f", "2d2aw", "|e f"),
        ("foo b|ar", "d5iw", "foo b|ar"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn bracket_objects_under_d() {
    table(&[
        ("f(a, |b)", "di(", "f(|)"),
        ("f(a, |b)", "da(", "|f"),
        ("f(a, |b) x", "da)", "f| x"),
        ("f(a, |b)", "dib", "f(|)"),
        ("f(a, |b)", "dab", "|f"),
        ("f|(a)", "di(", "f(|)"),
        ("f(a|)", "di(", "f(|)"),
        ("(a (b|) c)", "di(", "(a (|) c)"),
        ("(a (b|) c)", "d2i(", "(|)"),
        ("(a (b|) c)", "2di(", "(|)"),
        ("(a (b) |c)", "di)", "(|)"),
        ("(|)", "di(", "(|)"),
        ("a |b", "di(", "a |b"),
        ("a |b)", "di(", "a |b)"),
        ("x[1, |2]y", "di[", "x[|]y"),
        ("x[1, |2]y", "da]", "x|y"),
        ("x{ |a }y", "di{", "x{|}y"),
        ("x{ |a }y", "daB", "x|y"),
        ("x{ |a }y", "di}", "x{|}y"),
        ("x<|a>y", "di<lt>", "x<|>y"),
        ("x<|a>y", "da>", "x|y"),
        ("(a\n|b\nc)", "di(", "(|)"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn a_block_of_lines_keeps_its_brackets_on_their_own_lines() {
    table(&[
        ("fn {\n    |a\n    b\n}", "di{", "fn {\n|}"),
        ("fn {\n    |a\n}\nx", "di{", "fn {\n|}\nx"),
        ("fn {\n    |a\n}", "da{", "fn| "),
        ("fn {|\n    a\n}", "di{", "fn {\n|}"),
        ("{\n|a\n}", "ci{X<esc>", "{\n|X\n}"),
        ("(|a\n)", "di(", "|(\n)"),
        ("(|a\n  )", "di(", "|(\n  )"),
        ("(\n|a)", "di(", "(\n|)"),
        ("f(|a,\n  b\n)", "di(", "f|(\n)"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn quote_objects_under_d() {
    table(&[
        ("s = \"a|b\";", "di\"", "s = \"|\";"),
        ("s = \"a|b\";", "da\"", "s =|;"),
        ("s = \"a|b\" + x", "da\"", "s = |+ x"),
        ("|s = \"ab\";", "di\"", "s = \"|\";"),
        ("s = |\"ab\";", "di\"", "s = \"|\";"),
        ("s = \"ab|\";", "di\"", "s = \"|\";"),
        ("s = \"a\\\"|b\";", "di\"", "s = \"|\";"),
        ("x = 'a|b'", "di'", "x = '|'"),
        ("x = `a|b`", "di`", "x = `|`"),
        ("\"a\" |x \"b\"", "di\"", "\"a\"|\"b\""),
        ("a|b", "di\"", "a|b"),
        ("\"a\"\n|b \"c\"", "di\"", "\"a\"\nb \"|\""),
        ("s = \"a|b\";", "d2i\"", "s = |;"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn paragraph_objects_under_d() {
    table(&[
        ("a\n|b\n\nc", "dip", "|\nc"),
        ("a\n|b\n\nc", "dap", "|c"),
        ("a\n\n|c", "dap", "|a"),
        ("a\n|\n\nb", "dip", "a\n|b"),
        ("a\n|\n\nb", "dap", "|a"),
        ("a\n  \n|\nb", "dip", "a\n|b"),
        ("|a\n\nb\n\nc", "d2ap", "|c"),
        ("|a\n\nb\n\nc", "d3ip", "|\nc"),
        ("|a\r\nb\r\n\r\nc", "dap", "|c"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_under_c_enter_insert_mode_in_one_undo_step() {
    table(&[
        ("foo b|ar baz", "ciwX<esc>", "foo |X baz"),
        ("foo b|ar baz", "cawX<esc>", "foo |Xbaz"),
        ("f(a, |b)", "ci(x<esc>", "f(|x)"),
        ("s = \"a|b\";", "ci\"Z<esc>", "s = \"|Z\";"),
        ("a\n|b\n\nc", "cipX<esc>", "|X\n\nc"),
        ("a |b", "ci(X<esc>", "a |Xb"),
        ("foo b|ar baz", "ciwX<esc>u", "foo |bar baz"),
        ("f(a, |b)", "ci(x<esc>u", "f(|a, b)"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_under_y_yank_and_move_to_the_start() {
    reg_table(&[
        ("foo b|ar baz", "yiw", "foo |bar baz", '"', "bar", false),
        ("foo b|ar baz", "yaw", "foo |bar baz", '0', "bar ", false),
        ("f(a, |b)", "yi(", "f(|a, b)", '"', "a, b", false),
        ("f(a, |b)", "ya(", "f|(a, b)", '"', "(a, b)", false),
        ("s = \"a|b\";", "yi\"", "s = \"|ab\";", '"', "ab", false),
        ("a\n|b\n\nc", "yip", "|a\nb\n\nc", '"', "a\nb\n", true),
        ("a\n|b\n\nc", "yap", "|a\nb\n\nc", '"', "a\nb\n\n", true),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_in_visual_mode() {
    table(&[
        ("foo b|ar baz", "viwd", "foo | baz"),
        ("foo b|ar baz", "vawd", "foo |baz"),
        ("foo b|ar baz", "viwiwd", "foo |baz"),
        ("foo b|ar baz", "vi(d", "foo b|r baz"),
        ("f(a, |b)", "vi(d", "f(|)"),
        ("f(a, |b)", "va(d", "|f"),
        ("s = \"a|b\";", "vi\"d", "s = \"|\";"),
        ("a\n|b\n\nc", "vipd", "|\nc"),
        ("a\n|b\n\nc", "vapd", "|c"),
        ("|ab cd", "Viwd", "| cd"),
        ("foo b|ar baz", "viwy", "foo |bar baz"),
        ("foo b|ar baz", "viwc!<esc>", "foo |! baz"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_switch_between_the_visual_modes() {
    let mut v = Vim::new("a\n|b\n\nc");
    v.keys("vip");
    assert_eq!(v.mode(), "visual_line");
    let mut v = Vim::new("|ab cd");
    v.keys("Viw");
    assert_eq!(v.mode(), "visual");
    assert_eq!(v.show(), "^ab| cd");
    // `then_mode` switches even when the object finds nothing.
    let mut v = Vim::new("|ab cd");
    v.keys("Vi(");
    assert_eq!(v.mode(), "visual");
    assert_eq!(v.show(), "^a|b cd");
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_at_several_cursors() {
    table(&[
        ("|ab cd\n|ef gh", "diw", "| cd\n| gh"),
        ("f(|a) g(|b)", "di(", "f(|) g(|)"),
        ("a\n|b\n\n|c\nd", "dip", "|"),
        ("|a b\n|c d", "ciwX<esc>", "|X b\n|X d"),
    ]);
    let mut v = Vim::new("f(|a) g(|b)");
    v.keys("yi(");
    assert_eq!(
        v.reg('"'),
        Some((vec!["a".to_owned(), "b".to_owned()], false))
    );
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn objects_read_across_long_lines() {
    // Lines longer than a rope chunk, so the readers cross chunks.
    let long = "a".repeat(1500);
    let start = format!("x({long}|b{long})y");
    let mut v = Vim::new(&start);
    v.keys("di(");
    assert_eq!(v.show(), "x(|)y");

    let start = format!("s = \"{long}|b{long}\";");
    let mut v = Vim::new(&start);
    v.keys("di\"");
    assert_eq!(v.show(), "s = \"|\";");

    let start = format!("{long}{long}|b{long} z");
    let mut v = Vim::new(&start);
    v.keys("daw");
    assert_eq!(v.show(), "|z");

    let start = format!("{long}\n|{long}\n\n{long}");
    let mut v = Vim::new(&start);
    v.keys("dap");
    assert_eq!(v.show(), format!("|{long}"));
}
