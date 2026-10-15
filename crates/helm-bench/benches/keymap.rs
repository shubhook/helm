//! Key resolution benchmark for `helm-editor`, gated by the `bench` CI job
//! against `budgets.toml`. NF-1 gives a keypress 5 ms from input to pixel,
//! and resolving the key to a command is the first step.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use helm_editor::{
    Binding, CommandRegistry, Dispatcher, KeyEvent, Keymap, Keystroke, Mode, Resolution, Typing,
};

/// The chars bound with modifiers, as chord tails, and in normal mode.
const CHARS: &str = "abcdefghijklmnopqrstuvwxyz0123456789";

/// The named keys bound in insert mode with the `editor` context.
const NAMED: [&str; 12] = [
    "enter",
    "tab",
    "backspace",
    "delete",
    "escape",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
];

/// How many times the 100-event list repeats per iteration.
const REPEATS: usize = 100;

/// The two-mode keymap with the 300 bindings the task file lists, each
/// running a distinct command id.
fn build_keymap() -> Keymap {
    let mut keymap = Keymap::new(
        "bench",
        vec![
            Mode::new("insert", Typing::Insert),
            Mode::new("normal", Typing::Ignore),
        ],
    )
    .expect("distinct modes");
    let mut bindings = Vec::new();
    for prefix in ["ctrl-", "alt-", "ctrl-alt-", "ctrl-cmd-"] {
        for c in CHARS.chars() {
            bindings.push(format!("{prefix}{c}"));
        }
    }
    for chord in ["cmd-k", "cmd-j"] {
        for c in CHARS.chars() {
            bindings.push(format!("{chord} {c}"));
        }
    }
    let mut count = 0;
    let mut add = |keymap: &mut Keymap, binding: Binding| {
        keymap.bind(binding).expect("modes exist");
        count += 1;
    };
    for (i, keys) in bindings.iter().enumerate() {
        add(
            &mut keymap,
            Binding::run(keys, format!("bench.c{i}")).expect("valid keys"),
        );
    }
    for (i, c) in CHARS.chars().enumerate() {
        let binding = Binding::run(&c.to_string(), format!("bench.normal{i}")).expect("valid keys");
        add(&mut keymap, binding.in_mode("normal"));
    }
    for (i, name) in NAMED.iter().enumerate() {
        for (j, prefix) in ["", "shift-", "alt-", "cmd-"].iter().enumerate() {
            let binding = Binding::run(&format!("{prefix}{name}"), format!("bench.named{i}_{j}"))
                .expect("valid keys");
            add(&mut keymap, binding.in_mode("insert").when("editor"));
        }
    }
    assert_eq!(count, 300);
    keymap
}

/// The 100 events the task file lists: 70 typed chars, 10 bound single
/// keystrokes, 5 two-key chords and 10 bound named keys, interleaved.
fn build_events() -> Vec<KeyEvent> {
    let parse = |text: &str| KeyEvent::new(Keystroke::parse(text).expect("valid keystroke"));
    let typed: Vec<char> = CHARS.chars().chain([' ']).collect();
    let mut events = Vec::with_capacity(100);
    for i in 0..10 {
        for j in 0..7 {
            let c = typed[(i * 7 + j) % typed.len()];
            events.push(if c == ' ' {
                parse("space")
            } else {
                parse(&c.to_string())
            });
        }
        let c = CHARS.as_bytes()[i * 3] as char;
        let prefix = ["ctrl-", "alt-", "ctrl-alt-", "ctrl-cmd-"][i % 4];
        events.push(parse(&format!("{prefix}{c}")));
        let prefix = ["", "shift-", "alt-", "cmd-"][i % 4];
        events.push(parse(&format!("{prefix}{}", NAMED[i])));
        if i % 2 == 0 {
            events.push(parse(["cmd-k", "cmd-j"][i / 2 % 2]));
            events.push(parse(&(CHARS.as_bytes()[i + 10] as char).to_string()));
        }
    }
    assert_eq!(events.len(), 100);
    events
}

fn keymap(c: &mut Criterion) {
    let dispatcher = Dispatcher::new(CommandRegistry::new(), build_keymap());
    let mut state = dispatcher.key_state();
    let events = build_events();
    let contexts = ["editor"];

    // Check once, outside the timed code, that every event resolves as the
    // task file says: nothing left pending, and no key unbound.
    for event in &events {
        let resolution = dispatcher.resolve(&mut state, event, &contexts);
        assert!(
            matches!(resolution, Resolution::Run(_) | Resolution::Pending),
            "{:?} resolved to {resolution:?}",
            event.keystroke
        );
    }
    assert!(state.pending().is_empty());

    let mut group = c.benchmark_group("keymap");
    group.bench_function("resolve_10k_keys_300_bindings", |b| {
        b.iter(|| {
            for _ in 0..REPEATS {
                for event in &events {
                    black_box(dispatcher.resolve(&mut state, black_box(event), &contexts));
                }
            }
        })
    });
    group.finish();
}

criterion_group!(benches, keymap);
criterion_main!(benches);
