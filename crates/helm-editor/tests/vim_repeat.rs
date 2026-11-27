//! T-109 acceptance tests: Vim's `.` repeat and counts on `i`, `a`, `I`,
//! `A`, `o` and `O`, typed key by key into an `Editor` with a `Session`
//! (ED-5).
//!
//! Markers and key notation are as in `vim_harness`.

mod vim_harness;

use helm_editor::{CommandArgs, CommandId, vim, vim_commands};
use vim_harness::{Vim, keymap, table};

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_repeats_one_key_edits_and_operators() {
    table(&[
        ("|abcd", "x.", "|cd"),
        ("|abcdef", "2x.", "|ef"),
        ("|a b c d", "dw.", "|c d"),
        ("|a b c d e f g h i j k l m", "2d3w.", "|m"),
        ("|a\nb\nc", "dd.", "|c"),
        ("|ab\ncd", "Dj0.", "\n|"),
        ("ab|c", "X.", "|c"),
        ("|a b c d", "daw.", "|c d"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_with_a_count_replaces_the_recorded_count() {
    table(&[
        ("|abcdef", "2x3.", "|f"),
        ("|abcdefgh", "2x3..", "|"),
        ("|a b c d e f", "dw2.", "|d e f"),
        ("|a b c d e f g", "2dw.", "|e f g"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_repeats_a_change_with_its_typing() {
    table(&[
        ("|ab cd", "cwX<esc>w.", "X |X"),
        ("|ab cd", "ciwXY<esc>w.", "XY X|Y"),
        ("|a\nb", "A!<esc>j.", "a!\nb|!"),
        ("|a\nb", "ox<esc>j.", "a\nx\nb\n|x"),
        ("|a\nb", "Ox<esc>j.", "x\n|x\na\nb"),
        ("|ab", "ix<esc>.", "|xxab"),
        ("|ab", "ax<esc>.", "ax|xb"),
        ("|ab cd", "cwX<cr>Y<esc>w.", "X\nY X\n|Y"),
        ("|abc", "sX<esc>l.", "X|Xc"),
        ("|ab\ncd", "SX<esc>j.", "X\n|X"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_repeats_a_put() {
    table(&[
        ("|ab", "ylp.", "aa|ab"),
        ("|ab cd", "yiwwP.", "ab aa|bbcd"),
        ("|a\nb\nc\nd", "dddddd\"1p..", "d\nc\nb\n|a"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn each_dot_is_one_undo_step() {
    table(&[
        ("|abcd", "x.u", "|bcd"),
        ("|ab cd", "cwX<esc>w.u", "X |cd"),
        ("|ab", "3ix<esc>.u", "xx|xab"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn motions_yanks_undo_and_redo_are_not_changes() {
    table(&[
        ("|ab cd", "xwyiw.", "b |d"),
        ("|abc", "xu.", "|bc"),
        ("|abc", "xu<c-r>.", "|c"),
        ("|abc", "xhjl.", "|b"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn a_visual_change_is_not_recorded() {
    // Vim repeats a visual change over the same amount of text. That is
    // left out of T-109 (open question 4), so `.` repeats the change
    // before it.
    table(&[
        ("|abcdef", "xvld.", "|ef"),
        ("|ab\ncd\nef\ngh", "xVjd.", "|f\ngh"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_before_any_change_does_nothing() {
    table(&[("a|b", ".", "a|b"), ("a|b", "3.", "a|b")]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_works_at_every_cursor() {
    table(&[
        ("|ab\n|cd", "x.", "|\n|"),
        ("|ab x\n|cd y", "cwZ<esc>w.", "Z |Z\nZ |Z"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn the_last_change_is_the_recorded_invocations() {
    let mut v = Vim::new("|ab cd");
    v.keys("2d3w");
    let change = v.session.last_change().expect("a change");
    assert_eq!(change.len(), 1);
    assert_eq!(change[0].command, CommandId::from(vim_commands::DELETE));
    assert_eq!(
        change[0].args.motion,
        Some(CommandId::from(vim_commands::NEXT_WORD))
    );
    assert_eq!(change[0].args.count, Some(6));

    let mut v = Vim::new("|ab");
    v.keys("cwXY<esc>");
    let change = v.session.last_change().expect("a change");
    let ids: Vec<&str> = change.iter().map(|i| i.command.as_str()).collect();
    assert_eq!(
        ids,
        [
            vim_commands::CHANGE,
            "editor.insert",
            "editor.insert",
            vim_commands::NORMAL
        ]
    );
    assert_eq!(change[1].args.text.as_deref(), Some("X"));

    // `3.` keeps the new count for the next `.`.
    let mut v = Vim::new("|abcdefgh");
    v.keys("x3.");
    assert_eq!(v.session.last_change().unwrap()[0].args.count, Some(3));

    // While a change is still being typed, the last change is the one
    // before it.
    let mut v = Vim::new("|ab");
    v.keys("xiQ");
    let change = v.session.last_change().unwrap();
    assert_eq!(change[0].command.as_str(), vim_commands::DELETE_CHAR);
    v.keys("<esc>");
    assert_eq!(
        v.session.last_change().unwrap()[0].command.as_str(),
        vim_commands::INSERT
    );
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn dot_repeats_in_another_editor_on_the_same_session() {
    let mut a = Vim::new("|ab cd");
    a.keys("cwX<esc>");
    let mut b = Vim::new("|ef gh");
    b.session = std::mem::take(&mut a.session);
    b.keys(".");
    assert_eq!(b.show(), "|X gh");
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn a_change_ended_by_a_closed_group_still_repeats_and_ends_in_normal_mode() {
    // What `helm-ui` does on a keymap switch: close every open group.
    let mut v = Vim::new("|ab");
    v.keys("iQ");
    v.editor.end_undo_group();
    v.dispatcher.set_keymap(keymap());
    v.dispatcher.sync(&mut v.state);
    assert_eq!(v.mode(), vim::INSERT);
    v.keys("<esc>");
    assert_eq!(v.show(), "|Qab");
    v.keys("l.");
    assert_eq!(v.mode(), vim::NORMAL);
    assert!(!v.editor.in_undo_group());
    assert_eq!(v.text(), "QQab");
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn counted_inserts_repeat_the_typing() {
    table(&[
        ("|ab", "3ix<esc>", "xx|xab"),
        ("|ab", "3ax<esc>", "axx|xb"),
        ("a|b", "3Ix<esc>", "xx|xab"),
        ("a|b", "3Ax<esc>", "abxx|x"),
        ("|a", "3ox<esc>", "a\nx\nx\n|x"),
        ("|a", "3Ox<esc>", "x\nx\n|x\na"),
        ("|ab", "2ix<cr>y<esc>", "x\nyx\n|yab"),
        ("|a\r\nb", "2ox<esc>", "a\r\nx\r\n|x\r\nb"),
        ("|ab", "3ixy<bs><esc>", "xx|xab"),
        ("|ab", "3i<esc>", "|ab"),
        ("|ab", "3ix<esc>u", "|ab"),
        ("|ab", "3ix<esc>.", "xxxx|xxab"),
        ("|ab", "3ix<esc>2.", "xxx|xxab"),
        ("|ab\n|cd", "2ix<esc>", "x|xab\nx|xcd"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn c_s_and_their_forms_keep_t_108s_count() {
    table(&[
        ("|abc", "2sX<esc>", "|Xc"),
        ("|a b c", "2cwX<esc>", "|X c"),
        ("|ab\ncd\nef", "2SX<esc>", "|X\nef"),
        ("|ab\ncd\nef", "2CX<esc>", "|X\nef"),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn replaying_whole_invocations_leaves_room_for_more_arguments() {
    // `.` copies every `CommandArgs` field it recorded, so a field added
    // later (the char `f`, `t` and `r` will need) needs no change to `.`.
    // Here a recorded invocation with a text argument is replayed as is.
    let mut v = Vim::new("|ab");
    v.keys("ix<esc>");
    let recorded = v.session.last_change().unwrap().to_vec();
    v.keys("l.");
    let again = v.session.last_change().unwrap().to_vec();
    assert_eq!(recorded, again);
    assert_eq!(
        again[1].args,
        CommandArgs {
            text: Some("x".to_owned()),
            ..CommandArgs::default()
        }
    );
}
