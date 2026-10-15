//! T-106 acceptance tests: keystrokes in normal form, the key notation,
//! and the text a key event types.

use helm_editor::{Key, KeyEvent, KeyParseError, KeySequence, Keystroke, Modifiers};

/// `Keystroke::parse`, which must accept `text`.
fn ks(text: &str) -> Keystroke {
    Keystroke::parse(text).unwrap_or_else(|error| panic!("{text:?}: {error:?}"))
}

/// Modifiers from flags in the order ctrl, alt, shift, cmd.
fn mods(ctrl: bool, alt: bool, shift: bool, cmd: bool) -> Modifiers {
    Modifiers {
        ctrl,
        alt,
        shift,
        cmd,
    }
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn parses_modifiers_in_any_order() {
    let expected = Keystroke::new(Key::Char('k'), mods(true, true, false, true));
    for text in ["ctrl-alt-cmd-k", "cmd-alt-ctrl-k", "alt-cmd-ctrl-k"] {
        assert_eq!(ks(text), expected, "{text}");
    }
    assert_eq!(ks("cmd-s").key(), Key::Char('s'));
    assert_eq!(ks("cmd-s").modifiers(), Modifiers::CMD);
    assert_eq!(ks("x").modifiers(), Modifiers::NONE);
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn parses_every_key_name() {
    let names = [
        ("enter", Key::Enter),
        ("tab", Key::Tab),
        ("backspace", Key::Backspace),
        ("delete", Key::Delete),
        ("escape", Key::Escape),
        ("space", Key::Char(' ')),
        ("left", Key::Left),
        ("right", Key::Right),
        ("up", Key::Up),
        ("down", Key::Down),
        ("home", Key::Home),
        ("end", Key::End),
        ("pageup", Key::PageUp),
        ("pagedown", Key::PageDown),
        ("f1", Key::F(1)),
        ("f12", Key::F(12)),
        ("f20", Key::F(20)),
    ];
    for (name, key) in names {
        assert_eq!(ks(name), Keystroke::new(key, Modifiers::NONE), "{name}");
    }
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn single_chars_are_taken_as_they_are() {
    assert_eq!(ks("$").key(), Key::Char('$'));
    assert_eq!(ks("-").key(), Key::Char('-'));
    assert_eq!(ks("é").key(), Key::Char('é'));
    assert_eq!(ks("中").key(), Key::Char('中'));
    // The minus key with Cmd: the modifier's dash, then the key.
    assert_eq!(ks("cmd--"), Keystroke::new(Key::Char('-'), Modifiers::CMD));
    assert_eq!(
        ks("ctrl-alt--"),
        Keystroke::new(Key::Char('-'), mods(true, true, false, false))
    );
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn shift_folds_into_letters() {
    let upper = Keystroke::new(Key::Char('A'), Modifiers::NONE);
    assert_eq!(Keystroke::new(Key::Char('a'), Modifiers::SHIFT), upper);
    assert_eq!(Keystroke::new(Key::Char('A'), Modifiers::SHIFT), upper);
    assert_eq!(ks("shift-a"), upper);
    assert_eq!(ks("shift-A"), upper);
    assert_eq!(ks("A"), upper);
    assert!(!upper.modifiers().shift);

    // With other modifiers too.
    let cmd_p = Keystroke::new(Key::Char('P'), Modifiers::CMD);
    assert_eq!(ks("cmd-shift-p"), cmd_p);
    assert_eq!(ks("cmd-P"), cmd_p);
    assert_eq!(
        Keystroke::new(Key::Char('p'), mods(false, false, true, true)),
        cmd_p
    );

    // Lowercase and uppercase stay different keystrokes.
    assert_ne!(ks("a"), ks("A"));
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn shift_is_dropped_from_other_chars_and_kept_on_named_keys() {
    // helm-ui has already shifted the char.
    assert_eq!(
        Keystroke::new(Key::Char('$'), Modifiers::SHIFT),
        Keystroke::new(Key::Char('$'), Modifiers::NONE)
    );
    // Not an ASCII letter, so the char is kept as given.
    assert_eq!(
        Keystroke::new(Key::Char('é'), Modifiers::SHIFT),
        Keystroke::new(Key::Char('é'), Modifiers::NONE)
    );

    let shift_tab = ks("shift-tab");
    assert_eq!(shift_tab.key(), Key::Tab);
    assert!(shift_tab.modifiers().shift);
    assert_ne!(shift_tab, ks("tab"));
    assert_ne!(ks("shift-left"), ks("left"));
    assert_eq!(
        ks("cmd-shift-left"),
        Keystroke::new(Key::Left, mods(false, false, true, true))
    );
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn rejects_bad_keystrokes() {
    let cases = [
        ("cmd-", "missing"),
        ("ctrl-alt-", "missing"),
        ("cmd-cmd-k", "repeated"),
        ("shift-ctrl-shift-x", "repeated"),
        ("cmd-foo", "unknown"),
        ("Enter", "unknown"),
        ("Cmd-k", "unknown"),
        ("f0", "unknown"),
        ("f21", "unknown"),
        ("ab", "unknown"),
        ("\u{7}", "unknown"),
        ("shift-4", "shifted"),
        ("shift-$", "shifted"),
        ("shift-space", "shifted"),
        ("shift-é", "shifted"),
    ];
    for (text, kind) in cases {
        let keystroke = text.to_owned();
        let expected = match kind {
            "missing" => KeyParseError::MissingKey { keystroke },
            "repeated" => KeyParseError::RepeatedModifier { keystroke },
            "unknown" => KeyParseError::UnknownKey { keystroke },
            _ => KeyParseError::ShiftedChar { keystroke },
        };
        assert_eq!(Keystroke::parse(text), Err(expected), "{text:?}");
    }
    assert_eq!(Keystroke::parse(""), Err(KeyParseError::Empty));
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn display_writes_canonical_notation() {
    let cases = [
        ("cmd-alt-ctrl-k", "ctrl-alt-cmd-k"),
        ("cmd-shift-p", "cmd-P"),
        ("shift-a", "A"),
        ("shift-cmd-tab", "shift-cmd-tab"),
        ("space", "space"),
        ("ctrl-space", "ctrl-space"),
        ("cmd--", "cmd--"),
        ("f12", "f12"),
        ("pagedown", "pagedown"),
        ("中", "中"),
    ];
    for (text, written) in cases {
        assert_eq!(ks(text).to_string(), written, "{text}");
    }
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn display_round_trips_every_keystroke() {
    let keys = [
        Key::Char('a'),
        Key::Char('Z'),
        Key::Char(' '),
        Key::Char('-'),
        Key::Char('$'),
        Key::Char('😀'),
        Key::Enter,
        Key::Tab,
        Key::Backspace,
        Key::Delete,
        Key::Escape,
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::F(1),
        Key::F(20),
    ];
    for key in keys {
        for bits in 0..16u8 {
            let modifiers = mods(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let keystroke = Keystroke::new(key, modifiers);
            let written = keystroke.to_string();
            assert_eq!(Keystroke::parse(&written), Ok(keystroke), "{written:?}");
        }
    }
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn sequences_split_on_whitespace() {
    let chord = KeySequence::parse("cmd-k cmd-s").unwrap();
    assert_eq!(chord.as_slice(), &[ks("cmd-k"), ks("cmd-s")]);
    assert_eq!(chord.to_string(), "cmd-k cmd-s");

    let spaced = KeySequence::parse("  g \t g\n").unwrap();
    assert_eq!(spaced.as_slice(), &[ks("g"), ks("g")]);
    assert_eq!(spaced.to_string(), "g g");

    assert_eq!(KeySequence::from(ks("x")).as_slice(), &[ks("x")]);
    assert_eq!(
        KeySequence::new(vec![ks("a"), ks("b")]),
        Some(KeySequence::parse("a b").unwrap())
    );
    assert_eq!(KeySequence::new(Vec::new()), None);
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn sequences_reject_empty_text_and_bad_keystrokes() {
    assert_eq!(KeySequence::parse(""), Err(KeyParseError::Empty));
    assert_eq!(KeySequence::parse(" \t "), Err(KeyParseError::Empty));
    assert_eq!(
        KeySequence::parse("cmd-k nope"),
        Err(KeyParseError::UnknownKey {
            keystroke: "nope".to_owned()
        })
    );
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn plain_keys_type_their_char() {
    let typed = |text: &str| KeyEvent::parse(text).unwrap().text;
    assert_eq!(typed("a"), Some("a".to_owned()));
    assert_eq!(typed("A"), Some("A".to_owned()));
    assert_eq!(typed("shift-a"), Some("A".to_owned()));
    assert_eq!(typed("space"), Some(" ".to_owned()));
    assert_eq!(typed("😀"), Some("😀".to_owned()));

    for text in [
        "ctrl-a",
        "alt-a",
        "cmd-a",
        "enter",
        "tab",
        "shift-tab",
        "escape",
        "f1",
    ] {
        assert_eq!(typed(text), None, "{text}");
    }

    let event = KeyEvent::new(ks("x"));
    assert_eq!(event.keystroke, ks("x"));
    assert_eq!(event.text.as_deref(), Some("x"));
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn with_text_overrides_the_text() {
    // Option+A on macOS: the keystroke alt-a, typing å.
    let event = KeyEvent::with_text(ks("alt-a"), "å");
    assert_eq!(event.keystroke, ks("alt-a"));
    assert_eq!(event.text.as_deref(), Some("å"));
}
