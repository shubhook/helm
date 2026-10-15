//! Turning key events into commands: the active keymap, each editor's key
//! state, and the rule that picks a binding.

use helm_text::BufferSet;

use crate::{
    Action, Binding, CommandArgs, CommandError, CommandId, CommandRegistry, Editor, Invocation,
    KeyEvent, Keymap, Keystroke, ModeId, Typing,
};

/// One editor's input state: its mode, the keys of a chord typed so far,
/// and a pending operator.
///
/// Each editor has its own, so a chord half typed in one pane doesn't carry
/// over to another. A state belongs to the [`Dispatcher`] that made it with
/// [`Dispatcher::key_state`]. Handing it to another dispatcher is a logic
/// error: keys may resolve in the wrong mode, but nothing panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyState {
    mode: ModeId,
    /// The keys of a chord typed so far. Empty when no chord is pending.
    pending: Vec<Keystroke>,
    /// The pending operator, and the mode it was set in.
    operator: Option<(CommandId, ModeId)>,
    /// The dispatcher's keymap generation this state last synced to.
    generation: u64,
}

impl KeyState {
    /// The current mode.
    pub fn mode(&self) -> &ModeId {
        &self.mode
    }

    /// The keys of the chord typed so far, or an empty slice if no chord is
    /// pending. `helm-ui` shows these while it waits for the next key.
    pub fn pending(&self) -> &[Keystroke] {
        &self.pending
    }

    /// The pending operator, if one is held.
    pub fn operator(&self) -> Option<&CommandId> {
        self.operator.as_ref().map(|(command, _)| command)
    }

    /// Drops the pending chord and the pending operator, and returns to the
    /// mode the operator was set in, if one was held. Returns whether
    /// anything was pending. `helm-ui` calls this when the editor loses
    /// focus or the user clicks in it.
    pub fn cancel(&mut self) -> bool {
        let chord = !self.pending.is_empty();
        self.pending.clear();
        match self.operator.take() {
            Some((_, mode)) => {
                self.mode = mode;
                true
            }
            None => chord,
        }
    }
}

/// What a key event resolved to. The [`KeyState`] has already changed to
/// match when [`Dispatcher::resolve`] returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// The key started or continued a chord, or set a pending operator.
    /// Nothing runs yet.
    Pending,
    /// Run this command. Typed text is an invocation of
    /// [`editor_commands::INSERT`](crate::editor_commands::INSERT).
    Run(Invocation),
    /// A binding matched that runs nothing: it only switched mode, or it
    /// has no action.
    Done,
    /// No binding matched, and the key types nothing in this mode.
    Unbound,
    /// The key broke a pending chord or cancelled a pending operator.
    /// Nothing runs, and the key itself is dropped.
    Cancelled,
}

/// The active keymap and the command registry: what every editor's keys
/// go through.
///
/// # Switching keymaps (ED-6)
///
/// The app holds one dispatcher, and every editor resolves its keys through
/// it with its own [`KeyState`]. [`set_keymap`](Self::set_keymap) replaces
/// the keymap, so every open editor uses the new one from its next key, with
/// no restart and no list of editors to update. Each state notices the
/// switch the next time it is passed to [`resolve`](Self::resolve) or
/// [`sync`](Self::sync), and then:
///
/// - it cancels as [`KeyState::cancel`] does: it drops its pending chord
///   and pending operator, running nothing, and goes back to the mode the
///   operator was set in, if one was held;
/// - then it keeps its mode if the new keymap has a mode with the same id,
///   and otherwise goes to the new keymap's initial mode.
///
/// The key that arrives after a switch is resolved in the new keymap as if
/// nothing had been pending. Reloading the same keymap after an edit to the
/// keybindings file (ST-1) is a switch too, so a Vim user in insert mode
/// stays in insert mode.
#[derive(Clone, Debug)]
pub struct Dispatcher {
    registry: CommandRegistry,
    keymap: Keymap,
    /// Goes up by one on every `set_keymap`.
    generation: u64,
}

impl Dispatcher {
    /// A dispatcher running commands from `registry`, with `keymap` active.
    pub fn new(registry: CommandRegistry, keymap: Keymap) -> Self {
        Self {
            registry,
            keymap,
            generation: 0,
        }
    }

    /// The command registry.
    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    /// The active keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Makes `keymap` the active keymap and returns the old one. See the
    /// type docs for what happens to each editor's state.
    pub fn set_keymap(&mut self, keymap: Keymap) -> Keymap {
        self.generation += 1;
        std::mem::replace(&mut self.keymap, keymap)
    }

    /// A new state for one editor: in the active keymap's initial mode,
    /// with nothing pending.
    pub fn key_state(&self) -> KeyState {
        KeyState {
            mode: self.keymap.initial_mode().clone(),
            pending: Vec::new(),
            operator: None,
            generation: self.generation,
        }
    }

    /// Brings `state` up to date with the active keymap, as the type docs
    /// describe, if the keymap changed since `state` last saw it. `helm-ui`
    /// calls this on every open editor after a switch, so a status bar
    /// shows the right mode before the next key.
    pub fn sync(&self, state: &mut KeyState) {
        if state.generation == self.generation {
            return;
        }
        state.cancel();
        if self.keymap.mode(state.mode.as_str()).is_none() {
            state.mode = self.keymap.initial_mode().clone();
        }
        state.generation = self.generation;
    }

    /// Resolves one key event against the active keymap, updates `state`,
    /// and says what to run. `contexts` are the context names that hold
    /// right now.
    ///
    /// First `state` syncs, as [`sync`](Self::sync) does. Then, with `M` the
    /// state's mode and `S` the pending keys followed by the event's
    /// keystroke, a binding *applies* when its mode is `None` or `M` and
    /// its context is `None` or one of `contexts`. The first rule that fits
    /// decides:
    ///
    /// 1. **Chord prefix.** If `S` is a strict prefix of an applying
    ///    binding's keys, `S` becomes the pending chord: `Pending`. This
    ///    holds even when another applying binding's keys equal `S`. A
    ///    chord waits for its next key with no timeout.
    /// 2. **Match.** If applying bindings' keys equal `S`, one wins: the
    ///    one with a mode over one without, then the one with a context
    ///    over one without, then the one added later. The pending chord
    ///    clears, and the winner acts:
    ///    - [`Action::Run`](crate::Action::Run) with no operator pending:
    ///      `Run` of the command.
    ///    - `Action::Run` with an operator pending: `Run` of the operator,
    ///      with the binding's command as its
    ///      [`motion`](crate::CommandArgs::motion). The operator clears and
    ///      the mode goes back to the one the operator was set in.
    ///    - [`Action::Operator`](crate::Action::Operator) with no operator
    ///      pending: the command becomes the pending operator, set in mode
    ///      `M`: `Pending`.
    ///    - `Action::Operator` with an operator pending: the pending one is
    ///      cancelled as in rule 3: `Cancelled`. The binding's `then_mode`
    ///      doesn't apply.
    ///    - No action: a pending operator is cancelled, without returning
    ///      `Cancelled`: `Done`.
    ///
    ///    Then, except where noted, the binding's `then_mode`, if any,
    ///    becomes the mode, after any return to the operator's mode.
    /// 3. **Broken chord or operator.** If a chord or an operator is
    ///    pending, both clear, the mode goes back to the one the operator
    ///    was set in, if one was held, and the key is dropped:
    ///    `Cancelled`. So Escape cancels anything pending unless a binding
    ///    says otherwise.
    /// 4. **Typing.** If mode `M`'s [`Typing`](crate::Typing) is `Insert`,
    ///    the event has non-empty [`text`](KeyEvent::text), and neither
    ///    Ctrl nor Cmd is held: `Run` of [`Invocation::insert`] with the
    ///    text. Typing takes the same path as a bound command.
    /// 5. Otherwise `Unbound`.
    ///
    /// Mode changes happen here, before the caller runs the returned
    /// command, and stand even if the command then fails.
    pub fn resolve(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str]) -> Resolution {
        self.sync(state);
        let keystroke = event.keystroke;
        let depth = state.pending.len();
        let first = state.pending.first().copied().unwrap_or(keystroke);

        // Rules 1 and 2: look for a longer binding, and rank the exact ones.
        let mut prefix = false;
        let mut winner: Option<((bool, bool), &Binding)> = None;
        for binding in self.keymap.bindings_starting_with(first) {
            let keys = binding.keys.as_slice();
            if keys.len() <= depth
                || keys[depth] != keystroke
                || keys[..depth] != state.pending[..]
                || !applies(binding, &state.mode, contexts)
            {
                continue;
            }
            if keys.len() > depth + 1 {
                prefix = true;
                break;
            }
            // Later bindings come later, so `>=` lets one of equal rank win.
            let rank = (binding.mode.is_some(), binding.context.is_some());
            if winner.is_none_or(|(best, _)| rank >= best) {
                winner = Some((rank, binding));
            }
        }

        if prefix {
            state.pending.push(keystroke);
            return Resolution::Pending;
        }
        if let Some((_, binding)) = winner {
            state.pending.clear();
            let resolution = match &binding.action {
                Some(Action::Run(command)) => match state.operator.take() {
                    None => Resolution::Run(Invocation::new(command.clone())),
                    Some((operator, mode)) => {
                        state.mode = mode;
                        Resolution::Run(Invocation {
                            command: operator,
                            args: CommandArgs {
                                text: None,
                                motion: Some(command.clone()),
                            },
                        })
                    }
                },
                Some(Action::Operator(_)) if state.operator.is_some() => {
                    state.cancel();
                    return Resolution::Cancelled;
                }
                Some(Action::Operator(command)) => {
                    state.operator = Some((command.clone(), state.mode.clone()));
                    Resolution::Pending
                }
                None => {
                    state.cancel();
                    Resolution::Done
                }
            };
            if let Some(mode) = &binding.then_mode {
                state.mode = mode.clone();
            }
            return resolution;
        }

        // Rule 3.
        if state.cancel() {
            return Resolution::Cancelled;
        }

        // Rules 4 and 5.
        let modifiers = keystroke.modifiers();
        let types = self
            .keymap
            .mode(state.mode.as_str())
            .is_some_and(|mode| mode.typing == Typing::Insert);
        match &event.text {
            Some(text) if types && !text.is_empty() && !modifiers.ctrl && !modifiers.cmd => {
                Resolution::Run(Invocation::insert(text.clone()))
            }
            _ => Resolution::Unbound,
        }
    }

    /// [`resolve`](Self::resolve), then, for [`Resolution::Run`], runs the
    /// invocation with [`CommandRegistry::run`] on `editor` and `set`.
    /// Returns the resolution, or the command's error.
    pub fn dispatch(
        &self,
        state: &mut KeyState,
        event: &KeyEvent,
        contexts: &[&str],
        editor: &mut Editor,
        set: &mut BufferSet,
    ) -> Result<Resolution, CommandError> {
        let resolution = self.resolve(state, event, contexts);
        if let Resolution::Run(invocation) = &resolution {
            self.registry.run(invocation, editor, set)?;
        }
        Ok(resolution)
    }
}

/// Whether `binding` applies in mode `mode` with `contexts` holding.
fn applies(binding: &Binding, mode: &ModeId, contexts: &[&str]) -> bool {
    binding.mode.as_ref().is_none_or(|m| m == mode)
        && binding
            .context
            .as_deref()
            .is_none_or(|context| contexts.contains(&context))
}
