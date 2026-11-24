use std::ops::Range;

use gpui::{prelude::*, *};

pub(super) struct TextChanged;

pub(super) struct TextInput {
    id: &'static str,
    placeholder: &'static str,
    focus: FocusHandle,
    value: String,
    anchor: usize,
    cursor: usize,
    marked: Option<Range<usize>>,
    layout: Option<(ShapedLine, Bounds<Pixels>)>,
}

impl EventEmitter<TextChanged> for TextInput {}

impl TextInput {
    pub(super) fn new(id: &'static str, placeholder: &'static str, cx: &mut Context<Self>) -> Self {
        Self {
            id,
            placeholder,
            focus: cx.focus_handle(),
            value: String::new(),
            anchor: 0,
            cursor: 0,
            marked: None,
            layout: None,
        }
    }

    pub(super) fn value(&self) -> &str {
        &self.value
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        self.replace(0..self.value.len(), "", cx);
    }

    fn selection(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    fn move_to(&mut self, index: usize, select: bool) {
        self.cursor = index;
        if !select {
            self.anchor = index;
        }
    }

    fn index_at(&self, x: Pixels) -> usize {
        self.layout
            .as_ref()
            .map_or(self.value.len(), |(line, bounds)| {
                let index = line.closest_index_for_x(x - bounds.left());
                self.value.floor_char_boundary(index.min(self.value.len()))
            })
    }

    fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) -> Range<usize> {
        let text: String = text
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect();
        let end = range.start + text.len();
        self.value.replace_range(range.clone(), &text);
        self.marked = None;
        self.move_to(end, false);
        cx.emit(TextChanged);
        cx.notify();
        range.start..end
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let previous = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        let next = self.cursor
            + self.value[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        let selection = self.selection();
        match (key.modifiers.control, key.key.as_str()) {
            (true, "a") => {
                self.anchor = 0;
                self.cursor = self.value.len();
            }
            (true, "c" | "x") => {
                if !selection.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.value[selection.clone()].to_owned(),
                    ));
                    if key.key == "x" {
                        self.replace(selection, "", cx);
                    }
                }
            }
            (true, "v") => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.replace(selection, &text, cx);
                }
            }
            (false, "backspace") => {
                let range = if selection.is_empty() {
                    previous..self.cursor
                } else {
                    selection
                };
                self.replace(range, "", cx);
            }
            (false, "delete") => {
                let range = if selection.is_empty() {
                    self.cursor..next
                } else {
                    selection
                };
                self.replace(range, "", cx);
            }
            (false, "left") => {
                let target = if selection.is_empty() || key.modifiers.shift {
                    previous
                } else {
                    selection.start
                };
                self.move_to(target, key.modifiers.shift);
            }
            (false, "right") => {
                let target = if selection.is_empty() || key.modifiers.shift {
                    next
                } else {
                    selection.end
                };
                self.move_to(target, key.modifiers.shift);
            }
            (false, "home") => self.move_to(0, key.modifiers.shift),
            (false, "end") => self.move_to(self.value.len(), key.modifiers.shift),
            (false, "escape") => {
                self.clear(cx);
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn utf16_range(&self, range: Range<usize>) -> Range<usize> {
        self.value[..range.start].encode_utf16().count()
            ..self.value[..range.end].encode_utf16().count()
    }

    fn replacement_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        range
            .map(|range| byte_range(&self.value, range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection())
    }
}

fn byte_range(text: &str, range: Range<usize>) -> Range<usize> {
    fn offset(text: &str, target: usize) -> usize {
        let mut units = 0;
        for (index, ch) in text.char_indices() {
            if units >= target {
                return index;
            }
            units += ch.len_utf16();
        }
        text.len()
    }
    offset(text, range.start)..offset(text, range.end)
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = byte_range(&self.value, range);
        *actual = Some(self.utf16_range(range.clone()));
        Some(self.value[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16_range(self.selection()),
            reversed: self.cursor < self.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|range| self.utf16_range(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(self.replacement_range(range), text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.replace(self.replacement_range(range), text, cx);
        if let Some(selection) = selection {
            let selection = byte_range(&self.value[inserted.clone()], selection);
            self.anchor = inserted.start + selection.start;
            self.cursor = inserted.start + selection.end;
        }
        self.marked = (!inserted.is_empty()).then_some(inserted);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (line, bounds) = self.layout.as_ref()?;
        let range = byte_range(&self.value, range);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.value[..self.index_at(point.x)].encode_utf16().count())
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let focus = self.focus.clone();
        let entity = cx.entity();
        let selection = self.selection();
        let cursor = self.cursor;
        let placeholder = self.value.is_empty();
        let display: SharedString = if placeholder {
            self.placeholder.into()
        } else {
            self.value.clone().into()
        };
        div()
            .id(self.id)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .rounded_md()
            .border_1()
            .border_color(rgb(if focused { 0xba9cff } else { 0x48484c }))
            .bg(rgb(0x28282a))
            .overflow_hidden()
            .p(px(8.0))
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.focus.focus(window);
                    this.move_to(this.index_at(event.position.x), event.modifiers.shift);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) && this.focus.is_focused(window)
                {
                    this.move_to(this.index_at(event.position.x), true);
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |bounds, window, _| {
                        let run = TextRun {
                            len: display.len(),
                            font: window.text_style().font(),
                            color: rgb(if placeholder { 0xa8a8aa } else { 0xf4f4f5 }).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line = window
                            .text_system()
                            .shape_line(display, px(13.0), &[run], None);
                        let offset =
                            (line.x_for_index(cursor) - bounds.size.width + px(2.0)).max(px(0.0));
                        let origin = point(bounds.left() - offset, bounds.top());
                        (line, Bounds::new(origin, bounds.size))
                    },
                    move |bounds, (line, layout), window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        if focused && !selection.is_empty() {
                            let left = line.x_for_index(selection.start);
                            let right = line.x_for_index(selection.end);
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(layout.left() + left.min(right), layout.top()),
                                    size((right - left).abs(), layout.size.height),
                                ),
                                rgba(0xba9cff55),
                            ));
                        }
                        let _ = line.paint(layout.origin, layout.size.height, window, cx);
                        if focused && selection.is_empty() {
                            let x = layout.left() + line.x_for_index(cursor);
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x, layout.top() + px(2.0)),
                                    size(px(1.0), px(16.0)),
                                ),
                                rgb(0xba9cff),
                            ));
                        }
                        entity.update(cx, |input, _| input.layout = Some((line, layout)));
                    },
                )
                .w_full()
                .h(px(20.0)),
            )
    }
}
