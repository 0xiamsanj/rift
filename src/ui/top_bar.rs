use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::ui::icons::Icons;
use crate::ui::pane_state::PaneId;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_keycap(&self, key: &str) -> impl IntoElement {
        div()
            .px(px(3.0))
            .py(px(0.5))
            .rounded_sm()
            .bg(self.theme.keycap_bg)
            .border_1()
            .border_color(self.theme.keycap_border)
            .font_family(self.theme.font_mono)
            .text_size(self.theme.font_size_shortcut)
            .font_weight(FontWeight::NORMAL)
            .text_color(self.theme.keycap_text)
            .child(key.to_string())
    }

    pub fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_connected = self.remote_client.is_connected();
        let connected_label = if is_connected {
            if let Some(prof) = self.remote_client.current_profile.lock().as_ref() {
                format!("{}@{}", prof.username, prof.host)
            } else {
                "SFTP Connected".to_string()
            }
        } else {
            "Disconnected".to_string()
        };
        let is_left = self.active_pane == PaneId::Left;

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(32.0))
            .px_3()
            .bg(self.theme.title_bar_background)
            .border_b_1()
            .border_color(self.theme.border)
            // Left: branding & connection (flex_1 mirrors the right
            // group so the search field stays exactly centered, which is
            // what the palette dropdown aligns to).
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_start()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.accent)
                            .font_weight(FontWeight::NORMAL)
                            .text_size(self.theme.font_size_ui)
                            .child("✦ Rift"),
                    )
                    .child(div().w(px(1.0)).h(px(12.0)).bg(self.theme.border).mx_1())
                    .when(is_connected, |el| {
                        let conn_lbl = connected_label.clone();
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .px_1p5()
                                .py(px(1.5))
                                .rounded_md()
                                .child(
                                    div()
                                        .text_color(self.theme.success)
                                        .text_size(px(8.0))
                                        .font_family(Icons::FAMILY)
                                        .child(Icons::DOT),
                                )
                                .child(
                                    div()
                                        .font_family(self.theme.font_mono)
                                        .text_size(self.theme.font_size_caption)
                                        .text_color(self.theme.text_muted)
                                        .child(conn_lbl),
                                )
                                .child(crate::ui::controls::btn(
                                    self.theme,
                                    crate::ui::controls::BtnVariant::Secondary,
                                    "btn-disconnect",
                                    "Disconnect",
                                    true,
                                    cx.listener(|this, _ev, _window, cx| {
                                        this.disconnect_remote();
                                        cx.notify();
                                    }),
                                )),
                        )
                    })
                    .when(!is_connected, |el| {
                        el.child(
                            div()
                                .id("btn-quick-connect")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_1()
                                .px_2()
                                .py(px(2.0))
                                .rounded_md()
                                .bg(self.theme.bg_badge_accent)
                                .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                                .text_size(self.theme.font_size_caption)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(self.theme.accent)
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                    this.open_connect_modal();
                                    cx.notify();
                                }))
                                .child("+ Connect SFTP"),
                        )
                    }),
            )
            // Center: palette trigger that becomes the live query field
            // while the dropdown is open.
            .child({
                let palette_open = matches!(self.modal, crate::ui::modals::ActiveModal::CommandPalette(_));
                let query_text = match &self.modal {
                    crate::ui::modals::ActiveModal::CommandPalette(state) => state.query.clone(),
                    _ => String::new(),
                };
                div()
                    .id("btn-command-palette")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w(px(300.0))
                    .h(px(22.0))
                    .px_2p5()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(if palette_open { self.theme.accent } else { self.theme.border })
                    .hover(|s| s.border_color(self.theme.accent))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                        this.open_command_palette();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_size(self.theme.font_size_caption)
                                    .font_family(Icons::FAMILY)
                                    .text_color(if palette_open {
                                        self.theme.accent
                                    } else {
                                        self.theme.text_placeholder
                                    })
                                    .child(Icons::SEARCH),
                            )
                            .child(
                                div()
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(if palette_open {
                                        self.theme.text
                                    } else {
                                        self.theme.text_placeholder
                                    })
                                    .child(if palette_open {
                                        if query_text.is_empty() {
                                            "Type a command... |".to_string()
                                        } else {
                                            format!("{}|", query_text)
                                        }
                                    } else {
                                        "Commands, files, or paths...".to_string()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .px_1()
                            .py(px(0.5))
                            .rounded_md()
                            .bg(self.theme.keycap_bg)
                            .border_1()
                            .border_color(self.theme.keycap_border)
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_shortcut)
                            .text_color(self.theme.keycap_text)
                            .child(if palette_open { "esc" } else { "Ctrl+P" }),
                    )
            })
            // Right: Essential Quick Actions (mirrors left for centering)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(
                        div()
                            .id("btn-quick-transfer")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .h(px(22.0))
                            .px_2()
                            .rounded_md()
                            .bg(self.theme.bg_badge_accent)
                            .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                            .text_size(self.theme.font_size_caption)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.accent)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.transfer_selected(this.active_pane, cx);
                                cx.notify();
                            }))
                            .child(crate::ui::motion::flash(
                                self.animations_enabled,
                                div().child(if is_left { "Transfer →" } else { "← Transfer" }),
                                ElementId::Name(format!("transfer-label-{}", is_left).into()),
                            ))
                            .child(self.render_keycap("F5")),
                    )
                    .child(
                        div()
                            .id("btn-toggle-transfers")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .h(px(22.0))
                            .px_2()
                            .rounded_md()
                            .when(self.show_transfer_drawer, |s| s.bg(self.theme.element_selected))
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .text_size(self.theme.font_size_caption)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.show_transfer_drawer = !this.show_transfer_drawer;
                                cx.notify();
                            }))
                            .child("Queue")
                            .when(!self.transfer_items.is_empty(), |el| {
                                el.child(
                                    div()
                                        .px(px(3.0))
                                        .py(px(0.5))
                                        .rounded_md()
                                        .bg(self.theme.bg_badge_accent)
                                        .font_family(self.theme.font_mono)
                                        .text_size(self.theme.font_size_shortcut)
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(self.theme.accent)
                                        .child(self.transfer_items.len().to_string()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("btn-toggle-bookmarks")
                            .tooltip(tooltip_view(self.theme, "Bookmarks", Some("Ctrl+B")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .text_size(self.theme.font_size_caption)
                            .font_family(Icons::FAMILY)
                            .text_color(self.theme.text_muted)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.open_bookmarks_modal();
                                cx.notify();
                            }))
                            .child(Icons::BOOKMARK),
                    )
                    .child(
                        div()
                            .id("btn-toggle-theme")
                            .tooltip(tooltip_view(self.theme, "Toggle Theme", Some("Ctrl+T")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .text_size(self.theme.font_size_caption)
                            .text_color(self.theme.text_muted)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.toggle_theme();
                                cx.notify();
                            }))
                            .child(crate::ui::motion::flash(
                                self.animations_enabled,
                                div().child(if self.theme.is_dark { "☀" } else { "☾" }),
                                ElementId::Name(format!("theme-icon-{}", self.theme.is_dark).into()),
                            )),
                    ),
            )
    }
}
