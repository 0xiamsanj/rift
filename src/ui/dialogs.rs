use gpui::*;
use gpui::prelude::FluentBuilder;
use uuid::Uuid;
use crate::app::RiftApp;
use crate::bookmarks::{Bookmark, BookmarkStore};
use crate::fs::types::format_bytes;
use crate::ui::icons::Icons;
use crate::ui::modals::{
    ActiveModal, AddBookmarkModalState, CommandPaletteModalState, ConfirmDeleteModalState,
    ConflictModalState, ConnectModalState, HostKeyModalState, NewFolderModalState,
    RenameModalState,
};
use crate::ui::pane_state::PaneId;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_modal_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000bb))
            .child(match &self.modal {
                ActiveModal::CommandPalette(state) => self.render_command_palette(state, cx).into_any_element(),
                ActiveModal::Connect(state) => self.render_connect_dialog(state, cx).into_any_element(),
                ActiveModal::NewFolder(state) => self.render_new_folder_dialog(state, cx).into_any_element(),
                ActiveModal::Rename(state) => self.render_rename_dialog(state, cx).into_any_element(),
                ActiveModal::ConfirmDelete(state) => self.render_confirm_delete_dialog(state, cx).into_any_element(),
                ActiveModal::HostKeyTrust(state) => self.render_host_key_dialog(state, cx).into_any_element(),
                ActiveModal::Conflict(state) => self.render_conflict_dialog(state, cx).into_any_element(),
                ActiveModal::BookmarksList => self.render_bookmarks_dialog(cx).into_any_element(),
                ActiveModal::AddBookmark(state) => self.render_add_bookmark_dialog(state, cx).into_any_element(),
                _ => div().into_any_element(),
            })
    }

    pub fn render_command_palette(&self, state: &CommandPaletteModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let all_items = self.command_palette_items();
        let query_lower = state.query.to_lowercase();
        let filtered: Vec<(&'static str, u8, Option<&'static str>)> = all_items
            .into_iter()
            .filter(|(title, _, _)| {
                if query_lower.is_empty() {
                    true
                } else {
                    title.to_lowercase().contains(&query_lower)
                }
            })
            .collect();

        let selected_idx = state.selected_index.min(filtered.len().saturating_sub(1));

        div()
            .flex()
            .flex_col()
            .w(px(520.0))
            .max_h(px(420.0))
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.accent)
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .bg(self.theme.bg_input)
                    .border_b_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .text_color(self.theme.accent)
                            .font_weight(FontWeight::BOLD)
                            .text_size(self.theme.font_size_ui)
                            .child(">"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_ui)
                            .text_color(self.theme.text_primary)
                            .child(if state.query.is_empty() {
                                "Type a command... |".to_string()
                            } else {
                                format!("{}|", state.query)
                            }),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .py(px(1.0))
                            .rounded_sm()
                            .bg(self.theme.bg_badge_accent)
                            .text_size(px(9.5))
                            .font_family(self.theme.font_mono)
                            .text_color(self.theme.accent)
                            .child("ESC"),
                    ),
            )
            .child(
                div()
                    .id("palette-items-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_1()
                    .children(filtered.into_iter().enumerate().map(|(idx, (title, action_id, shortcut))| {
                        let is_sel = idx == selected_idx;
                        div()
                            .id(ElementId::Name(format!("palette-item-{}", action_id).into()))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py(px(4.5))
                            .rounded_sm()
                            .bg(if is_sel { self.theme.bg_selected } else { gpui::rgba(0x00000000) })
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.execute_palette_action(action_id, cx);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_caption)
                                            .text_color(if is_sel { self.theme.accent } else { self.theme.text_muted })
                                            .child(if is_sel { "▶" } else { " " }),
                                    )
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_ui)
                                            .font_weight(if is_sel { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                            .text_color(if is_sel { self.theme.text_primary } else { self.theme.text_secondary })
                                            .child(title),
                                    ),
                            )
                            .when_some(shortcut, |el, sc| {
                                el.child(
                                    div()
                                        .px_1p5()
                                        .py(px(1.0))
                                        .rounded_sm()
                                        .bg(if is_sel { self.theme.bg_badge_accent } else { self.theme.bg_input })
                                        .border_1()
                                        .border_color(self.theme.border)
                                        .font_family(self.theme.font_mono)
                                        .text_size(px(10.0))
                                        .text_color(if is_sel { self.theme.accent } else { self.theme.text_muted })
                                        .child(sc),
                                )
                            })
                    })),
            )
    }

    pub fn render_connect_dialog(&self, state: &ConnectModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let is_connecting = state.is_connecting;
        let err_opt = state.error_msg.clone();
        let profiles = state.profiles.clone();

        div()
            .flex()
            .flex_col()
            .w(px(460.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .gap_3()
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
                            .child(
                                div()
                                    .text_color(self.theme.accent)
                                    .text_size(px(14.0))
                                    .child("✦"),
                            )
                            .child(
                                div()
                                    .text_size(self.theme.font_size_modal_title)
                                    .font_weight(FontWeight::BOLD)
                                    .child("Connect to SFTP Server"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-modal-close")
                            .tooltip(tooltip_view("Close", Some("Esc")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .text_size(self.theme.font_size_ui)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .when(!profiles.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Saved Profiles:"))
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .children(profiles.into_iter().map(|p| {
                                    let p_clone = p.clone();
                                    div()
                                        .id(ElementId::Name(format!("select-prof-{}", p.id).into()))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(self.theme.bg_input)
                                        .border_1()
                                        .border_color(self.theme.border)
                                        .hover(|s| s.border_color(self.theme.accent))
                                        .text_size(self.theme.font_size_caption)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(self.theme.text_secondary)
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                            if let ActiveModal::Connect(ref mut s) = this.modal {
                                                s.host_input = p_clone.host.clone();
                                                s.port_input = p_clone.port.to_string();
                                                s.username_input = p_clone.username.clone();
                                                s.selected_profile_id = Some(p_clone.id.clone());
                                            }
                                            cx.notify();
                                        }))
                                        .child(p.name)
                                })),
                        ),
                )
            })
            .when_some(err_opt, |el, err| {
                el.child(
                    div()
                        .p_2()
                        .rounded_sm()
                        .bg(self.theme.bg_badge_danger)
                        .border_1()
                        .border_color(self.theme.danger)
                        .text_size(self.theme.font_size_caption)
                        .text_color(self.theme.danger)
                        .child(err),
                )
            })
            // Inputs grid
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    // Host and Port
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Host:"))
                                    .child(
                                        div()
                                            .id("input-connect-host")
                                            .cursor_pointer()
                                            .p_2()
                                            .rounded_sm()
                                            .bg(self.theme.bg_input)
                                            .border_1()
                                            .border_color(if state.focused_field == 0 { self.theme.accent } else { self.theme.border })
                                            .font_family(self.theme.font_mono)
                                            .text_size(self.theme.font_size_body)
                                            .text_color(self.theme.text_primary)
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                                if let ActiveModal::Connect(ref mut s) = this.modal { s.focused_field = 0; }
                                                cx.notify();
                                            }))
                                            .child(if state.host_input.is_empty() { "example.com| ".to_string() } else { format!("{}|", state.host_input) }),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(80.0))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Port:"))
                                    .child(
                                        div()
                                            .id("input-connect-port")
                                            .cursor_pointer()
                                            .p_2()
                                            .rounded_sm()
                                            .bg(self.theme.bg_input)
                                            .border_1()
                                            .border_color(if state.focused_field == 1 { self.theme.accent } else { self.theme.border })
                                            .font_family(self.theme.font_mono)
                                            .text_size(self.theme.font_size_body)
                                            .text_color(self.theme.text_primary)
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                                if let ActiveModal::Connect(ref mut s) = this.modal { s.focused_field = 1; }
                                                cx.notify();
                                            }))
                                            .child(if state.port_input.is_empty() { "22|".to_string() } else { format!("{}|", state.port_input) }),
                                    ),
                            ),
                    )
                    // Username
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Username:"))
                            .child(
                                div()
                                    .id("input-connect-user")
                                    .cursor_pointer()
                                    .p_2()
                               .rounded_sm()
                                    .bg(self.theme.bg_input)
                                    .border_1()
                                    .border_color(if state.focused_field == 2 { self.theme.accent } else { self.theme.border })
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_body)
                                    .text_color(self.theme.text_primary)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                        if let ActiveModal::Connect(ref mut s) = this.modal { s.focused_field = 2; }
                                        cx.notify();
                                    }))
                                    .child(if state.username_input.is_empty() { "username| ".to_string() } else { format!("{}|", state.username_input) }),
                            ),
                    )
                    // Password
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Password:"))
                            .child(
                                div()
                                    .id("input-connect-pass")
                                    .cursor_pointer()
                                    .p_2()
                                    .rounded_sm()
                                    .bg(self.theme.bg_input)
                                    .border_1()
                                    .border_color(if state.focused_field == 3 { self.theme.accent } else { self.theme.border })
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_body)
                                    .text_color(self.theme.text_primary)
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                        if let ActiveModal::Connect(ref mut s) = this.modal { s.focused_field = 3; }
                                        cx.notify();
                                    }))
                                    .child(if state.password_input.is_empty() { "••••••••| ".to_string() } else { "•".repeat(state.password_input.len()) + "|" }),
                            ),
                    ),
            )
            // Actions
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        div()
                            .id("btn-cancel-connect")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-submit-connect")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.execute_connect(cx);
                            }))
                            .child(if is_connecting { "Connecting..." } else { "Connect" }),
                    ),
            )
    }

    pub fn render_new_folder_dialog(&self, state: &NewFolderModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let pane_id = state.target_pane;
        let folder_name = state.folder_name.clone();

        div()
            .flex()
            .flex_col()
            .w(px(360.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.dir_icon)
                            .font_weight(FontWeight::BOLD)
                            .child("+"),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .child("Create New Directory"),
                    ),
            )
            .child(
                div()
                    .id("input-newfolder-name")
                    .p_2()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.accent)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_body)
                    .text_color(if folder_name.is_empty() { self.theme.text_muted } else { self.theme.text_primary })
                    .child(if folder_name.is_empty() { "folder_name| ".to_string() } else { format!("{}|", folder_name) }),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-cancel-newfolder")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-submit-newfolder")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let submit_name = if folder_name.is_empty() { "New Folder".to_string() } else { folder_name.clone() };
                                this.modal = ActiveModal::None;
                                this.create_folder(pane_id, &submit_name);
                                cx.notify();
                            }))
                            .child("Create"),
                    ),
            )
    }

    pub fn render_rename_dialog(&self, state: &RenameModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let pane_id = state.target_pane;
        let orig = state.original_name.clone();
        let new_name = state.new_name.clone();

        div()
            .flex()
            .flex_col()
            .w(px(360.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.accent)
                            .child("✎"),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Rename '{}'", orig)),
                    ),
            )
            .child(
                div()
                    .id("input-rename-name")
                    .p_2()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.accent)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_body)
                    .text_color(self.theme.text_primary)
                    .child(format!("{}|", new_name)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-cancel-rename")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-submit-rename")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                this.rename_entry(pane_id, &orig, &new_name);
                                cx.notify();
                            }))
                            .child("Rename"),
                    ),
            )
    }

    pub fn render_confirm_delete_dialog(&self, state: &ConfirmDeleteModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let pane_id = state.target_pane;
        let items = state.item_names.clone();

        div()
            .flex()
            .flex_col()
            .w(px(400.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.danger)
                            .text_size(px(14.0))
                            .child("×"),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.theme.danger)
                            .child("Confirm Permanent Deletion"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_secondary)
                    .child(format!("Are you sure you want to permanently delete {} selected item(s)?", items.len())),
            )
            .child(
                div()
                    .max_h(px(110.0))
                    .overflow_hidden()
                    .p_2()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .text_color(self.theme.text_primary)
                    .children(items.iter().map(|it| div().child(it.clone()))),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-cancel-delete")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-confirm-delete")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.danger)
                            .hover(|s| s.bg(gpui::rgb(0xf07178)))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.confirm_delete(pane_id, items.clone());
                                cx.notify();
                            }))
                            .child("Delete"),
                    ),
            )
    }

    pub fn render_host_key_dialog(&self, state: &HostKeyModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let host = state.host.clone();
        let port = state.port;
        let fp = state.fingerprint.clone();

        div()
            .flex()
            .flex_col()
            .w(px(460.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.warning)
                            .child("▲"),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.theme.warning)
                            .child("Unknown SSH Host Key"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_secondary)
                    .child(format!("The authenticity of host '{host}:{port}' cannot be established.")),
            )
            .child(
                div()
                    .p_2()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.theme.warning)
                    .child(fp),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_breadcrumbs)
                    .text_color(self.theme.text_muted)
                    .child("Are you sure you want to trust this key and connect?"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-reject-hostkey")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Abort"),
                    )
                    .child(
                        div()
                            .id("btn-trust-hostkey")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.execute_connect(cx);
                            }))
                            .child("Trust & Connect"),
                    ),
            )
    }

    pub fn render_conflict_dialog(&self, state: &ConflictModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let filename = state.filename.clone();
        let src_sz = format_bytes(state.source_size);
        let dst_sz = format_bytes(state.dest_size);

        div()
            .flex()
            .flex_col()
            .w(px(420.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.warning)
                            .text_size(px(14.0))
                            .child(Icons::WARNING),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.theme.text_primary)
                            .child("File Conflict"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_secondary)
                    .child(format!("A file named '{}' already exists at the target destination.", filename)),
            )
            .child(
                div()
                    .p_2()
                    .rounded_sm()
                    .bg(self.theme.bg_input)
                    .border_1()
                    .border_color(self.theme.border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(div().text_color(self.theme.text_muted).child("Source size:"))
                            .child(div().text_color(self.theme.text_primary).child(src_sz)),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(div().text_color(self.theme.text_muted).child("Existing size:"))
                            .child(div().text_color(self.theme.text_primary).child(dst_sz)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-conflict-cancel")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-conflict-skip")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                this.status_message = Some("Skipped conflicting file".to_string());
                                cx.notify();
                            }))
                            .child("Skip"),
                    )
                    .child(
                        div()
                            .id("btn-conflict-overwrite")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.danger)
                            .hover(|s| s.bg(gpui::rgb(0xf07178)))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                this.status_message = Some("Overwriting file".to_string());
                                cx.notify();
                            }))
                            .child("Overwrite"),
                    ),
            )
    }

    pub fn render_bookmarks_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let saved_bookmarks = BookmarkStore::load_saved_bookmarks();
        let session_bookmarks = self.bookmark_store.session_bookmarks.clone();
        let all_empty = saved_bookmarks.is_empty() && session_bookmarks.is_empty();
        let active_pane = self.active_pane;
        let active_path = match active_pane {
            PaneId::Left => self.left_pane.current_path.clone(),
            PaneId::Right => self.right_pane.current_path.clone(),
        };
        let is_remote = match active_pane {
            PaneId::Left => self.left_pane.is_remote,
            PaneId::Right => self.right_pane.is_remote,
        };

        div()
            .flex()
            .flex_col()
            .w(px(500.0))
            .max_h(px(460.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .gap_3()
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
                            .child(
                                div()
                                    .text_color(self.theme.dir_icon)
                                    .text_size(px(14.0))
                                    .child(Icons::BOOKMARK),
                            )
                            .child(
                                div()
                                    .text_size(self.theme.font_size_modal_title)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(self.theme.text_primary)
                                    .child("Bookmarks"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-close-bookmarks")
                            .tooltip(tooltip_view("Close", Some("Esc")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .text_size(self.theme.font_size_ui)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child(Icons::CLOSE),
                    ),
            )
            .child(
                div()
                    .id("bookmarks-scroll-list")
                    .flex_1()
                    .max_h(px(280.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(all_empty, |el| {
                        el.child(
                            div()
                                .p_4()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(self.theme.font_size_ui)
                                .text_color(self.theme.text_muted)
                                .child("No bookmarks saved yet"),
                        )
                    })
                    .children(saved_bookmarks.into_iter().map(|b| {
                        let bp = b.path.clone();
                        let bid = b.id.clone();
                        let is_rem = b.is_remote;
                        div()
                            .id(ElementId::Name(format!("bm-item-{}", bid).into()))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py(px(4.0))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.modal = ActiveModal::None;
                                        this.navigate_pane(active_pane, bp.clone());
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(if is_rem { self.theme.syntax_cyan } else { self.theme.dir_icon })
                                            .child(if is_rem { Icons::NETWORK } else { Icons::FOLDER })
                                    )
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_ui)
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(self.theme.text_primary)
                                            .child(b.label.clone())
                                    )
                                    .child(
                                        div()
                                            .font_family(self.theme.font_mono)
                                            .text_size(self.theme.font_size_mono)
                                            .text_color(self.theme.text_muted)
                                            .child(b.path.clone())
                                    )
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("bm-del-{}", bid).into()))
                                    .tooltip(tooltip_view("Delete Bookmark", None::<&str>))
                                    .cursor_pointer()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_badge_danger).text_color(self.theme.danger))
                                    .text_size(px(11.0))
                                    .text_color(self.theme.text_muted)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _ev, _window, cx| {
                                        let _ = BookmarkStore::delete_saved_bookmark(&bid);
                                        cx.notify();
                                    }))
                                    .child(Icons::DELETE)
                            )
                    }))
                    .children(session_bookmarks.into_iter().map(|b| {
                        let bp = b.path.clone();
                        let is_rem = b.is_remote;
                        div()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py(px(4.0))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                this.navigate_pane(active_pane, bp.clone());
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(if is_rem { self.theme.syntax_cyan } else { self.theme.dir_icon })
                                            .child(if is_rem { Icons::NETWORK } else { Icons::FOLDER })
                                    )
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_ui)
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(self.theme.text_primary)
                                            .child(b.label.clone())
                                    )
                                    .child(
                                        div()
                                            .font_family(self.theme.font_mono)
                                            .text_size(self.theme.font_size_mono)
                                            .text_color(self.theme.text_muted)
                                            .child(b.path.clone())
                                    )
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py(px(0.5))
                                    .rounded_sm()
                                    .bg(self.theme.bg_input)
                                    .font_family(self.theme.font_mono)
                                    .text_size(px(9.0))
                                    .text_color(self.theme.text_muted)
                                    .child("session"),
                            )
                    }))
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .pt_2()
                    .border_t_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .id("btn-bookmark-current")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.bg_badge_accent)
                            .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.accent)
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let label = std::path::Path::new(&active_path)
                                    .file_name()
                                    .map(|f| f.to_string_lossy().to_string())
                                    .unwrap_or_else(|| active_path.clone());
                                this.modal = ActiveModal::AddBookmark(AddBookmarkModalState {
                                    path: active_path.clone(),
                                    label,
                                    is_remote,
                                    is_session_only: false,
                                });
                                cx.notify();
                            }))
                            .child("+ Bookmark Current Path"),
                    )
                    .child(
                        div()
                            .id("btn-close-bm-dialog")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Close"),
                    ),
            )
    }

    pub fn render_add_bookmark_dialog(&self, state: &AddBookmarkModalState, cx: &mut Context<Self>) -> impl IntoElement {
        let label = state.label.clone();
        let path = state.path.clone();
        let is_remote = state.is_remote;
        let is_session = state.is_session_only;

        div()
            .flex()
            .flex_col()
            .w(px(400.0))
            .p_4()
            .rounded_md()
            .bg(self.theme.bg_modal)
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.dir_icon)
                            .text_size(px(14.0))
                            .child(Icons::BOOKMARK),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.theme.text_primary)
                            .child("Add Bookmark"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Label:"))
                    .child(
                        div()
                            .id("input-bookmark-label")
                            .p_2()
                            .rounded_sm()
                            .bg(self.theme.bg_input)
                            .border_1()
                            .border_color(self.theme.accent)
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_body)
                            .text_color(self.theme.text_primary)
                            .child(if label.is_empty() { "label| ".to_string() } else { format!("{}|", label) }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(self.theme.font_size_caption).text_color(self.theme.text_muted).child("Path:"))
                    .child(
                        div()
                            .p_2()
                            .rounded_sm()
                            .bg(self.theme.bg_input)
                            .border_1()
                            .border_color(self.theme.border)
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_mono)
                            .text_color(self.theme.text_secondary)
                            .child(path.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        div()
                            .id("btn-cancel-add-bm")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(self.theme.text_secondary)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("btn-submit-add-bm")
                            .cursor_pointer()
                            .px_3()
                            .py(px(3.5))
                            .rounded_sm()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                if is_session {
                                    this.bookmark_store.add_session_bookmark(label.clone(), path.clone(), is_remote, None);
                                } else {
                                    let _ = BookmarkStore::save_persistent_bookmark(Bookmark {
                                        id: Uuid::new_v4().to_string(),
                                        label: label.clone(),
                                        path: path.clone(),
                                        is_remote,
                                        profile_id: None,
                                        is_session_only: false,
                                    });
                                }
                                this.modal = ActiveModal::None;
                                this.status_message = Some(format!("Bookmarked '{}'", label));
                                cx.notify();
                            }))
                            .child("Save Bookmark"),
                    ),
            )
    }
}
