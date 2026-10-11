use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::theme::Theme;
use crate::ui::icons::Icons;
use crate::ui::modals::ActiveModal;
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
            .text_size(self.theme.font_size_caption)
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
            .h(px(30.0))
            .px_3()
            .bg(self.theme.bg_header)
            .border_b_1()
            .border_color(self.theme.border)
            // Left: Clean Branding & Connection State
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.accent)
                            .font_weight(FontWeight::BOLD)
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
                                .rounded_sm()
                                .child(
                                    div()
                                        .text_color(self.theme.success)
                                        .text_size(px(7.5))
                                        .child(Icons::DOT),
                                )
                                .child(
                                    div()
                                        .font_family(self.theme.font_mono)
                                        .text_size(self.theme.font_size_caption)
                                        .text_color(self.theme.text_secondary)
                                        .child(conn_lbl),
                                )
                                .child(
                                    div()
                                        .id("btn-disconnect")
                                        .cursor_pointer()
                                        .px_1()
                                        .py(px(0.5))
                                        .rounded_sm()
                                        .hover(|s| s.bg(self.theme.bg_badge_danger).text_color(self.theme.danger))
                                        .text_size(self.theme.font_size_caption)
                                        .text_color(self.theme.text_muted)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                            this.disconnect_remote();
                                            cx.notify();
                                        }))
                                        .child("Disconnect"),
                                ),
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
                                .rounded_sm()
                                .bg(self.theme.bg_badge_accent)
                                .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                                .text_size(self.theme.font_size_caption)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(self.theme.accent)
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                    this.open_connect_modal();
                                    cx.notify();
                                }))
                                .child("+ Connect SFTP"),
                        )
                    }),
            )
            // Center: Zed Command Palette Search Input
            .child(
                div()
                    .id("btn-command-palette")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w(px(280.0))
                    .h(px(22.0))
                    .px_2p5()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.border)
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
                                    .text_color(self.theme.text_muted)
                                    .child(Icons::SEARCH),
                            )
                            .child(
                                div()
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_muted)
                                    .child("Commands, files, or paths..."),
                            ),
                    )
                    .child(
                        div()
                            .px_1()
                            .py(px(0.5))
                            .rounded_sm()
                            .bg(gpui::rgba(0xffffff0a))
                            .font_family(self.theme.font_mono)
                            .text_size(px(8.5))
                            .text_color(self.theme.text_muted)
                            .child("Ctrl+P"),
                    ),
            )
            // Right: Essential Quick Actions
            .child(
                div()
                    .flex()
                    .items_center()
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
                            .rounded_sm()
                            .bg(self.theme.bg_badge_accent)
                            .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                            .text_size(self.theme.font_size_caption)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.accent)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.transfer_selected(this.active_pane);
                                cx.notify();
                            }))
                            .child(if is_left { "Transfer →" } else { "← Transfer" })
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
                            .rounded_sm()
                            .when(self.show_transfer_drawer, |s| s.bg(self.theme.bg_selected))
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .text_size(self.theme.font_size_caption)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
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
                                        .rounded_sm()
                                        .bg(self.theme.bg_badge_accent)
                                        .font_family(self.theme.font_mono)
                                        .text_size(px(8.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(self.theme.accent)
                                        .child(self.transfer_items.len().to_string()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("btn-toggle-bookmarks")
                            .tooltip(tooltip_view("Bookmarks", Some("Ctrl+B")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .text_size(self.theme.font_size_caption)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::BookmarksList;
                                cx.notify();
                            }))
                            .child(Icons::BOOKMARK),
                    )
                    .child(
                        div()
                            .id("btn-toggle-theme")
                            .tooltip(tooltip_view("Toggle Theme", Some("Ctrl+T")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .text_size(self.theme.font_size_caption)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.toggle_theme();
                                cx.notify();
                            }))
                            .child(if self.theme.bg_pane == Theme::zed_dark().bg_pane { "☀" } else { "☾" }),
                    ),
            )
    }
}
