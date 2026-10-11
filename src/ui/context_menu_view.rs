use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::fs::path_utils::join_path;
use crate::ui::context_menu::ContextMenuState;
use crate::ui::icons::Icons;
use crate::ui::modals::{ActiveModal, AddBookmarkModalState, NewFolderModalState, RenameModalState};
use crate::ui::pane_state::PaneId;

impl RiftApp {
    pub fn render_context_menu(
        &self,
        menu: &ContextMenuState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane_id = menu.target_pane;
        let is_left = pane_id == PaneId::Left;
        let pane = match pane_id {
            PaneId::Left => &self.left_pane,
            PaneId::Right => &self.right_pane,
        };
        let target_entry = menu.target_entry.clone();
        let current_path = pane.current_path.clone();
        let is_remote = pane.is_remote;

        let menu_width: f32 = 220.0;
        let menu_height: f32 = 270.0;
        let viewport = window.viewport_size();
        let pos_x = menu.position.x.to_f64() as f32;
        let pos_y = menu.position.y.to_f64() as f32;
        let max_x = (viewport.width.to_f64() as f32 - menu_width).max(8.0);
        let max_y = (viewport.height.to_f64() as f32 - menu_height).max(8.0);
        let x = pos_x.clamp(8.0f32, max_x);
        let y = pos_y.clamp(8.0f32, max_y);

        let transfer_label = if is_left { "Transfer to Remote" } else { "Transfer to Local" };

        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("context-menu-backdrop")
                    .absolute()
                    .inset_0()
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
                    .w(px(menu_width))
                    .bg(self.theme.bg_modal)
                    .border_1()
                    .border_color(self.theme.border)
                    .rounded_md()
                    .shadow_lg()
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
                    .when_some(target_entry.as_ref(), |el, entry| {
                        let e_name = entry.name.clone();
                        let is_dir = entry.is_dir();
                        let target_path = if is_remote {
                            join_path(&current_path, &e_name, true)
                        } else {
                            entry.path.to_string_lossy().to_string()
                        };

                        el.child(
                            div()
                                .px_2()
                                .py(px(3.0))
                                .border_b_1()
                                .border_color(self.theme.border)
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(if is_dir { self.theme.dir_icon } else { self.theme.file_icon })
                                        .child(if is_dir { Icons::FOLDER } else { Icons::for_file(&entry.extension) })
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(self.theme.text_secondary)
                                        .overflow_hidden()
                                        .child(entry.name.clone())
                                )
                        )
                        .when(is_dir, |el| {
                            let tp = target_path.clone();
                            el.child(self.render_menu_item(
                                "menu-open",
                                Icons::FOLDER_OPEN,
                                "Open Directory",
                                Some("Enter"),
                                false,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.context_menu = None;
                                    this.navigate_pane(pane_id, tp.clone(), cx);
                                    cx.notify();
                                }),
                            ))
                        })
                        .child(self.render_menu_item(
                            "menu-transfer",
                            if is_left { Icons::TRANSFER_RIGHT } else { Icons::TRANSFER_LEFT },
                            transfer_label,
                            Some("F5"),
                            false,
                            cx.listener(move |this, _ev, _window, cx| {
                                this.context_menu = None;
                                this.transfer_selected(pane_id, cx);
                                cx.notify();
                            }),
                        ))
                        .child(self.render_menu_separator())
                        .child(self.render_menu_item(
                            "menu-rename",
                            Icons::EDIT,
                            "Rename",
                            Some("F2"),
                            false,
                            cx.listener(move |this, _ev, _window, cx| {
                                this.context_menu = None;
                                this.modal = ActiveModal::Rename(RenameModalState {
                                    target_pane: pane_id,
                                    original_name: e_name.clone(),
                                    new_name: e_name.clone(),
                                });
                                cx.notify();
                            }),
                        ))
                        .child(self.render_menu_item(
                            "menu-delete",
                            Icons::DELETE,
                            "Delete",
                            Some("Del"),
                            true,
                            cx.listener(move |this, _ev, _window, cx| {
                                this.context_menu = None;
                                this.delete_selected(pane_id);
                                cx.notify();
                            }),
                        ))
                        .child(self.render_menu_separator())
                        .child({
                            let tp = target_path.clone();
                            self.render_menu_item(
                                "menu-copy-path",
                                Icons::COPY,
                                "Copy Path",
                                Some("Ctrl+C"),
                                false,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.context_menu = None;
                                    cx.write_to_clipboard(ClipboardItem::new_string(tp.clone()));
                                    this.status_message = Some(format!("Copied: {}", tp));
                                    cx.notify();
                                }),
                            )
                        })
                        .child({
                            let ename = entry.name.clone();
                            self.render_menu_item(
                                "menu-copy-name",
                                Icons::FILE,
                                "Copy Name",
                                None,
                                false,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.context_menu = None;
                                    cx.write_to_clipboard(ClipboardItem::new_string(ename.clone()));
                                    this.status_message = Some(format!("Copied: {}", ename));
                                    cx.notify();
                                }),
                            )
                        })
                        .child({
                            let tp = target_path.clone();
                            let ename = entry.name.clone();
                            self.render_menu_item(
                                "menu-add-bookmark",
                                Icons::BOOKMARK,
                                "Add to Bookmarks",
                                Some("Ctrl+Shift+B"),
                                false,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.context_menu = None;
                                    this.modal = ActiveModal::AddBookmark(AddBookmarkModalState {
                                        path: tp.clone(),
                                        label: ename.clone(),
                                        is_remote,
                                        is_session_only: false,
                                    });
                                    cx.notify();
                                }),
                            )
                        })
                    })
                    .when(target_entry.is_some(), |el| {
                        el.child(self.render_menu_separator())
                    })
                    .child(self.render_menu_item(
                        "menu-new-folder",
                        Icons::NEW_FOLDER,
                        "New Folder...",
                        Some("F7"),
                        false,
                        cx.listener(move |this, _ev, _window, cx| {
                            this.context_menu = None;
                            this.modal = ActiveModal::NewFolder(NewFolderModalState {
                                target_pane: pane_id,
                                folder_name: String::new(),
                            });
                            cx.notify();
                        }),
                    ))
                    .child(self.render_menu_item(
                        "menu-parent",
                        Icons::UP_ARROW,
                        "Parent Directory",
                        Some("Backspace"),
                        false,
                        cx.listener(move |this, _ev, _window, cx| {
                            this.context_menu = None;
                            this.go_parent(pane_id, cx);
                            cx.notify();
                        }),
                    ))
                    .child(self.render_menu_item(
                        "menu-select-all",
                        Icons::CHECK,
                        "Select All",
                        Some("Ctrl+A"),
                        false,
                        cx.listener(move |this, _ev, _window, cx| {
                            this.context_menu = None;
                            let p = match pane_id {
                                PaneId::Left => &mut this.left_pane,
                                PaneId::Right => &mut this.right_pane,
                            };
                            p.select_all();
                            cx.notify();
                        }),
                    ))
                    .child(self.render_menu_item(
                        "menu-refresh",
                        Icons::REFRESH,
                        "Refresh",
                        Some("Ctrl+R"),
                        false,
                        cx.listener(move |this, _ev, _window, cx| {
                            this.context_menu = None;
                            this.refresh_pane(pane_id, cx);
                            cx.notify();
                        }),
                    ))
            )
    }

    pub fn render_menu_item(
        &self,
        id: &'static str,
        icon: &'static str,
        label: &'static str,
        shortcut: Option<&'static str>,
        is_danger: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .h(px(22.0))
            .px_2()
            .rounded_sm()
            .hover(move |s| {
                if is_danger {
                    s.bg(theme.bg_badge_danger).text_color(theme.danger)
                } else {
                    s.bg(theme.bg_row_hover).text_color(theme.text_primary)
                }
            })
            .text_color(if is_danger { theme.danger } else { theme.text_secondary })
            .text_size(theme.font_size_ui)
            .on_mouse_down(MouseButton::Left, on_click)
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
                            .text_size(px(10.0))
                            .child(icon)
                    )
                    .child(div().child(label))
            )
            .when_some(shortcut, |el, sc| {
                el.child(
                    div()
                        .font_family(theme.font_mono)
                        .text_size(theme.font_size_caption)
                        .text_color(theme.text_muted)
                        .child(sc)
                )
            })
    }

    pub fn render_menu_separator(&self) -> impl IntoElement {
        div()
            .h(px(1.0))
            .my(px(2.0))
            .mx_1()
            .bg(self.theme.border)
    }
}
