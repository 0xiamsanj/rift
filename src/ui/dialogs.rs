use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::bookmarks::BookmarkStore;
use crate::fs::types::format_bytes;
use crate::remote::profile::ProfileStore;
use crate::ui::icons::Icons;
use crate::ui::modals::{
    ActiveModal, AddBookmarkModalState, AuthKind, CommandPaletteModalState,
    ConfirmDeleteModalState, ConflictModalState, ConnectModalState, HostKeyModalState,
    NewFolderModalState, PropertiesModalState, RenameModalState,
};
use crate::ui::pane_state::PaneId;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_modal_overlay(&self, cx: &mut Context<Self>) -> Div {
        // The palette drops down from the top search field; dialogs stay centered.
        let is_palette = matches!(self.modal, ActiveModal::CommandPalette(_));
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            // Dialogs dim the workspace; the palette dropdown keeps it
            // visible so it reads attached to the search field, not modal.
            .when(is_palette, |el| el.bg(gpui::rgba(0x00000000)))
            .when(!is_palette, |el| el.bg(rgba(0x000000bb)))
            // Block hover/clicks reaching the panes behind any modal.
            .occlude()
            .when(!is_palette, |el| el.justify_center())
            // Clicking outside the palette dropdown dismisses it.
            .when(is_palette, |el| {
                el.on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                    this.modal = ActiveModal::None;
                    cx.notify();
                }))
            })
            .when(!is_palette, |el| el.justify_center())
            .child({
                let (key, dialog) = match &self.modal {
                    ActiveModal::CommandPalette(state) => ("palette", self.render_command_palette(state, cx)),
                    ActiveModal::Connect(state) => ("connect", self.render_connect_dialog(state, cx)),
                    ActiveModal::NewFolder(state) => ("new-folder", self.render_new_folder_dialog(state, cx)),
                    ActiveModal::Rename(state) => ("rename", self.render_rename_dialog(state, cx)),
                    ActiveModal::ConfirmDelete(state) => ("delete", self.render_confirm_delete_dialog(state, cx)),
                    ActiveModal::HostKeyTrust(state) => ("hostkey", self.render_host_key_dialog(state, cx)),
                    ActiveModal::Conflict(state) => ("conflict", self.render_conflict_dialog(state, cx)),
                    ActiveModal::BookmarksList => ("bookmarks", self.render_bookmarks_dialog(cx)),
                    ActiveModal::AddBookmark(state) => ("add-bookmark", self.render_add_bookmark_dialog(state, cx)),
                    ActiveModal::Properties(state) => ("properties", self.render_properties_dialog(state, cx)),
                    ActiveModal::None => ("none", div()),
                };
                let animated = crate::ui::motion::entrance(
                    self.animations_enabled,
                    dialog,
                    ElementId::Name(format!("modal-{key}").into()),
                );
                // The palette hangs below the top search field (the title
                // bar is 32 px); every other dialog stays centered.
                if is_palette {
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .mt(px(36.0))
                        .child(animated)
                        .into_any_element()
                } else {
                    animated
                }
            })
    }

    pub fn render_command_palette(&self, state: &CommandPaletteModalState, cx: &mut Context<Self>) -> Div {
        let filtered = self.filtered_palette(&state.query);
        let total_count = Self::command_palette_items().len();
        let selected_idx = if filtered.is_empty() {
            0
        } else {
            state.selected_index.min(filtered.len() - 1)
        };

        // Flat Zed-style rows: one icon per category, no group headers.
        // Build rows imperatively: nested `map` closures cannot share `cx`.
        let mut palette_rows: Vec<AnyElement> = Vec::new();
        for (idx, m) in filtered.iter().enumerate() {
            let is_sel = idx == selected_idx;
            let action_id = m.item.action;
            let shortcut = m.item.shortcut;
            let theme = self.theme;
            let runs = crate::ui::palette::highlight_runs(m.item.title, &m.matched);
            let kind_icon = match m.item.category {
                "Connection" => Icons::CONNECT,
                "Files" => Icons::FILE,
                "Navigate" => Icons::CHEVRON_RIGHT,
                "Bookmarks" => Icons::BOOKMARK,
                _ => Icons::DOT_HOLLOW,
            };
            let row =
            div()
                .id(ElementId::Name(format!("palette-item-{}", action_id).into()))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py(px(4.0))
                .rounded_sm()
                .bg(if is_sel { theme.element_selected } else { gpui::rgba(0x00000000) })
                .hover(|s| s.bg(theme.ghost_element_hover))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                    this.execute_palette_action(action_id, cx);
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .w(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.0))
                                .font_family(Icons::FAMILY)
                                .text_color(theme.text_muted)
                                .child(kind_icon),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .overflow_hidden()
                                .text_size(theme.font_size_ui)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(if is_sel { theme.text } else { theme.text_muted })
                                .children(runs.into_iter().map(|(text, hit)| {
                                    div()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(if hit { theme.text } else { theme.text_muted })
                                        .child(text)
                                })),
                        ),
                )
                .when_some(shortcut, |el, sc| {
                    if is_sel {
                        el.child(
                            div()
                                .px_1p5()
                                .py(px(1.0))
                                .rounded_sm()
                                .bg(theme.bg_badge_accent)
                                .border_1()
                                .border_color(theme.accent)
                                .font_family(theme.font_mono)
                                .text_size(theme.font_size_shortcut)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(theme.accent)
                                .child(sc),
                        )
                    } else {
                        el.child(
                            div()
                                .font_family(theme.font_mono)
                                .text_size(theme.font_size_shortcut)
                                .text_color(theme.text_placeholder)
                                .child(sc),
                        )
                    }
                });
            palette_rows.push(crate::ui::motion::staggered(
                self.animations_enabled,
                row,
                ElementId::Name(format!("palette-row-{}", action_id).into()),
                idx,
                5,
            ));
        }

        div()
            .flex()
            .flex_col()
            .w(px(300.0))
            .max_h(px(320.0))
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _ev, _window, cx| {
                cx.stop_propagation();
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(|_this, _ev, _window, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("palette-items-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.palette_scroll_handle)
                    .p_1()
                    .when(filtered.is_empty(), |el| {
                        el.child(
                            div()
                                .px_3()
                                .py_4()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(self.theme.font_size_ui)
                                .text_color(self.theme.text_placeholder)
                                .child(format!("No commands match '{}'", state.query)),
                        )
                    })
                    .children(palette_rows),
            )
            // Footer: key hints left, Run action right (Zed picker footer).
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_3()
                    .py(px(4.0))
                    .bg(self.theme.element_background)
                    .border_t_1()
                    .border_color(self.theme.border_variant)
                    .child(
                        div()
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_caption)
                            .text_color(self.theme.text_muted)
                            .child(format!("{}/{}", filtered.len(), total_count)),
                    )
                    .child({
                        let run_action =
                            filtered.get(selected_idx).map(|m| m.item.action);
                        let has_match = run_action.is_some();
                        crate::ui::controls::btn(
                            self.theme,
                            crate::ui::controls::BtnVariant::Primary,
                            "btn-palette-run",
                            "Run",
                            has_match,
                            cx.listener(move |this, _ev, _window, cx| {
                                if let Some(act) = run_action {
                                    this.execute_palette_action(act, cx);
                                }
                            }),
                        )
                    }),
            )
    }

    pub fn render_connect_dialog(&self, state: &ConnectModalState, cx: &mut Context<Self>) -> Div {
        use crate::ui::controls::{btn, dialog_footer, dialog_shell, error_box, field_label, text_input, BtnVariant, DialogHeader};
        let theme = self.theme;
        let is_connecting = state.is_connecting;
        let profiles = state.profiles.clone();
        let focus = |field: usize| {
            cx.listener(move |this, _ev, _window, cx| {
                if let ActiveModal::Connect(ref mut s) = this.modal {
                    s.focused_field = field;
                }
                cx.notify();
            })
        };

        let content = div()
            .flex()
            .flex_col()
            .gap_2()
            .when(!profiles.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(field_label(theme, "Saved Profiles:"))
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .children(profiles.into_iter().map(|p| {
                                    let p_clone = p.clone();
                                    let pid_del = p.id.clone();
                                    let is_sel = state.selected_profile_id.as_deref() == Some(p.id.as_str());
                                    div()
                                        .id(ElementId::Name(format!("select-prof-{}", p.id).into()))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if is_sel { self.theme.bg_badge_accent } else { self.theme.element_background })
                                        .border_1()
                                        .border_color(if is_sel { self.theme.border_selected } else { self.theme.border })
                                        .hover(|s| s.border_color(self.theme.accent))
                                        .text_size(self.theme.font_size_caption)
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(if is_sel { self.theme.accent } else { self.theme.text_muted })
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
                                        .child(
                                            div()
                                                .id(ElementId::Name(format!("del-prof-{}", pid_del).into()))
                                                .tooltip(tooltip_view(self.theme, "Delete Profile", None::<&str>))
                                                .cursor_pointer()
                                                .px_1()
                                                .rounded_md()
                                                .hover(|s| s.text_color(self.theme.danger))
                                                .text_size(px(11.0))
                                                .text_color(self.theme.text_muted)
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev: &MouseDownEvent, _window, cx| {
                                                    let _ = ProfileStore::delete_profile(&pid_del);
                                                    if let ActiveModal::Connect(ref mut s) = this.modal {
                                                        s.profiles.retain(|x| x.id != pid_del);
                                                        if s.selected_profile_id.as_deref() == Some(pid_del.as_str()) {
                                                            s.selected_profile_id = None;
                                                        }
                                                    }
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }))
                                                .child(Icons::CLOSE),
                                        )
                                })),
                        ),
                )
            })
            .when_some(state.error_msg.clone(), |el, err| {
                el.child(error_box(theme, &err))
            })
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
                            .child(field_label(theme, "Host:"))
                            .child(text_input(
                                theme,
                                "input-connect-host",
                                state.focused_field == 0,
                                if state.host_input.is_empty() { "example.com| ".to_string() } else { format!("{}|", state.host_input) },
                                true,
                                false,
                                focus(0),
                            )),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(field_label(theme, "Port:"))
                            .child(text_input(
                                theme,
                                "input-connect-port",
                                state.focused_field == 1,
                                if state.port_input.is_empty() { "22|".to_string() } else { format!("{}|", state.port_input) },
                                true,
                                false,
                                focus(1),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(field_label(theme, "Username:"))
                    .child(text_input(
                        theme,
                        "input-connect-user",
                        state.focused_field == 2,
                        if state.username_input.is_empty() { "username| ".to_string() } else { format!("{}|", state.username_input) },
                        true,
                        false,
                        focus(2),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(field_label(theme, "Authentication:"))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .children(
                                        [("Password", AuthKind::Password), ("Private Key", AuthKind::PrivateKey), ("SSH Agent", AuthKind::Agent)]
                                            .into_iter()
                                            .map(|(label, kind)| {
                                let is_sel = state.auth_kind == kind;
                                div()
                                    .id(ElementId::Name(format!("auth-type-{}", kind as u8).into()))
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if is_sel { self.theme.bg_badge_accent } else { self.theme.element_background })
                                    .border_1()
                                    .border_color(if is_sel { self.theme.border_selected } else { self.theme.border })
                                    .hover(|s| s.border_color(self.theme.accent))
                                    .text_size(self.theme.font_size_caption)
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(if is_sel { self.theme.accent } else { self.theme.text_muted })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        if let ActiveModal::Connect(ref mut s) = this.modal {
                                            s.auth_kind = kind;
                                            s.focused_field = 3;
                                        }
                                        cx.notify();
                                    }))
                                    .child(label)
                            })),
                    ),
            )
            .when(state.auth_kind == AuthKind::Password, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(field_label(theme, "Password:"))
                        .child(text_input(
                            theme,
                            "input-connect-pass",
                            state.focused_field == 3,
                            if state.password_input.is_empty() { "••••••••| ".to_string() } else { "•".repeat(state.password_input.len()) + "|" },
                            true,
                            true,
                            focus(3),
                        )),
                )
            })
            .when(state.auth_kind == AuthKind::PrivateKey, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(field_label(theme, "Private Key Path:"))
                        .child(text_input(
                            theme,
                            "input-connect-keypath",
                            state.focused_field == 3,
                            if state.key_path_input.is_empty() { "~/.ssh/id_rsa| ".to_string() } else { format!("{}|", state.key_path_input) },
                            true,
                            false,
                            focus(3),
                        )),
                )
            })
            .when(state.auth_kind == AuthKind::Agent, |el| {
                el.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(self.theme.bg_badge_accent)
                        .text_size(self.theme.font_size_caption)
                        .text_color(self.theme.text_muted)
                        .child("Uses keys offered by your local SSH agent. No password needed."),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("toggle-remember-creds")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                if let ActiveModal::Connect(ref mut s) = this.modal {
                                    s.remember_credentials = !s.remember_credentials;
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(13.0))
                                    .h(px(13.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .bg(if state.remember_credentials { self.theme.accent } else { self.theme.element_background })
                                    .border_1()
                                    .border_color(if state.remember_credentials { self.theme.border_focused } else { self.theme.border })
                                    .text_size(px(11.0))
                                    .font_family(Icons::FAMILY)
                                    .text_color(rgb(0xffffff))
                                    .child(if state.remember_credentials { Icons::CHECK } else { "" }),
                            )
                            .child(
                                div()
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_muted)
                                    .child("Remember credentials"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(field_label(theme, "Save as profile (optional):"))
                    .child(text_input(
                        theme,
                        "input-connect-profile-name",
                        state.focused_field == 4,
                        if state.save_as_profile_name.is_empty() { "my-server| ".to_string() } else { format!("{}|", state.save_as_profile_name) },
                        true,
                        false,
                        focus(4),
                    )),
            );

        let footer = dialog_footer(
            theme,
            vec![
            btn(
                theme,
                BtnVariant::Secondary,
                "btn-cancel-connect",
                "Cancel",
                true,
                cx.listener(|this, _ev, _window, cx| {
                    this.modal = ActiveModal::None;
                    cx.notify();
                }),
            )
            .into_any_element(),
            {
                let submit = btn(
                    theme,
                    BtnVariant::Primary,
                    "btn-submit-connect",
                    if is_connecting { "Connecting..." } else { "Connect" },
                    !is_connecting,
                    cx.listener(|this, _ev, _window, cx| {
                        this.execute_connect(cx);
                    }),
                );
                if is_connecting {
                    crate::ui::motion::pulse(
                        self.animations_enabled,
                        submit,
                        ElementId::Name("connecting".into()),
                    )
                } else {
                    submit.into_any_element()
                }
            },
        ]);

        dialog_shell(
            theme,
            cx,
            DialogHeader {
                title: "Connect to SFTP Server",
                icon: ("✦", theme.accent),
                description: None,
            },
            content.into_any_element(),
            Some(footer.into_any_element()),
            self.theme.modal_w_md,
        )
    }

    pub fn render_new_folder_dialog(&self, state: &NewFolderModalState, cx: &mut Context<Self>) -> Div {
        let pane_id = state.target_pane;
        let folder_name = state.folder_name.clone();

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
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
                            .font_weight(FontWeight::NORMAL)
                            .child("+"),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child("Create New Directory"),
                    ),
            )
            .child(
                div()
                    .id("input-newfolder-name")
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border_focused)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_body)
                    .text_color(if folder_name.is_empty() { self.theme.text_placeholder } else { self.theme.text })
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let submit_name = if folder_name.is_empty() { "New Folder".to_string() } else { folder_name.clone() };
                                this.modal = ActiveModal::None;
                                this.create_folder(pane_id, &submit_name, cx);
                                cx.notify();
                            }))
                            .child("Create"),
                    ),
            )
    }

    pub fn render_rename_dialog(&self, state: &RenameModalState, cx: &mut Context<Self>) -> Div {
        let pane_id = state.target_pane;
        let orig = state.original_name.clone();
        let new_name = state.new_name.clone();

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
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
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child(format!("Rename '{}'", orig)),
                    ),
            )
            .child(
                div()
                    .id("input-rename-name")
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border_focused)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_body)
                    .text_color(self.theme.text)
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                this.rename_entry(pane_id, &orig, &new_name, cx);
                                cx.notify();
                            }))
                            .child("Rename"),
                    ),
            )
    }

    pub fn render_confirm_delete_dialog(&self, state: &ConfirmDeleteModalState, cx: &mut Context<Self>) -> Div {
        let pane_id = state.target_pane;
        let items = state.item_names.clone();

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
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
                            .font_family(Icons::FAMILY)
                            .child(Icons::DELETE),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.danger)
                            .child("Confirm Permanent Deletion"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_muted)
                    .child(format!("Are you sure you want to permanently delete {} selected item(s)?", items.len())),
            )
            .child(
                div()
                    .max_h(px(110.0))
                    .overflow_hidden()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .text_color(self.theme.text)
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.danger)
                            .hover(|s| s.bg(self.theme.danger_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.confirm_delete(pane_id, items.clone(), cx);
                                cx.notify();
                            }))
                            .child("Delete"),
                    ),
            )
    }

    pub fn render_host_key_dialog(&self, state: &HostKeyModalState, cx: &mut Context<Self>) -> Div {
        let host = state.host.clone();
        let port = state.port;
        let fp = state.fingerprint.clone();

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_md)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(self.theme.warning)
                            .child(Icons::WARNING),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.warning)
                            .child("Unknown SSH Host Key"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_muted)
                    .child(format!("The authenticity of host '{host}:{port}' cannot be established.")),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .font_weight(FontWeight::NORMAL)
                    .text_color(self.theme.warning)
                    .child(fp),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_breadcrumbs)
                    .text_color(self.theme.text_placeholder)
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.trust_pending_connection(cx);
                            }))
                            .child("Trust & Connect"),
                    ),
            )
    }

    pub fn render_conflict_dialog(&self, state: &ConflictModalState, cx: &mut Context<Self>) -> Div {
        let filename = state.filename.clone();
        let src_sz = format_bytes(state.source_size);
        let dst_sz = format_bytes(state.dest_size);

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
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
                            .font_family(Icons::FAMILY)
                            .child(Icons::WARNING),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child("File Conflict"),
                    ),
            )
            .child(
                div()
                    .text_size(self.theme.font_size_ui)
                    .text_color(self.theme.text_muted)
                    .child(format!("A file named '{}' already exists at the target destination.", filename)),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
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
                            .child(div().text_color(self.theme.text).child(src_sz)),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(div().text_color(self.theme.text_muted).child("Existing size:"))
                            .child(div().text_color(self.theme.text).child(dst_sz)),
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.danger)
                            .hover(|s| s.bg(self.theme.danger_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
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

    pub fn render_bookmarks_dialog(&self, cx: &mut Context<Self>) -> Div {
        // Render from the in-memory cache: this runs every frame, so it must
        // not touch the filesystem. The cache is refreshed when the dialog
        // opens and after every mutation.
        let saved_bookmarks = self.bookmark_store.cached_saved_bookmarks.clone();
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
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
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
                                    .font_family(Icons::FAMILY)
                                    .child(Icons::BOOKMARK),
                            )
                            .child(
                                div()
                                    .text_size(self.theme.font_size_modal_title)
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(self.theme.text_muted)
                                    .child("Bookmarks"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-close-bookmarks")
                            .tooltip(tooltip_view(self.theme, "Close", Some("Esc")))
                            .cursor_pointer()
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_family(Icons::FAMILY)
                            .text_color(self.theme.text_muted)
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
                                .text_color(self.theme.text_placeholder)
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
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .overflow_hidden()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.navigate_bookmark(active_pane, bp.clone(), is_rem, cx);
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(Icons::FAMILY)
                                            .text_color(if is_rem { self.theme.syntax_cyan } else { self.theme.dir_icon })
                                            .child(if is_rem { Icons::NETWORK } else { Icons::FOLDER })
                                    )
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_ui)
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(self.theme.text)
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
                                    .tooltip(tooltip_view(self.theme, "Delete Bookmark", None::<&str>))
                                    .cursor_pointer()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .hover(|s| s.bg(self.theme.bg_badge_danger).text_color(self.theme.danger))
                                    .text_size(px(11.0))
                                    .text_color(self.theme.text_muted)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        let _ = BookmarkStore::delete_saved_bookmark(&bid);
                                        this.bookmark_store.refresh_saved_cache();
                                        cx.notify();
                                    }))
                                    .child(Icons::DELETE)
                            )
                    }))
                    .children(session_bookmarks.into_iter().map(|b| {
                        let bp = b.path.clone();
                        let bid_del = b.id.clone();
                        let is_rem = b.is_remote;
                        div()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.navigate_bookmark(active_pane, bp.clone(), is_rem, cx);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(Icons::FAMILY)
                                            .text_color(if is_rem { self.theme.syntax_cyan } else { self.theme.dir_icon })
                                            .child(if is_rem { Icons::NETWORK } else { Icons::FOLDER })
                                    )
                                    .child(
                                        div()
                                            .text_size(self.theme.font_size_ui)
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(self.theme.text)
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
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py(px(0.5))
                                            .rounded_md()
                                            .bg(self.theme.element_background)
                                            .font_family(self.theme.font_mono)
                                            .text_size(self.theme.font_size_shortcut)
                                            .text_color(self.theme.text_muted)
                                            .child("session"),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::Name(format!("bm-sess-del-{}", bid_del).into()))
                                            .tooltip(tooltip_view(self.theme, "Delete Bookmark", None::<&str>))
                                            .cursor_pointer()
                                            .w(px(20.0))
                                            .h(px(20.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .hover(|s| s.bg(self.theme.bg_badge_danger).text_color(self.theme.danger))
                                            .text_size(px(11.0))
                                            .font_family(Icons::FAMILY)
                                            .text_color(self.theme.text_muted)
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev: &MouseDownEvent, _window, cx| {
                                                this.bookmark_store.remove_session_bookmark(&bid_del);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }))
                                            .child(Icons::DELETE)
                                    )
                            )
                    }))
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .pt_2()
                    .border_t_1()
                    .border_color(self.theme.border_variant)
                    .child(
                        div()
                            .id("btn-bookmark-current")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.bg_badge_accent)
                            .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
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
                                    error_msg: None,
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Close"),
                    ),
            )
    }

    pub fn render_add_bookmark_dialog(&self, state: &AddBookmarkModalState, cx: &mut Context<Self>) -> Div {
        let label = state.label.clone();
        let path = state.path.clone();
        let is_remote = state.is_remote;
        let is_session = state.is_session_only;
        let err_opt = state.error_msg.clone();

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
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
                            .font_family(Icons::FAMILY)
                            .child(Icons::BOOKMARK),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child("Add Bookmark"),
                    ),
            )
            .when_some(err_opt, |el, err| {
                el.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(self.theme.bg_badge_danger)
                        .border_1()
                        .border_color(self.theme.danger)
                        .text_size(self.theme.font_size_caption)
                        .text_color(self.theme.danger)
                        .child(err),
                )
            })
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
                            .rounded_md()
                            .bg(self.theme.element_background)
                            .border_1()
                            .border_color(self.theme.border_focused)
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_body)
                            .text_color(self.theme.text)
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
                            .rounded_md()
                            .bg(self.theme.element_background)
                            .border_1()
                            .border_color(self.theme.border)
                            .font_family(self.theme.font_mono)
                            .text_size(self.theme.font_size_mono)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
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
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.submit_add_bookmark(label.clone(), path.clone(), is_remote, is_session, cx);
                            }))
                            .child("Save Bookmark"),
                    ),
            )
    }

    pub fn render_properties_dialog(&self, state: &PropertiesModalState, cx: &mut Context<Self>) -> Div {
        let rows = vec![
            ("Type", if state.is_dir { "Directory".to_string() } else { "File".to_string() }),
            ("Location", if state.is_remote { "Remote (SFTP)".to_string() } else { "Local".to_string() }),
            ("Size", state.size_str.clone()),
            ("Modified", state.modified_str.clone()),
            ("Permissions", state.permissions.clone()),
            ("Extension", state.extension.clone()),
        ];

        div()
            .flex()
            .flex_col()
            .w(self.theme.modal_w_sm)
            .p_4()
            .rounded_lg()
            .bg(self.theme.elevated_surface_background)
            .border_1()
            .border_color(self.theme.border)
            .shadow_md()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(Icons::FAMILY)
                            .text_color(if state.is_dir { self.theme.dir_icon } else { self.theme.file_icon })
                            .child(if state.is_dir { Icons::FOLDER } else { Icons::FILE }),
                    )
                    .child(
                        div()
                            .text_size(self.theme.font_size_modal_title)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(self.theme.text_muted)
                            .child("Properties"),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .text_color(self.theme.text)
                    .child(state.name.clone()),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_mono)
                    .children(rows.into_iter().map(|(k, v)| {
                        div()
                            .flex()
                            .justify_between()
                            .child(div().text_color(self.theme.text_muted).child(k))
                            .child(div().text_color(self.theme.text).child(v))
                    })),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(self.theme.element_background)
                    .border_1()
                    .border_color(self.theme.border)
                    .font_family(self.theme.font_mono)
                    .text_size(self.theme.font_size_caption)
                    .text_color(self.theme.text_muted)
                    .child(state.path.clone()),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .child(
                        div()
                            .id("btn-close-properties")
                            .cursor_pointer()
                            .px_3()
                            .py(px(4.0))
                            .rounded_md()
                            .bg(self.theme.accent)
                            .hover(|s| s.bg(self.theme.accent_hover))
                            .text_size(self.theme.font_size_ui)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                this.modal = ActiveModal::None;
                                cx.notify();
                            }))
                            .child("Close"),
                    ),
            )
    }
}
