use std::sync::Arc;

use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::app::RiftApp;
use crate::fs::path_utils::join_path;
use crate::fs::types::{format_bytes, FileEntry, SortColumn, SortDirection};
use crate::theme::Theme;
use crate::ui::icons::Icons;
use crate::ui::pane_state::PaneId;
use crate::ui::tooltip::tooltip_view;

impl RiftApp {
    pub fn render_pane(&self, pane_id: PaneId, cx: &mut Context<Self>) -> Div {
        let is_left = pane_id == PaneId::Left;
        let is_active = self.active_pane == pane_id;
        let pane = if is_left {
            &self.left_pane
        } else {
            &self.right_pane
        };

        let (dir_count, file_count, total_size, sel_count, sel_size) = pane.stats();
        let crumbs = pane.cached_crumbs.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(if is_active {
                self.theme.surface_background
            } else {
                self.theme.panel_background
            })
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
                    .h(px(28.0))
                    .px_3()
                    .bg(self.theme.tab_bar_background)
                    .border_b_1()
                    .border_color(if is_active { self.theme.border_focused } else { self.theme.border })
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
                                            .text_color(self.theme.text_placeholder)
                                            .text_size(px(8.0))
                                            .font_family(Icons::FAMILY)
                                            .child(Icons::DOT),
                                    )
                                    .child(
                                        div()
                                            .font_weight(FontWeight::NORMAL)
                                            .text_size(self.theme.font_size_caption)
                                            .text_color(if is_active {
                                                self.theme.text
                                            } else {
                                                self.theme.text_muted
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
                                    .bg(self.theme.border_variant)
                                    .mx_1(),
                            )
                            .child(
                                if pane.path_input_active {
                                    crate::ui::motion::flash(
                                        self.animations_enabled,
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
                                                    .font_weight(FontWeight::NORMAL)
                                                    .child("Go:"),
                                            )
                                            .child(
                                                div()
                                                    .font_family(self.theme.font_mono)
                                                    .text_size(self.theme.font_size_mono)
                                                    .text_color(self.theme.text)
                                                    .flex()
                                                    .items_center()
                                                    .child(format!("{}|", pane.path_input_buffer)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(self.theme.font_size_caption)
                                                    .text_color(self.theme.text_placeholder)
                                                    .child("⏎ jump · esc cancel"),
                                            ),
                                        ElementId::Name(format!("path-input-flash-{:?}", pane_id).into()),
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
                                                        .rounded_md()
                                                        .hover(|s| s.bg(self.theme.ghost_element_hover).text_color(self.theme.text))
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(if is_last {
                                                            self.theme.text
                                                        } else {
                                                            self.theme.text_muted
                                                        })
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                                            this.navigate_pane(pane_id, target_path.clone(), cx);
                                                            cx.stop_propagation();
                                                            cx.notify();
                                                        }))
                                                        .child(crumb.label),
                                                )
                                                .when(!is_last, |el| {
                                                    el.child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_family(Icons::FAMILY)
                                                            .text_color(self.theme.text_placeholder)
                                                            .child(Icons::CHEVRON_RIGHT),
                                                    )
                                                })
                                        }))
                                        .into_any_element()
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
                                    .tooltip(tooltip_view(self.theme, "Back", Some("Alt+Left")))
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
                                    .text_color(if pane.history_back.is_empty() {
                                        self.theme.text_disabled
                                    } else {
                                        self.theme.text
                                    })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_back(pane_id, cx);
                                        cx.notify();
                                    }))
                                    .child(Icons::CHEVRON_LEFT),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-forward-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view(self.theme, "Forward", Some("Alt+Right")))
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
                                    .text_color(if pane.history_forward.is_empty() {
                                        self.theme.text_disabled
                                    } else {
                                        self.theme.text
                                    })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_forward(pane_id, cx);
                                        cx.notify();
                                    }))
                                    .child(Icons::CHEVRON_RIGHT),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-parent-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view(self.theme, "Parent Directory", Some("Backspace")))
                                    .cursor_pointer()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .hover(|s| s.bg(self.theme.ghost_element_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .font_family(Icons::FAMILY)
                                    .text_color(self.theme.text_muted)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.go_parent(pane_id, cx);
                                        cx.notify();
                                    }))
                                    .child(Icons::UP_ARROW),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-refresh-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view(self.theme, "Refresh", Some("Ctrl+R")))
                                    .cursor_pointer()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .hover(|s| s.bg(self.theme.ghost_element_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .text_color(self.theme.text_muted)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.refresh_pane(pane_id, cx);
                                        cx.notify();
                                    }))
                                    .child({
                                        let icon = div().font_family(Icons::FAMILY).child(Icons::REFRESH);
                                        if pane.is_loading {
                                            crate::ui::motion::pulse(
                                                self.animations_enabled,
                                                icon,
                                                ElementId::Name(format!("refresh-spin-{:?}", pane_id).into()),
                                            )
                                        } else {
                                            icon.into_any_element()
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-hidden-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view(self.theme, "Toggle Hidden Files", Some("Ctrl+H")))
                                    .cursor_pointer()
                                    .px_1()
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .when(pane.show_hidden, |s| s.bg(self.theme.bg_badge_accent))
                                    .hover(|s| s.bg(self.theme.ghost_element_hover))
                                    .text_size(self.theme.font_size_caption)
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(if pane.show_hidden { self.theme.accent } else { self.theme.text_muted })
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                        this.toggle_hidden(pane_id, cx);
                                        cx.notify();
                                    }))
                                    .child(Icons::HIDDEN),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("btn-filter-toggle-{:?}", pane_id).into()))
                                    .tooltip(tooltip_view(self.theme, "Filter Files", Some("Ctrl+F")))
                                    .cursor_pointer()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .when(pane.filter_active || !pane.filter.is_empty(), |s| s.bg(self.theme.bg_badge_accent))
                                    .hover(|s| s.bg(self.theme.ghost_element_hover))
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
                el.child(crate::ui::motion::flash(
                    self.animations_enabled,
                    div()
                        .id(ElementId::Name(format!("filter-bar-{:?}", pane_id).into()))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h(px(22.0))
                        .bg(self.theme.element_background)
                        .border_b_1()
                        .border_color(self.theme.accent)
                        .child(
                            div()
                                .text_size(self.theme.font_size_caption)
                                .font_family(Icons::FAMILY)
                                .text_color(self.theme.accent)
                                .child(Icons::SEARCH),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .font_family(self.theme.font_mono)
                                .text_size(self.theme.font_size_caption)
                                .text_color(self.theme.text)
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
                                .rounded_md()
                                .hover(|s| s.text_color(self.theme.text))
                                .text_size(self.theme.font_size_caption)
                                .text_color(self.theme.text_placeholder)
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                    let p = match pane_id {
                                        PaneId::Left => &mut this.left_pane,
                                        PaneId::Right => &mut this.right_pane,
                                    };
                                    p.filter.clear();
                                    p.filter_active = false;
                                    this.refresh_pane(pane_id, cx);
                                    cx.notify();
                                }))
                                .child(format!("{} Esc", Icons::CLOSE)),
                        ),
                    ElementId::Name(format!("filter-bar-flash-{:?}", pane_id).into()),
                ))
            })
            // Table Header Row
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(22.0))
                    .px_3()
                    .gap_2()
                    .bg(self.theme.tab_bar_background)
                    .border_b_1()
                    .border_color(self.theme.border)
                    .text_size(self.theme.font_size_table_header)
                    .font_weight(FontWeight::NORMAL)
                    .text_color(self.theme.text_muted)
                    .child(
                        div()
                            .flex_1()
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text))
                            .text_color(if pane.sort_column == SortColumn::Name { self.theme.accent } else { self.theme.text_muted })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Name);
                                this.refresh_pane(pane_id, cx);
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
                            .hover(|s| s.text_color(self.theme.text))
                            .text_color(if pane.sort_column == SortColumn::Extension { self.theme.accent } else { self.theme.text_muted })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Extension);
                                this.refresh_pane(pane_id, cx);
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
                            .flex()
                            .justify_end()
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text))
                            .text_color(if pane.sort_column == SortColumn::Size { self.theme.accent } else { self.theme.text_muted })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Size);
                                this.refresh_pane(pane_id, cx);
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
                            .w(px(140.0))
                            .cursor_pointer()
                            .hover(|s| s.text_color(self.theme.text))
                            .text_color(if pane.sort_column == SortColumn::Modified { self.theme.accent } else { self.theme.text_muted })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Modified);
                                this.refresh_pane(pane_id, cx);
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
                            .hover(|s| s.text_color(self.theme.text))
                            .text_color(if pane.sort_column == SortColumn::Permissions { self.theme.accent } else { self.theme.text_muted })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                let p = match pane_id {
                                    PaneId::Left => &mut this.left_pane,
                                    PaneId::Right => &mut this.right_pane,
                                };
                                p.toggle_sort(SortColumn::Permissions);
                                this.refresh_pane(pane_id, cx);
                                cx.notify();
                            }))
                            .child(format!(
                                "PERM{}",
                                if pane.sort_column == SortColumn::Permissions {
                                    if pane.sort_direction == SortDirection::Ascending {
                                        Icons::SORT_ASC
                                    } else {
                                        Icons::SORT_DESC
                                    }
                                } else {
                                    ""
                                }
                            )),
                    ),
            )
            // Scrollable File List Body (virtualized: only the visible
            // rows are built, so huge directories scroll without jank).
            .child(
                div()
                    .id(ElementId::Name(format!("body-scroll-{:?}", pane_id).into()))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        this.open_context_menu(pane_id, None, ev.position);
                        cx.notify();
                    }))
                    // Blank-space click clears the selection; rows stop
                    // propagation so they keep their own behavior.
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                        this.active_pane = pane_id;
                        let p = match pane_id {
                            PaneId::Left => &mut this.left_pane,
                            PaneId::Right => &mut this.right_pane,
                        };
                        p.clear_selection();
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
                                        .font_family(Icons::FAMILY)
                                        .text_color(self.theme.text_muted)
                                        .child(Icons::DISCONNECTED),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_size(self.theme.font_size_modal_title)
                                        .text_color(self.theme.text)
                                        .child("Remote SFTP Disconnected"),
                                )
                                .child(
                                    div()
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.text_muted)
                                        .child("Connect to an SSH/SFTP server to view remote files"),
                                )
                                .child(
                                    div()
                                        .id("btn-connect-in-pane")
                                        .cursor_pointer()
                                        .px_4()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(self.theme.bg_badge_accent)
                                        .border_1()
                                        .border_color(self.theme.accent)
                                        .hover(|s| s.bg(self.theme.accent).text_color(rgb(0xffffff)))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.accent)
                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| {
                                            this.open_connect_modal();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }))
                                        .child("Connect SFTP"),
                                ),
                        )
                    })
                    .when(pane.is_loading, |el| {
                        el.child(crate::ui::motion::pulse(
                            self.animations_enabled,
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
                                        .font_family(Icons::FAMILY)
                                        .child(Icons::REFRESH),
                                )
                                .child(
                                    div()
                                        .text_size(self.theme.font_size_ui)
                                        .text_color(self.theme.text_placeholder)
                                        .child("Loading directory..."),
                                ),
                            ElementId::Name(format!("pane-loading-{:?}", pane_id).into()),
                        ))
                    })
                    .when_some(pane.error_message.as_ref(), |el, err| {
                        el.child(
                            div()
                                .m_2()
                                .p_3()
                                .bg(self.theme.element_background)
                                .border_1()
                                .border_color(self.theme.danger)
                                .text_size(self.theme.font_size_ui)
                                .text_color(self.theme.danger)
                                .child(err.clone()),
                        )
                    })
                    .child({
                        // Snapshot owned row data: the virtual list renderer
                        // only receives `&mut App`, so everything rows need
                        // is cloned here once per outer render.
                        let theme = self.theme;
                        let entity = self.self_entity();
                        let row_active = is_active;
                        let selected = pane.selected_items.clone();
                        let focused = pane.focused_index;
                        let remote = pane.is_remote;
                        let dir = pane.current_path.clone();
                        let entries = Arc::clone(&pane.entries);
                        let handle = if is_left {
                            self.left_scroll_handle.clone()
                        } else {
                            self.right_scroll_handle.clone()
                        };
                        uniform_list(
                            ElementId::Name(format!("rows-{:?}", pane_id).into()),
                            entries.len() + 1,
                            move |range, _window: &mut Window, _cx: &mut App| {
                                range
                                    .map(|i| {
                                        if i == 0 {
                                            parent_row_view(theme, pane_id, entity.clone())
                                        } else if let Some(entry) = entries.get(i - 1) {
                                            file_row_view(
                                                theme,
                                                pane_id,
                                                entry.clone(),
                                                i - 1,
                                                selected.contains(&entry.name),
                                                focused == Some(i - 1),
                                                row_active,
                                                remote,
                                                dir.clone(),
                                                entity.clone(),
                                            )
                                        } else {
                                            div().h(px(22.0)).w_full().into_any_element()
                                        }
                                    })
                                    .collect()
                            },
                        )
                        .flex_1()
                        .track_scroll(handle)
                    })
            )
            // Pane Footer
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(22.0))
                    .px_3()
                    .gap_2()
                    .bg(self.theme.tab_bar_background)
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
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(self.theme.text_muted)
                                    .child(format!("{} files · {} folders", file_count, dir_count)),
                            )
                            .child(div().text_color(self.theme.border_variant).child("|"))
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
                                .rounded_md()
                                .bg(self.theme.bg_badge_accent)
                                .border_1()
                                .border_color(self.theme.accent)
                                .font_weight(FontWeight::NORMAL)
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

/// Parent (`..`) row for the virtualized file list. Actions dispatch
/// through the stored app entity because list renderers only get `&mut App`.
fn parent_row_view(
    theme: Theme,
    pane_id: PaneId,
    entity: Option<WeakEntity<RiftApp>>,
) -> AnyElement {
    let left_entity = entity.clone();
    let right_entity = entity.clone();
    div()
        .id(ElementId::Integer(0))
        .cursor_pointer()
        .flex()
        .items_center()
        .h(px(22.0))
        .w_full()
        .px_3()
        .gap_2()
        .bg(theme.surface_background)
        .hover(|s| s.bg(theme.ghost_element_hover))
        .on_mouse_down(
            MouseButton::Left,
            move |_ev: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                if let Some(view) = left_entity.clone().and_then(|e| e.upgrade()) {
                    view.update(cx, |this, cx| {
                        this.go_parent(pane_id, cx);
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let pos = ev.position;
                if let Some(view) = right_entity.clone().and_then(|e| e.upgrade()) {
                    view.update(cx, |this, cx| {
                        this.open_context_menu(pane_id, None, pos);
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_family(Icons::FAMILY)
                        .text_color(theme.accent)
                        .font_weight(FontWeight::NORMAL)
                        .child(Icons::UP_ARROW),
                )
                .child(
                    div()
                        .font_family(theme.font_ui)
                        .text_size(theme.font_size_body)
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme.text_muted)
                        .child(".."),
                ),
        )
        .child(
            div()
                .w(px(46.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(theme.text_placeholder)
                .child(""),
        )
        .child(
            div()
                .w(px(72.0))
                .flex()
                .justify_end()
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(theme.text_muted)
                .child("<DIR>"),
        )
        .child(
            div()
                .w(px(140.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(theme.text_placeholder)
                .child(""),
        )
        .child(
            div()
                .w(px(64.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_caption)
                .text_color(theme.text_placeholder)
                .child(""),
        )
        .into_any_element()
}

/// One file row for the virtualized file list. See `parent_row_view`
/// for why actions go through the stored entity.
#[allow(clippy::too_many_arguments)]
fn file_row_view(
    theme: Theme,
    pane_id: PaneId,
    entry: FileEntry,
    idx: usize,
    is_selected: bool,
    is_cursor: bool,
    is_active: bool,
    is_remote: bool,
    dir: String,
    entity: Option<WeakEntity<RiftApp>>,
) -> AnyElement {
    let name = entry.name.clone();
    let entry_is_dir = entry.is_dir();
    let target_path = if is_remote {
        join_path(&dir, &entry.name, true)
    } else {
        entry.path.to_string_lossy().to_string()
    };
    let icon = if entry_is_dir {
        Icons::FOLDER
    } else {
        Icons::for_file(&entry.extension)
    };
    let icon_color = if entry_is_dir {
        theme.dir_icon
    } else {
        theme.file_icon
    };
    let row_bg = if is_selected {
        theme.element_selected
    } else if is_cursor && is_active {
        // Cursor glow only in the active pane: an inactive pane's stale
        // cursor must not look selected.
        theme.ghost_element_hover
    } else {
        // Flat rows: alternating backgrounds read as phantom selection,
        // so every row is the surface color.
        theme.surface_background
    };

    let left_name = name.clone();
    let left_path = target_path.clone();
    let left_entity = entity.clone();
    let right_name = name.clone();
    let right_entry = entry.clone();
    let right_entity = entity.clone();

    div()
        .id(ElementId::Integer(idx as u64 + 1))
        .cursor_pointer()
        .flex()
        .items_center()
        .h(px(22.0))
        .w_full()
        .px_3()
        .gap_2()
        .bg(row_bg)
        .hover(|s| s.bg(theme.ghost_element_hover))
        .on_mouse_down(
            MouseButton::Left,
            move |ev: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let shifted = ev.modifiers.shift;
                let ctrled = ev.modifiers.control || ev.modifiers.platform;
                let double = ev.click_count == 2;
                if let Some(view) = left_entity.clone().and_then(|e| e.upgrade()) {
                    let row_name = left_name.clone();
                    let row_path = left_path.clone();
                    view.update(cx, move |this, cx| {
                        this.active_pane = pane_id;
                        let p = match pane_id {
                            PaneId::Left => &mut this.left_pane,
                            PaneId::Right => &mut this.right_pane,
                        };
                        if double {
                            if entry_is_dir {
                                this.navigate_pane(pane_id, row_path, cx);
                            }
                        } else if shifted {
                            p.select_range_to(idx);
                        } else if ctrled {
                            p.toggle_selection(&row_name, idx);
                        } else {
                            p.set_single_selection(&row_name, idx);
                        }
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let pos = ev.position;
                if let Some(view) = right_entity.clone().and_then(|e| e.upgrade()) {
                    let row_name = right_name.clone();
                    let row_entry = right_entry.clone();
                    view.update(cx, move |this, cx| {
                        this.active_pane = pane_id;
                        let p = match pane_id {
                            PaneId::Left => &mut this.left_pane,
                            PaneId::Right => &mut this.right_pane,
                        };
                        if !p.selected_items.contains(&row_name) {
                            p.set_single_selection(&row_name, idx);
                        }
                        this.open_context_menu(pane_id, Some(row_entry), pos);
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .overflow_hidden()
                .child(
                    div()
                        .w(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_family(Icons::FAMILY)
                        .font_weight(FontWeight::NORMAL)
                        .text_color(icon_color)
                        .child(icon)
                )
                .child(
                    div()
                        .font_family(theme.font_ui)
                        .text_size(theme.font_size_body)
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme.text)
                        .whitespace_nowrap()
                        .child(name),
                ),
        )
        .child(
            div()
                .w(px(46.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(theme.text_muted)
                .child(entry.extension.clone()),
        )
        .child(
            div()
                .w(px(72.0))
                .flex()
                .justify_end()
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(if entry_is_dir {
                    theme.dir_icon
                } else {
                    theme.text_muted
                })
                .child(entry.formatted_size()),
        )
        .child(
            div()
                .w(px(140.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_mono)
                .text_color(theme.text_muted)
                .child(entry.formatted_modified()),
        )
        .child(
            div()
                .w(px(64.0))
                .font_family(theme.font_mono)
                .text_size(theme.font_size_caption)
                .text_color(theme.text_muted)
                .child(entry.metadata.permissions_str.clone()),
        )
        .into_any_element()
}
