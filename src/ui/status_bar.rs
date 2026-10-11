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
            .h(px(20.0))
            .px_3()
            .bg(self.theme.status_bar_background)
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
                            .text_color(self.theme.text_placeholder)
                            .text_size(px(8.0))
                            .font_family(Icons::FAMILY)
                            .child(Icons::DOT),
                    )
                    .child(crate::ui::motion::flash(
                        self.animations_enabled,
                        div()
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child(msg.to_string()),
                        ElementId::Name(format!("status-{msg}").into()),
                    )),
            )
            // Right: Clean, Unobtrusive Shortcut Hints
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_color(self.theme.text_placeholder)
                    .text_size(self.theme.font_size_caption)
                    .child("Tab: Switch  ·  F5: Transfer  ·  F7: New Folder  ·  Ctrl+P: Palette"),
            )
    }
}
