use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::ui::context_menu::{ContextMenuState, MenuAction};
use crate::ui::icons::Icons;
use crate::ui::motion;
use crate::ui::pane_state::PaneId;

/// Shared inputs for menu-row builders, bundled to keep signatures small.
struct MenuCtx {
    pane_id: PaneId,
    is_left: bool,
    show_hidden: bool,
    transfer_label: String,
    animations_enabled: bool,
}

/// Visual definition of one menu row (behavior attaches separately).
struct MenuActionVisual {
    icon: &'static str,
    label: SharedString,
    shortcut: Option<&'static str>,
    is_danger: bool,
    enabled: bool,
    selected: bool,
}

impl RiftApp {
    pub fn render_context_menu(
        &self,
        menu: &ContextMenuState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let pane_id = menu.target_pane;
        let is_left = pane_id == PaneId::Left;
        let show_hidden = match pane_id {
            PaneId::Left => self.left_pane.show_hidden,
            PaneId::Right => self.right_pane.show_hidden,
        };

        let ctx = MenuCtx {
            pane_id,
            is_left,
            show_hidden,
            transfer_label: if is_left {
                "Transfer to Remote".to_string()
            } else {
                "Transfer to Local".to_string()
            },
            animations_enabled: self.animations_enabled,
        };

        // Estimated popover height per variant so edge clamping stays tight.
        let menu_width: f32 = 240.0;
        let menu_height: f32 = match &menu.target_entry {
            Some(e) if e.is_dir() => 230.0,
            Some(_) => 185.0,
            None => 175.0,
        };
        let viewport = window.viewport_size();
        let pos_x = menu.position.x.to_f64() as f32;
        let pos_y = menu.position.y.to_f64() as f32;
        let max_x = (viewport.width.to_f64() as f32 - menu_width).max(8.0);
        let max_y = (viewport.height.to_f64() as f32 - menu_height).max(8.0);
        let x = pos_x.clamp(8.0f32, max_x);
        let y = pos_y.clamp(8.0f32, max_y);

        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("context-menu-backdrop")
                    .absolute()
                    .inset_0()
                    // Block hover/clicks reaching the panes behind the menu.
                    .occlude()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                        this.context_menu = None;
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(|this, _ev, _window, cx| {
                        this.context_menu = None;
                        cx.notify();
                    }))
            )
            .child(
                div()
                    .id("context-menu-popover")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .min_w(px(200.0))
                    .max_w(px(320.0))
                    .bg(self.theme.elevated_surface_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .rounded_lg()
                    .shadow_md()
                    .p_1()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_this, _ev, _window, cx| {
                        cx.stop_propagation();
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(|_this, _ev, _window, cx| {
                        cx.stop_propagation();
                    }))
                    .children(self.context_menu_rows(&ctx, menu, cx))
            )
    }

    /// Rows for the file-targeted menu or the background menu, driven by
    /// `menu.items` so mouse and keyboard share one dispatch path.
    /// Separators precede fixed action keys; headers precede everything.
    fn context_menu_rows(
        &self,
        ctx: &MenuCtx,
        menu: &ContextMenuState,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();

        if let Some(entry) = menu.target_entry.clone() {
            let is_dir = entry.is_dir();
            rows.push(motion::staggered(
                ctx.animations_enabled,
                div()
                    .px_3()
                    .py(px(3.0))
                    .border_b_1()
                    .border_color(self.theme.border_variant)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(Icons::FAMILY)
                            .text_color(if is_dir { self.theme.dir_icon } else { self.theme.file_icon })
                            .child(if is_dir { Icons::FOLDER } else { Icons::for_file(&entry.extension) })
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .overflow_hidden()
                            .child(entry.name.clone())
                    ),
                ElementId::Name("menu-header".into()),
                0,
                3,
            ));
        }

        for (idx, (action, enabled)) in menu.items.iter().enumerate() {
            // Separators before fixed keys keep file and background
            // menus grouped without a separate separator list.
            match action.key() {
                "rename" | "copy-path" | "select-all" => {
                    rows.push(motion::staggered(
                        ctx.animations_enabled,
                        self.render_menu_separator(),
                        ElementId::Name(format!("menu-sep-{}", action.key()).into()),
                        idx,
                        3,
                    ));
                }
                _ => {}
            }
            let selected = menu.selected_index == Some(idx);
            let visual = self.menu_visual(ctx, action, *enabled, selected);
            let dispatch = action.clone();
            let pane_id = ctx.pane_id;
            rows.push(motion::staggered(
                ctx.animations_enabled,
                self.render_menu_item(
                    action.key(),
                    visual,
                    cx.listener(move |this, _ev, _window, cx| {
                        this.execute_menu_action(pane_id, dispatch.clone(), cx);
                    }),
                ),
                ElementId::Name(format!("menu-row-{}", action.key()).into()),
                idx + 1,
                3,
            ));
        }

        rows
    }

    /// Icon/label/shortcut/danger for an action (Zed §5: aligned rows).
    fn menu_visual(
        &self,
        ctx: &MenuCtx,
        action: &MenuAction,
        enabled: bool,
        selected: bool,
    ) -> MenuActionVisual {
        let (icon, label, shortcut, is_danger) = match action {
            MenuAction::OpenDirectory { .. } => (Icons::FOLDER_OPEN, "Open Directory".into(), Some("Enter"), false),
            MenuAction::OpenFile => (Icons::FILE, "Open".into(), Some("Enter"), false),
            MenuAction::Transfer => (
                if ctx.is_left { Icons::TRANSFER_RIGHT } else { Icons::TRANSFER_LEFT },
                ctx.transfer_label.clone().into(),
                Some("F5"),
                false,
            ),
            MenuAction::Properties => (Icons::FILE, "Properties".into(), Some("Alt+Enter"), false),
            MenuAction::Rename { .. } => (Icons::EDIT, "Rename".into(), Some("F2"), false),
            MenuAction::Delete => (Icons::DELETE, "Delete".into(), Some("Del"), true),
            MenuAction::CopyPath { .. } => (Icons::COPY, "Copy Path".into(), Some("Ctrl+C"), false),
            MenuAction::CopyName { .. } => (Icons::FILE, "Copy Name".into(), None, false),
            MenuAction::AddBookmark { .. } => (
                Icons::BOOKMARK,
                "Add to Bookmarks".into(),
                Some("Ctrl+Shift+B"),
                false,
            ),
            MenuAction::NewFolder => (Icons::NEW_FOLDER, "New Folder...".into(), Some("F7"), false),
            MenuAction::BookmarkFolder { .. } => (
                Icons::BOOKMARK,
                "Bookmark This Folder".into(),
                Some("Ctrl+Shift+B"),
                false,
            ),
            MenuAction::SelectAll => (Icons::CHECK, "Select All".into(), Some("Ctrl+A"), false),
            MenuAction::Refresh => (Icons::REFRESH, "Refresh".into(), Some("Ctrl+R"), false),
            MenuAction::GoParent => (Icons::UP_ARROW, "Parent Directory".into(), Some("Backspace"), false),
            MenuAction::ToggleHidden => (
                Icons::HIDDEN,
                if ctx.show_hidden {
                    "Hide Hidden Files".into()
                } else {
                    "Show Hidden Files".into()
                },
                Some("Ctrl+H"),
                false,
            ),
        };
        MenuActionVisual {
            icon,
            label,
            shortcut,
            is_danger,
            enabled,
            selected,
        }
    }

    fn render_menu_item(
        &self,
        id: &'static str,
        def: MenuActionVisual,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let enabled = def.enabled;
        let is_danger = def.is_danger;
        let selected = def.selected;
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_between()
            .h(px(22.0))
            .px_3()
            .rounded_sm()
            .when(selected, |el| el.bg(theme.ghost_element_hover))
            .when(enabled, |el| {
                el.cursor_pointer().hover(move |s| {
                    if is_danger {
                        s.bg(theme.bg_badge_danger).text_color(theme.danger)
                    } else {
                        s.bg(theme.ghost_element_hover).text_color(theme.text)
                    }
                })
            })
            .text_color(if !enabled {
                theme.text_disabled
            } else if is_danger {
                theme.danger
            } else {
                theme.text_muted
            })
            .text_size(theme.font_size_ui)
            .when(enabled, |el| el.on_mouse_down(MouseButton::Left, on_click))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(14.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.0))
                            .child(def.icon)
                    )
                    .child(div().child(def.label))
            )
            .when_some(def.shortcut, |el, sc| {
                el.child(
                    div()
                        .font_family(theme.font_mono)
                        .text_size(theme.font_size_shortcut)
                        .text_color(theme.text_placeholder)
                        .child(sc)
                )
            })
    }

    fn render_menu_separator(&self) -> Div {
        div()
            .h(px(1.0))
            .my(px(2.0))
            .mx_1()
            .bg(self.theme.border_variant)
    }
}
