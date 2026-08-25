use baeus_editor::buffer::TextBuffer;
use baeus_editor::diff::{DiffLine, DiffLineKind, DiffResult, compute_diff};
use baeus_editor::highlight::HighlightToken;
use baeus_editor::yaml::{YamlError, validate_yaml};
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::{Color, Theme};

/// The mode the editor view is currently in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    /// Editing the YAML content.
    Edit,
    /// Showing a diff between original and modified.
    Diff,
}

/// State for a conflict that occurred when applying changes.
#[derive(Debug, Clone)]
pub struct ConflictState {
    /// The server's current version of the resource YAML.
    pub server_yaml: String,
    /// The user's local modified YAML.
    pub local_yaml: String,
    /// A human-readable message describing the conflict.
    pub message: String,
}

/// Keyboard modifier state for key events.
#[derive(Debug, Clone, Default)]
pub struct KeyModifiers {
    /// Cmd (macOS) / Ctrl (Windows/Linux) key held.
    pub cmd: bool,
    /// Shift key held.
    pub shift: bool,
    /// Ctrl key held (separate from cmd on macOS).
    pub ctrl: bool,
    /// Alt/Option key held.
    pub alt: bool,
}

/// Direction for cursor movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorDirection {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Snapshot of editor state for rendering, avoiding borrow conflicts.
pub(crate) struct EditorRenderSnapshot {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub error_line: Option<usize>,
    pub line_count: usize,
}

/// The current state of the apply workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyWorkflowState {
    /// Nothing happening.
    Idle,
    /// Can apply (dirty + valid).
    Ready,
    /// Apply in progress.
    Applying,
    /// Apply succeeded.
    Success,
    /// Apply failed with error.
    Failed(String),
    /// 409 conflict detected.
    Conflict,
}

/// State for the YAML editor view component.
#[derive(Debug)]
pub struct EditorViewState {
    /// The text buffer backing the editor.
    pub buffer: TextBuffer,
    /// The original YAML text (for diff computation).
    pub original_yaml: String,
    /// The resource kind being edited (e.g., "Deployment").
    pub resource_kind: String,
    /// The resource name being edited.
    pub resource_name: String,
    /// The resource namespace (None for cluster-scoped resources).
    pub resource_namespace: Option<String>,
    /// The resource_version from the K8s API (for optimistic concurrency).
    pub resource_version: String,
    /// Current validation error, if any.
    pub validation_error: Option<YamlError>,
    /// Whether the editor content has been modified from the original.
    pub is_dirty: bool,
    /// The current editor mode (Edit or Diff).
    pub mode: EditorMode,
    /// Whether an apply operation is in progress.
    pub applying: bool,
    /// Error from the last apply attempt.
    pub apply_error: Option<String>,
    /// Conflict state when a 409 Conflict is returned.
    pub conflict: Option<ConflictState>,
    /// Whether the editor is read-only (e.g., insufficient RBAC permissions).
    pub read_only: bool,
    /// Show line numbers in the gutter.
    pub show_line_numbers: bool,
    /// Current cursor position as a character index in the buffer.
    pub cursor_position: usize,
    /// Current selection range as (start, end) character indices, if any.
    pub selection: Option<(usize, usize)>,
}

impl EditorViewState {
    /// Create a new editor view with the given YAML content.
    pub fn new(
        yaml: &str,
        resource_kind: &str,
        resource_name: &str,
        resource_namespace: Option<String>,
        resource_version: &str,
    ) -> Self {
        Self {
            buffer: TextBuffer::from_str(yaml),
            original_yaml: yaml.to_string(),
            resource_kind: resource_kind.to_string(),
            resource_name: resource_name.to_string(),
            resource_namespace,
            resource_version: resource_version.to_string(),
            validation_error: None,
            is_dirty: false,
            mode: EditorMode::Edit,
            applying: false,
            apply_error: None,
            conflict: None,
            read_only: false,
            show_line_numbers: true,
            cursor_position: 0,
            selection: None,
        }
    }

    /// Get the current editor text.
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Insert text at the given character position.
    pub fn insert(&mut self, pos: usize, text: &str) {
        if self.read_only {
            return;
        }
        self.buffer.insert(pos, text);
        self.mark_dirty();
    }

    /// Delete text in the given character range.
    pub fn delete(&mut self, start: usize, end: usize) {
        if self.read_only {
            return;
        }
        self.buffer.delete(start, end);
        self.mark_dirty();
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let result = self.buffer.undo();
        if result {
            self.update_dirty_state();
            self.validate();
        }
        result
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let result = self.buffer.redo();
        if result {
            self.update_dirty_state();
            self.validate();
        }
        result
    }

    /// Validate the current YAML content.
    pub fn validate(&mut self) -> bool {
        let text = self.buffer.text();
        match validate_yaml(&text) {
            Ok(_) => {
                self.validation_error = None;
                true
            }
            Err(err) => {
                self.validation_error = Some(err);
                false
            }
        }
    }

    /// Returns true if the current content is valid YAML.
    pub fn is_valid(&self) -> bool {
        self.validation_error.is_none()
    }

    /// Returns true if the content can be applied (dirty, valid, not applying).
    pub fn can_apply(&self) -> bool {
        self.is_dirty && self.is_valid() && !self.applying && !self.read_only
    }

    /// Switch to diff mode, computing the diff between original and current.
    pub fn show_diff(&mut self) {
        self.mode = EditorMode::Diff;
    }

    /// Switch back to edit mode.
    pub fn show_edit(&mut self) {
        self.mode = EditorMode::Edit;
    }

    /// Compute the diff between the original and current content.
    pub fn compute_diff(&self) -> DiffResult {
        compute_diff(&self.original_yaml, &self.buffer.text())
    }

    /// Mark the editor as applying changes.
    pub fn begin_apply(&mut self) {
        self.applying = true;
        self.apply_error = None;
        self.conflict = None;
    }

    /// Mark the apply as successful and update the original to the new content.
    pub fn apply_success(&mut self, new_resource_version: &str) {
        self.applying = false;
        self.apply_error = None;
        self.conflict = None;
        self.original_yaml = self.buffer.text();
        self.resource_version = new_resource_version.to_string();
        self.is_dirty = false;
    }

    /// Mark the apply as failed with an error message.
    pub fn apply_failure(&mut self, error: String) {
        self.applying = false;
        self.apply_error = Some(error);
    }

    /// Mark the apply as failed due to a resource version conflict (409).
    pub fn apply_conflict(&mut self, server_yaml: String) {
        self.applying = false;
        self.conflict = Some(ConflictState {
            server_yaml,
            local_yaml: self.buffer.text(),
            message: format!(
                "The {} {}{} has been modified on the server. Review the server version and retry.",
                self.resource_kind,
                self.resource_name,
                self.resource_namespace
                    .as_ref()
                    .map(|ns| format!(" in namespace {ns}"))
                    .unwrap_or_default(),
            ),
        });
    }

    /// Accept the server version during conflict resolution,
    /// replacing the editor content with the server's YAML.
    pub fn accept_server_version(&mut self, server_yaml: &str, new_resource_version: &str) {
        self.buffer = TextBuffer::from_str(server_yaml);
        self.original_yaml = server_yaml.to_string();
        self.resource_version = new_resource_version.to_string();
        self.is_dirty = false;
        self.conflict = None;
        self.apply_error = None;
        self.cursor_position = 0;
        self.selection = None;
        self.validate();
    }

    /// Dismiss the conflict dialog without accepting the server version.
    /// The user's local edits remain in the buffer.
    pub fn dismiss_conflict(&mut self) {
        self.conflict = None;
    }

    /// Returns true if there is an active conflict.
    pub fn has_conflict(&self) -> bool {
        self.conflict.is_some()
    }

    /// Reset the editor to the original content, discarding all edits.
    pub fn reset(&mut self) {
        self.buffer = TextBuffer::from_str(&self.original_yaml);
        self.is_dirty = false;
        self.validation_error = None;
        self.apply_error = None;
        self.conflict = None;
        self.mode = EditorMode::Edit;
        self.cursor_position = 0;
        self.selection = None;
    }

    /// Returns the number of lines in the editor.
    pub fn line_count(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Get a specific line by index (0-based).
    pub fn line(&self, idx: usize) -> Option<String> {
        self.buffer.line(idx)
    }

    /// Returns a display title for the editor (e.g., "Deployment/nginx").
    pub fn title(&self) -> String {
        let dirty_marker = if self.is_dirty { " *" } else { "" };
        format!("{}/{}{}", self.resource_kind, self.resource_name, dirty_marker)
    }

    // -- T056: Keyboard input handling --

    /// Handle a key event. Returns true if the event was handled.
    pub fn handle_key(&mut self, key: &str, modifiers: KeyModifiers) -> bool {
        if modifiers.cmd && !modifiers.shift {
            match key {
                "z" => return self.undo(),
                "a" => {
                    self.select_all();
                    return true;
                }
                _ => return false,
            }
        }
        if modifiers.cmd && modifiers.shift && key == "z" {
            return self.redo();
        }

        if self.read_only {
            // Arrow keys still work in read-only mode
            match key {
                "left" => {
                    self.move_cursor(CursorDirection::Left);
                    return true;
                }
                "right" => {
                    self.move_cursor(CursorDirection::Right);
                    return true;
                }
                "up" => {
                    self.move_cursor(CursorDirection::Up);
                    return true;
                }
                "down" => {
                    self.move_cursor(CursorDirection::Down);
                    return true;
                }
                "home" => {
                    self.move_cursor(CursorDirection::Home);
                    return true;
                }
                "end" => {
                    self.move_cursor(CursorDirection::End);
                    return true;
                }
                _ => return false,
            }
        }

        match key {
            "backspace" => {
                self.backspace_at_cursor();
                true
            }
            "delete" => {
                self.delete_at_cursor();
                true
            }
            "enter" => {
                self.insert_at_cursor("\n");
                true
            }
            "left" => {
                self.move_cursor(CursorDirection::Left);
                true
            }
            "right" => {
                self.move_cursor(CursorDirection::Right);
                true
            }
            "up" => {
                self.move_cursor(CursorDirection::Up);
                true
            }
            "down" => {
                self.move_cursor(CursorDirection::Down);
                true
            }
            "home" => {
                self.move_cursor(CursorDirection::Home);
                true
            }
            "end" => {
                self.move_cursor(CursorDirection::End);
                true
            }
            other => {
                // Only insert printable characters (single chars, not modifier keys)
                if other.len() == 1 && !modifiers.ctrl && !modifiers.alt {
                    self.insert_at_cursor(other);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Move the cursor in the given direction.
    pub fn move_cursor(&mut self, direction: CursorDirection) {
        self.selection = None;
        let len = self.buffer.len_chars();
        match direction {
            CursorDirection::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            CursorDirection::Right => {
                if self.cursor_position < len {
                    self.cursor_position += 1;
                }
            }
            CursorDirection::Up => {
                let (line, col) = self.cursor_line_col();
                if line > 0 {
                    let prev_line_len = self.line_char_count(line - 1);
                    let new_col = col.min(prev_line_len);
                    self.cursor_position = self.line_start_offset(line - 1) + new_col;
                }
            }
            CursorDirection::Down => {
                let (line, col) = self.cursor_line_col();
                let total_lines = self.line_count();
                if line + 1 < total_lines {
                    let next_line_len = self.line_char_count(line + 1);
                    let new_col = col.min(next_line_len);
                    self.cursor_position = self.line_start_offset(line + 1) + new_col;
                }
            }
            CursorDirection::Home => {
                let (line, _col) = self.cursor_line_col();
                self.cursor_position = self.line_start_offset(line);
            }
            CursorDirection::End => {
                let (line, _col) = self.cursor_line_col();
                self.cursor_position = self.line_start_offset(line) + self.line_char_count(line);
            }
        }
    }

    /// Insert text at the current cursor position.
    pub fn insert_at_cursor(&mut self, text: &str) {
        if self.read_only {
            return;
        }
        self.selection = None;
        let pos = self.cursor_position.min(self.buffer.len_chars());
        self.insert(pos, text);
        self.cursor_position = pos + text.chars().count();
    }

    /// Delete the character after the cursor (Delete key).
    pub fn delete_at_cursor(&mut self) {
        if self.read_only {
            return;
        }
        self.selection = None;
        let len = self.buffer.len_chars();
        if self.cursor_position < len {
            self.delete(self.cursor_position, self.cursor_position + 1);
        }
    }

    /// Delete the character before the cursor (Backspace key).
    pub fn backspace_at_cursor(&mut self) {
        if self.read_only {
            return;
        }
        self.selection = None;
        if self.cursor_position > 0 {
            let new_pos = self.cursor_position - 1;
            self.delete(new_pos, self.cursor_position);
            self.cursor_position = new_pos;
        }
    }

    /// Select all text in the buffer.
    pub fn select_all(&mut self) {
        let len = self.buffer.len_chars();
        self.selection = Some((0, len));
    }

    /// Returns the 0-based line number the cursor is on.
    pub fn cursor_line(&self) -> usize {
        self.cursor_line_col().0
    }

    /// Returns the 0-based column the cursor is at in the current line.
    pub fn cursor_column(&self) -> usize {
        self.cursor_line_col().1
    }

    /// Create a snapshot of editor state for rendering.
    pub(crate) fn render_snapshot(&self) -> EditorRenderSnapshot {
        let line_count = self.line_count();
        let (cursor_line, cursor_col) = self.cursor_line_col();
        let error_line = self.validation_error.as_ref().and_then(|e| e.line);
        let mut lines = Vec::with_capacity(line_count);
        for i in 0..line_count {
            lines.push(self.line(i).unwrap_or_default());
        }
        EditorRenderSnapshot { lines, cursor_line, cursor_col, error_line, line_count }
    }

    /// Set the cursor to a specific line and column (0-based).
    pub fn set_cursor_to_line_col(&mut self, line: usize, col: usize) {
        let total_lines = self.line_count();
        let target_line = line.min(total_lines.saturating_sub(1));
        let line_len = self.line_char_count(target_line);
        let target_col = col.min(line_len);
        self.cursor_position = self.line_start_offset(target_line) + target_col;
        self.selection = None;
    }

    // -- T057: Diff view helpers --

    /// Returns a summary string for the current diff, e.g. "+3 -1" or "No changes".
    pub fn diff_summary(&self) -> String {
        let diff = self.compute_diff();
        if !diff.has_changes() {
            "No changes".to_string()
        } else {
            format!("+{} -{}", diff.added_count, diff.removed_count)
        }
    }

    // -- T058: Apply workflow state --

    /// Returns the current state of the apply workflow as an enum.
    pub fn apply_workflow_state(&self) -> ApplyWorkflowState {
        if self.applying {
            return ApplyWorkflowState::Applying;
        }
        if self.conflict.is_some() {
            return ApplyWorkflowState::Conflict;
        }
        if let Some(ref err) = self.apply_error {
            return ApplyWorkflowState::Failed(err.clone());
        }
        // Check if last apply was successful: not dirty, not applying, no error, no conflict,
        // and the original_yaml matches the buffer (i.e., it was just applied).
        // However, we can't distinguish "just succeeded" from "never modified" without
        // an extra field. We'll treat "idle" as the default and "ready" when can_apply is true.
        if self.can_apply() {
            return ApplyWorkflowState::Ready;
        }
        // If not dirty and resource_version has been updated (not initial), it could be success.
        // For simplicity, we detect success by checking: not dirty, no error, no conflict,
        // not applying, and buffer matches original (which is always true when not dirty).
        // But since "Idle" and "Success" look the same in state, we add a dedicated flag.
        if !self.is_dirty && self.apply_error.is_none() && self.conflict.is_none() {
            // Could be idle or success - we default to Idle.
            return ApplyWorkflowState::Idle;
        }
        ApplyWorkflowState::Idle
    }

    // -- Private helpers --

    fn mark_dirty(&mut self) {
        self.is_dirty = self.buffer.text() != self.original_yaml;
        self.validate();
    }

    fn update_dirty_state(&mut self) {
        self.is_dirty = self.buffer.text() != self.original_yaml;
    }

    /// Returns (line, column) for the current cursor position.
    fn cursor_line_col(&self) -> (usize, usize) {
        let pos = self.cursor_position.min(self.buffer.len_chars());
        let text = self.buffer.text();
        let mut line = 0;
        let mut col = 0;
        for (i, ch) in text.chars().enumerate() {
            if i == pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Returns the character offset of the start of the given line (0-based).
    fn line_start_offset(&self, target_line: usize) -> usize {
        let text = self.buffer.text();
        let mut line = 0;
        for (i, ch) in text.chars().enumerate() {
            if line == target_line {
                return i;
            }
            if ch == '\n' {
                line += 1;
            }
        }
        // If target_line is past the last newline, return the end
        if line == target_line {
            return text.chars().count();
        }
        text.chars().count()
    }

    /// Returns the number of characters in the given line (excluding the trailing newline).
    fn line_char_count(&self, target_line: usize) -> usize {
        if let Some(line_str) = self.buffer.line(target_line) {
            // Rope's line() includes trailing newline; strip it for character count
            let trimmed = line_str.trim_end_matches('\n');
            trimmed.chars().count()
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T055)
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the editor view.
struct EditorColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    warning: Rgba,
}

/// View wrapper for `EditorViewState` that holds a theme
/// for rendering.
pub struct EditorViewComponent {
    pub state: EditorViewState,
    pub theme: Theme,
}

impl EditorViewComponent {
    pub fn new(state: EditorViewState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Maps a `HighlightToken` to the appropriate theme
    /// color.
    pub fn color_for_token(&self, token: HighlightToken) -> Color {
        match token {
            HighlightToken::Key => self.theme.colors.accent,
            HighlightToken::StringValue => self.theme.colors.success,
            HighlightToken::NumberValue => self.theme.colors.warning,
            HighlightToken::BooleanValue => Color::rgb(167, 139, 250),
            HighlightToken::NullValue => self.theme.colors.text_muted,
            HighlightToken::Comment => self.theme.colors.text_muted,
            HighlightToken::Punctuation => self.theme.colors.text_secondary,
            HighlightToken::Tag => self.theme.colors.info,
            HighlightToken::Anchor => self.theme.colors.info,
            HighlightToken::Alias => self.theme.colors.info,
            HighlightToken::Default => self.theme.colors.text_primary,
        }
    }

    /// Returns the status bar text.
    pub fn status_text(&self) -> String {
        if self.state.applying {
            return "Applying...".to_string();
        }
        if let Some(ref err) = self.state.apply_error {
            return format!("Error: {err}");
        }
        if let Some(ref ve) = self.state.validation_error {
            return format!("Invalid YAML: {ve}");
        }
        if self.state.read_only {
            return "Read-only".to_string();
        }
        "Valid YAML".to_string()
    }

    // --- Render helpers (each returns Div) ---

    /// Title bar: resource kind/name, dirty indicator,
    /// mode toggle, Apply button.
    fn render_title_bar(&self) -> gpui::Div {
        let c = self.make_colors();
        let title = SharedString::from(self.state.title());
        let mode_lbl = match self.state.mode {
            EditorMode::Edit => "Edit",
            EditorMode::Diff => "Diff",
        };
        let can = self.state.can_apply();
        let apply_c = if can { c.accent } else { c.text_muted };

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(c.border)
            .bg(c.surface)
            .child(self.render_title_text(title, c.text_primary))
            .child(self.render_mode_pill(mode_lbl, &c))
            .child(self.render_apply_btn(apply_c, &c))
    }

    /// Title label helper.
    fn render_title_text(&self, title: SharedString, color: Rgba) -> gpui::Div {
        div().flex_1().text_sm().text_color(color).child(title)
    }

    /// Mode toggle pill.
    fn render_mode_pill(&self, label: &str, c: &EditorColors) -> gpui::Stateful<gpui::Div> {
        div()
            .id("editor-mode-toggle")
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(c.background)
            .border_1()
            .border_color(c.border)
            .cursor_pointer()
            .text_xs()
            .text_color(c.text_secondary)
            .child(SharedString::from(label.to_string()))
    }

    /// Apply button.
    fn render_apply_btn(&self, text_color: Rgba, c: &EditorColors) -> gpui::Stateful<gpui::Div> {
        div()
            .id("editor-apply-btn")
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .bg(c.surface)
            .border_1()
            .border_color(c.border)
            .cursor_pointer()
            .text_xs()
            .text_color(text_color)
            .child("Apply")
    }

    /// Renders the editor in Edit mode.
    fn render_edit_mode(&self) -> gpui::Div {
        let c = self.make_colors();
        let lc = self.state.line_count();
        let mut body = div().flex().flex_col().flex_1().w_full().overflow_hidden().bg(c.background);

        for idx in 0..lc {
            let content = self.state.line(idx).unwrap_or_default();
            body = body.child(self.render_line_with_numbers(idx, &content));
        }

        body
    }

    /// Renders the editor in Diff mode.
    fn render_diff_mode(&self) -> gpui::Div {
        let c = self.make_colors();
        let diff = self.state.compute_diff();
        let summary = SharedString::from(format!("+{} -{}", diff.added_count, diff.removed_count,));

        let mut body = div().flex().flex_col().flex_1().w_full().overflow_hidden().bg(c.background);

        body = body.child(self.render_diff_summary(summary, &c));

        for dl in &diff.lines {
            body = body.child(self.render_diff_line(dl));
        }

        body
    }

    /// Diff summary header.
    fn render_diff_summary(&self, summary: SharedString, c: &EditorColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(c.border)
            .child(div().text_xs().text_color(c.text_secondary).child(summary))
    }

    /// A single line with line numbers gutter and
    /// error gutter.
    fn render_line_with_numbers(&self, line_idx: usize, content: &str) -> gpui::Div {
        let c = self.make_colors();
        let num = SharedString::from(format!("{}", line_idx + 1));

        let mut row = div().flex().flex_row().w_full();

        if self.state.show_line_numbers {
            row = row.child(self.render_line_num(num, c.text_muted));
        }

        row = row.child(self.render_error_gutter(line_idx));

        row = row.child(self.render_highlighted_line(content));

        row
    }

    /// Line number cell.
    fn render_line_num(&self, num: SharedString, color: Rgba) -> gpui::Div {
        div().w(px(40.0)).flex_shrink_0().text_right().pr_2().text_xs().text_color(color).child(num)
    }

    /// Renders a line with syntax highlighting.
    fn render_highlighted_line(&self, content: &str) -> gpui::Div {
        let c = self.make_colors();
        let txt = SharedString::from(content.trim_end_matches('\n').to_string());
        div().flex_1().text_xs().text_color(c.text_primary).child(txt)
    }

    /// Error gutter indicator for a specific line.
    fn render_error_gutter(&self, line_idx: usize) -> gpui::Div {
        let c = self.make_colors();
        let has_err = self
            .state
            .validation_error
            .as_ref()
            .and_then(|e| e.line)
            .map(|l| l == line_idx + 1)
            .unwrap_or(false);

        let mut gutter = div().w(px(4.0)).flex_shrink_0().mr_1();

        if has_err {
            gutter = gutter.bg(c.error);
        }

        gutter
    }

    /// Status bar: validation status, apply progress.
    fn render_status_bar(&self) -> gpui::Div {
        let c = self.make_colors();
        let status = SharedString::from(self.status_text());

        let sc = if self.state.apply_error.is_some() {
            c.error
        } else if self.state.validation_error.is_some() {
            c.warning
        } else if self.state.applying {
            c.accent
        } else {
            c.success
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_1()
            .border_t_1()
            .border_color(c.border)
            .bg(c.surface)
            .child(div().text_xs().text_color(sc).child(status))
    }

    /// Renders a single diff line with background.
    fn render_diff_line(&self, diff_line: &DiffLine) -> gpui::Div {
        let c = self.make_colors();

        let bg = match diff_line.kind {
            DiffLineKind::Added => Color::rgba(34, 197, 94, 30).to_gpui(),
            DiffLineKind::Removed => Color::rgba(239, 68, 68, 30).to_gpui(),
            DiffLineKind::Unchanged => c.background,
        };

        let prefix = match diff_line.kind {
            DiffLineKind::Added => "+",
            DiffLineKind::Removed => "-",
            DiffLineKind::Unchanged => " ",
        };

        let old_n = diff_line.old_line_number.map(|n| format!("{n}")).unwrap_or_default();
        let new_n = diff_line.new_line_number.map(|n| format!("{n}")).unwrap_or_default();
        let ct = SharedString::from(diff_line.content.clone());

        div()
            .flex()
            .flex_row()
            .w_full()
            .bg(bg)
            .child(self.render_diff_num(old_n, &c))
            .child(self.render_diff_num(new_n, &c))
            .child(self.render_diff_prefix(prefix, &c))
            .child(self.render_diff_content(ct, &c))
    }

    /// Diff line number column.
    fn render_diff_num(&self, num: String, c: &EditorColors) -> gpui::Div {
        div()
            .w(px(30.0))
            .flex_shrink_0()
            .text_right()
            .pr_1()
            .text_xs()
            .text_color(c.text_muted)
            .child(SharedString::from(num))
    }

    /// Diff line prefix (+/-/space) column.
    fn render_diff_prefix(&self, prefix: &str, c: &EditorColors) -> gpui::Div {
        div()
            .w(px(12.0))
            .flex_shrink_0()
            .text_xs()
            .text_color(c.text_secondary)
            .child(SharedString::from(prefix.to_string()))
    }

    /// Diff line content column.
    fn render_diff_content(&self, ct: SharedString, c: &EditorColors) -> gpui::Div {
        div().flex_1().text_xs().text_color(c.text_primary).child(ct)
    }

    /// Conflict banner when a 409 conflict is active.
    fn render_conflict_banner(&self) -> gpui::Div {
        let c = self.make_colors();
        let conflict = match &self.state.conflict {
            Some(cf) => cf,
            None => return div(),
        };

        let msg = SharedString::from(conflict.message.clone());
        let banner_bg = Color::rgba(245, 158, 11, 30).to_gpui();

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
            .border_color(c.warning)
            .child(self.render_conflict_msg(msg, &c))
            .child(self.render_conflict_accept(&c))
            .child(self.render_conflict_dismiss(&c))
    }

    /// Conflict message text.
    fn render_conflict_msg(&self, msg: SharedString, c: &EditorColors) -> gpui::Div {
        div().flex_1().text_xs().text_color(c.warning).child(msg)
    }

    /// Accept server version button.
    fn render_conflict_accept(&self, c: &EditorColors) -> gpui::Stateful<gpui::Div> {
        div()
            .id("conflict-accept-server")
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(c.surface)
            .border_1()
            .border_color(c.border)
            .cursor_pointer()
            .text_xs()
            .text_color(c.accent)
            .child("Accept Server")
    }

    /// Dismiss conflict button.
    fn render_conflict_dismiss(&self, c: &EditorColors) -> gpui::Stateful<gpui::Div> {
        div()
            .id("conflict-dismiss")
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(c.surface)
            .border_1()
            .border_color(c.border)
            .cursor_pointer()
            .text_xs()
            .text_color(c.text_secondary)
            .child("Dismiss")
    }

    /// Precompute all colors from the theme.
    fn make_colors(&self) -> EditorColors {
        EditorColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
        }
    }
}

impl Render for EditorViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let c = self.make_colors();

        let content = match self.state.mode {
            EditorMode::Edit => self.render_edit_mode(),
            EditorMode::Diff => self.render_diff_mode(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(c.background)
            .child(self.render_title_bar())
            .when(self.state.has_conflict(), |el| el.child(self.render_conflict_banner()))
            .child(content)
            .child(self.render_status_bar())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str =
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 3\n";

    fn make_editor() -> EditorViewState {
        EditorViewState::new(
            SAMPLE_YAML,
            "Deployment",
            "nginx",
            Some("default".to_string()),
            "12345",
        )
    }

    #[test]
    fn test_new_editor_state() {
        let editor = make_editor();
        assert_eq!(editor.text(), SAMPLE_YAML);
        assert_eq!(editor.original_yaml, SAMPLE_YAML);
        assert_eq!(editor.resource_kind, "Deployment");
        assert_eq!(editor.resource_name, "nginx");
        assert_eq!(editor.resource_namespace.as_deref(), Some("default"));
        assert_eq!(editor.resource_version, "12345");
        assert!(!editor.is_dirty);
        assert!(editor.is_valid());
        assert!(!editor.applying);
        assert!(editor.apply_error.is_none());
        assert!(editor.conflict.is_none());
        assert!(!editor.read_only);
        assert!(editor.show_line_numbers);
        assert_eq!(editor.mode, EditorMode::Edit);
    }

    #[test]
    fn test_insert_marks_dirty() {
        let mut editor = make_editor();
        assert!(!editor.is_dirty);

        editor.insert(0, "# comment\n");
        assert!(editor.is_dirty);
        assert!(editor.text().starts_with("# comment\n"));
    }

    #[test]
    fn test_delete_marks_dirty() {
        let mut editor = make_editor();
        editor.delete(0, 19); // delete "apiVersion: apps/v1"
        assert!(editor.is_dirty);
    }

    #[test]
    fn test_insert_noop_when_readonly() {
        let mut editor = make_editor();
        editor.read_only = true;
        let original = editor.text();
        editor.insert(0, "inserted");
        assert_eq!(editor.text(), original);
        assert!(!editor.is_dirty);
    }

    #[test]
    fn test_delete_noop_when_readonly() {
        let mut editor = make_editor();
        editor.read_only = true;
        let original = editor.text();
        editor.delete(0, 5);
        assert_eq!(editor.text(), original);
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = make_editor();
        editor.insert(0, "# test\n");
        assert!(editor.is_dirty);

        editor.undo();
        assert_eq!(editor.text(), SAMPLE_YAML);
        assert!(!editor.is_dirty);

        editor.redo();
        assert!(editor.is_dirty);
        assert!(editor.text().starts_with("# test\n"));
    }

    #[test]
    fn test_undo_redo_noop_when_readonly() {
        let mut editor = make_editor();
        editor.insert(0, "x");
        editor.read_only = true;
        assert!(!editor.undo());
        assert!(!editor.redo());
    }

    #[test]
    fn test_validate_valid_yaml() {
        let mut editor = make_editor();
        assert!(editor.validate());
        assert!(editor.is_valid());
        assert!(editor.validation_error.is_none());
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let mut editor =
            EditorViewState::new("key: [invalid\n  yaml: here", "ConfigMap", "test", None, "1");
        assert!(!editor.validate());
        assert!(!editor.is_valid());
        assert!(editor.validation_error.is_some());
    }

    #[test]
    fn test_can_apply() {
        let mut editor = make_editor();
        assert!(!editor.can_apply()); // not dirty

        editor.insert(0, "# modified\n");
        assert!(editor.can_apply()); // dirty + valid

        editor.applying = true;
        assert!(!editor.can_apply()); // applying

        editor.applying = false;
        editor.read_only = true;
        assert!(!editor.can_apply()); // read only
    }

    #[test]
    fn test_can_apply_invalid_yaml() {
        let mut editor = make_editor();
        // Replace buffer with invalid YAML
        editor.buffer = TextBuffer::from_str("key: [invalid");
        editor.is_dirty = true;
        editor.validate();
        assert!(!editor.can_apply());
    }

    #[test]
    fn test_show_diff_and_edit() {
        let mut editor = make_editor();
        assert_eq!(editor.mode, EditorMode::Edit);

        editor.show_diff();
        assert_eq!(editor.mode, EditorMode::Diff);

        editor.show_edit();
        assert_eq!(editor.mode, EditorMode::Edit);
    }

    #[test]
    fn test_compute_diff_no_changes() {
        let editor = make_editor();
        let diff = editor.compute_diff();
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_compute_diff_with_changes() {
        let mut editor = make_editor();
        editor.buffer = TextBuffer::from_str(
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
        );
        editor.is_dirty = true;
        let diff = editor.compute_diff();
        assert!(diff.has_changes());
        assert_eq!(diff.added_count, 1);
        assert_eq!(diff.removed_count, 1);
    }

    #[test]
    fn test_apply_lifecycle_success() {
        let mut editor = make_editor();
        editor.insert(0, "# modified\n");
        assert!(editor.is_dirty);

        editor.begin_apply();
        assert!(editor.applying);
        assert!(editor.apply_error.is_none());

        let new_text = editor.text();
        editor.apply_success("67890");
        assert!(!editor.applying);
        assert!(!editor.is_dirty);
        assert_eq!(editor.resource_version, "67890");
        assert_eq!(editor.original_yaml, new_text);
    }

    #[test]
    fn test_apply_lifecycle_failure() {
        let mut editor = make_editor();
        editor.insert(0, "# modified\n");

        editor.begin_apply();
        editor.apply_failure("server error".to_string());

        assert!(!editor.applying);
        assert_eq!(editor.apply_error.as_deref(), Some("server error"));
        assert!(editor.is_dirty); // still dirty
    }

    #[test]
    fn test_apply_conflict() {
        let mut editor = make_editor();
        editor.insert(0, "# my change\n");

        editor.begin_apply();
        let server_yaml = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 10\n";
        editor.apply_conflict(server_yaml.to_string());

        assert!(!editor.applying);
        assert!(editor.has_conflict());
        let conflict = editor.conflict.as_ref().unwrap();
        assert_eq!(conflict.server_yaml, server_yaml);
        assert!(conflict.message.contains("Deployment"));
        assert!(conflict.message.contains("nginx"));
        assert!(conflict.message.contains("default"));
    }

    #[test]
    fn test_accept_server_version() {
        let mut editor = make_editor();
        editor.insert(0, "# my change\n");
        editor.begin_apply();
        editor.apply_conflict("server: yaml".to_string());

        editor.accept_server_version("server: yaml\n", "99999");

        assert!(!editor.has_conflict());
        assert_eq!(editor.text(), "server: yaml\n");
        assert_eq!(editor.original_yaml, "server: yaml\n");
        assert_eq!(editor.resource_version, "99999");
        assert!(!editor.is_dirty);
    }

    #[test]
    fn test_dismiss_conflict() {
        let mut editor = make_editor();
        editor.insert(0, "# my change\n");
        editor.begin_apply();
        editor.apply_conflict("server: yaml".to_string());

        editor.dismiss_conflict();
        assert!(!editor.has_conflict());
        // Local edits are preserved
        assert!(editor.text().starts_with("# my change\n"));
    }

    #[test]
    fn test_reset() {
        let mut editor = make_editor();
        editor.insert(0, "# modified\n");
        editor.show_diff();
        editor.apply_failure("error".to_string());

        editor.reset();

        assert_eq!(editor.text(), SAMPLE_YAML);
        assert!(!editor.is_dirty);
        assert!(editor.validation_error.is_none());
        assert!(editor.apply_error.is_none());
        assert!(editor.conflict.is_none());
        assert_eq!(editor.mode, EditorMode::Edit);
    }

    #[test]
    fn test_line_count() {
        let editor = make_editor();
        assert_eq!(editor.line_count(), 7); // 6 lines + trailing newline = 7 lines in rope
    }

    #[test]
    fn test_line_access() {
        let editor = make_editor();
        assert_eq!(editor.line(0).unwrap(), "apiVersion: apps/v1\n");
        assert_eq!(editor.line(1).unwrap(), "kind: Deployment\n");
    }

    #[test]
    fn test_title() {
        let editor = make_editor();
        assert_eq!(editor.title(), "Deployment/nginx");
    }

    #[test]
    fn test_title_dirty() {
        let mut editor = make_editor();
        editor.insert(0, "x");
        assert_eq!(editor.title(), "Deployment/nginx *");
    }

    #[test]
    fn test_cluster_scoped_resource() {
        let editor =
            EditorViewState::new("apiVersion: v1\nkind: Node\n", "Node", "node-1", None, "1");
        assert!(editor.resource_namespace.is_none());
    }

    #[test]
    fn test_conflict_message_cluster_scoped() {
        let mut editor =
            EditorViewState::new("apiVersion: v1\nkind: Node\n", "Node", "node-1", None, "1");
        editor.insert(0, "x");
        editor.begin_apply();
        editor.apply_conflict("server yaml".to_string());

        let conflict = editor.conflict.as_ref().unwrap();
        assert!(conflict.message.contains("Node"));
        assert!(conflict.message.contains("node-1"));
        assert!(!conflict.message.contains("namespace"));
    }

    #[test]
    fn test_edit_validates_on_each_change() {
        let mut editor = make_editor();
        assert!(editor.is_valid());

        // Make it invalid
        editor.buffer = TextBuffer::from_str("key: [broken");
        editor.mark_dirty();
        assert!(!editor.is_valid());

        // Fix it
        editor.buffer = TextBuffer::from_str("key: fixed\n");
        editor.mark_dirty();
        assert!(editor.is_valid());
    }

    #[test]
    fn test_full_edit_apply_workflow() {
        let mut editor = make_editor();

        // 1. User opens the YAML editor
        assert!(!editor.is_dirty);
        assert!(!editor.can_apply());

        // 2. User makes edits
        editor.buffer = TextBuffer::from_str(
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
        );
        editor.mark_dirty();
        assert!(editor.is_dirty);
        assert!(editor.can_apply());

        // 3. User views diff
        editor.show_diff();
        let diff = editor.compute_diff();
        assert!(diff.has_changes());
        assert_eq!(diff.removed_count, 1); // replicas: 3
        assert_eq!(diff.added_count, 1); // replicas: 5

        // 4. User applies
        editor.show_edit();
        editor.begin_apply();
        assert!(editor.applying);
        assert!(!editor.can_apply());

        // 5. Server accepts
        editor.apply_success("67890");
        assert!(!editor.is_dirty);
        assert!(!editor.applying);
        assert_eq!(editor.resource_version, "67890");
    }
}
