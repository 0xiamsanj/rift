use gpui::prelude::FluentBuilder;
use gpui::*;
use crate::theme::Theme;

#[derive(Clone)]
pub struct TooltipView {
    pub text: SharedString,
    pub shortcut: Option<SharedString>,
    pub theme: Theme,
}

impl TooltipView {
    pub fn new(
        theme: Theme,
        text: impl Into<SharedString>,
        shortcut: Option<impl Into<SharedString>>,
    ) -> Self {
        Self {
            text: text.into(),
            shortcut: shortcut.map(Into::into),
            theme,
        }
    }
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .bg(theme.elevated_surface_background)
            .border_1()
            .border_color(theme.border)
            .shadow_md()
            .child(
                div()
                    .font_family(theme.font_ui)
                    .text_color(theme.text)
                    .text_size(theme.font_size_ui)
                    .child(self.text.clone()),
            )
            .when_some(self.shortcut.clone(), |el, sc| {
                el.child(
                    div()
                        .px(px(4.0))
                        .py(px(1.0))
                        .rounded(px(3.0))
                        .bg(theme.keycap_bg)
                        .border_1()
                        .border_color(theme.keycap_border)
                        .font_family(theme.font_mono)
                        .text_color(theme.keycap_text)
                        .text_size(theme.font_size_shortcut)
                        .child(sc),
                )
            })
    }
}

pub fn tooltip_view(
    theme: Theme,
    text: impl Into<SharedString> + 'static,
    shortcut: Option<impl Into<SharedString> + 'static>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    let shortcut = shortcut.map(Into::into);
    move |_window, cx| {
        let text = text.clone();
        let shortcut = shortcut.clone();
        cx.new(|_cx| TooltipView { text, shortcut, theme }).into()
    }
}
