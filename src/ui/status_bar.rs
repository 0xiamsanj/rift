use gpui::*;
use crate::app::RiftApp;
use crate::ui::icons::Icons;

impl RiftApp {
    pub fn render_status_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let msg = self.status_message.as_deref().unwrap_or("Ready");

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(22.0))
            .px_3()
            .bg(self.theme.bg_status)
            .border_t_1()
            .border_color(self.theme.border)
            .text_size(self.theme.font_size_caption)
            // Left: Status Indicator & Message
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.accent)
                            .text_size(px(8.0))
                            .child(Icons::DOT),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_secondary)
                            .child(msg.to_string()),
                    ),
            )
            // Right: Clean, Unobtrusive Shortcut Hints
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_color(self.theme.text_muted)
                    .text_size(self.theme.font_size_caption)
                    .child("Tab: Switch  ·  F5: Transfer  ·  F7: New Folder  ·  Ctrl+P: Palette"),
            )
    }
}
