//! A single-line text field on GPUI's `EntityInputHandler`: the smallest one that exercises the platform's text
//! input (IME composition, selection, UTF-16 ranges). Upstream GPUI has no ready field outside Zed's editor
//! (GPL, never copied); this is cut down from upstream's own `examples/input.rs` (Apache-2.0, Zed Industries).

use std::ops::Range;

use parallax::gpui::{
    App, Autocapitalize, Bounds, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style, TextInputAction,
    TextInputConfiguration, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    hsla, point, prelude::*, px, relative, rgb, rgba, size,
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

pub struct TextField {
    focus_handle: FocusHandle,
    /// UTF-8; GPUI's input handler speaks UTF-16, converted at the trait boundary.
    content: String,
    placeholder: SharedString,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
}

impl TextField {
    pub fn new(placeholder: &str, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: placeholder.to_string().into(),
            selected: 0..0,
            reversed: false,
            marked: None,
            last_layout: None,
            last_bounds: None,
        }
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = offset..offset;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
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
        if self.selected.is_empty() {
            let cursor = self.cursor();
            let other = towards(self, cursor);
            self.selected = cursor.min(other)..cursor.max(other);
        }
        self.replace_text_in_range(None, "", window, cx);
        window.notify_text_input_changed(&self.focus_handle);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        // Reopens a dismissed keyboard: Android shows it when the field gains focus, not on later taps.
        window.request_virtual_keyboard();
        let offset = match (&self.last_bounds, &self.last_layout) {
            (Some(bounds), Some(line)) => {
                line.closest_index_for_x(event.position.x - bounds.left())
            }
            (None, _) | (_, None) => self.content.len(),
        };
        self.move_to(offset, window, cx);
        cx.stop_propagation();
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
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = self.range_from_utf16(&range_utf16);
        self.reversed = false;
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A single-line field: line breaks become spaces.
        let new_text = new_text.replace(['\n', '\r'], " ");
        let range = range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.content.replace_range(range.clone(), &new_text);
        let inserted = range.start..range.start + new_text.len();
        self.marked = (!new_text.is_empty()).then_some(inserted.clone());
        self.selected = match new_selected_utf16 {
            Some(selected) => {
                let start = self.offset_to_utf16(inserted.start);
                self.range_from_utf16(&(start + selected.start..start + selected.end))
            }
            None => inserted.end..inserted.end,
        };
        self.reversed = false;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    /// Prose: the keyboard may suggest, correct and capitalize (GPUI's default turns all of that off).
    fn text_input_configuration(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        TextInputConfiguration {
            autocorrect: true,
            autocapitalize: Autocapitalize::Sentences,
            suggestions: true,
            input_action: TextInputAction::Done,
        }
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
                let to = this.previous_boundary(this.cursor());
                this.move_to(to, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Right, window, cx| {
                let to = this.next_boundary(this.cursor());
                this.move_to(to, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Home, window, cx| this.move_to(0, window, cx)))
            .on_action(
                cx.listener(|this, _: &End, window, cx| {
                    this.move_to(this.content.len(), window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.selected = 0..this.content.len();
                window.notify_text_selection_changed(&this.focus_handle);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Submit, _window, _cx| {
                log::info!("text field submitted: {:?}", this.content);
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
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

/// Paints the field's one shaped line, its selection or caret, and the composing underline.
struct TextElement {
    field: Entity<TextField>,
}

struct Prepaint {
    line: ShapedLine,
    marker: PaintQuad,
}

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
        let (selected, cursor) = (field.selected.clone(), field.cursor());
        let marker = if selected.is_empty() {
            fill(
                Bounds::new(
                    point(bounds.left() + line.x_for_index(cursor), bounds.top()),
                    size(px(2.), bounds.size.height),
                ),
                rgb(0x3b82f6),
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
        Prepaint { line, marker }
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
        let selecting = !self.field.read(cx).selected.is_empty();
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
        let line = prepaint.line.clone();
        self.field.update(cx, |field, _| {
            // The layout belongs to the content only when the placeholder is not showing.
            field.last_layout = (!field.content.is_empty()).then_some(line);
            field.last_bounds = Some(bounds);
        });
    }
}
