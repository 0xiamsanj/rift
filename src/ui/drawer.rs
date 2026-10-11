use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::fs::types::{format_bytes, format_speed};
use crate::transfer::types::{TransferDirection, TransferState};
use crate::ui::icons::Icons;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_transfer_drawer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items = &self.transfer_items;

        div()
            .flex()
            .flex_col()
            .h(px(180.0))
            .overflow_hidden()
            .bg(self.theme.bg_pane)
            .border_t_1()
            .border_color(self.theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .bg(self.theme.bg_toolbar)
                    .border_b_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(self.theme.font_size_ui)
                                    .child("TRANSFERS"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(1.0))
                                    .rounded_full()
                                    .bg(if !items.is_empty() { self.theme.bg_badge_accent } else { self.theme.bg_input })
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_caption)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if !items.is_empty() { self.theme.accent } else { self.theme.text_muted })
                                    .child(format!("{} queue", items.len())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("btn-clear-transfers")
                                    .cursor_pointer()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .bg(self.theme.bg_input)
                                    .border_1()
                                    .border_color(self.theme.border)
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(self.theme.text_secondary)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                        this.transfer_manager.clear_completed();
                                        this.transfer_items.retain(|i| {
                                            matches!(i.state, TransferState::Queued | TransferState::Running)
                                        });
                                        cx.notify();
                                    }))
                                    .child("Clear Finished"),
                            )
                            .child(
                                div()
                                    .id("btn-close-transfers")
                                    .tooltip(tooltip_view(self.theme, "Close Drawer", Some("Esc")))
                                    .cursor_pointer()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_ui)
                                    .text_color(self.theme.text_muted)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                        this.show_transfer_drawer = false;
                                        cx.notify();
                                    }))
                                    .child(Icons::CLOSE),
                            ),
                    ),
            )
            .child(
                div()
                    .id("transfer-drawer-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .track_scroll(&self.drawer_scroll_handle)
                    .children(items.iter().map(|item| {
                        let id_clone = item.id.clone();
                        let dir_arrow = match item.direction {
                            TransferDirection::LocalToRemote => "Local → Remote",
                            TransferDirection::RemoteToLocal => "Remote → Local",
                            TransferDirection::LocalToLocal => "Local → Local",
                        };

                        let ratio = item.progress_ratio().clamp(0.0, 1.0);
                        let pct_int = (ratio * 100.0) as u32;

                        let (state_bg, state_color, state_str) = match &item.state {
                            TransferState::Queued => (self.theme.bg_input, self.theme.text_muted, "Queued".to_string()),
                            TransferState::Running => (
                                self.theme.bg_badge_accent,
                                self.theme.accent,
                                format!("{}% · {}", pct_int, format_speed(item.speed_bps)),
                            ),
                            TransferState::Completed => (self.theme.bg_badge_success, self.theme.success, format!("Completed {}", Icons::CHECK)),
                            TransferState::Failed(err) => (self.theme.bg_badge_danger, self.theme.danger, format!("Failed: {err}")),
                            TransferState::Cancelled => (self.theme.bg_input, self.theme.text_muted, "Cancelled".to_string()),
                        };

                        let bar_color = match &item.state {
                            TransferState::Completed => self.theme.success,
                            TransferState::Failed(_) => self.theme.danger,
                            _ => self.theme.accent,
                        };

                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .rounded_sm()
                            .bg(self.theme.bg_card)
                            .border_1()
                            .border_color(self.theme.border)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_size(self.theme.font_size_body)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(item.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py(px(1.0))
                                                    .rounded_sm()
                                                    .bg(self.theme.bg_input)
                                                    .font_family(self.theme.font_mono)
                                                    .text_size(self.theme.font_size_caption)
                                                    .text_color(self.theme.text_secondary)
                                                    .child(dir_arrow),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .font_family(self.theme.font_mono)
                                                    .text_size(self.theme.font_size_mono)
                                                    .text_color(self.theme.text_muted)
                                                    .child(format!(
                                                        "{} / {}",
                                                        format_bytes(item.transferred_bytes),
                                                        format_bytes(item.total_bytes)
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py(px(1.0))
                                                    .rounded_sm()
                                                    .bg(state_bg)
                                                    .font_family(self.theme.font_mono)
                                                    .text_size(self.theme.font_size_mono)
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(state_color)
                                                    .child(state_str),
                                            )
                                            .when(matches!(item.state, TransferState::Running | TransferState::Queued), |el| {
                                                el.child(
                                                    div()
                                                        .id(ElementId::Name(format!("cancel-transfer-{}", item.id).into()))
                                                        .cursor_pointer()
                                                        .px_2()
                                                        .py(px(1.0))
                                                        .rounded_sm()
                                                        .bg(self.theme.bg_input)
                                                        .border_1()
                                                        .border_color(self.theme.danger)
                                                        .text_size(self.theme.font_size_caption)
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(self.theme.danger)
                                                        .hover(|s| s.bg(self.theme.danger).text_color(self.theme.text_primary))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                                            this.transfer_manager.cancel_transfer(&id_clone);
                                                            cx.notify();
                                                        }))
                                                        .child("Cancel"),
                                                )
                                            }),
                                    ),
                            )
                            // Real visual progress bar track (fractional fill
                            // scales with the drawer width instead of a fixed px).
                            .child(
                                div()
                                    .h(px(4.0))
                                    .w_full()
                                    .rounded_full()
                                    .bg(self.theme.bg_input)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(gpui::relative(ratio))
                                            .rounded_full()
                                            .bg(bar_color),
                                    ),
                            )
                    })),
            )
    }
}
