//! A single-line text field on GPUI's `EntityInputHandler`: the smallest one that exercises the platform's text
//! input (IME composition, selection, UTF-16 ranges). Upstream GPUI has no ready field outside Zed's editor
//! (GPL, never copied); this is cut down from upstream's own `examples/input.rs` (Apache-2.0, Zed Industries).
//!
//! Pointer selection (taps, double taps, long presses, handles, toolbar, magnifier) is Flutter's, run by
//! `parallax_ext::text_selection` on every target but iOS, which keeps UIKit's; this file draws its overlay as plain
//! shapes (docs/text-selection.md).

use std::ops::Range;

use parallax::gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, GlobalElementId, Hsla,
    KeyBinding, LayoutId, LongPressEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, PointerDevice, ShapedLine, SharedString, Size, Style,
    TextInputConfiguration, TextRun, TouchDragEvent, TouchDragStart, TouchPhase, UTF16Selection,
    UnderlineStyle, Window, actions, canvas, div, fill, hsla, point, prelude::*, px, relative, rgb,
    rgba, size,
};
use parallax::parallax_platform::behaviour::current_behaviour;
use parallax_ext::text_selection::{
    Field, Handle, HandleGeometry, HandleType, Input, Lines, MagnifierGeometry, Selection,
    TextLayout, TextSelection, toolbar_origin,
};
use unicode_segmentation::UnicodeSegmentation;

// `actions!` expands to paths through a crate named `gpui`; through the facade it is `parallax::gpui`.
use parallax::gpui;

actions!(
    text_field,
    [Backspace, Delete, Left, Right, Home, End, SelectAll, Submit]
);

/// The field's key bindings; call once at launch. Add a row to bind another key.
pub fn bind_keys(cx: &mut App) {
    let context = Some("TextField");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
        KeyBinding::new("enter", Submit, context),
    ]);
}

/// What a toolbar button does to the field.
type ToolbarAction = fn(&mut TextField, &mut Window, &mut Context<TextField>);

/// The selection toolbar's buttons, in order. To add one: a row with its label and what it does.
const TOOLBAR: [(&str, ToolbarAction); 3] = [
    ("Copy", TextField::copy),
    ("Paste", TextField::paste),
    ("Select all", TextField::select_all),
];

const ACCENT: u32 = 0x3b82f6;

pub struct TextField {
    focus_handle: FocusHandle,
    /// UTF-8; GPUI's input handler speaks UTF-16, converted at the trait boundary.
    content: String,
    placeholder: SharedString,
    /// What the platform's keyboard and autofill are told about this field.
    configuration: TextInputConfiguration,
    /// Byte offsets; `extent` is the caret end.
    selection: Selection,
    marked: Option<Range<usize>>,
    /// Flutter's selection gestures and overlay; `None` where the platform selects natively (iOS).
    selection_ui: Option<TextSelection>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// The caret (or selection) last painted while focused, reported to the IME when it moves.
    ime_caret: Option<Bounds<Pixels>>,
}

/// The shaped line as `parallax_ext`'s layout: one line in `bounds`, byte offsets, words by Unicode's rules.
struct ShapedField<'a> {
    content: &'a str,
    /// `None` while the placeholder shows.
    line: Option<&'a ShapedLine>,
    bounds: Bounds<Pixels>,
}

impl ShapedField<'_> {
    fn x(&self, offset: usize) -> Pixels {
        self.line.map_or(px(0.), |line| line.x_for_index(offset))
    }
}

impl TextLayout for ShapedField<'_> {
    fn len(&self) -> usize {
        self.content.len()
    }

    fn position_for_point(&self, point: Point<Pixels>) -> usize {
        self.line.map_or(0, |line| {
            line.closest_index_for_x(point.x - self.bounds.left())
        })
    }

    fn word_boundary(&self, offset: usize) -> Range<usize> {
        self.content
            .split_word_bound_indices()
            .map(|(start, word)| start..start + word.len())
            .find(|word| word.contains(&offset))
            .unwrap_or(offset..offset)
    }

    fn line_at(&self, _offset: usize) -> Range<usize> {
        0..self.content.len()
    }

    fn paragraph_at(&self, _offset: usize) -> Range<usize> {
        0..self.content.len()
    }

    fn caret_rect(&self, offset: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(self.bounds.left() + self.x(offset), self.bounds.top()),
            size(px(2.), self.bounds.size.height),
        )
    }

    fn preferred_line_height(&self) -> Pixels {
        self.bounds.size.height
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

impl TextField {
    pub fn new(
        placeholder: &str,
        configuration: TextInputConfiguration,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: placeholder.to_string().into(),
            configuration,
            selection: Selection::default(),
            marked: None,
            selection_ui: current_behaviour()
                .ok()
                .and_then(|behaviour| TextSelection::for_behaviour(&behaviour)),
            last_layout: None,
            last_bounds: None,
            ime_caret: None,
        }
    }

    fn selected(&self) -> Range<usize> {
        self.selection.start()..self.selection.end()
    }

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection::collapsed(offset);
        window.notify_text_selection_changed(&self.focus_handle);
        self.selection_input(Input::Typed, window, cx);
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    /// Deletes the selection, or the grapheme `towards` picks from the cursor.
    fn delete_towards(
        &mut self,
        towards: fn(&Self, usize) -> usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection.is_collapsed() {
            let cursor = self.selection.extent;
            self.selection = Selection {
                base: towards(self, cursor),
                extent: cursor,
            };
        }
        self.replace_text_in_range(None, "", window, cx);
        window.notify_text_input_changed(&self.focus_handle);
    }

    /// Runs one pointer input through Flutter's selection engine and applies what it asks for.
    fn selection_input(&mut self, input: Input, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(engine), Some(bounds)) = (self.selection_ui.as_mut(), self.last_bounds) else {
            return;
        };
        let layout = ShapedField {
            content: &self.content,
            line: self.last_layout.as_ref(),
            bounds,
        };
        let field = Field {
            selection: self.selection,
            focused: self.focus_handle.is_focused(window),
            lines: Lines::Single,
            screen: Bounds::new(Point::default(), window.viewport_size()),
        };
        let effects = engine.handle(input, &field, &layout);
        if let Some(selection) = effects.selection {
            self.selection = selection;
            window.notify_text_selection_changed(&self.focus_handle);
        }
        if effects.request_keyboard {
            window.focus(&self.focus_handle, cx);
            // Reopens a dismissed keyboard: Android shows it when the field gains focus, not on later taps.
            window.request_virtual_keyboard();
        }
        if let Some(haptic) = effects.haptic {
            window.perform_haptic(haptic);
        }
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if self.selection_ui.is_some() {
            let device = window
                .pointer_source()
                .map_or(PointerDevice::Mouse, |source| source.device);
            let input = match event.button {
                MouseButton::Right => Input::SecondaryTap {
                    position: event.position,
                    device,
                },
                _ => Input::Down {
                    position: event.position,
                    device,
                    tap_count: event.click_count,
                    shift: event.modifiers.shift,
                },
            };
            return self.selection_input(input, window, cx);
        }
        // iOS: UIKit selects; a tap only places the caret.
        window.focus(&self.focus_handle, cx);
        window.request_virtual_keyboard();
        let offset = match (&self.last_bounds, &self.last_layout) {
            (Some(bounds), Some(line)) => {
                line.closest_index_for_x(event.position.x - bounds.left())
            }
            (None, _) | (_, None) => self.content.len(),
        };
        self.move_to(offset, window, cx);
    }

    fn copy(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.content[self.selected()].to_string(),
        ));
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
            window.notify_text_input_changed(&self.focus_handle);
        }
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection = Selection {
            base: 0,
            extent: self.content.len(),
        };
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.content[..offset].encode_utf16().count()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (index, character) in self.content.char_indices() {
            if units >= offset {
                return index;
            }
            units += character.len_utf16();
        }
        self.content.len()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected()),
            reversed: self.selection.extent < self.selection.base,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.range_from_utf16(&range_utf16);
        self.selection = Selection {
            base: range.start,
            extent: range.end,
        };
        self.selection_input(Input::Typed, window, cx);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_and_mark_text_in_range(range_utf16, new_text, None, window, cx);
        self.marked = None;
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A single-line field: line breaks become spaces.
        let new_text = new_text.replace(['\n', '\r'], " ");
        let range = range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or(self.marked.clone())
            .unwrap_or(self.selected());
        self.content.replace_range(range.clone(), &new_text);
        let inserted = range.start..range.start + new_text.len();
        self.marked = (!new_text.is_empty()).then_some(inserted.clone());
        let selected = match new_selected_utf16 {
            Some(selected) => {
                let start = self.offset_to_utf16(inserted.start);
                self.range_from_utf16(&(start + selected.start..start + selected.end))
            }
            None => inserted.end..inserted.end,
        };
        self.selection = Selection {
            base: selected.start,
            extent: selected.end,
        };
        self.selection_input(Input::Typed, window, cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        // An empty field has no shaped line: its caret is at the start (the IME and autofill still need a rect).
        let x = |index| {
            self.last_layout
                .as_ref()
                .map_or(px(0.), |line| line.x_for_index(index))
        };
        Some(Bounds::from_corners(
            point(bounds.left() + x(range.start), bounds.top()),
            point(bounds.left() + x(range.end), bounds.bottom()),
        ))
    }

    fn text_input_configuration(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.configuration.clone()
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        let index = self.last_layout.as_ref()?.closest_index_for_x(local.x);
        Some(self.offset_to_utf16(index))
    }
}

impl Render for TextField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("TextField")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(|this, _: &Backspace, window, cx| {
                this.delete_towards(Self::previous_boundary, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Delete, window, cx| {
                this.delete_towards(Self::next_boundary, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Left, window, cx| {
                let to = this.previous_boundary(this.selection.extent);
                this.move_to(to, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Right, window, cx| {
                let to = this.next_boundary(this.selection.extent);
                this.move_to(to, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Home, window, cx| this.move_to(0, window, cx)))
            .on_action(
                cx.listener(|this, _: &End, window, cx| {
                    this.move_to(this.content.len(), window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.select_all(window, cx);
                this.selection_input(Input::Typed, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Submit, _window, _cx| {
                // The length only: a field may hold an email or a password, and logs are readable.
                log::info!("text field submitted: {} bytes", this.content.len());
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .w_full()
            .p(px(8.))
            .rounded_md()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x1e1e2e))
            .line_height(px(32.))
            .text_size(px(22.))
            .child(TextElement { field: cx.entity() })
    }
}

/// Paints the field's one shaped line, its selection or caret, the composing underline, and the selection overlay.
struct TextElement {
    field: Entity<TextField>,
}

/// One toolbar button: its label, shaped, and where it sits.
#[derive(Clone)]
struct ToolbarButton {
    label: ShapedLine,
    bounds: Bounds<Pixels>,
    action: ToolbarAction,
}

struct Prepaint {
    line: ShapedLine,
    marker: PaintQuad,
    overlay: DrawnOverlay,
}

/// The selection overlay as laid out this frame: drawn on top of the window (deferred), and hit-tested by the
/// gesture listeners.
#[derive(Clone)]
struct DrawnOverlay {
    handles: Vec<HandleGeometry>,
    toolbar: Vec<ToolbarButton>,
    magnifier: Option<MagnifierGeometry>,
}

// The toolbar's look: a 44 px bar (Material's `_kToolbarHeight`), each button padded by this much.
const TOOLBAR_HEIGHT: Pixels = px(44.);
const TOOLBAR_PADDING: Pixels = px(12.);

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&parallax::gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&parallax::gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let field = self.field.read(cx);
        let style = window.text_style();
        let (text, color): (SharedString, _) = if field.content.is_empty() {
            (field.placeholder.clone(), hsla(0., 0., 0., 0.35))
        } else {
            (field.content.clone().into(), style.color)
        };
        let run = |len, underline| TextRun {
            len,
            font: style.font(),
            color,
            background_color: None,
            underline,
            strikethrough: None,
        };
        let underline = Some(UnderlineStyle {
            color: Some(color),
            thickness: px(1.),
            wavy: false,
        });
        let runs: Vec<TextRun> = match &field.marked {
            Some(marked) => vec![
                run(marked.start, None),
                run(marked.len(), underline),
                run(text.len() - marked.end, None),
            ],
            None => vec![run(text.len(), None)],
        }
        .into_iter()
        .filter(|run| run.len > 0)
        .collect();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);
        let (selected, cursor) = (field.selected(), field.selection.extent);
        let marker = if selected.is_empty() {
            fill(
                Bounds::new(
                    point(bounds.left() + line.x_for_index(cursor), bounds.top()),
                    size(px(2.), bounds.size.height),
                ),
                rgb(ACCENT),
            )
        } else {
            fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected.end),
                        bounds.bottom(),
                    ),
                ),
                rgba(0x3b82f640),
            )
        };
        let focused = field.focus_handle.is_focused(window);
        let layout = ShapedField {
            content: &field.content,
            line: (!field.content.is_empty()).then_some(&line),
            bounds,
        };
        let overlay = field.selection_ui.as_ref().map(|engine| {
            engine.overlay(
                &Field {
                    selection: field.selection,
                    focused,
                    lines: Lines::Single,
                    screen: Bounds::new(Point::default(), window.viewport_size()),
                },
                &layout,
            )
        });
        let (handles, toolbar, magnifier) = match overlay {
            Some(overlay) => (
                overlay.handles,
                overlay.toolbar.map_or_else(Vec::new, |anchors| {
                    let labels: Vec<_> = TOOLBAR
                        .iter()
                        .map(|(label, action)| {
                            let runs = [TextRun {
                                len: label.len(),
                                color: hsla(0., 0., 1., 1.),
                                ..run(0, None)
                            }];
                            let label = window.text_system().shape_line(
                                (*label).into(),
                                px(16.),
                                &runs,
                                None,
                            );
                            (label, *action)
                        })
                        .collect();
                    let width = labels
                        .iter()
                        .map(|(label, _)| label.width + TOOLBAR_PADDING * 2.)
                        .fold(px(0.), |sum, width| sum + width);
                    let screen = Bounds::new(Point::default(), window.viewport_size());
                    let origin = toolbar_origin(
                        anchors,
                        Size {
                            width,
                            height: TOOLBAR_HEIGHT,
                        },
                        screen,
                        // The status bar takes touches there (Android draws edge to edge).
                        window.insets().safe_area.top,
                    );
                    let mut x = origin.x;
                    labels
                        .into_iter()
                        .map(|(label, action)| {
                            let width = label.width + TOOLBAR_PADDING * 2.;
                            let bounds =
                                Bounds::new(point(x, origin.y), size(width, TOOLBAR_HEIGHT));
                            x += width;
                            ToolbarButton {
                                label,
                                bounds,
                                action,
                            }
                        })
                        .collect()
                }),
                overlay.magnifier,
            ),
            None => (Vec::new(), Vec::new(), None),
        };
        let overlay = DrawnOverlay {
            handles,
            toolbar,
            magnifier,
        };
        // On top of everything painted after the field: handles and the toolbar hang outside it.
        let drawn = overlay.clone();
        let mut on_top = canvas(
            |_, _, _| {},
            move |_, _, window, cx| paint_overlay(&drawn, window, cx),
        )
        .into_any_element();
        on_top.layout_as_root(AvailableSpace::min_size(), window, cx);
        window.defer_draw(on_top, Point::default(), 1, None);
        Prepaint {
            line,
            marker,
            overlay,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&parallax::gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.field.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let focused = focus_handle.is_focused(window);
        let selecting = !self.field.read(cx).selection.is_collapsed();
        if focused || selecting {
            window.paint_quad(prepaint.marker.clone());
        }
        if let Err(error) = prepaint.line.paint(
            bounds.origin,
            window.line_height(),
            parallax::gpui::TextAlign::Left,
            None,
            window,
            cx,
        ) {
            log::error!("cannot paint the text field: {error:#}");
        }
        listen_for_selection_gestures(&self.field, bounds, &prepaint.overlay, window);
        let line = prepaint.line.clone();
        let caret = focused.then_some(prepaint.marker.bounds);
        let caret_moved = self.field.update(cx, |field, _| {
            // The layout belongs to the content only when the placeholder is not showing.
            field.last_layout = (!field.content.is_empty()).then_some(line);
            field.last_bounds = Some(bounds);
            std::mem::replace(&mut field.ime_caret, caret) != caret
        });
        // A moved caret moves the IME's candidate window (the web puts its IME mirror there). Asked here, after
        // the layout that moved it, so the next frame reads the new bounds.
        if caret_moved && focused {
            window.invalidate_character_coordinates();
        }
    }
}

/// Draws the overlay `parallax_ext` laid out, as plain shapes: Material's handle (a circle with a square in the
/// quadrant pointing at the text), a bordered loupe (GPUI can't magnify what's behind it), a dark toolbar.
fn paint_overlay(overlay: &DrawnOverlay, window: &mut Window, cx: &mut App) {
    let accent: Hsla = rgb(ACCENT).into();
    for handle in &overlay.handles {
        let rect = handle.rect;
        let half = rect.size.width / 2.;
        window.paint_quad(fill(rect, accent).corner_radii(half));
        let corner = match handle.kind {
            HandleType::Left => {
                Bounds::new(point(rect.left() + half, rect.top()), size(half, half))
            }
            HandleType::Right => Bounds::new(rect.origin, size(half, half)),
            // Points straight up: a square stem on the circle's top.
            HandleType::Collapsed => Bounds::new(
                point(rect.left() + half / 2., rect.top() - half / 2.),
                size(half, half),
            ),
        };
        window.paint_quad(fill(corner, accent));
    }
    if let Some(magnifier) = overlay.magnifier {
        window.paint_quad(
            fill(magnifier.rect, hsla(0., 0., 0.62, 0.15))
                .corner_radii(magnifier.rect.size.height / 2.)
                .border_widths(px(1.5))
                .border_color(hsla(0., 0., 0., 0.25)),
        );
    }
    if let (Some(first), Some(last)) = (overlay.toolbar.first(), overlay.toolbar.last()) {
        let bar = Bounds::from_corners(first.bounds.origin, last.bounds.bottom_right());
        window.paint_quad(fill(bar, rgb(0x45475a)).corner_radii(px(8.)));
        for button in &overlay.toolbar {
            let origin = point(
                button.bounds.left() + TOOLBAR_PADDING,
                button.bounds.top() + (TOOLBAR_HEIGHT - px(24.)) / 2.,
            );
            if let Err(error) = button.label.paint(
                origin,
                px(24.),
                parallax::gpui::TextAlign::Left,
                None,
                window,
                cx,
            ) {
                log::error!("cannot paint the selection toolbar: {error:#}");
            }
        }
    }
}

/// The selection gestures GPUI delivers outside the field's own mouse-down: mouse moves and releases (they may
/// leave the field), the long press and handle drags (touch) and the toolbar's buttons.
fn listen_for_selection_gestures(
    field: &Entity<TextField>,
    bounds: Bounds<Pixels>,
    overlay: &DrawnOverlay,
    window: &mut Window,
) {
    let send = |field: &Entity<TextField>, input, window: &mut Window, cx: &mut App| {
        field.update(cx, |field, cx| field.selection_input(input, window, cx));
    };
    let entity = field.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.pressed_button == Some(MouseButton::Left) {
            send(
                &entity,
                Input::Move {
                    position: event.position,
                },
                window,
                cx,
            );
        }
    });
    let entity = field.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
            send(&entity, Input::Up, window, cx);
        }
    });
    // The field's text area plus its padding: where a long press selects.
    let pressable = bounds.dilate(px(8.));
    let entity = field.clone();
    window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let input = match event.phase {
            TouchPhase::Started if pressable.contains(&event.start_position) => {
                // Claims the press, so its moves follow.
                window.prevent_default();
                cx.stop_propagation();
                Input::LongPressStart {
                    position: event.start_position,
                }
            }
            TouchPhase::Started => return,
            TouchPhase::Moved => Input::LongPressMove {
                position: event.position,
            },
            TouchPhase::Ended => Input::LongPressEnd,
            TouchPhase::Cancelled => Input::LongPressCancel,
        };
        send(&entity, input, window, cx);
    });
    let hits: Vec<(Handle, Bounds<Pixels>)> = overlay
        .handles
        .iter()
        .map(|handle| (handle.handle, handle.hit))
        .collect();
    // The toolbar sits above the handles (Flutter inserts it after them), so its buttons win a touch both reach.
    let on_toolbar: Vec<Bounds<Pixels>> =
        overlay.toolbar.iter().map(|button| button.bounds).collect();
    let entity = field.clone();
    window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        // A drag claimed at touch down is a handle's; one won past the slop moves the caret (or selects).
        let input = match (event.start, event.phase) {
            (TouchDragStart::Down, TouchPhase::Started) => {
                let toolbar = on_toolbar
                    .iter()
                    .any(|button| button.contains(&event.start_position));
                let handle = hits
                    .iter()
                    .find(|(_, hit)| hit.contains(&event.start_position));
                match (toolbar, handle) {
                    // Claims the touch: a drag on a handle never becomes a tap or a scroll.
                    (false, Some(&(handle, _))) => {
                        window.prevent_default();
                        cx.stop_propagation();
                        Input::HandleDragStart {
                            handle,
                            position: event.start_position,
                        }
                    }
                    // Competes, as Flutter's tap-and-drag recognizer does: taps and long presses stay the
                    // field's until the touch passes the drag's slop.
                    (false, None) if pressable.contains(&event.start_position) => {
                        let kind = entity
                            .read(cx)
                            .selection_ui
                            .as_ref()
                            .map(TextSelection::touch_drag_kind);
                        if let Some(kind) = kind {
                            window.compete_for_touch_drag(kind);
                            cx.stop_propagation();
                        }
                        return;
                    }
                    (true, _) | (false, None) => return,
                }
            }
            (TouchDragStart::Down, TouchPhase::Moved) => Input::HandleDragUpdate {
                position: event.position,
            },
            (TouchDragStart::Down, TouchPhase::Ended | TouchPhase::Cancelled) => {
                Input::HandleDragEnd
            }
            // Every field listens: a won drag is this field's only if it started on it.
            (TouchDragStart::PastSlop(_), _) if !pressable.contains(&event.start_position) => {
                return;
            }
            // Won: the press the drag grew from, where the touch started, then the move that won it.
            (TouchDragStart::PastSlop(_), TouchPhase::Started) => {
                cx.stop_propagation();
                send(
                    &entity,
                    Input::Down {
                        position: event.start_position,
                        device: window
                            .pointer_source()
                            .map_or(PointerDevice::Touch, |source| source.device),
                        tap_count: 1,
                        shift: false,
                    },
                    window,
                    cx,
                );
                Input::Move {
                    position: event.position,
                }
            }
            (TouchDragStart::PastSlop(_), TouchPhase::Moved) => Input::Move {
                position: event.position,
            },
            (TouchDragStart::PastSlop(_), TouchPhase::Ended | TouchPhase::Cancelled) => Input::Up,
        };
        send(&entity, input, window, cx);
    });
    let buttons: Vec<_> = overlay
        .toolbar
        .iter()
        .map(|button| (button.bounds, button.action))
        .collect();
    let entity = field.clone();
    // Capture: the toolbar sits over other elements, and its press must not reach them.
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        if let Some(&(_, action)) = buttons
            .iter()
            .find(|(bounds, _)| bounds.contains(&event.position))
        {
            cx.stop_propagation();
            entity.update(cx, |field, cx| action(field, window, cx));
        }
    });
}

/// The field on GPUI's test platform (fixed-width glyphs) under real mouse dispatch, by the host desktop's row of
/// the behaviour table: clicks, multi-clicks, drags and shift-clicks select as Flutter's desktop rules say.
#[cfg(test)]
mod tests {
    use parallax::gpui::{
        Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext,
        TextInputConfiguration, VisualTestContext, point,
    };

    use std::time::Duration;

    use parallax::gpui::{
        Context, Entity, IntoElement, Pixels, Point, PointerDevice, Render, TouchEvent, TouchId,
        TouchPhase, Window, div, prelude::*, px,
    };
    use parallax::parallax_platform::{Target, behaviour::behaviour};

    use super::{Selection, TextField, TextSelection};

    /// One mouse event at a text offset: down with a click count (and shift), a pressed move, or up.
    #[derive(Clone, Copy)]
    enum Mouse {
        Down(usize, usize, bool),
        Drag(usize),
        Up(usize),
    }

    fn click(offset: usize, count: usize) -> [Mouse; 2] {
        [Mouse::Down(offset, count, false), Mouse::Up(offset)]
    }

    #[gpui::test]
    fn mouse_selects_by_the_desktop_row(cx: &mut TestAppContext) {
        use Mouse::{Down, Drag, Up};
        // (Flutter behaviour, mouse events, expected selection)
        let rows: [(&str, Vec<Mouse>, (usize, usize)); 6] = [
            (
                "a click places the caret on tap down",
                click(5, 1).to_vec(),
                (5, 5),
            ),
            (
                "a double click selects the word",
                [click(5, 1), click(5, 2)].concat(),
                (4, 7),
            ),
            (
                "a triple click selects all of a single-line field",
                [click(5, 1), click(5, 2), click(5, 3)].concat(),
                (0, 11),
            ),
            (
                "a drag selects from where it started",
                vec![Down(4, 1, false), Drag(7), Drag(10), Up(10)],
                (4, 10),
            ),
            (
                "a double click and drag selects word by word",
                [&click(5, 1)[..], &[Down(5, 2, false), Drag(9), Up(9)]].concat(),
                (4, 11),
            ),
            (
                "a shift click extends the selection",
                [&click(3, 1)[..], &[Down(8, 1, true), Up(8)]].concat(),
                (3, 8),
            ),
        ];
        for (name, events, (base, extent)) in rows {
            let (field, cx) = cx.add_window_view(|_, cx| TextField {
                content: "abc def ghi".into(),
                ..TextField::new("", TextInputConfiguration::default(), cx)
            });
            cx.run_until_parked();
            let at = |cx: &mut VisualTestContext, offset| {
                field.read_with(cx, |field, _| {
                    let bounds = field.last_bounds.expect("the field was drawn");
                    let line = field.last_layout.as_ref().expect("the field has text");
                    point(bounds.left() + line.x_for_index(offset), bounds.center().y)
                })
            };
            for event in events {
                match event {
                    Down(offset, click_count, shift) => {
                        let position = at(cx, offset);
                        cx.simulate_event(MouseDownEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers {
                                shift,
                                ..Modifiers::default()
                            },
                            click_count,
                            first_mouse: false,
                        })
                    }
                    Drag(offset) => {
                        let position = at(cx, offset);
                        cx.simulate_event(MouseMoveEvent {
                            position,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Modifiers::default(),
                        })
                    }
                    Up(offset) => {
                        let position = at(cx, offset);
                        cx.simulate_event(MouseUpEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                        })
                    }
                }
            }
            let found = field.read_with(cx, |field, _| field.selection);
            assert_eq!(found, Selection { base, extent }, "{name}");
        }
    }

    /// One touch over the field, at text offsets: a tap, a press held past the long-press deadline, a horizontal
    /// drag from one offset to another, or a vertical drag of some pixels from an offset.
    #[derive(Clone, Copy)]
    enum Touch {
        Tap(usize),
        Hold(usize),
        Drag(usize, usize),
        Vertical(usize, f32),
    }

    /// Two fields one above the other, as in hello's window: each listens to every touch drag.
    struct TwoFields([Entity<TextField>; 2]);

    impl Render for TwoFields {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().flex().flex_col().gap_4().children(self.0.clone())
        }
    }

    #[gpui::test]
    fn touch_selects_by_the_android_row(cx: &mut TestAppContext) {
        use Touch::{Drag, Hold, Tap, Vertical};
        let android = behaviour(Some(Target::Android)).expect("Android has a row");
        // (Flutter behaviour, touches, expected selection)
        let rows: [(&str, Vec<Touch>, (usize, usize)); 5] = [
            ("a tap places the caret", vec![Tap(5)], (5, 5)),
            ("a long press selects the word", vec![Hold(5)], (4, 7)),
            (
                "a horizontal finger drag moves the caret in a focused field",
                vec![Tap(1), Drag(2, 9)],
                (9, 9),
            ),
            (
                "a finger drag does not start in an unfocused field",
                vec![Drag(2, 9)],
                (0, 0),
            ),
            (
                "a vertical drag is the pan's: the caret stays",
                vec![Tap(1), Vertical(2, 60.)],
                (1, 1),
            ),
        ];
        for (name, touches, (base, extent)) in rows {
            let (fields, cx) = cx.add_window_view(|_, cx| {
                TwoFields([(); 2].map(|()| {
                    cx.new(|cx| TextField {
                        content: "abc def ghi".into(),
                        selection_ui: TextSelection::for_behaviour(android),
                        ..TextField::new("", TextInputConfiguration::default(), cx)
                    })
                }))
            });
            cx.run_until_parked();
            let [field, other] = fields.read_with(cx, |fields, _| fields.0.clone());
            let at = |cx: &mut VisualTestContext, offset| {
                field.read_with(cx, |field, _| {
                    let bounds = field.last_bounds.expect("the field was drawn");
                    let line = field.last_layout.as_ref().expect("the field has text");
                    point(bounds.left() + line.x_for_index(offset), bounds.center().y)
                })
            };
            for (id, touch) in touches.into_iter().enumerate() {
                // A frame apart, and each touch later than a double tap's interval after the last.
                let send = |cx: &mut VisualTestContext, phase, position| {
                    cx.simulate_event(TouchEvent {
                        id: TouchId(id as u64),
                        phase,
                        position,
                        predicted_position: None,
                        force: None,
                        device: PointerDevice::Touch,
                        time_stamp: None,
                    });
                    cx.executor().advance_clock(Duration::from_millis(16));
                    cx.run_until_parked();
                };
                let (start, path): (_, Vec<Point<Pixels>>) = match touch {
                    Tap(offset) | Hold(offset) => (at(cx, offset), Vec::new()),
                    Drag(from, to) => {
                        let (from, to) = (at(cx, from), at(cx, to));
                        let steps = (1..=8).map(|step| from + (to - from) * (step as f32 / 8.));
                        (from, steps.collect())
                    }
                    Vertical(from, dy) => {
                        let from = at(cx, from);
                        (
                            from,
                            (1..=8)
                                .map(|step| from + point(px(0.), px(dy * step as f32 / 8.)))
                                .collect(),
                        )
                    }
                };
                send(cx, TouchPhase::Started, start);
                if let Hold(_) = touch {
                    cx.executor().advance_clock(Duration::from_millis(600));
                    cx.run_until_parked();
                }
                for position in &path {
                    send(cx, TouchPhase::Moved, *position);
                }
                send(cx, TouchPhase::Ended, path.last().copied().unwrap_or(start));
                cx.executor().advance_clock(Duration::from_millis(500));
                cx.run_until_parked();
            }
            let found = field.read_with(cx, |field, _| field.selection);
            assert_eq!(found, Selection { base, extent }, "{name}");
            let untouched = other.read_with(cx, |field, _| field.selection);
            assert_eq!(untouched, Selection::default(), "{name}: the other field");
        }
    }
}
