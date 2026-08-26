//! YAML editor rendering methods for `AppShell`.
//!
//! Follows the `pod_detail_render.rs` pattern — `impl AppShell` methods in a
//! separate module to keep `app_shell.rs` slim. Renders the YAML editor tab
//! for resource detail views with edit/diff modes, toolbar, and status bar.

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::components::editor_view::{EditorMode, EditorRenderSnapshot, EditorViewState};
use crate::layout::app_shell::{AppShell, ResourceDetailKey};
use crate::theme::Color;

/// Approximate width of a monospace character at text_xs size.
const MONO_CHAR_WIDTH: f32 = 7.2;

impl AppShell {
    /// Top-level YAML editor container for a resource detail tab.
    pub(crate) fn render_yaml_editor_content(
        &mut self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Stateful<Div> {
        let border = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();

        // Get focus handle early so we can pass it to child renderers
        let maybe_fh = self.yaml_editor_focus_handles.get(key).cloned();

        let mut container = div().flex().flex_col().flex_1().min_h(px(0.0)).w_full().bg(bg);

        // Toolbar
        container = container.child(self.render_yaml_toolbar(
            cx,
            key,
            text,
            text_secondary,
            surface,
            border,
            accent,
        ));

        // Conflict banner
        if let Some(editor) = self.yaml_editors.get(key) {
            if editor.has_conflict() {
                container = container.child(Self::render_yaml_conflict_banner(
                    cx,
                    key,
                    editor,
                    text_secondary,
                    border,
                ));
            }
        }

        // Main text area or diff view
        if let Some(editor) = self.yaml_editors.get(key) {
            match editor.mode {
                EditorMode::Edit => {
                    let snapshot = editor.render_snapshot();
                    container = container.child(self.render_yaml_text_area_interactive(
                        cx,
                        key,
                        &snapshot,
                        text,
                        text_secondary,
                        bg,
                        &maybe_fh,
                    ));
                }
                EditorMode::Diff => {
                    container = container.child(Self::render_yaml_diff_view(
                        editor,
                        text,
                        text_secondary,
                        bg,
                        border,
                    ));
                }
            }
        }

        // Status bar
        if let Some(editor) = self.yaml_editors.get(key) {
            container = container.child(Self::render_yaml_status_bar(
                editor,
                text_secondary,
                surface,
                border,
                accent,
            ));
        }

        // Keyboard input
        let key_for_handler = key.clone();
        let mut stateful = container
            .id(ElementId::Name(SharedString::from(format!(
                "yaml-editor-{}-{}",
                key.kind, key.name,
            ))))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
                let is_cmd = event.keystroke.modifiers.platform;
                let key_str = event.keystroke.key.as_ref();

                // Cmd+S → apply (check before taking mutable borrow)
                if is_cmd && key_str == "s" {
                    let can_apply = this
                        .yaml_editors
                        .get(&key_for_handler)
                        .map(|e| e.can_apply())
                        .unwrap_or(false);
                    if can_apply {
                        let dk = key_for_handler.clone();
                        this.handle_yaml_apply(cx, dk);
                    }
                    return;
                }

                if let Some(editor) = this.yaml_editors.get_mut(&key_for_handler) {
                    let mods = crate::components::editor_view::KeyModifiers {
                        cmd: event.keystroke.modifiers.platform,
                        shift: event.keystroke.modifiers.shift,
                        ctrl: event.keystroke.modifiers.control,
                        alt: event.keystroke.modifiers.alt,
                    };
                    let handled = editor.handle_key(key_str, mods);
                    if handled {
                        cx.notify();
                    }
                }
            }));

        // Track focus so keyboard events reach this container
        if let Some(ref fh) = maybe_fh {
            stateful = stateful.track_focus(fh);
        }

        // Click anywhere on the editor to grab focus
        if let Some(fh) = maybe_fh {
            let fh_click = fh.clone();
            stateful =
                stateful.on_click(cx.listener(move |_this, _event: &ClickEvent, window, _cx| {
                    window.focus(&fh_click);
                }));
        }

        stateful
    }

    /// Toolbar with Apply / Revert / Edit|Diff toggle buttons.
    #[allow(clippy::too_many_arguments)]
    fn render_yaml_toolbar(
        &self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let can_apply = self.yaml_editors.get(key).map(|e| e.can_apply()).unwrap_or(false);

        let is_dirty = self.yaml_editors.get(key).map(|e| e.is_dirty).unwrap_or(false);

        let is_diff =
            self.yaml_editors.get(key).map(|e| e.mode == EditorMode::Diff).unwrap_or(false);

        let is_applying = self.yaml_editors.get(key).map(|e| e.applying).unwrap_or(false);

        let apply_color = if can_apply { accent } else { text_secondary };
        let apply_label = if is_applying { "Applying..." } else { "Apply" };

        let key_for_apply = key.clone();
        let key_for_revert = key.clone();
        let key_for_toggle = key.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(border)
            .bg(surface)
            // Apply button
            .child(
                div()
                    .id("yaml-apply-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(apply_color)
                    .when(!can_apply, |el| el.opacity(0.5))
                    .child(SharedString::from(apply_label))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let dk = key_for_apply.clone();
                        if this.yaml_editors.get(&dk).map(|e| e.can_apply()).unwrap_or(false) {
                            this.handle_yaml_apply(cx, dk);
                        }
                    })),
            )
            // Revert button
            .child(
                div()
                    .id("yaml-revert-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text_secondary)
                    .when(!is_dirty, |el| el.opacity(0.5))
                    .child("Revert")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key_for_revert) {
                            editor.reset();
                            cx.notify();
                        }
                    })),
            )
            // Spacer
            .child(div().flex_1())
            // Edit/Diff toggle
            .child(
                div()
                    .id("yaml-mode-toggle")
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .when(!is_dirty, |el| el.opacity(0.5))
                    .child(if is_diff { "Edit" } else { "Diff" })
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key_for_toggle) {
                            if editor.mode == EditorMode::Diff {
                                editor.show_edit();
                            } else {
                                editor.show_diff();
                            }
                            cx.notify();
                        }
                    })),
            )
    }

    /// Interactive scrollable YAML text area with cursor and line highlight.
    #[allow(clippy::too_many_arguments)]
    fn render_yaml_text_area_interactive(
        &self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        snapshot: &EditorRenderSnapshot,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
        focus_handle: &Option<FocusHandle>,
    ) -> Stateful<Div> {
        let muted = Rgba { r: text_secondary.r, g: text_secondary.g, b: text_secondary.b, a: 0.5 };
        let error_bg = Color::rgba(239, 68, 68, 30).to_gpui();
        let cursor_line_bg =
            Rgba { r: text_secondary.r, g: text_secondary.g, b: text_secondary.b, a: 0.08 };
        let cursor_color = self.theme.colors.accent.to_gpui();

        let mut body = div()
            .id("yaml-text-area")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .overflow_y_scroll()
            .bg(bg)
            .font_family("monospace")
            .p_1();

        for idx in 0..snapshot.line_count {
            body = body.child(self.render_yaml_line(
                cx,
                key,
                snapshot,
                idx,
                text,
                muted,
                error_bg,
                cursor_line_bg,
                cursor_color,
                focus_handle,
            ));
        }

        body
    }

    /// Render a single line of the YAML text area.
    #[allow(clippy::too_many_arguments)]
    fn render_yaml_line(
        &self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        snapshot: &EditorRenderSnapshot,
        idx: usize,
        text: Rgba,
        muted: Rgba,
        error_bg: Rgba,
        cursor_line_bg: Rgba,
        cursor_color: Rgba,
        focus_handle: &Option<FocusHandle>,
    ) -> Stateful<Div> {
        let content = &snapshot.lines[idx];
        let line_text = content.trim_end_matches('\n');
        let line_num = SharedString::from(format!("{}", idx + 1));

        let has_error = snapshot.error_line.map(|l| l == idx + 1).unwrap_or(false);
        let is_cursor_line = idx == snapshot.cursor_line;

        let mut row = div().flex().flex_row().w_full();

        // Background: error takes precedence, then cursor line highlight
        if has_error {
            row = row.bg(error_bg);
        } else if is_cursor_line {
            row = row.bg(cursor_line_bg);
        }

        // Line number gutter
        row = row.child(
            div()
                .w(px(40.0))
                .flex_shrink_0()
                .text_right()
                .pr_2()
                .text_xs()
                .text_color(muted)
                .child(line_num),
        );

        // Error gutter marker
        let mut gutter = div().w(px(4.0)).flex_shrink_0().mr_1();
        if has_error {
            gutter = gutter.bg(Color::rgba(239, 68, 68, 255).to_gpui());
        }
        row = row.child(gutter);

        // Content with cursor
        let content_child = if is_cursor_line {
            Self::render_line_with_cursor(line_text, snapshot.cursor_col, text, cursor_color)
        } else {
            div()
                .flex_1()
                .text_xs()
                .text_color(text)
                .child(SharedString::from(line_text.to_string()))
        };
        row = row.child(content_child);

        // Click-to-position handler (also focuses the editor)
        let key_for_click = key.clone();
        let line_idx = idx;
        let fh_for_click = focus_handle.clone();
        row.id(ElementId::Name(SharedString::from(format!(
            "yaml-line-{}-{}-{}",
            key.kind, key.name, idx
        ))))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                // Focus the editor so keyboard events work
                if let Some(ref fh) = fh_for_click {
                    window.focus(fh);
                }
                // Approximate column from click x position.
                // Subtract gutter width (40 + 4 + margins ~= 52px)
                let gutter_px = px(52.0);
                let click_x = event.position.x - gutter_px;
                let char_w = px(MONO_CHAR_WIDTH);
                let col: usize = if click_x > px(0.0) {
                    // Pixels / Pixels -> f32
                    let ratio = click_x / char_w;
                    ratio.max(0.0) as usize
                } else {
                    0
                };
                if let Some(editor) = this.yaml_editors.get_mut(&key_for_click) {
                    editor.set_cursor_to_line_col(line_idx, col);
                    cx.notify();
                }
            }),
        )
    }

    /// Render a line of text with a cursor bar at the given column.
    fn render_line_with_cursor(
        line_text: &str,
        cursor_col: usize,
        text_color: Rgba,
        cursor_color: Rgba,
    ) -> Div {
        let col = cursor_col.min(line_text.len());
        let (before, after) = line_text.split_at(col);

        let before_str = if before.is_empty() {
            SharedString::default()
        } else {
            SharedString::from(before.to_string())
        };
        let after_str = if after.is_empty() {
            SharedString::default()
        } else {
            SharedString::from(after.to_string())
        };

        let mut row = div().flex_1().flex().flex_row().text_xs().text_color(text_color);

        // Text before cursor
        if !before.is_empty() {
            row = row.child(div().child(before_str));
        }

        // Cursor bar (2px wide)
        row = row.child(div().w(px(2.0)).h(px(14.0)).flex_shrink_0().bg(cursor_color));

        // Text after cursor
        if !after.is_empty() {
            row = row.child(div().child(after_str));
        }

        row
    }

    /// Diff view with colored +/- lines.
    fn render_yaml_diff_view(
        editor: &EditorViewState,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
        border: Rgba,
    ) -> Stateful<Div> {
        let diff = editor.compute_diff();
        let muted = Rgba { r: text_secondary.r, g: text_secondary.g, b: text_secondary.b, a: 0.5 };
        let added_bg = Color::rgba(34, 197, 94, 30).to_gpui();
        let removed_bg = Color::rgba(239, 68, 68, 30).to_gpui();

        let summary_text =
            SharedString::from(format!("+{} -{}", diff.added_count, diff.removed_count,));

        let mut body = div()
            .id("yaml-diff-view")
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_y_scroll()
            .bg(bg)
            .font_family("monospace");

        // Summary header
        body = body.child(
            div()
                .flex()
                .flex_row()
                .px_3()
                .py_1()
                .border_b_1()
                .border_color(border)
                .child(div().text_xs().text_color(text_secondary).child(summary_text)),
        );

        use baeus_editor::diff::DiffLineKind;

        for dl in &diff.lines {
            let line_bg = match dl.kind {
                DiffLineKind::Added => added_bg,
                DiffLineKind::Removed => removed_bg,
                DiffLineKind::Unchanged => bg,
            };
            let prefix = match dl.kind {
                DiffLineKind::Added => "+",
                DiffLineKind::Removed => "-",
                DiffLineKind::Unchanged => " ",
            };

            let old_n = dl.old_line_number.map(|n| format!("{n}")).unwrap_or_default();
            let new_n = dl.new_line_number.map(|n| format!("{n}")).unwrap_or_default();
            let ct = SharedString::from(dl.content.clone());

            body = body.child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .bg(line_bg)
                    // Old line number
                    .child(
                        div()
                            .w(px(30.0))
                            .flex_shrink_0()
                            .text_right()
                            .pr_1()
                            .text_xs()
                            .text_color(muted)
                            .child(SharedString::from(old_n)),
                    )
                    // New line number
                    .child(
                        div()
                            .w(px(30.0))
                            .flex_shrink_0()
                            .text_right()
                            .pr_1()
                            .text_xs()
                            .text_color(muted)
                            .child(SharedString::from(new_n)),
                    )
                    // Prefix
                    .child(
                        div()
                            .w(px(12.0))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(prefix)),
                    )
                    // Content
                    .child(div().flex_1().text_xs().text_color(text).child(ct)),
            );
        }

        body
    }

    /// Conflict banner with Accept Server / Dismiss buttons.
    fn render_yaml_conflict_banner(
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        editor: &EditorViewState,
        text_secondary: Rgba,
        border: Rgba,
    ) -> Div {
        let conflict = match &editor.conflict {
            Some(cf) => cf,
            None => return div(),
        };

        let msg = SharedString::from(conflict.message.clone());
        let banner_bg = Color::rgba(245, 158, 11, 30).to_gpui();
        let warning = Color::rgba(245, 158, 11, 255).to_gpui();

        let key_for_accept = key.clone();
        let key_for_dismiss = key.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .bg(banner_bg)
            .border_b_1()
            .border_color(warning)
            // Message
            .child(div().flex_1().text_xs().text_color(warning).child(msg))
            // Accept Server button
            .child(
                div()
                    .id("yaml-conflict-accept")
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text_secondary)
                    .child("Accept Server")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let dk = key_for_accept.clone();
                        this.handle_yaml_accept_server(cx, dk);
                    })),
            )
            // Dismiss button
            .child(
                div()
                    .id("yaml-conflict-dismiss")
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text_secondary)
                    .child("Dismiss")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key_for_dismiss) {
                            editor.dismiss_conflict();
                            cx.notify();
                        }
                    })),
            )
    }

    /// Status bar showing validation errors and apply status.
    fn render_yaml_status_bar(
        editor: &EditorViewState,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let error_color = Color::rgba(239, 68, 68, 255).to_gpui();
        let warning_color = Color::rgba(245, 158, 11, 255).to_gpui();
        let success_color = Color::rgba(34, 197, 94, 255).to_gpui();

        let (status_text, status_color) = if let Some(ref err) = editor.apply_error {
            (format!("Error: {err}"), error_color)
        } else if let Some(ref ve) = editor.validation_error {
            let loc = ve.line.map(|l| format!(" (line {l})")).unwrap_or_default();
            (format!("YAML error{loc}: {}", ve.message), warning_color)
        } else if editor.applying {
            ("Applying...".to_string(), accent)
        } else if editor.is_dirty {
            ("Modified".to_string(), text_secondary)
        } else {
            ("Valid YAML".to_string(), success_color)
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_1()
            .border_t_1()
            .border_color(border)
            .bg(surface)
            .child(div().text_xs().text_color(status_color).child(SharedString::from(status_text)))
    }
}
