//! Text input between GPUI's `InputHandler` and the ported Flutter `TextInputPlugin` (Java). Both count in
//! UTF-16 code units, so offsets pass through unchanged. Pure: the shell in `host.rs` reads and writes GPUI.
//!
//! The sync protocol (docs/android-text-input.md):
//! - **Java → GPUI, synchronous.** After each IME batch edit Java sends its full editing state
//!   (`updateEditingState`); [`Session::java_edit`] carries it onto GPUI's state ([`merge`]) and [`edits`] turns
//!   the difference into `InputHandler` calls.
//! - **GPUI → Java, at the end of the frame** (and right after a key event): GPUI's notifications fold into one
//!   [`Pending`]; Java gets them as Flutter's `flutter/textinput` calls ([`Session::send`]), run outside GPUI's draw.
//! - **Race: merge, don't drop.** When Java edits while a GPUI change is still on its way, Java's edit is the
//!   difference from the state Java's mirror held ([`Session`]'s `mirror`), applied on top of GPUI's state.

use std::ops::Range;

use gpui::{
    Autocapitalize, AutofillHint, Bounds, FocusId, KeyboardType, Pixels, TextInputAction,
    TextInputConfiguration, TextInputStateChange,
};
use serde_json::{Value, json};
use slotmap::Key as _;

/// One side's editing state, as Flutter's `TextEditState`: text, selection and composing (marked) range.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EditingState {
    pub(crate) text: String,
    /// Flutter's `(selectionBase, selectionExtent)`; the extent comes first when the selection is reversed.
    pub(crate) selection: Option<(usize, usize)>,
    /// Never empty.
    pub(crate) composing: Option<Range<usize>>,
}

impl EditingState {
    /// Java's values, `-1` for none. Out-of-range values (an IME's bug) count as none.
    pub(crate) fn from_java(
        text: String,
        selection: (i32, i32),
        composing: (i32, i32),
    ) -> EditingState {
        let length = text.encode_utf16().count();
        let offset = |value: i32| usize::try_from(value).ok().filter(|&value| value <= length);
        let selection = match (offset(selection.0), offset(selection.1)) {
            (Some(base), Some(extent)) => Some((base, extent)),
            (None, _) | (_, None) => None,
        };
        let composing = match (offset(composing.0), offset(composing.1)) {
            (Some(start), Some(end)) if start < end => Some(start..end),
            (Some(_), Some(_)) | (None, _) | (_, None) => None,
        };
        EditingState {
            text,
            selection,
            composing,
        }
    }

    /// GPUI's values (or a merge's), clamped to the text: Java's `TextEditState` throws on an offset past the end,
    /// which would end the app from `doFrame`. A composing range that clamps to nothing is none.
    pub(crate) fn clamped(
        text: String,
        selection: Option<(usize, usize)>,
        composing: Option<Range<usize>>,
    ) -> EditingState {
        let length = text.encode_utf16().count();
        let clamp = |offset: usize| offset.min(length);
        EditingState {
            selection: selection.map(|(base, extent)| (clamp(base), clamp(extent))),
            composing: composing
                .map(|range| clamp(range.start)..clamp(range.end))
                .filter(|range| range.start < range.end),
            text,
        }
    }

    /// Flutter's `TextEditState` JSON (`TextEditState.fromJson`).
    fn to_json(&self) -> Value {
        let (base, extent) = self
            .selection
            .map_or((-1, -1), |(base, extent)| (base as i64, extent as i64));
        let (start, end) = self
            .composing
            .as_ref()
            .map_or((-1, -1), |range| (range.start as i64, range.end as i64));
        json!({
            "text": self.text,
            "selectionBase": base,
            "selectionExtent": extent,
            "composingBase": start,
            "composingExtent": end,
        })
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        self.selection
            .map(|(base, extent)| base.min(extent)..base.max(extent))
    }
}

/// One `InputHandler` call, UTF-16 ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Edit {
    /// `replace_text_in_range`: plain text; ends any composition, caret after the text.
    Replace { range: Range<usize>, text: String },
    /// `replace_and_mark_text_in_range`: the text becomes the composing range; `selected` is relative to it.
    Mark {
        range: Range<usize>,
        text: String,
        selected: Option<Range<usize>>,
    },
    /// `unmark_text`.
    Unmark,
    /// `set_selected_text_range`.
    Select(Range<usize>),
}

/// The calls that bring GPUI from `gpui` to the `java` state. A text change is one replace (a mark when the
/// new text is the composing range, as `setComposingText`); then the composing range and the selection are fixed
/// up only where the calls before left them different.
pub(crate) fn edits(gpui: &EditingState, java: &EditingState) -> Vec<Edit> {
    let old: Vec<u16> = gpui.text.encode_utf16().collect();
    let new: Vec<u16> = java.text.encode_utf16().collect();
    let mut edits = Vec::new();
    // What GPUI holds after the calls so far.
    let mut marked = gpui.composing.clone();
    let mut caret = gpui.selected_range();
    if old != new {
        let (mut replaced, mut inserted) = changed_span(&old, &new);
        // The composing range covers the change: replace all of it in one mark (setComposingText's shape).
        let covering = java
            .composing
            .clone()
            .filter(|composing| composing.start <= inserted.start && inserted.end <= composing.end);
        if let Some(composing) = &covering {
            replaced = composing.start..replaced.end + (composing.end - inserted.end);
            inserted = composing.clone();
        }
        let text = String::from_utf16_lossy(&new[inserted.clone()]);
        match covering {
            Some(composing) => {
                let (edit, after) = mark(java, composing.clone(), replaced, text);
                edits.push(edit);
                caret = Some(after);
                marked = Some(composing);
            }
            None => {
                edits.push(Edit::Replace {
                    range: replaced,
                    text,
                });
                marked = None;
                caret = Some(inserted.end..inserted.end);
            }
        }
    }
    if java.composing != marked {
        match java.composing.clone() {
            Some(composing) => {
                let text = String::from_utf16_lossy(&new[composing.clone()]);
                let (edit, after) = mark(java, composing.clone(), composing, text);
                edits.push(edit);
                caret = Some(after);
            }
            None => edits.push(Edit::Unmark),
        }
    }
    if let Some(selected) = java.selected_range()
        && caret.as_ref() != Some(&selected)
    {
        edits.push(Edit::Select(selected));
    }
    edits
}

/// The mark that makes `java`'s `composing` range out of `range` (replaced by `text`), and the caret GPUI holds
/// after it: Java's selection when it lies inside the composing range, else its end.
fn mark(
    java: &EditingState,
    composing: Range<usize>,
    range: Range<usize>,
    text: String,
) -> (Edit, Range<usize>) {
    let selected = relative(java.selected_range(), &composing);
    let caret = match &selected {
        Some(selected) => composing.start + selected.start..composing.start + selected.end,
        None => composing.end..composing.end,
    };
    let edit = Edit::Mark {
        range,
        text,
        selected,
    };
    (edit, caret)
}

/// Java's edit, made on `base` (the state Java's mirror held), carried onto `gpui` (which the app changed since):
/// the state GPUI takes. Each side's change is one span of `base` ([`changed_span`]); Java's span lands where it
/// is in `gpui`. An offset inside the span GPUI changed moves to that span's end, so GPUI's change stays whole and
/// Java's lands next to it. Java's selection and composing range move the same way. `gpui == base` gives `java`.
pub(crate) fn merge(base: &EditingState, java: &EditingState, gpui: &EditingState) -> EditingState {
    let base_units: Vec<u16> = base.text.encode_utf16().collect();
    let java_units: Vec<u16> = java.text.encode_utf16().collect();
    let gpui_units: Vec<u16> = gpui.text.encode_utf16().collect();
    let (java_old, java_new) = changed_span(&base_units, &java_units);
    let (gpui_old, gpui_new) = changed_span(&base_units, &gpui_units);
    // A `base` offset in `gpui`.
    let to_gpui = |offset: usize| {
        if offset <= gpui_old.start {
            offset
        } else if offset >= gpui_old.end {
            offset - gpui_old.end + gpui_new.end
        } else {
            gpui_new.end
        }
    };
    let start = to_gpui(java_old.start);
    let end = to_gpui(java_old.end).max(start);
    let inserted_end = start + java_new.len();
    // A `java` offset in the merged text: before Java's inserted text, inside it, or after it.
    let to_merged = |offset: usize| {
        if offset <= java_new.start {
            to_gpui(offset)
        } else if offset <= java_new.end {
            start + offset - java_new.start
        } else {
            inserted_end + to_gpui(offset - java_new.end + java_old.end).saturating_sub(end)
        }
    };
    let mut text = gpui_units[..start].to_vec();
    text.extend_from_slice(&java_units[java_new.clone()]);
    text.extend_from_slice(&gpui_units[end..]);
    EditingState::clamped(
        String::from_utf16_lossy(&text),
        java.selection
            .map(|(base, extent)| (to_merged(base), to_merged(extent))),
        java.composing
            .clone()
            .map(|range| to_merged(range.start)..to_merged(range.end)),
    )
}

/// Whether GPUI's state after Java's edit is news for Java: the app refused or changed the edit (a filter, a length
/// limit, a moved caret). What GPUI does not report is not news, so the IME is not restarted every keystroke (a
/// `setEditingState` that changes the composing range restarts it): a selection GPUI keeps forward (reversed only in
/// Java), and no selection or no composing range (a field without marked text).
pub(crate) fn differs(java: &EditingState, gpui: &EditingState) -> bool {
    java.text != gpui.text
        || gpui
            .selection
            .is_some_and(|_| gpui.selected_range() != java.selected_range())
        || gpui
            .composing
            .as_ref()
            .is_some_and(|composing| java.composing.as_ref() != Some(composing))
}

/// The changed span of `old` → `new`, as (`old` range, `new` range): what is left after the common prefix and
/// suffix, widened so it never splits a surrogate pair.
fn changed_span(old: &[u16], new: &[u16]) -> (Range<usize>, Range<usize>) {
    let is_high = |unit: u16| (0xD800..0xDC00).contains(&unit);
    let is_low = |unit: u16| (0xDC00..0xE000).contains(&unit);
    let mut prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    if prefix > 0 && is_high(old[prefix - 1]) {
        prefix -= 1;
    }
    let most = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(most)
        .take_while(|(a, b)| a == b)
        .count();
    if suffix > 0 && is_low(old[old.len() - suffix]) {
        suffix -= 1;
    }
    (prefix..old.len() - suffix, prefix..new.len() - suffix)
}

/// `selected` relative to `within`'s start, if `within` holds it.
fn relative(selected: Option<Range<usize>>, within: &Range<usize>) -> Option<Range<usize>> {
    selected
        .filter(|selected| within.start <= selected.start && selected.end <= within.end)
        .map(|selected| selected.start - within.start..selected.end - within.start)
}

/// A GPUI text-input notification, as the window receives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    /// `TextInputStateChange::FocusGained`, or the focused field changed (another `FocusId`).
    Focused,
    /// `TextInputStateChange::FocusLost`.
    Blurred,
    /// `SelectionChanged` or `ContentChanged`, from the app (not from Java's own edit).
    Changed,
    /// `set_text_input_configuration` while a field is focused.
    Configured,
    /// `show_soft_keyboard` (`Window::request_virtual_keyboard`).
    Show,
    /// `hide_soft_keyboard` (`Window::dismiss_virtual_keyboard`).
    Hide,
}

/// The text input client on the Java side (Flutter's `TextInput.setClient` / `clearClient`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Client {
    /// The same client; `sync`: send GPUI's editing state (`TextInput.setEditingState`).
    Keep { sync: bool },
    /// Set a new client with the current configuration, then GPUI's editing state (Java restarts the IME on it).
    Set,
    /// No client: nothing to sync.
    Clear,
}

/// The soft keyboard (Flutter's `TextInput.show` / `hide`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Keyboard {
    #[default]
    Keep,
    Show,
    Hide,
}

/// What Java must do at the end of this frame. Empty (the default) sends nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    client: Client,
    keyboard: Keyboard,
}

impl Default for Pending {
    fn default() -> Self {
        Pending {
            client: Client::Keep { sync: false },
            keyboard: Keyboard::Keep,
        }
    }
}

impl Pending {
    /// Folds one notice in. Focus replaces everything; later keyboard requests win.
    fn next(self, notice: Notice) -> Pending {
        let client = match (notice, self.client) {
            (Notice::Focused, _) => Client::Set,
            (Notice::Blurred, _) => Client::Clear,
            // A cleared client has no state to sync or configure; a new one sends the state anyway.
            (Notice::Changed | Notice::Configured, Client::Clear) => Client::Clear,
            (Notice::Changed, Client::Keep { .. }) => Client::Keep { sync: true },
            (Notice::Changed, Client::Set)
            | (Notice::Configured, Client::Keep { .. } | Client::Set) => Client::Set,
            (Notice::Show | Notice::Hide, client) => client,
        };
        let keyboard = match notice {
            Notice::Focused | Notice::Show => Keyboard::Show,
            Notice::Blurred | Notice::Hide => Keyboard::Hide,
            Notice::Changed | Notice::Configured => self.keyboard,
        };
        Pending { client, keyboard }
    }

    /// Whether the calls carry GPUI's editing state: after a new client always, else on a sync.
    pub(crate) fn sends_state(&self) -> bool {
        match self.client {
            Client::Set | Client::Keep { sync: true } => true,
            Client::Keep { sync: false } | Client::Clear => false,
        }
    }

    /// Whether the calls set a new client (once per focus).
    pub(crate) fn sets_client(&self) -> bool {
        match self.client {
            Client::Set => true,
            Client::Keep { .. } | Client::Clear => false,
        }
    }

    /// Whether an IME edit now is for the client Java has: not while a new client or none is on its way (the edit
    /// belongs to the field GPUI left).
    fn takes_java_edit(&self) -> bool {
        match self.client {
            Client::Keep { .. } => true,
            Client::Set | Client::Clear => false,
        }
    }

    /// Flutter's `flutter/textinput` calls for Java (`TextInputChannel.handleFrameworkCalls`), in the framework's
    /// order: clear or set the client, then the editing state, then the keyboard. `None` when there is nothing to
    /// send. A new client with autofill (a hint, and GPUI has named the `field`) also gets the field's `bounds`
    /// right after `setClient` and `requestAutofill` last, as Flutter's `EditableText` attaches: Java's
    /// `notifyViewEntered` needs the rect.
    fn calls(
        &self,
        configuration: &TextInputConfiguration,
        field: Option<FocusId>,
        bounds: Option<Bounds<Pixels>>,
        state: Option<&EditingState>,
    ) -> Option<Value> {
        let autofill = match self.client {
            Client::Set => {
                state.and_then(|state| autofill_json(configuration.autofill, field, state))
            }
            Client::Keep { .. } | Client::Clear => None,
        };
        let client = match self.client {
            Client::Keep { .. } => None,
            Client::Set => Some(json!({
                "method": "TextInput.setClient",
                "args": [0, configuration_json(configuration, autofill.clone())],
            })),
            Client::Clear => Some(json!({"method": "TextInput.clearClient"})),
        };
        let size = autofill.as_ref().map(|_| {
            json!({
                "method": "TextInput.setEditableSizeAndTransform",
                "args": size_and_transform(bounds.unwrap_or_default()),
            })
        });
        let request = autofill
            .as_ref()
            .map(|_| json!({"method": "TextInput.requestAutofill"}));
        let sync = state
            .map(|state| json!({"method": "TextInput.setEditingState", "args": state.to_json()}));
        let keyboard = match self.keyboard {
            Keyboard::Keep => None,
            Keyboard::Show => Some(json!({"method": "TextInput.show"})),
            Keyboard::Hide => Some(json!({"method": "TextInput.hide"})),
        };
        let calls: Vec<Value> = [client, size, sync, keyboard, request]
            .into_iter()
            .flatten()
            .collect();
        (!calls.is_empty()).then_some(Value::Array(calls))
    }
}

/// Who is editing GPUI's text right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Origin {
    /// The app: its change notices are news for Java.
    #[default]
    Gpui,
    /// Java's edit is being applied: change notices are its echo.
    Java,
}

/// GPUI's focused text field. Tracked only between `FocusGained` and `FocusLost`: GPUI installs handlers of fields
/// that do not take text input too, and those send no focus change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Field {
    #[default]
    Blurred,
    /// The `FocusId` once GPUI installs the field's handler (after `FocusGained`, in the same or the next frame).
    Focused(Option<FocusId>),
}

/// The window's text-input session, as GPUI reports it. Main thread only, in the shell.
#[derive(Debug, Default)]
pub(crate) struct Session {
    field: Field,
    configuration: TextInputConfiguration,
    pending: Pending,
    origin: Origin,
    /// Java's mirror (`ListenableEditingState`) as Rust last knew it: the state it was sent, or the state it sent.
    /// Java's next edit is made on it ([`merge`]). `None` without a client.
    mirror: Option<EditingState>,
}

impl Session {
    /// `set_input_handler`: GPUI installs the focused field's handler (every frame, and around each query).
    pub(crate) fn installed(&mut self, id: Option<FocusId>) {
        let (field, notice) = match (self.field, id) {
            (Field::Blurred, _) | (Field::Focused(_), None) => (self.field, None),
            // The first id after FocusGained, which announced the field already: again only for autofill, which
            // needs the id (a `setClient` still pending takes it anyway).
            (Field::Focused(None), Some(id)) => (
                Field::Focused(Some(id)),
                self.configuration.autofill.map(|_| Notice::Configured),
            ),
            (Field::Focused(Some(previous)), Some(id)) => (
                Field::Focused(Some(id)),
                (previous != id).then_some(Notice::Focused),
            ),
        };
        self.field = field;
        if let Some(notice) = notice {
            self.notice(notice);
        }
    }

    pub(crate) fn state_changed(&mut self, change: TextInputStateChange) {
        let notice = match change {
            TextInputStateChange::FocusGained => {
                self.field = Field::Focused(None);
                Notice::Focused
            }
            TextInputStateChange::FocusLost => {
                self.field = Field::Blurred;
                Notice::Blurred
            }
            TextInputStateChange::SelectionChanged | TextInputStateChange::ContentChanged => {
                Notice::Changed
            }
        };
        self.notice(notice);
    }

    pub(crate) fn configure(&mut self, configuration: TextInputConfiguration) {
        self.configuration = configuration;
        match self.field {
            Field::Focused(_) => self.notice(Notice::Configured),
            Field::Blurred => {}
        }
    }

    /// Folds `notice` into the pending calls, except the echo of Java's own edit.
    pub(crate) fn notice(&mut self, notice: Notice) {
        match (self.origin, notice) {
            (Origin::Java, Notice::Changed) => {}
            (
                Origin::Java,
                Notice::Focused
                | Notice::Blurred
                | Notice::Configured
                | Notice::Show
                | Notice::Hide,
            )
            | (Origin::Gpui, _) => self.pending = self.pending.next(notice),
        }
    }

    /// Another Activity's view took the window: its `TextInputPlugin` has no client, so a focused field is
    /// announced again, from scratch.
    pub(crate) fn owner_changed(&mut self) {
        self.mirror = None;
        self.pending = match self.field {
            Field::Focused(_) => Pending::default().next(Notice::Focused),
            Field::Blurred => Pending::default(),
        };
    }

    /// The calls to send now, emptied; `None` when there are none. The shell then reads GPUI's state if they carry
    /// it ([`Pending::sends_state`]), with the session free (GPUI may re-enter the window), and calls [`Self::send`].
    pub(crate) fn take(&mut self) -> Option<Pending> {
        let pending = std::mem::take(&mut self.pending);
        (pending != Pending::default()).then_some(pending)
    }

    /// `pending` as Flutter's calls, with GPUI's `state` where they carry it (empty for a new client when the field
    /// is unreadable: Java still needs a state to restart the IME) and the field's text `bounds` for autofill (read
    /// for a new client). Java's mirror becomes what is sent.
    pub(crate) fn send(
        &mut self,
        pending: Pending,
        state: Option<EditingState>,
        bounds: Option<Bounds<Pixels>>,
    ) -> Option<Value> {
        let state = match pending.client {
            Client::Set => Some(state.unwrap_or_default()),
            Client::Keep { sync: true } => state,
            Client::Keep { sync: false } | Client::Clear => None,
        };
        let field = match self.field {
            Field::Focused(id) => id,
            Field::Blurred => None,
        };
        let calls = pending.calls(&self.configuration, field, bounds, state.as_ref());
        match (pending.client, state) {
            (Client::Clear, _) => self.mirror = None,
            (Client::Set | Client::Keep { .. }, Some(state)) => self.mirror = Some(state),
            (Client::Set | Client::Keep { .. }, None) => {}
        }
        calls
    }

    /// Java's edit, given GPUI's current state: the state GPUI takes ([`merge`] onto Java's mirror), or `None` when
    /// the edit is for a client GPUI is replacing. From here until [`Self::java_edited`], change notices are its echo.
    pub(crate) fn java_edit(
        &mut self,
        java: &EditingState,
        gpui: &EditingState,
    ) -> Option<EditingState> {
        if !self.pending.takes_java_edit() {
            return None;
        }
        self.origin = Origin::Java;
        Some(merge(self.mirror.as_ref().unwrap_or(gpui), java, gpui))
    }

    /// Java's edit is applied and GPUI holds `gpui`. Java's mirror holds `java`; it gets GPUI's state at the frame's
    /// end only if they [`differs`] (a GPUI change merged under Java's edit, or the app changed the edit).
    pub(crate) fn java_edited(&mut self, java: EditingState, gpui: &EditingState) {
        self.origin = Origin::Gpui;
        if let Client::Keep { .. } = self.pending.client {
            self.pending.client = Client::Keep {
                sync: differs(&java, gpui),
            };
        }
        self.mirror = Some(java);
    }
}

/// GPUI's capitalization → Flutter's `TextCapitalization` name. Exhaustive: a new GPUI variant needs an arm.
fn capitalization(autocapitalize: Autocapitalize) -> &'static str {
    match autocapitalize {
        Autocapitalize::None => "TextCapitalization.none",
        Autocapitalize::Words => "TextCapitalization.words",
        Autocapitalize::Sentences => "TextCapitalization.sentences",
        Autocapitalize::Characters => "TextCapitalization.characters",
    }
}

/// GPUI's confirm-key action → Flutter's `TextInputAction` name (`Configuration.fromJson` maps it to
/// `EditorInfo.IME_ACTION_*`). Unspecified lets the IME choose. Exhaustive: a new GPUI variant needs an arm.
fn action(input_action: TextInputAction) -> &'static str {
    match input_action {
        TextInputAction::Unspecified => "TextInputAction.unspecified",
        TextInputAction::Enter => "TextInputAction.newline",
        TextInputAction::Done => "TextInputAction.done",
        TextInputAction::Go => "TextInputAction.go",
        TextInputAction::Next => "TextInputAction.next",
        TextInputAction::Previous => "TextInputAction.previous",
        TextInputAction::Search => "TextInputAction.search",
        TextInputAction::Send => "TextInputAction.send",
    }
}

/// GPUI's keyboard type and `secure` → Flutter's `TextInputType` JSON (`InputType.fromJson`; `TextInputPlugin` turns
/// it into `EditorInfo.inputType`). Exhaustive: a new GPUI variant needs an arm. Search is Flutter's `webSearch`,
/// which Flutter's web engine maps back to `inputmode="search"`. Flutter's number branch returns before
/// `obscureText`, so a secure number says `password` itself; Android has no phone password, so a secure phone
/// field is a number password. A secure field whose type has a variation of its own ([`has_own_variation`]) is a text
/// password: the password variation replaces email's or URL's instead of corrupting it.
fn input_type(keyboard_type: KeyboardType, secure: bool) -> Value {
    let number = |decimal: bool| json!({"name": "TextInputType.number", "signed": false, "decimal": decimal, "password": secure});
    match (keyboard_type, secure) {
        (kind, true) if has_own_variation(kind) => json!({"name": "TextInputType.text"}),
        (KeyboardType::Text, _) => json!({"name": "TextInputType.text"}),
        (KeyboardType::Number, _) | (KeyboardType::Phone, true) => number(false),
        (KeyboardType::Decimal, _) => number(true),
        (KeyboardType::Phone, false) => json!({"name": "TextInputType.phone"}),
        (KeyboardType::Email, _) => json!({"name": "TextInputType.emailAddress"}),
        (KeyboardType::Url, _) => json!({"name": "TextInputType.url"}),
        (KeyboardType::Search, _) => json!({"name": "TextInputType.webSearch"}),
    }
}

/// Whether Flutter's `TextInputType` for this keyboard sets an Android text variation of its own (email, URI).
/// Android's variation is one value, not flags, yet `inputTypeFromTextInputType` ORs one in for `obscureText`
/// (`VARIATION_PASSWORD`) and for `enableSuggestions: false` (`VARIATION_VISIBLE_PASSWORD`): email's 0x20 | 0x90 is
/// 0xb0, `VARIATION_FILTER`, which loses the email keyboard. Number and phone return before either. Exhaustive: a new
/// GPUI variant needs an arm.
fn has_own_variation(keyboard_type: KeyboardType) -> bool {
    match keyboard_type {
        KeyboardType::Email | KeyboardType::Url | KeyboardType::Search => true,
        KeyboardType::Text | KeyboardType::Number | KeyboardType::Decimal | KeyboardType::Phone => {
            false
        }
    }
}

/// GPUI's autofill hint → Flutter's `AutofillHints` name (`autofill.dart`), which `Configuration.Autofill` turns into
/// Android's `View.AUTOFILL_HINT_*` / androidx `HintConstants` value. Exhaustive: a new GPUI variant needs an arm.
fn autofill_hint(hint: AutofillHint) -> &'static str {
    match hint {
        AutofillHint::Username => "username",
        AutofillHint::Password => "password",
        AutofillHint::NewPassword => "newPassword",
        AutofillHint::OneTimeCode => "oneTimeCode",
        AutofillHint::Email => "email",
        AutofillHint::Phone => "telephoneNumber",
        AutofillHint::Name => "name",
        // GPUI's is a full street address (HTML `street-address`, iOS `fullStreetAddress`).
        AutofillHint::PostalAddress => "fullStreetAddress",
        AutofillHint::PostalCode => "postalCode",
        AutofillHint::CreditCardNumber => "creditCardNumber",
    }
}

/// Flutter's `Configuration.Autofill` JSON for the focused field: `None` without a hint, or while GPUI has not named
/// the field yet. `uniqueIdentifier` is the field's `FocusId`, so it stays the same while the field lives (Java keys
/// the field's virtual autofill view by its hash); `editingValue` is the state sent with it.
fn autofill_json(
    hint: Option<AutofillHint>,
    field: Option<FocusId>,
    state: &EditingState,
) -> Option<Value> {
    let (hint, field) = (hint?, field?);
    Some(json!({
        "uniqueIdentifier": format!("parallax.field.{}", field.data().as_ffi()),
        "hints": [autofill_hint(hint)],
        "editingValue": state.to_json(),
    }))
}

/// Flutter's `setEditableSizeAndTransform` arguments for the field's `bounds` (logical pixels; the view fills the
/// window): its size and a column-major 4x4 translation to its origin. Java turns them into the autofill rect.
fn size_and_transform(bounds: Bounds<Pixels>) -> Value {
    let (x, y) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    json!({
        "width": f32::from(bounds.size.width),
        "height": f32::from(bounds.size.height),
        "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, 0.0, 1.0],
    })
}

/// Flutter's `TextInputConfiguration` JSON (`Configuration.fromJson`) for GPUI's configuration, with the field's
/// `autofill` ([`autofill_json`]) when it has one. A secure field is `obscureText` with personalized learning off,
/// as the patch 0005 docs promise (kept out of dictionaries). A type with a variation of its own
/// ([`has_own_variation`]) always enables suggestions, Flutter's default: Flutter turns them off with the
/// visible-password variation, which would replace that type's keyboard (and GPUI's default is off).
fn configuration_json(configuration: &TextInputConfiguration, autofill: Option<Value>) -> Value {
    let mut out = json!({
        "inputType": input_type(configuration.keyboard_type, configuration.secure),
        "obscureText": configuration.secure,
        "autocorrect": configuration.autocorrect,
        "enableSuggestions": configuration.suggestions || has_own_variation(configuration.keyboard_type),
        "enableIMEPersonalizedLearning": !configuration.secure,
        "textCapitalization": capitalization(configuration.autocapitalize),
        "inputAction": action(configuration.input_action),
    });
    if let (Some(autofill), Value::Object(fields)) = (autofill, &mut out) {
        fields.insert("autofill".to_string(), autofill);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, selection: (i32, i32), composing: (i32, i32)) -> EditingState {
        EditingState::from_java(text.to_string(), selection, composing)
    }

    fn replace(range: Range<usize>, text: &str) -> Edit {
        Edit::Replace {
            range,
            text: text.to_string(),
        }
    }

    fn mark(range: Range<usize>, text: &str, selected: Option<Range<usize>>) -> Edit {
        Edit::Mark {
            range,
            text: text.to_string(),
            selected,
        }
    }

    /// A test's JSON literal; `None` if it does not parse, which fails the comparison.
    fn json(literal: &str) -> Option<Value> {
        serde_json::from_str(literal).ok()
    }

    fn focus_id(id: u64) -> FocusId {
        FocusId::from(slotmap::KeyData::from_ffi(id))
    }

    const EMPTY: Pending = Pending {
        client: Client::Keep { sync: false },
        keyboard: Keyboard::Keep,
    };

    #[test]
    fn java_values() {
        // (name, Java's text, selection and composing) → the state's selection and composing range.
        let rows = [
            ("none", state("ab", (-1, -1), (-1, -1)), None, None),
            (
                "caret and composing",
                state("ab", (2, 2), (0, 2)),
                Some((2, 2)),
                Some(0..2),
            ),
            (
                "reversed selection kept",
                state("abc", (3, 1), (-1, -1)),
                Some((3, 1)),
                None,
            ),
            (
                "empty composing is none",
                state("ab", (1, 1), (1, 1)),
                Some((1, 1)),
                None,
            ),
            (
                "past the end is none",
                state("ab", (3, 3), (0, 3)),
                None,
                None,
            ),
            (
                "utf-16 length: emoji is 2",
                state("😀", (2, 2), (0, 2)),
                Some((2, 2)),
                Some(0..2),
            ),
        ];
        for (name, state, selection, composing) in rows {
            assert_eq!(state.selection, selection, "{name}: selection");
            assert_eq!(state.composing, composing, "{name}: composing");
        }
    }

    #[test]
    fn gpui_values_clamped() {
        // (name, text, GPUI's selection and composing) → what Java may be sent.
        type Row = (
            &'static str,
            &'static str,
            Option<(usize, usize)>,
            Option<Range<usize>>,
            Option<(usize, usize)>,
            Option<Range<usize>>,
        );
        let rows: [Row; 5] = [
            (
                "in range: kept",
                "ab",
                Some((2, 1)),
                Some(0..2),
                Some((2, 1)),
                Some(0..2),
            ),
            (
                "selection past the end",
                "ab",
                Some((5, 7)),
                None,
                Some((2, 2)),
                None,
            ),
            (
                "composing past the end",
                "ab",
                None,
                Some(1..9),
                None,
                Some(1..2),
            ),
            (
                "composing wholly past the end: none",
                "ab",
                None,
                Some(4..9),
                None,
                None,
            ),
            (
                "utf-16 length: emoji is 2",
                "😀",
                Some((3, 3)),
                None,
                Some((2, 2)),
                None,
            ),
        ];
        for (name, text, selection, composing, want_selection, want_composing) in rows {
            let clamped = EditingState::clamped(text.to_string(), selection, composing);
            assert_eq!(clamped.selection, want_selection, "{name}: selection");
            assert_eq!(clamped.composing, want_composing, "{name}: composing");
        }
    }

    #[test]
    fn edits_from_java() {
        // (name, GPUI state, Java state, calls). States: (text, selection, composing), -1 for none.
        type Row = (&'static str, EditingState, EditingState, Vec<Edit>);
        let rows: Vec<Row> = vec![
            (
                "same state: nothing",
                state("ab", (2, 2), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                vec![],
            ),
            (
                "commitText appends",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
                vec![replace(2..2, "c")],
            ),
            (
                "commitText inserts mid",
                state("ac", (1, 1), (-1, -1)),
                state("abc", (2, 2), (-1, -1)),
                vec![replace(1..1, "b")],
            ),
            (
                "delete before caret",
                state("abc", (3, 3), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                vec![replace(2..3, "")],
            ),
            (
                "replace selection",
                state("abc", (0, 3), (-1, -1)),
                state("x", (1, 1), (-1, -1)),
                vec![replace(0..3, "x")],
            ),
            (
                "insert, caret elsewhere",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (0, 0), (-1, -1)),
                vec![replace(2..2, "c"), Edit::Select(0..0)],
            ),
            (
                "setComposingText starts",
                state("a ", (2, 2), (-1, -1)),
                state("a h", (3, 3), (2, 3)),
                vec![mark(2..2, "h", Some(1..1))],
            ),
            (
                "setComposingText grows",
                state("a h", (3, 3), (2, 3)),
                state("a he", (4, 4), (2, 4)),
                vec![mark(2..3, "he", Some(2..2))],
            ),
            (
                "repeated letter stays one mark",
                state("a", (1, 1), (0, 1)),
                state("aa", (2, 2), (0, 2)),
                vec![mark(0..1, "aa", Some(2..2))],
            ),
            (
                "autocorrect replaces composing",
                state("teh", (3, 3), (0, 3)),
                state("the", (3, 3), (0, 3)),
                vec![mark(0..3, "the", Some(3..3))],
            ),
            (
                "finishComposingText unmarks",
                state("he", (2, 2), (0, 2)),
                state("he", (2, 2), (-1, -1)),
                vec![Edit::Unmark],
            ),
            (
                "commitText over composing",
                state("hel", (3, 3), (0, 3)),
                state("hello ", (6, 6), (-1, -1)),
                vec![replace(3..3, "lo ")],
            ),
            (
                "setComposingRegion on old text",
                state("hello", (5, 5), (-1, -1)),
                state("hello", (5, 5), (0, 5)),
                vec![mark(0..5, "hello", Some(5..5))],
            ),
            (
                "composing moved, caret outside",
                state("ab cd", (5, 5), (3, 5)),
                state("ab cd", (4, 4), (0, 2)),
                vec![mark(0..2, "ab", None), Edit::Select(4..4)],
            ),
            (
                "setSelection only",
                state("abc", (3, 3), (-1, -1)),
                state("abc", (0, 3), (-1, -1)),
                vec![Edit::Select(0..3)],
            ),
            (
                "reversed selection is a range",
                state("abc", (3, 3), (-1, -1)),
                state("abc", (3, 1), (-1, -1)),
                vec![Edit::Select(1..3)],
            ),
            (
                "CJK composing (pinyin → 你好)",
                state("nihao", (5, 5), (0, 5)),
                state("你好", (2, 2), (-1, -1)),
                vec![replace(0..5, "你好")],
            ),
            (
                "Arabic append",
                state("سلا", (3, 3), (-1, -1)),
                state("سلام", (4, 4), (-1, -1)),
                vec![replace(3..3, "م")],
            ),
            (
                "emoji is two units",
                state("a", (1, 1), (-1, -1)),
                state("a😀", (3, 3), (-1, -1)),
                vec![replace(1..1, "😀")],
            ),
            (
                "delete emoji whole",
                state("a😀", (3, 3), (-1, -1)),
                state("a", (1, 1), (-1, -1)),
                vec![replace(1..3, "")],
            ),
            // 😀 is D83D DE00, 😁 is D83D DE01: same high surrogate; the change must not split the pair.
            (
                "swap emoji, shared high surrogate",
                state("😀", (2, 2), (-1, -1)),
                state("😁", (2, 2), (-1, -1)),
                vec![replace(0..2, "😁")],
            ),
            // 😀 is D83D DE00, 𝘀 (U+1D600) is D835 DE00: same low surrogate.
            (
                "swap emoji, shared low surrogate",
                state("😀", (2, 2), (-1, -1)),
                state("𝘀", (2, 2), (-1, -1)),
                vec![replace(0..2, "𝘀")],
            ),
            (
                "insert before emoji",
                state("😀", (0, 0), (-1, -1)),
                state("x😀", (1, 1), (-1, -1)),
                vec![replace(0..0, "x")],
            ),
            (
                "ZWJ family appended",
                state("", (0, 0), (-1, -1)),
                state("👨‍👩‍👧", (8, 8), (-1, -1)),
                vec![replace(0..0, "👨‍👩‍👧")],
            ),
        ];
        for (name, gpui, java, want) in rows {
            assert_eq!(edits(&gpui, &java), want, "{name}");
        }
    }

    #[test]
    fn merge_java_onto_gpui() {
        // (name, Java's mirror, Java's new state, GPUI's state now, the state GPUI takes).
        let rows = [
            (
                "nothing pending: Java's state",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
            ),
            (
                "nothing pending, composing: Java's state",
                state("a h", (3, 3), (2, 3)),
                state("a he", (4, 4), (2, 4)),
                state("a h", (3, 3), (2, 3)),
                state("a he", (4, 4), (2, 4)),
            ),
            (
                "Gboard: DEL key in GPUI, then commitText",
                state("abc", (3, 3), (-1, -1)),
                state("abcx", (4, 4), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                state("abx", (3, 3), (-1, -1)),
            ),
            (
                "app inserted before, Java appends",
                state("world", (5, 5), (-1, -1)),
                state("world!", (6, 6), (-1, -1)),
                state("hello world", (11, 11), (-1, -1)),
                state("hello world!", (12, 12), (-1, -1)),
            ),
            (
                "app appended, Java inserts before",
                state("bc", (0, 0), (-1, -1)),
                state("abc", (1, 1), (-1, -1)),
                state("bcd", (3, 3), (-1, -1)),
                state("abcd", (1, 1), (-1, -1)),
            ),
            (
                "app inserted before the composing range: it moves",
                state("a h", (3, 3), (2, 3)),
                state("a he", (4, 4), (2, 4)),
                state("!a h", (4, 4), (3, 4)),
                state("!a he", (5, 5), (3, 5)),
            ),
            (
                "Java deletes, app inserted after",
                state("abc", (1, 1), (-1, -1)),
                state("bc", (0, 0), (-1, -1)),
                state("abcX", (4, 4), (-1, -1)),
                state("bcX", (0, 0), (-1, -1)),
            ),
            (
                "overlap: GPUI's change kept whole, Java's insert after it",
                state("abc", (2, 2), (-1, -1)),
                state("abzc", (3, 3), (-1, -1)),
                state("XYZ", (3, 3), (-1, -1)),
                state("XYZz", (4, 4), (-1, -1)),
            ),
            (
                "overlap: Java's delete keeps GPUI's replacement",
                state("abcd", (4, 4), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                state("aXd", (3, 3), (-1, -1)),
                state("aX", (2, 2), (-1, -1)),
            ),
            (
                "app moved the caret only: Java's edit and caret",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
                state("ab", (0, 0), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
            ),
            (
                "emoji: offsets in UTF-16",
                state("😀", (2, 2), (-1, -1)),
                state("😀a", (3, 3), (-1, -1)),
                state("x😀", (3, 3), (-1, -1)),
                state("x😀a", (4, 4), (-1, -1)),
            ),
        ];
        for (name, base, java, gpui, want) in rows {
            assert_eq!(merge(&base, &java, &gpui), want, "{name}");
        }
    }

    #[test]
    fn gpui_differs_from_java() {
        // (name, Java's state, GPUI's after Java's edit) → whether Java must take GPUI's state.
        let rows = [
            (
                "same",
                state("ab", (2, 2), (0, 2)),
                state("ab", (2, 2), (0, 2)),
                false,
            ),
            (
                "reversed selection kept forward: benign",
                state("abc", (3, 1), (-1, -1)),
                state("abc", (1, 3), (-1, -1)),
                false,
            ),
            (
                "field without marked text: benign",
                state("ab", (2, 2), (0, 2)),
                state("ab", (2, 2), (-1, -1)),
                false,
            ),
            (
                "field without a selection: benign",
                state("ab", (2, 2), (-1, -1)),
                state("ab", (-1, -1), (-1, -1)),
                false,
            ),
            (
                "app filtered the text",
                state("ab1", (3, 3), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                true,
            ),
            (
                "app moved the caret",
                state("ab", (2, 2), (-1, -1)),
                state("ab", (0, 0), (-1, -1)),
                true,
            ),
            (
                "app marked another range",
                state("ab cd", (5, 5), (3, 5)),
                state("ab cd", (5, 5), (0, 5)),
                true,
            ),
            (
                "app marked, Java did not",
                state("ab", (2, 2), (-1, -1)),
                state("ab", (2, 2), (0, 2)),
                true,
            ),
        ];
        for (name, java, gpui, want) in rows {
            assert_eq!(differs(&java, &gpui), want, "{name}");
        }
    }

    #[test]
    fn pending_transitions() {
        // One row per (state, notice) → next, with whether an IME edit is taken and the state sent in each state.
        let pending = |client, keyboard| Pending { client, keyboard };
        let keep = Client::Keep { sync: false };
        let sync = Client::Keep { sync: true };
        let (set, cleared, synced) = (
            pending(Client::Set, Keyboard::Show),
            pending(Client::Clear, Keyboard::Hide),
            pending(sync, Keyboard::Keep),
        );
        let rows = [
            ("empty, focused", EMPTY, Notice::Focused, set),
            ("empty, blurred", EMPTY, Notice::Blurred, cleared),
            ("empty, changed", EMPTY, Notice::Changed, synced),
            (
                "empty, configured",
                EMPTY,
                Notice::Configured,
                pending(Client::Set, Keyboard::Keep),
            ),
            (
                "empty, show",
                EMPTY,
                Notice::Show,
                pending(keep, Keyboard::Show),
            ),
            (
                "empty, hide",
                EMPTY,
                Notice::Hide,
                pending(keep, Keyboard::Hide),
            ),
            ("synced, focused", synced, Notice::Focused, set),
            ("synced, blurred", synced, Notice::Blurred, cleared),
            ("synced, changed", synced, Notice::Changed, synced),
            (
                "synced, configured",
                synced,
                Notice::Configured,
                pending(Client::Set, Keyboard::Keep),
            ),
            (
                "synced, show",
                synced,
                Notice::Show,
                pending(sync, Keyboard::Show),
            ),
            (
                "synced, hide",
                synced,
                Notice::Hide,
                pending(sync, Keyboard::Hide),
            ),
            ("set, focused", set, Notice::Focused, set),
            ("set, blurred", set, Notice::Blurred, cleared),
            ("set, changed", set, Notice::Changed, set),
            ("set, configured", set, Notice::Configured, set),
            ("set, show", set, Notice::Show, set),
            (
                "set, hide",
                set,
                Notice::Hide,
                pending(Client::Set, Keyboard::Hide),
            ),
            ("cleared, focused", cleared, Notice::Focused, set),
            ("cleared, blurred", cleared, Notice::Blurred, cleared),
            (
                "cleared, changed: ignored",
                cleared,
                Notice::Changed,
                cleared,
            ),
            (
                "cleared, configured: ignored",
                cleared,
                Notice::Configured,
                cleared,
            ),
            (
                "cleared, show",
                cleared,
                Notice::Show,
                pending(Client::Clear, Keyboard::Show),
            ),
            ("cleared, hide", cleared, Notice::Hide, cleared),
        ];
        for (name, before, notice, want) in rows {
            assert_eq!(before.next(notice), want, "{name}");
        }
        // (name, state) → (an IME edit is for Java's client, the calls carry GPUI's state, they set a client).
        let states = [
            ("empty", EMPTY, true, false, false),
            ("synced", synced, true, true, false),
            ("set", set, false, true, true),
            ("cleared", cleared, false, false, false),
        ];
        for (name, state, takes_java_edit, sends_state, sets_client) in states {
            assert_eq!(state.sets_client(), sets_client, "{name}: sets a client");
            assert_eq!(
                state.takes_java_edit(),
                takes_java_edit,
                "{name}: takes Java's edit"
            );
            assert_eq!(state.sends_state(), sends_state, "{name}: sends the state");
        }
    }

    #[test]
    fn session_notices() {
        // One row per (origin, notice) → the pending calls, from none. Java's own edit's changes are its echo.
        let rows = [
            (
                "app: focused",
                Origin::Gpui,
                Notice::Focused,
                EMPTY.next(Notice::Focused),
            ),
            (
                "app: blurred",
                Origin::Gpui,
                Notice::Blurred,
                EMPTY.next(Notice::Blurred),
            ),
            (
                "app: changed",
                Origin::Gpui,
                Notice::Changed,
                EMPTY.next(Notice::Changed),
            ),
            (
                "app: configured",
                Origin::Gpui,
                Notice::Configured,
                EMPTY.next(Notice::Configured),
            ),
            (
                "app: show",
                Origin::Gpui,
                Notice::Show,
                EMPTY.next(Notice::Show),
            ),
            (
                "app: hide",
                Origin::Gpui,
                Notice::Hide,
                EMPTY.next(Notice::Hide),
            ),
            (
                "Java's edit: focused",
                Origin::Java,
                Notice::Focused,
                EMPTY.next(Notice::Focused),
            ),
            (
                "Java's edit: blurred",
                Origin::Java,
                Notice::Blurred,
                EMPTY.next(Notice::Blurred),
            ),
            (
                "Java's edit: changed is its echo",
                Origin::Java,
                Notice::Changed,
                EMPTY,
            ),
            (
                "Java's edit: configured",
                Origin::Java,
                Notice::Configured,
                EMPTY.next(Notice::Configured),
            ),
            (
                "Java's edit: show",
                Origin::Java,
                Notice::Show,
                EMPTY.next(Notice::Show),
            ),
            (
                "Java's edit: hide",
                Origin::Java,
                Notice::Hide,
                EMPTY.next(Notice::Hide),
            ),
        ];
        for (name, origin, notice, want) in rows {
            let mut session = Session {
                origin,
                ..Session::default()
            };
            session.notice(notice);
            assert_eq!(session.pending, want, "{name}");
        }
    }

    #[test]
    fn session_fields() {
        // (name, field before, what GPUI does) → field after, pending calls (from none).
        enum Step {
            Installed(Option<FocusId>),
            /// Installed, the field configured with an autofill hint.
            InstalledAutofill(Option<FocusId>),
            Changed(TextInputStateChange),
            Configured,
        }
        let (a, b) = (focus_id(1), focus_id(2));
        let set = EMPTY.next(Notice::Focused);
        let rows = [
            (
                "blurred, installed: not tracked",
                Field::Blurred,
                Step::Installed(Some(a)),
                Field::Blurred,
                EMPTY,
            ),
            (
                "focused, first id: no new client",
                Field::Focused(None),
                Step::Installed(Some(a)),
                Field::Focused(Some(a)),
                EMPTY,
            ),
            (
                "focused, first id, autofill: new client with the id",
                Field::Focused(None),
                Step::InstalledAutofill(Some(a)),
                Field::Focused(Some(a)),
                EMPTY.next(Notice::Configured),
            ),
            (
                "same field reinstalled, autofill: no new client",
                Field::Focused(Some(a)),
                Step::InstalledAutofill(Some(a)),
                Field::Focused(Some(a)),
                EMPTY,
            ),
            (
                "same field reinstalled: no new client",
                Field::Focused(Some(a)),
                Step::Installed(Some(a)),
                Field::Focused(Some(a)),
                EMPTY,
            ),
            (
                "other field: new client",
                Field::Focused(Some(a)),
                Step::Installed(Some(b)),
                Field::Focused(Some(b)),
                set,
            ),
            (
                "handler without an id: kept",
                Field::Focused(Some(a)),
                Step::Installed(None),
                Field::Focused(Some(a)),
                EMPTY,
            ),
            (
                "focus gained",
                Field::Blurred,
                Step::Changed(TextInputStateChange::FocusGained),
                Field::Focused(None),
                set,
            ),
            (
                "focus lost",
                Field::Focused(Some(a)),
                Step::Changed(TextInputStateChange::FocusLost),
                Field::Blurred,
                EMPTY.next(Notice::Blurred),
            ),
            (
                "content changed",
                Field::Focused(Some(a)),
                Step::Changed(TextInputStateChange::ContentChanged),
                Field::Focused(Some(a)),
                EMPTY.next(Notice::Changed),
            ),
            (
                "selection changed",
                Field::Focused(Some(a)),
                Step::Changed(TextInputStateChange::SelectionChanged),
                Field::Focused(Some(a)),
                EMPTY.next(Notice::Changed),
            ),
            (
                "configured, blurred: no client",
                Field::Blurred,
                Step::Configured,
                Field::Blurred,
                EMPTY,
            ),
            (
                "configured, focused: new client",
                Field::Focused(Some(a)),
                Step::Configured,
                Field::Focused(Some(a)),
                EMPTY.next(Notice::Configured),
            ),
        ];
        for (name, field, step, want_field, want_pending) in rows {
            let mut session = Session {
                field,
                ..Session::default()
            };
            match step {
                Step::Installed(id) => session.installed(id),
                Step::InstalledAutofill(id) => {
                    session.configuration.autofill = Some(AutofillHint::Email);
                    session.installed(id)
                }
                Step::Changed(change) => session.state_changed(change),
                Step::Configured => session.configure(TextInputConfiguration::default()),
            }
            assert_eq!(session.field, want_field, "{name}: field");
            assert_eq!(session.pending, want_pending, "{name}: pending");
        }
    }

    #[test]
    fn session_owner_changed() {
        // (name, field) → what the new Activity's view gets. Its TextInputPlugin starts with no client.
        let rows = [
            ("blurred: nothing", Field::Blurred, EMPTY),
            (
                "focused: the field again",
                Field::Focused(Some(focus_id(1))),
                EMPTY.next(Notice::Focused),
            ),
            (
                "focused, no id yet: the field again",
                Field::Focused(None),
                EMPTY.next(Notice::Focused),
            ),
        ];
        for (name, field, want) in rows {
            let mut session = Session {
                field,
                pending: EMPTY.next(Notice::Hide),
                mirror: Some(state("ab", (2, 2), (-1, -1))),
                ..Session::default()
            };
            session.owner_changed();
            assert_eq!(session.pending, want, "{name}: pending");
            assert_eq!(session.mirror, None, "{name}: no mirror");
        }
    }

    #[test]
    fn session_java_edits() {
        // (name, pending, Java's mirror, Java's state, GPUI's state, whether the app keeps its state instead of
        // the edit) → the state GPUI takes (`None`: dropped), then the pending calls. A taken edit makes Java's
        // state the mirror.
        let synced = EMPTY.next(Notice::Changed);
        let set = EMPTY.next(Notice::Focused);
        let cleared = EMPTY.next(Notice::Blurred);
        let ab = state("ab", (2, 2), (-1, -1));
        let abc = state("abc", (3, 3), (-1, -1));
        type Row = (
            &'static str,
            Pending,
            Option<EditingState>,
            EditingState,
            EditingState,
            bool,
            Option<EditingState>,
            Pending,
        );
        let rows: [Row; 6] = [
            (
                "nothing pending: taken, no sync",
                EMPTY,
                Some(ab.clone()),
                abc.clone(),
                ab.clone(),
                false,
                Some(abc.clone()),
                EMPTY,
            ),
            (
                "no mirror yet: diffed against GPUI",
                EMPTY,
                None,
                abc.clone(),
                ab.clone(),
                false,
                Some(abc.clone()),
                EMPTY,
            ),
            (
                "GPUI change pending: merged, Java gets the merge",
                synced,
                Some(abc.clone()),
                state("abcx", (4, 4), (-1, -1)),
                ab.clone(),
                false,
                Some(state("abx", (3, 3), (-1, -1))),
                synced,
            ),
            (
                "app refuses the edit: Java gets GPUI's state",
                EMPTY,
                Some(ab.clone()),
                abc.clone(),
                ab.clone(),
                true,
                Some(abc.clone()),
                synced,
            ),
            (
                "new client on its way: dropped",
                set,
                Some(ab.clone()),
                abc.clone(),
                ab.clone(),
                false,
                None,
                set,
            ),
            (
                "client cleared: dropped",
                cleared,
                Some(ab.clone()),
                abc.clone(),
                ab.clone(),
                false,
                None,
                cleared,
            ),
        ];
        for (name, pending, mirror, java, gpui, app_keeps, want, want_pending) in rows {
            let mut session = Session {
                pending,
                mirror: mirror.clone(),
                ..Session::default()
            };
            let taken = session.java_edit(&java, &gpui);
            assert_eq!(taken, want, "{name}: state");
            if let Some(target) = &taken {
                assert_eq!(session.origin, Origin::Java, "{name}: echo while applied");
                session.java_edited(java.clone(), if app_keeps { &gpui } else { target });
            }
            let want_mirror = if taken.is_some() { Some(java) } else { mirror };
            assert_eq!(session.mirror, want_mirror, "{name}: mirror");
            assert_eq!(session.origin, Origin::Gpui, "{name}: origin");
            assert_eq!(session.pending, want_pending, "{name}: pending");
        }
    }

    #[test]
    fn session_sends() {
        // (name, pending, GPUI's state as read) → the calls, and Java's mirror after (it was `ab`).
        let gpui = state("xy", (2, 2), (-1, -1));
        let ab = state("ab", (2, 2), (-1, -1));
        type Row = (
            &'static str,
            Pending,
            Option<EditingState>,
            Option<&'static str>,
            Option<EditingState>,
        );
        let rows: [Row; 6] = [
            ("nothing", EMPTY, None, None, Some(ab.clone())),
            (
                "sync",
                EMPTY.next(Notice::Changed),
                Some(gpui.clone()),
                Some(concat!(
                    r#"[{"method":"TextInput.setEditingState","args":{"text":"xy","selectionBase":2,"#,
                    r#""selectionExtent":2,"composingBase":-1,"composingExtent":-1}}]"#
                )),
                Some(gpui.clone()),
            ),
            (
                "sync, unreadable field: nothing",
                EMPTY.next(Notice::Changed),
                None,
                None,
                Some(ab.clone()),
            ),
            (
                "new client, unreadable field: an empty state still",
                EMPTY.next(Notice::Configured),
                None,
                Some(concat!(
                    r#"[{"method":"TextInput.setClient","args":[0,{"inputType":{"name":"TextInputType.text"},"#,
                    r#""obscureText":false,"autocorrect":false,"enableSuggestions":false,"#,
                    r#""enableIMEPersonalizedLearning":true,"textCapitalization":"TextCapitalization.none","#,
                    r#""inputAction":"TextInputAction.unspecified"}]},"#,
                    r#"{"method":"TextInput.setEditingState","args":{"text":"","selectionBase":-1,"#,
                    r#""selectionExtent":-1,"composingBase":-1,"composingExtent":-1}}]"#
                )),
                Some(EditingState::default()),
            ),
            (
                "blur: clear, hide",
                EMPTY.next(Notice::Blurred),
                None,
                Some(r#"[{"method":"TextInput.clearClient"},{"method":"TextInput.hide"}]"#),
                None,
            ),
            (
                "hide only",
                EMPTY.next(Notice::Hide),
                None,
                Some(r#"[{"method":"TextInput.hide"}]"#),
                Some(ab.clone()),
            ),
        ];
        for (name, pending, read, want, want_mirror) in rows {
            let mut session = Session {
                pending,
                mirror: Some(ab.clone()),
                ..Session::default()
            };
            let calls = session
                .take()
                .and_then(|pending| session.send(pending, read, None));
            assert_eq!(calls, want.and_then(json), "{name}: calls");
            assert_eq!(session.mirror, want_mirror, "{name}: mirror");
            assert_eq!(session.pending, EMPTY, "{name}: emptied");
        }
    }

    #[test]
    fn calls_for_java() {
        let configuration = TextInputConfiguration {
            autocorrect: true,
            autocapitalize: Autocapitalize::Sentences,
            suggestions: true,
            input_action: TextInputAction::Send,
            ..TextInputConfiguration::default()
        };
        let gpui = state("a\"😀\n", (4, 4), (1, 4));
        let want = concat!(
            r#"[{"method":"TextInput.setClient","args":[0,{"inputType":{"name":"TextInputType.text"},"#,
            r#""obscureText":false,"autocorrect":true,"enableSuggestions":true,"#,
            r#""enableIMEPersonalizedLearning":true,"textCapitalization":"TextCapitalization.sentences","#,
            r#""inputAction":"TextInputAction.send"}]},"#,
            r#"{"method":"TextInput.setEditingState","args":{"text":"a\"😀\u000a","selectionBase":4,"#,
            r#""selectionExtent":4,"composingBase":1,"composingExtent":4}},"#,
            r#"{"method":"TextInput.show"}]"#
        );
        assert_eq!(
            EMPTY
                .next(Notice::Focused)
                .calls(&configuration, None, None, Some(&gpui)),
            json(want),
            "focus: client, state, show"
        );
        assert_eq!(
            EMPTY.calls(&configuration, None, None, None),
            None,
            "nothing"
        );
    }

    #[test]
    fn autofill_for_java() {
        // (name, hint, GPUI's field id, field bounds) → the calls for a new focused client.
        let a = focus_id(1);
        let gpui = state("a@b", (3, 3), (-1, -1));
        let bounds = Bounds::new(
            gpui::point(gpui::px(16.), gpui::px(40.)),
            gpui::size(gpui::px(200.), gpui::px(24.)),
        );
        let set_client = |autofill: Option<Value>| {
            json!({"method": "TextInput.setClient", "args": [0, configuration_json(&TextInputConfiguration {
                autofill: Some(AutofillHint::Email),
                ..TextInputConfiguration::default()
            }, autofill)]})
        };
        let sync = json!({"method": "TextInput.setEditingState", "args": gpui.to_json()});
        let show = json!({"method": "TextInput.show"});
        let autofill = json!({
            "uniqueIdentifier": format!("parallax.field.{}", a.data().as_ffi()),
            "hints": ["email"],
            "editingValue": gpui.to_json(),
        });
        let size = |x: f32, y: f32, width: f32, height: f32| {
            json!({"method": "TextInput.setEditableSizeAndTransform", "args": {
                "width": width, "height": height,
                "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, 0.0, 1.0],
            }})
        };
        let request = json!({"method": "TextInput.requestAutofill"});
        let rows = [
            (
                "hint and id: autofill, rect, request last",
                Some(AutofillHint::Email),
                Some(a),
                Some(bounds),
                vec![
                    set_client(Some(autofill.clone())),
                    size(16., 40., 200., 24.),
                    sync.clone(),
                    show.clone(),
                    request,
                ],
            ),
            (
                "no bounds: an empty rect at the origin",
                Some(AutofillHint::Email),
                Some(a),
                None,
                vec![
                    set_client(Some(autofill)),
                    size(0., 0., 0., 0.),
                    sync.clone(),
                    show.clone(),
                    json!({"method": "TextInput.requestAutofill"}),
                ],
            ),
            (
                // installed() announces the field again once GPUI names it.
                "hint, no id yet: no autofill",
                Some(AutofillHint::Email),
                None,
                Some(bounds),
                vec![set_client(None), sync.clone(), show.clone()],
            ),
            (
                "no hint: no autofill",
                None,
                Some(a),
                Some(bounds),
                vec![
                    json!({"method": "TextInput.setClient", "args": [0, configuration_json(&TextInputConfiguration::default(), None)]}),
                    sync,
                    show,
                ],
            ),
        ];
        for (name, hint, field, bounds, want) in rows {
            let configuration = TextInputConfiguration {
                autofill: hint,
                ..TextInputConfiguration::default()
            };
            assert_eq!(
                EMPTY
                    .next(Notice::Focused)
                    .calls(&configuration, field, bounds, Some(&gpui)),
                Some(Value::Array(want)),
                "{name}"
            );
        }
        let update = EMPTY.next(Notice::Changed).calls(
            &TextInputConfiguration {
                autofill: Some(AutofillHint::Email),
                ..TextInputConfiguration::default()
            },
            Some(a),
            Some(bounds),
            Some(&gpui),
        );
        assert_eq!(
            update,
            Some(json!([{"method": "TextInput.setEditingState", "args": gpui.to_json()}])),
            "same client: only the state"
        );
    }

    #[test]
    fn autofill_hints() {
        // GPUI's hint → Flutter's AutofillHints name (Configuration.Autofill translates it for Android).
        let rows = [
            (AutofillHint::Username, "username"),
            (AutofillHint::Password, "password"),
            (AutofillHint::NewPassword, "newPassword"),
            (AutofillHint::OneTimeCode, "oneTimeCode"),
            (AutofillHint::Email, "email"),
            (AutofillHint::Phone, "telephoneNumber"),
            (AutofillHint::Name, "name"),
            (AutofillHint::PostalAddress, "fullStreetAddress"),
            (AutofillHint::PostalCode, "postalCode"),
            (AutofillHint::CreditCardNumber, "creditCardNumber"),
        ];
        let field = focus_id(7);
        for (hint, want) in rows {
            let out = autofill_json(Some(hint), Some(field), &EditingState::default());
            assert_eq!(
                out.as_ref().map(|out| &out["hints"]),
                Some(&json!([want])),
                "{hint:?}"
            );
            assert_eq!(
                out.as_ref().map(|out| &out["uniqueIdentifier"]),
                autofill_json(Some(hint), Some(field), &state("x", (1, 1), (-1, -1)))
                    .as_ref()
                    .map(|out| &out["uniqueIdentifier"]),
                "{hint:?}: the id depends on the field only"
            );
        }
        assert_ne!(
            autofill_json(
                Some(AutofillHint::Email),
                Some(focus_id(1)),
                &EditingState::default()
            ),
            autofill_json(
                Some(AutofillHint::Email),
                Some(focus_id(2)),
                &EditingState::default()
            ),
            "another field, another id"
        );
    }

    #[test]
    fn configuration_for_java() {
        // One row per keyboard type, secure and not, with GPUI's default (no suggestions): GPUI's configuration →
        // Configuration.fromJson's `inputType`, `enableSuggestions`, `obscureText` and
        // `enableIMEPersonalizedLearning`. Email, URL and search keep suggestions on and are text when secure, so
        // Flutter's visible-password and password variations never land on top of theirs.
        let keyboard = |keyboard_type, secure| TextInputConfiguration {
            keyboard_type,
            secure,
            ..TextInputConfiguration::default()
        };
        let text = r#"{"name":"TextInputType.text"}"#;
        let number_password =
            r#"{"name":"TextInputType.number","signed":false,"decimal":false,"password":true}"#;
        let rows = [
            ("text", keyboard(KeyboardType::Text, false), text, false),
            (
                "text, secure",
                keyboard(KeyboardType::Text, true),
                text,
                false,
            ),
            (
                "number",
                keyboard(KeyboardType::Number, false),
                r#"{"name":"TextInputType.number","signed":false,"decimal":false,"password":false}"#,
                false,
            ),
            (
                "number, secure: password",
                keyboard(KeyboardType::Number, true),
                number_password,
                false,
            ),
            (
                "decimal",
                keyboard(KeyboardType::Decimal, false),
                r#"{"name":"TextInputType.number","signed":false,"decimal":true,"password":false}"#,
                false,
            ),
            (
                "decimal, secure: password",
                keyboard(KeyboardType::Decimal, true),
                r#"{"name":"TextInputType.number","signed":false,"decimal":true,"password":true}"#,
                false,
            ),
            (
                "phone",
                keyboard(KeyboardType::Phone, false),
                r#"{"name":"TextInputType.phone"}"#,
                false,
            ),
            (
                "phone, secure: number password",
                keyboard(KeyboardType::Phone, true),
                number_password,
                false,
            ),
            (
                "email",
                keyboard(KeyboardType::Email, false),
                r#"{"name":"TextInputType.emailAddress"}"#,
                true,
            ),
            (
                "email, secure: text password",
                keyboard(KeyboardType::Email, true),
                text,
                true,
            ),
            (
                "url",
                keyboard(KeyboardType::Url, false),
                r#"{"name":"TextInputType.url"}"#,
                true,
            ),
            (
                "url, secure: text password",
                keyboard(KeyboardType::Url, true),
                text,
                true,
            ),
            (
                "search",
                keyboard(KeyboardType::Search, false),
                r#"{"name":"TextInputType.webSearch"}"#,
                true,
            ),
            (
                "search, secure: text password",
                keyboard(KeyboardType::Search, true),
                text,
                true,
            ),
        ];
        for (name, configuration, input_type, suggestions) in rows {
            let out = configuration_json(&configuration, None);
            assert_eq!(
                Some(&out["inputType"]),
                json(input_type).as_ref(),
                "{name}: inputType"
            );
            assert_eq!(out["enableSuggestions"], suggestions, "{name}: suggestions");
            assert_eq!(
                out["obscureText"], configuration.secure,
                "{name}: obscureText"
            );
            assert_eq!(
                out["enableIMEPersonalizedLearning"], !configuration.secure,
                "{name}: personalized learning"
            );
        }
    }

    #[test]
    fn configuration_names() {
        // One row per capitalization and per action → the whole JSON. A hint without autofill JSON (GPUI has not
        // named the field) sends no autofill.
        let configuration = |autocapitalize, input_action| TextInputConfiguration {
            autocapitalize,
            input_action,
            autocorrect: true,
            suggestions: true,
            autofill: Some(gpui::AutofillHint::Password),
            ..TextInputConfiguration::default()
        };
        let head = concat!(
            r#"{"inputType":{"name":"TextInputType.text"},"obscureText":false,"autocorrect":true,"#,
            r#""enableSuggestions":true,"enableIMEPersonalizedLearning":true,"#
        );
        let rows = [
            (
                Autocapitalize::None,
                TextInputAction::Unspecified,
                r#""textCapitalization":"TextCapitalization.none","inputAction":"TextInputAction.unspecified"}"#,
            ),
            (
                Autocapitalize::Words,
                TextInputAction::Enter,
                r#""textCapitalization":"TextCapitalization.words","inputAction":"TextInputAction.newline"}"#,
            ),
            (
                Autocapitalize::Sentences,
                TextInputAction::Done,
                r#""textCapitalization":"TextCapitalization.sentences","inputAction":"TextInputAction.done"}"#,
            ),
            (
                Autocapitalize::Characters,
                TextInputAction::Go,
                r#""textCapitalization":"TextCapitalization.characters","inputAction":"TextInputAction.go"}"#,
            ),
            (
                Autocapitalize::None,
                TextInputAction::Next,
                r#""textCapitalization":"TextCapitalization.none","inputAction":"TextInputAction.next"}"#,
            ),
            (
                Autocapitalize::None,
                TextInputAction::Previous,
                r#""textCapitalization":"TextCapitalization.none","inputAction":"TextInputAction.previous"}"#,
            ),
            (
                Autocapitalize::None,
                TextInputAction::Search,
                r#""textCapitalization":"TextCapitalization.none","inputAction":"TextInputAction.search"}"#,
            ),
            (
                Autocapitalize::None,
                TextInputAction::Send,
                r#""textCapitalization":"TextCapitalization.none","inputAction":"TextInputAction.send"}"#,
            ),
        ];
        for (autocapitalize, input_action, tail) in rows {
            assert_eq!(
                Some(configuration_json(
                    &configuration(autocapitalize, input_action),
                    None
                )),
                json(&format!("{head}{tail}")),
                "{autocapitalize:?}, {input_action:?}"
            );
        }
    }
}
