use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::fs::path_utils::{build_breadcrumbs, join_path};
use crate::fs::types::{format_bytes, SortColumn, SortDirection};
use crate::ui::icons::Icons;
use crate::ui::pane_state::PaneId;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_pane(&self, pane_id: PaneId, cx: &mut Context<Self>) -> impl IntoElement {
        let is_left = pane_id == PaneId::Left;
        let is_active = self.active_pane == pane_id;
        let pane = if is_left {
            &self.left_pane
        } else {
            &self.right_pane
        };

        let (dir_count, file_count, total_size, sel_count, sel_size) = pane.stats();
        let crumbs = build_breadcrumbs(&pane.current_path, pane.is_remote);

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(self.theme.bg_pane)
            .border_r_1()
            .border_color(if is_active {
                self.theme.border_focused
            } else {
                self.theme.border
            })
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                this.active_pane = pane_id;
                cx.notify();
            }))
            // Unified Minimal Pane Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(26.0))
                    .px_2()
                    .bg(self.theme.bg_header)
                    .border_b_1()
                    .border_color(if is_active { self.theme.accent } else { self.theme.border })
                    // Left: Identity & Breadcrumbs
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_color(if is_left {
                                                self.theme.dir_icon
                                            } else {
                                                self.theme.syntax_cyan
                                            })
                                            .text_size(px(7.5))
                                            .child(Icons::DOT),
                                    )
                                    .child(
                                        div()
                                            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::MEDIUM })
                                            .text_size(self.theme.font_size_caption)
                                            .text_color(if is_active {
                                                self.theme.text_primary
                                            } else {
                                                self.theme.text_secondary
                                            })
                                            .child(if is_left {
                                                "Local"
                                            } else if pane.is_remote {
                                                "Remote"
                                            } else {
                                                "Local 2"
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(10.0))
                                    .bg(self.theme.border)
                                    .mx_1(),
                            )
                            .child(
                                if pane.path_input_active {
                                    div()
                                        .id(ElementId::Name(format!("path-input-bar-{:?}", pane_id).into()))
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .flex_1()
                                        .child(
                                            div()
                                                .text_size(self.theme.font_size_caption)
                                                .text_color(self.theme.accent)
                                                .font_weight(FontWeight::BOLD)
                                                .child("Go:"),
                                        )
                                        .child(
                                            div()
                                                .font_family(self.theme.font_mono)
                                                .text_size(self.theme.font_size_mono)
                                                .text_color(self.theme.text_primary)
                                                .child(format!("{}|", pane.path_input_buffer)),
                                        )
                                        .child(
                                            div()
                                                .text_size(self.theme.font_size_caption)
                                                .text_color(self.theme.text_muted)
                                                .child("⏎ jump · esc cancel"),
                                        )
                                } else {
                                    div()
                                        .id(ElementId::Name(format!("breadcrumb-bar-{:?}", pane_id).into()))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_0p5()
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_size(self.theme.font_size_breadcrumbs)
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                            this.active_pane = pane_id;
                                            let p = match pane_id {
                                                PaneId::Left => &mut this.left_pane,
                                                PaneId::Right => &mut this.right_pane,
                                            };
                                            p.path_input_active = true;
                                            p.path_input_buffer = p.current_path.clone();
                                            cx.notify();
                                        }))
                                        .children(crumbs.into_iter().map(|crumb| {
                                            let target_path = crumb.path.clone();
                                            let is_last = crumb.is_last;
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .id(ElementId::Name(format!("crumb-{:?}-{}", pane_id, crumb.path).into()))
                                                        .cursor_pointer()
                                                        .px(px(2.0))
                                                        .py(px(0.5))
                                                        .rounded_sm()
                                                        .hover(|s| s.bg(self.theme.bg_row_hover).text_color(self.theme.text_primary))
                                                        .font_weight(if is_last {
                                                            FontWeight::SEMIBOLD
                                                        } else {
                                                            FontWeight::NORMAL
                                                        })
                                                        .text_color(if is_last {
                                                            self.theme.text_primary
                                                        } else {
                                                            self.theme.text_secondary
                                                        })
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                                            this.navigate_pane(pane_id, target_path.clone());
                                                            cx.stop_propagation();
                                                            cx.notify();
                                                        }))
                                                        .child(crumb.label),
                                                )
                                                .when(!is_last, |el| {
                                                    el.child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .text_color(self.theme.text_muted)
                                                            .child("/"),
                                                    )
                                                })
                                        }))
                                }
                            ),
                    )
                    // Right: Fast Toolbar Action Icons with Tooltips
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_0p5()
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-back-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Back", Some("Alt+Left")))
                                    .cursor_pointer()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_ui)
                                    .text_color(if pane.history_back.is_empty() {
                                        self.theme.text_muted
                                    } else {
                                        self.theme.text_primary
                                    })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_back(pane_id);
                                        cx.notify();
                                    }))
                                    .child("‹"),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-forward-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Forward", Some("Alt+Right")))
                                    .cursor_pointer()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_ui)
                                    .text_color(if pane.history_forward.is_empty() {
                                        self.theme.text_muted
                                    } else {
                                        self.theme.text_primary
                                    })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_forward(pane_id);
                                        cx.notify();
                                    }))
                                    .child("›"),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-parent-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Parent Directory", Some("Backspace")))
                                    .cursor_pointer()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_secondary)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_parent(pane_id);
                                        cx.notify();
                                    }))
                                    .child("↑"),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-refresh-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Refresh", Some("Ctrl+R")))
                                    .cursor_pointer()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_secondary)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.refresh_pane(pane_id);
                                        cx.notify();
                                    }))
                                    .child("⟳"),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-hidden-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Toggle Hidden Files", Some("Ctrl+H")))
                                    .cursor_pointer()
                                    .px_1()
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .when(pane.show_hidden, |s| s.bg(self.theme.bg_badge_accent))
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if pane.show_hidden { self.theme.accent } else { self.theme.text_muted })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.toggle_hidden(pane_id);
                                        cx.notify();
                                    }))
                                    .child(".*"),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-filter-toggle-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view("Filter Files", Some("Ctrl+F")))
                                    .cursor_pointer()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .when(pane.filter_active || !pane.filter.is_empty(), |s| s.bg(self.theme.bg_badge_accent))
                                    .hover(|s| s.bg(self.theme.bg_row_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(if pane.filter_active || !pane.filter.is_empty() { self.theme.accent } else { self.theme.text_muted })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.active_pane = pane_id;
                                        let p = match pane_id {
                                            PaneId::Left => &mut this.left_pane,
                                            PaneId::Right => &mut this.right_pane,
                                        };
                                        p.filter_active = !p.filter_active;
                                        cx.notify();
                                    }))
                                    .child(Icons::SEARCH),
                            ),
                    ),
            )
            // Contextual Filter Bar (Only shown when active or filter is applied)
            .when(pane.filter_active || !pane.filter.is_empty(), |el| {
                el.child(
                    div()
                        .id(ElementId::Name(format!("filter-bar-{:?}", pane_id).into()))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h(px(22.0))
                        .bg(self.theme.bg_input)
                        .border_b_1()
                        .border_color(self.theme.accent)
                        .child(
                            div()
                                .text_size(self.theme.font_size_caption)
                                .text_color(self.theme.accent)
                                .child(Icons::SEARCH),
                        )
                        .child(
                            div()
                                .flex_1()
                                .font_family(self.theme.font_mono)
                                .text_size(self.theme.font_size_caption)
                                .text_color(self.theme.text_primary)
                                .child(if pane.filter.is_empty() {
                                    "Filter files... |".to_string()
                                } else {
                                    format!("{}|", pane.filter)
                                }),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("btn-clear-filter-{:?}", pane_id).into()))
                                .cursor_pointer()
                                .px_1()
                                .py(px(0.5))
                                .rounded_sm()
                                .hover(|s| s.text_color(self.theme.text_primary))
                                .text_size(self.theme.font_size_caption)
                                .text_color(self.theme.text_muted)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                    let p = match pane_id {
                                        PaneId::Left => &mut this.left_pane,
                                        PaneId::Right => &mut this.right_pane,
                                    };
                                    p.filter.clear();
                                    p.filter_active = false;
                                    this.refresh_pane(pane_id);
                                    cx.notify();
                                }))
                                .child("✕ Esc"),
                        ),
                )
            })
            // Table Header Row
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(22.0))
                    .px_2p5()
                    .bg(self.theme.bg_header)
                    .border_b_1()
                    .border_color(self.theme.border)
                    .text_size(self.theme.font_size_table_header)
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.theme.text_muted)
                    .child(
                        div()
                            .flex_1()
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text_primary))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Name);
                                this.refresh_pane(pane_id);
                                cx.notify();
                            }))
                            .child(format!(
                                "NAME{}",
                                if pane.sort_column == SortColumn::Name {
                                    if pane.sort_direction == SortDirection::Ascending {
                                        Icons::SORT_ASC
                                    } else {
                                        Icons::SORT_DESC
                                    }
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .w(px(46.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text_primary))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Extension);
                                this.refresh_pane(pane_id);
                                cx.notify();
                            }))
                            .child(format!(
                                "EXT{}",
                                if pane.sort_column == SortColumn::Extension {
                                    if pane.sort_direction == SortDirection::Ascending {
                                        Icons::SORT_ASC
                                    } else {
                                        Icons::SORT_DESC
                                    }
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .w(px(72.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text_primary))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Size);
                                this.refresh_pane(pane_id);
                                cx.notify();
                            }))
                            .child(format!(
                                "SIZE{}",
                                if pane.sort_column == SortColumn::Size {
                                    if pane.sort_direction == SortDirection::Ascending {
                                        Icons::SORT_ASC
                                    } else {
                                        Icons::SORT_DESC
                                    }
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .w(px(130.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text_primary))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Modified);
                                this.refresh_pane(pane_id);
                                cx.notify();
                            }))
                            .child(format!(
                                "MODIFIED{}",
                                if pane.sort_column == SortColumn::Modified {
                                    if pane.sort_direction == SortDirection::Ascending {
                                        Icons::SORT_ASC
                                    } else {
                                        Icons::SORT_DESC
                                    }
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .w(px(64.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text_primary))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Permissions);
                                this.refresh_pane(pane_id);
                                cx.notify();
                            }))
                            .child("PERM"),
                    ),
            )
            // Scrollable File List Body
            .child(
                div()
                    .id(ElementId::Name(format!("body-scroll-{:?}", pane_id).into()))
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(if is_left {
                        &self.left_scroll_handle
                    } else {
                        &self.right_scroll_handle
                    })
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        this.open_context_menu(pane_id, None, ev.position);
                        cx.notify();
                    }))
                    .when(pane.is_remote && !self.remote_client.is_connected(), |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .h_full()
                                .p_6()
                                .gap_3()
                                .child(
                                    div()
                                        .text_size(px(24.0))
                                        .text_color(self.theme.syntax_cyan)
                                        .child("☁"),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(self.theme.font_size_modal_title)
                                        .text_color(self.theme.text_primary)
                                        .child("Remote SFTP Disconnected"),
                                )
                                .child(
                                    div()
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.text_secondary)
                                        .child("Connect to an SSH/SFTP server to view remote files"),
                                )
                                .child(
                                    div()
                                        .id("btn-connect-in-pane")
                                        .cursor_pointer()
                                        .px_4()
                                        .py_1p5()
                                        .rounded_sm()
                                        .bg(self.theme.bg_badge_accent)
                                        .border_1()
                                        .border_color(self.theme.accent)
                                        .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.accent)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                            this.open_connect_modal();
                                            cx.notify();
                                        }))
                                        .child("Connect SFTP"),
                                ),
                        )
                    })
                    .when(pane.is_loading, |el| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(40.0))
                                .gap_2()
                                .child(
                                    div()
                                        .text_color(self.theme.accent)
                                        .text_size(self.theme.font_size_ui)
                                        .child("⟳"),
                                )
                                .child(
                                    div()
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.text_muted)
                                        .child("Loading directory..."),
                                ),
                        )
                    })
                    .when_some(pane.error_message.as_ref(), |el, err| {
                        el.child(
                            div()
                                .m_2()
                                .p_3()
                                .bg(self.theme.bg_input)
                                .border_1()
                                .border_color(self.theme.danger)
                                .text_size(self.theme.font_size_ui)
                                .text_color(self.theme.danger)
                                .child(err.clone()),
                        )
                    })
                    // Up directory '..' row
                    .child(
                        div()
                            .id(ElementId::Name(format!("row-{:?}-parent", pane_id).into()))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .h(px(21.0))
                            .px_2p5()
                            .bg(self.theme.bg_pane)
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.go_parent(pane_id);
                                cx.notify();
                            }))
                            .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                this.open_context_menu(pane_id, None, ev.position);
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex_1()
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
                                            .text_color(self.theme.accent)
                                            .font_weight(FontWeight::BOLD)
                                            .child(Icons::UP_ARROW),
                                    )
                                    .child(
                                        div()
                                            .font_family(self.theme.font_ui)
                                            .text_size(self.theme.font_size_body)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(self.theme.text_secondary)
                                            .child(".."),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(46.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(self.theme.text_muted)
                                    .child(""),
                            )
                            .child(
                                div()
                                    .w(px(72.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(self.theme.text_muted)
                                    .child("<DIR>"),
                            )
                            .child(
                                div()
                                    .w(px(130.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(self.theme.text_muted)
                                    .child(""),
                            )
                            .child(
                                div()
                                    .w(px(64.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_muted)
                                    .child(""),
                            ),
                    )
                    // File items
                    .children(pane.entries.iter().enumerate().map(|(idx, entry)| {
                        let is_cursor = pane.focused_index == Some(idx);
                        let is_selected = pane.selected_items.contains(&entry.name);
                        let is_dir = entry.is_dir();
                        let target_path = if pane.is_remote {
                            join_path(&pane.current_path, &entry.name, true)
                        } else {
                            entry.path.to_string_lossy().to_string()
                        };

                        let icon = if is_dir {
                            Icons::FOLDER
                        } else {
                            Icons::for_file(&entry.extension)
                        };

                        let icon_color = if is_dir {
                            self.theme.dir_icon
                        } else {
                            self.theme.file_icon
                        };

                        let entry_name_clone = entry.name.clone();
                        let name_clone_for_right = entry.name.clone();
                        let entry_clone_for_right = entry.clone();

                        let row_bg = if is_selected {
                            self.theme.bg_selected
                        } else if is_cursor {
                            self.theme.bg_row_hover
                        } else if idx % 2 == 1 {
                            self.theme.bg_row_alt
                        } else {
                            self.theme.bg_pane
                        };

                        div()
                            .id(ElementId::Name(format!("row-{:?}-{}", pane_id, entry.name).into()))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .h(px(21.0))
                            .px_2p5()
                            .bg(row_bg)
                            .hover(|s| s.bg(self.theme.bg_row_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                this.active_pane = pane_id;
                                let shift = ev.modifiers.shift;
                                let ctrl = ev.modifiers.control || ev.modifiers.platform;

                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };

                                if ev.click_count == 2 {
                                    if is_dir {
                                        this.navigate_pane(pane_id, target_path.clone());
                                    }
                                } else if shift {
                                    p.select_range_to(idx);
                                } else if ctrl {
                                    p.toggle_selection(&entry_name_clone, idx);
                                } else {
                                    p.set_single_selection(&entry_name_clone, idx);
                                }
                                cx.notify();
                            }))
                            .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                this.active_pane = pane_id;
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                if !p.selected_items.contains(&name_clone_for_right) {
                                    p.set_single_selection(&name_clone_for_right, idx);
                                }
                                this.open_context_menu(pane_id, Some(entry_clone_for_right.clone()), ev.position);
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(px(14.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(10.0))
                                            .font_weight(if is_dir { FontWeight::BOLD } else { FontWeight::NORMAL })
                                            .text_color(icon_color)
                                            .child(icon)
                                    )
                                    .child(
                                        div()
                                            .font_family(self.theme.font_ui)
                                            .text_size(self.theme.font_size_body)
                                            .font_weight(if is_dir {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(if is_selected {
                                                self.theme.text_primary
                                            } else if is_dir {
                                                self.theme.dir_icon
                                            } else {
                                                self.theme.text_primary
                                            })
                                            .child(entry.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(46.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(self.theme.text_muted)
                                    .child(entry.extension.clone()),
                            )
                            .child(
                                div()
                                    .w(px(72.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(if is_dir {
                                        self.theme.dir_icon
                                    } else {
                                        self.theme.text_secondary
                                    })
                                    .child(entry.formatted_size()),
                            )
                            .child(
                                div()
                                    .w(px(130.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_mono)
                                    .text_color(self.theme.text_muted)
                                    .child(entry.formatted_modified()),
                            )
                            .child(
                                div()
                                    .w(px(64.0))
                                    .font_family(self.theme.font_mono)
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_muted)
                                    .child(entry.metadata.permissions_str.clone()),
                            )
                    })),
            )
            // Pane Footer
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(22.0))
                    .px_2p5()
                    .bg(self.theme.bg_header)
                    .border_t_1()
                    .border_color(self.theme.border)
                    .text_size(self.theme.font_size_caption)
                    .text_color(self.theme.text_muted)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(self.theme.text_secondary)
                                    .child(format!("{} files · {} folders", file_count, dir_count)),
                            )
                            .child(div().text_color(self.theme.border).child("|"))
                            .child(
                                div()
                                    .font_family(self.theme.font_mono)
                                    .child(format_bytes(total_size)),
                            ),
                    )
                    .child(
                        if sel_count > 0 {
                            div()
                                .px_2()
                                .py(px(1.5))
                                .rounded_sm()
                                .bg(self.theme.bg_badge_accent)
                                .border_1()
                                .border_color(self.theme.accent)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(self.theme.accent)
                                .child(format!("{} selected ({})", sel_count, format_bytes(sel_size)))
                        } else {
                            div()
                                .text_color(self.theme.text_muted)
                                .child("0 selected")
                        },
                    ),
            )
    }
}
