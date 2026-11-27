//! T-109 acceptance tests: Vim's `:w`, `:q` and `:e`, typed key by key
//! into the command line, and the requests they hand to `helm-ui` (ED-5).
//!
//! Markers and key notation are as in `vim_harness`.

mod vim_harness;

use std::path::PathBuf;

use helm_editor::{CommandError, Request, Resolution, vim};
use vim_harness::Vim;

fn save(path: Option<&str>, force: bool) -> Request {
    Request::Save {
        path: path.map(PathBuf::from),
        force,
    }
}

fn close(force: bool) -> Request {
    Request::Close { force }
}

fn open(path: Option<&str>, force: bool) -> Request {
    Request::Open {
        path: path.map(PathBuf::from),
        force,
    }
}

/// Each row: the keys after `:`, before Enter, and the requests.
fn requests(rows: &[(&str, Vec<Request>)]) {
    for (line, expected) in rows {
        let mut v = Vim::new("a|b");
        v.keys(":");
        v.keys(line);
        v.keys("<cr>");
        assert_eq!(&v.requests, expected, ":{line}");
        assert_eq!(v.show(), "a|b", ":{line}");
        assert_eq!(v.mode(), vim::NORMAL, ":{line}");
    }
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn w_q_and_e_make_requests() {
    requests(&[
        ("w", vec![save(None, false)]),
        ("write", vec![save(None, false)]),
        ("wri", vec![save(None, false)]),
        ("w!", vec![save(None, true)]),
        ("w a.txt", vec![save(Some("a.txt"), false)]),
        ("w! a.txt", vec![save(Some("a.txt"), true)]),
        ("w my file.txt ", vec![save(Some("my file.txt"), false)]),
        ("q", vec![close(false)]),
        ("quit", vec![close(false)]),
        ("q!", vec![close(true)]),
        ("e", vec![open(None, false)]),
        ("edit", vec![open(None, false)]),
        ("e!", vec![open(None, true)]),
        ("e src/main.rs", vec![open(Some("src/main.rs"), false)]),
        ("e! b.rs", vec![open(Some("b.rs"), true)]),
        ("wq", vec![save(None, false), close(false)]),
        ("wq!", vec![save(None, true), close(false)]),
        ("wq c.txt", vec![save(Some("c.txt"), false), close(false)]),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn leading_colons_and_white_space_are_skipped() {
    requests(&[
        ("  w", vec![save(None, false)]),
        (":w", vec![save(None, false)]),
        (": q", vec![close(false)]),
        ("w   a.txt", vec![save(Some("a.txt"), false)]),
        ("", vec![]),
        ("   ", vec![]),
    ]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn lines_helm_doesnt_run_are_errors() {
    for (line, error) in [
        ("foo", vim::ExError::NotACommand),
        ("wx", vim::ExError::NotACommand),
        ("x", vim::ExError::NotACommand),
        ("writex", vim::ExError::NotACommand),
        ("q file", vim::ExError::TrailingCharacters),
        ("q! x", vim::ExError::TrailingCharacters),
        ("1,3w", vim::ExError::Range),
        ("%w", vim::ExError::Range),
    ] {
        let mut v = Vim::new("a|b");
        v.keys(":");
        v.keys(line);
        let result = v.try_keys("<cr>");
        assert_eq!(
            result,
            Err(CommandError::Ex {
                line: line.to_owned(),
                error
            }),
            ":{line}"
        );
        assert!(v.requests.is_empty(), ":{line}");
        assert_eq!(v.show(), "a|b", ":{line}");
        assert_eq!(v.mode(), vim::NORMAL, ":{line}");
    }
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn the_line_is_typed_into_the_key_state() {
    let mut v = Vim::new("a|b");
    assert_eq!(v.state.line(), None);
    assert_eq!(v.keys(":"), vec![Resolution::Done]);
    assert_eq!(v.mode(), vim::COMMAND_LINE);
    assert_eq!(v.state.line(), Some(""));
    assert_eq!(v.keys("wq"), vec![Resolution::Pending, Resolution::Pending]);
    assert_eq!(v.state.line(), Some("wq"));
    v.keys("<bs>");
    assert_eq!(v.state.line(), Some("w"));
    assert_eq!(v.text(), "ab");
    v.keys("<cr>");
    assert_eq!(v.state.line(), None);
    assert_eq!(v.requests, vec![save(None, false)]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn escape_and_backspace_leave_the_command_line() {
    let mut v = Vim::new("a|b");
    v.keys(":wq<esc>");
    assert_eq!(v.mode(), vim::NORMAL);
    assert_eq!(v.state.line(), None);
    assert!(v.requests.is_empty());
    // Coming back starts an empty line.
    v.keys(":");
    assert_eq!(v.state.line(), Some(""));

    let mut v = Vim::new("a|b");
    v.keys(":w<bs>");
    assert_eq!(v.mode(), vim::COMMAND_LINE);
    assert_eq!(v.keys("<bs>"), vec![Resolution::Done]);
    assert_eq!(v.mode(), vim::NORMAL);
    assert_eq!(v.state.line(), None);
    // Normal-mode keys work again.
    v.keys("x");
    assert_eq!(v.show(), "|a");
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn a_count_before_the_colon_is_dropped() {
    let mut v = Vim::new("a|b");
    v.keys("3:");
    assert_eq!(v.state.count(), None);
    v.keys("q<cr>");
    assert_eq!(v.requests, vec![close(false)]);
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn commands_on_the_line_are_not_changes() {
    let mut v = Vim::new("|abc");
    v.keys("x:w<cr>.");
    assert_eq!(v.show(), "|c");
}

#[test]
#[ignore = "T-109: not implemented yet"]
fn a_keymap_switch_or_cancel_leaves_the_command_line() {
    // As with a pending operator, the state goes back to the mode the line
    // was entered from, and the line is dropped.
    let mut v = Vim::new("a|b");
    v.keys(":wq");
    v.dispatcher.set_keymap(vim_harness::keymap());
    v.dispatcher.sync(&mut v.state);
    assert_eq!(v.state.line(), None);
    assert_eq!(v.mode(), vim::NORMAL);
    v.keys("<cr>");
    assert!(v.requests.is_empty());

    let mut v = Vim::new("a|b");
    v.keys(":q");
    assert!(v.state.cancel());
    assert_eq!(v.state.line(), None);
    assert_eq!(v.mode(), vim::NORMAL);
    assert!(!v.state.cancel());
}
