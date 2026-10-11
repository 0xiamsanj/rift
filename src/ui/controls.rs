use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::theme::Theme;
use crate::ui::icons::Icons;
use crate::ui::modals::ActiveModal;
use crate::ui::tooltip::tooltip_view;

/// Shared dialog and control primitives (zed_ui.md §3–§4).
/// Every modal must compose these instead of hand-rolling its own
/// container, buttons, or inputs, so fixes land at the system level.
pub struct DialogHeader<'a> {
    pub title: &'a str,
    pub icon: (&'a str, Rgba),
    pub description: Option<&'a str>,
}

/// Dialog container with icon title, dismiss button, optional description,
/// content, and optional footer. Widths come from theme modal tokens.
pub fn dialog_shell(
    theme: Theme,
    cx: &mut Context<RiftApp>,
    header: DialogHeader,
    content: AnyElement,
    footer: Option<AnyElement>,
    width: Pixels,
) -> Div {
    let (glyph, color) = header.icon;
    div()
        .flex()
        .flex_col()
        .w(width)
        .px_5()
        .py_4()
        .rounded_md()
        .bg(theme.elevated_surface_background)
        .border_1()
        .border_color(theme.border)
        .shadow_md()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(40.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(color)
                                .text_size(px(14.0))
                                .child(glyph.to_string()),
                        )
                        .child(
                            div()
                                .text_size(theme.font_size_modal_title)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(theme.text_muted)
                                .child(header.title.to_string()),
                        ),
                )
                .child(dialog_close_btn(cx, theme)),
        )
        .when_some(header.description, |el, desc: &str| {
            el.child(
                div()
                    .text_size(theme.font_size_ui)
                    .text_color(theme.text_muted)
                    .child(desc.to_string()),
            )
        })
        .child(content)
        .when_some(footer, |el, f| el.child(f))
}

/// Right-aligned footer row for dialog action buttons (Zed ModalFooter:
/// top border, end-aligned actions).
pub fn dialog_footer(theme: Theme, buttons: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .pt_2()
        .border_t_1()
        .border_color(theme.border_variant)
        .children(buttons)
}

#[derive(Clone, Copy)]
pub enum BtnVariant {
    Primary,
    Secondary,
    Danger,
}

/// Button with primary/secondary/danger variants and a disabled state
/// (dimmed, clicks ignored). Covers normal/hover/disabled/loading looks;
/// GPUI exposes no pressed pseudo-state, so hover doubles as feedback.
pub fn btn(
    theme: Theme,
    variant: BtnVariant,
    id: &'static str,
    label: &str,
    enabled: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let (bg, bg_hover, fg, hover_fg) = match variant {
        BtnVariant::Primary => (theme.accent, theme.accent_hover, rgb(0xffffff), rgb(0xffffff)),
        BtnVariant::Secondary => (
            theme.element_background,
            theme.element_hover,
            theme.text_muted,
            theme.text,
        ),
        BtnVariant::Danger => (theme.danger, theme.danger_hover, rgb(0xffffff), rgb(0xffffff)),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(26.0))
        .gap_1()
        .px(px(14.0))
        .py(px(5.0))
        .rounded(px(5.0))
        .bg(bg)
        .text_size(theme.font_size_ui)
        .font_weight(FontWeight::NORMAL)
        .text_color(fg)
        .when(enabled, |el| {
            el.cursor_pointer()
                .hover(move |s| s.bg(bg_hover).text_color(hover_fg))
        })
        .when(!enabled, |el| el.opacity(0.55))
        .when(enabled, |el| el.on_mouse_down(MouseButton::Left, on_click))
        .child(label.to_string())
}

/// Labeled text-input lookalike (fake inputs render text + caret;
/// behavior attaches via `on_click`). `focused` uses the focus token.
pub fn text_input(
    theme: Theme,
    id: &'static str,
    focused: bool,
    text: String,
    mono: bool,
    masked: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let display = if masked {
        // Mask everything but a trailing caret ("|" or "| "), which the
        // fake inputs append to the value string.
        let suffix_len = if text.ends_with("| ") {
            2
        } else if text.ends_with('|') {
            1
        } else {
            0
        };
        let body: String = text
            .chars()
            .take(text.chars().count() - suffix_len)
            .collect();
        "•".repeat(body.chars().count()) + &text[text.len() - suffix_len..]
    } else {
        text
    };
    let mut el = div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .h(px(26.0))
        .px_3()
        .py(px(5.0))
        .rounded_md()
        .bg(theme.element_background)
        .border_1()
        .border_color(if focused {
            theme.border_focused
        } else {
            theme.border
        })
        .text_size(theme.font_size_body)
        .text_color(theme.text);
    if mono {
        el = el.font_family(theme.font_mono);
    }
    el.on_mouse_down(MouseButton::Left, on_click)
        .child(display)
}

/// Small caption label above an input.
pub fn field_label(theme: Theme, text: &str) -> Div {
    div()
        .text_size(theme.font_size_caption)
        .text_color(theme.text_muted)
        .child(text.to_string())
}

/// Validation/error box (danger tint per Zed status tokens).
pub fn error_box(theme: Theme, msg: &str) -> Div {
    div()
        .p_2()
        .rounded_md()
        .bg(theme.bg_badge_danger)
        .border_1()
        .border_color(theme.danger)
        .text_size(theme.font_size_caption)
        .text_color(theme.danger)
        .child(msg.to_string())
}

/// Standard dismiss (×) button for dialog corners.
pub fn dialog_close_btn(cx: &mut Context<RiftApp>, theme: Theme) -> Stateful<Div> {
    div()
        .id("btn-modal-close")
        .tooltip(tooltip_view(theme, "Close", Some("Esc")))
        .cursor_pointer()
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .hover(|s| s.bg(theme.ghost_element_hover))
        .text_size(theme.font_size_ui)
        .font_family(Icons::FAMILY)
        .text_color(theme.text_muted)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _ev, _window, cx| {
                this.modal = ActiveModal::None;
                cx.notify();
            }),
        )
        .child(Icons::CLOSE)
}
