// T054: Render tests for EditorView (state-level, no GPUI window needed).
//
// Verifies:
// - Text content renders with line numbers
// - Syntax highlight tokens map to correct theme colors
// - Cursor position tracking
// - Error gutter shows for invalid YAML
// - Dirty state tracking
// - Editor mode switching (Edit vs Diff)
// - Diff view: DiffResult lines have correct DiffLineKind
// - Read-only mode disables editing
// - Apply state transitions
// - Conflict state handling
// - Title includes dirty marker

use baeus_editor::buffer::TextBuffer;
use baeus_editor::diff::DiffLineKind;
use baeus_editor::highlight::HighlightToken;
use baeus_ui::components::editor_view::{EditorMode, EditorViewComponent, EditorViewState};
use baeus_ui::theme::{Color, Theme};

const SAMPLE_YAML: &str =
    "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 3\n";

fn make_state() -> EditorViewState {
    EditorViewState::new(SAMPLE_YAML, "Deployment", "nginx", Some("default".to_string()), "12345")
}

fn make_component() -> EditorViewComponent {
    EditorViewComponent::new(make_state(), Theme::dark())
}

// ========================================================================
// Text content renders with line numbers
// ========================================================================

#[test]
fn test_line_count_matches_yaml() {
    let comp = make_component();
    // 6 content lines + trailing empty = 7 rope lines
    assert_eq!(comp.state.line_count(), 7);
}

#[test]
fn test_line_content_at_index_zero() {
    let comp = make_component();
    assert_eq!(comp.state.line(0).unwrap(), "apiVersion: apps/v1\n");
}

#[test]
fn test_line_content_at_index_one() {
    let comp = make_component();
    assert_eq!(comp.state.line(1).unwrap(), "kind: Deployment\n");
}

#[test]
fn test_line_content_last_data_line() {
    let comp = make_component();
    assert_eq!(comp.state.line(5).unwrap(), "  replicas: 3\n");
}

#[test]
fn test_line_out_of_bounds() {
    let comp = make_component();
    assert!(comp.state.line(100).is_none());
}

#[test]
fn test_show_line_numbers_on_by_default() {
    let comp = make_component();
    assert!(comp.state.show_line_numbers);
}

// ========================================================================
// Syntax highlight tokens map to correct theme colors
// ========================================================================

#[test]
fn test_token_key_maps_to_accent() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::Key), Theme::dark().colors.accent,);
}

#[test]
fn test_token_string_maps_to_success() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::StringValue), Theme::dark().colors.success,);
}

#[test]
fn test_token_number_maps_to_warning() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::NumberValue), Theme::dark().colors.warning,);
}

#[test]
fn test_token_boolean_maps_to_purple() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::BooleanValue), Color::rgb(167, 139, 250),);
}

#[test]
fn test_token_null_maps_to_muted() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::NullValue), Theme::dark().colors.text_muted,);
}

#[test]
fn test_token_comment_maps_to_muted() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::Comment), Theme::dark().colors.text_muted,);
}

#[test]
fn test_token_punctuation_maps_to_secondary() {
    let comp = make_component();
    assert_eq!(
        comp.color_for_token(HighlightToken::Punctuation),
        Theme::dark().colors.text_secondary,
    );
}

#[test]
fn test_token_default_maps_to_primary() {
    let comp = make_component();
    assert_eq!(comp.color_for_token(HighlightToken::Default), Theme::dark().colors.text_primary,);
}

#[test]
fn test_token_colors_with_light_theme() {
    let state = make_state();
    let comp = EditorViewComponent::new(state, Theme::light());
    assert_eq!(comp.color_for_token(HighlightToken::Key), Theme::light().colors.accent,);
    assert_eq!(comp.color_for_token(HighlightToken::StringValue), Theme::light().colors.success,);
}

// ========================================================================
// Cursor position tracking
// ========================================================================

#[test]
fn test_cursor_position_initial_zero() {
    let comp = make_component();
    assert_eq!(comp.state.cursor_position, 0);
}

#[test]
fn test_cursor_position_after_insert_at_cursor() {
    let mut comp = make_component();
    comp.state.insert_at_cursor("# test\n");
    assert_eq!(comp.state.cursor_position, "# test\n".len());
}

#[test]
fn test_cursor_line_after_insert() {
    let mut comp = make_component();
    comp.state.insert_at_cursor("# test\n");
    // Cursor is now on line 1 (after the newline)
    assert_eq!(comp.state.cursor_line(), 1);
}

#[test]
fn test_cursor_column_at_start() {
    let comp = make_component();
    assert_eq!(comp.state.cursor_column(), 0);
}

// ========================================================================
// Error gutter shows for invalid YAML
// ========================================================================

#[test]
fn test_validation_error_none_for_valid_yaml() {
    let comp = make_component();
    assert!(comp.state.validation_error.is_none());
}

#[test]
fn test_validation_error_some_for_invalid_yaml() {
    let mut state =
        EditorViewState::new("key: [invalid\n  yaml: here", "ConfigMap", "test", None, "1");
    state.validate();
    assert!(state.validation_error.is_some());
}

#[test]
fn test_validation_error_has_line_number() {
    let mut state =
        EditorViewState::new("key: [invalid\n  yaml: here", "ConfigMap", "test", None, "1");
    state.validate();
    let err = state.validation_error.as_ref().unwrap();
    // serde_yaml_ng reports a line number for syntax errors
    assert!(err.line.is_some() || err.message.contains("line"));
}

#[test]
fn test_validation_clears_on_fix() {
    let mut state =
        EditorViewState::new("key: [invalid\n  yaml: here", "ConfigMap", "test", None, "1");
    state.validate();
    assert!(state.validation_error.is_some());

    // Fix the content
    state.buffer = TextBuffer::from_str("key: valid\n");
    state.validate();
    assert!(state.validation_error.is_none());
}

// ========================================================================
// Dirty state tracking
// ========================================================================

#[test]
fn test_not_dirty_initially() {
    let comp = make_component();
    assert!(!comp.state.is_dirty);
}

#[test]
fn test_dirty_after_insert() {
    let mut comp = make_component();
    comp.state.insert(0, "# modified\n");
    assert!(comp.state.is_dirty);
}

#[test]
fn test_not_dirty_after_undo() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    assert!(comp.state.is_dirty);
    comp.state.undo();
    assert!(!comp.state.is_dirty);
}

#[test]
fn test_dirty_after_redo() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    comp.state.undo();
    comp.state.redo();
    assert!(comp.state.is_dirty);
}

// ========================================================================
// Editor mode switching (Edit vs Diff)
// ========================================================================

#[test]
fn test_default_mode_is_edit() {
    let comp = make_component();
    assert_eq!(comp.state.mode, EditorMode::Edit);
}

#[test]
fn test_switch_to_diff_mode() {
    let mut comp = make_component();
    comp.state.show_diff();
    assert_eq!(comp.state.mode, EditorMode::Diff);
}

#[test]
fn test_switch_back_to_edit_mode() {
    let mut comp = make_component();
    comp.state.show_diff();
    comp.state.show_edit();
    assert_eq!(comp.state.mode, EditorMode::Edit);
}

// ========================================================================
// Diff view: DiffResult lines have correct DiffLineKind
// ========================================================================

#[test]
fn test_diff_no_changes() {
    let comp = make_component();
    let diff = comp.state.compute_diff();
    assert!(!diff.has_changes());
    assert_eq!(diff.added_count, 0);
    assert_eq!(diff.removed_count, 0);
}

#[test]
fn test_diff_with_modification() {
    let mut comp = make_component();
    comp.state.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    comp.state.is_dirty = true;
    let diff = comp.state.compute_diff();
    assert!(diff.has_changes());
    assert_eq!(diff.added_count, 1);
    assert_eq!(diff.removed_count, 1);
}

#[test]
fn test_diff_lines_have_correct_kinds() {
    let mut comp = make_component();
    comp.state.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    let diff = comp.state.compute_diff();

    let unchanged: Vec<_> =
        diff.lines.iter().filter(|l| l.kind == DiffLineKind::Unchanged).collect();
    let added: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Added).collect();
    let removed: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).collect();

    assert_eq!(unchanged.len(), 5);
    assert_eq!(added.len(), 1);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].content, "  replicas: 3");
    assert_eq!(added[0].content, "  replicas: 5");
}

#[test]
fn test_diff_line_numbers_present() {
    let mut comp = make_component();
    comp.state.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    let diff = comp.state.compute_diff();

    let added: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Added).collect();
    assert!(added[0].new_line_number.is_some());
    assert!(added[0].old_line_number.is_none());

    let removed: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).collect();
    assert!(removed[0].old_line_number.is_some());
    assert!(removed[0].new_line_number.is_none());
}

#[test]
fn test_diff_summary_text() {
    let mut comp = make_component();
    comp.state.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    comp.state.is_dirty = true;
    let summary = comp.state.diff_summary();
    assert_eq!(summary, "+1 -1");
}

#[test]
fn test_diff_summary_no_changes() {
    let comp = make_component();
    let summary = comp.state.diff_summary();
    assert_eq!(summary, "No changes");
}

// ========================================================================
// Read-only mode disables editing
// ========================================================================

#[test]
fn test_readonly_prevents_insert() {
    let mut comp = make_component();
    comp.state.read_only = true;
    let original = comp.state.text();
    comp.state.insert(0, "inserted");
    assert_eq!(comp.state.text(), original);
    assert!(!comp.state.is_dirty);
}

#[test]
fn test_readonly_prevents_delete() {
    let mut comp = make_component();
    comp.state.read_only = true;
    let original = comp.state.text();
    comp.state.delete(0, 5);
    assert_eq!(comp.state.text(), original);
}

#[test]
fn test_readonly_prevents_undo() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    comp.state.read_only = true;
    assert!(!comp.state.undo());
}

#[test]
fn test_readonly_prevents_redo() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    comp.state.undo();
    comp.state.read_only = true;
    assert!(!comp.state.redo());
}

#[test]
fn test_readonly_cannot_apply() {
    let mut comp = make_component();
    comp.state.insert(0, "# modified\n");
    comp.state.read_only = true;
    assert!(!comp.state.can_apply());
}

#[test]
fn test_readonly_status_text() {
    let mut comp = make_component();
    comp.state.read_only = true;
    assert_eq!(comp.status_text(), "Read-only");
}

// ========================================================================
// Apply state transitions
// ========================================================================

#[test]
fn test_begin_apply() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    comp.state.begin_apply();
    assert!(comp.state.applying);
    assert!(comp.state.apply_error.is_none());
    assert!(comp.state.conflict.is_none());
}

#[test]
fn test_apply_success() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    let new_text = comp.state.text();
    comp.state.begin_apply();
    comp.state.apply_success("67890");
    assert!(!comp.state.applying);
    assert!(!comp.state.is_dirty);
    assert_eq!(comp.state.resource_version, "67890");
    assert_eq!(comp.state.original_yaml, new_text);
}

#[test]
fn test_apply_failure() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    comp.state.begin_apply();
    comp.state.apply_failure("server error".to_string());
    assert!(!comp.state.applying);
    assert_eq!(comp.state.apply_error.as_deref(), Some("server error"));
    assert!(comp.state.is_dirty);
}

#[test]
fn test_apply_conflict() {
    let mut comp = make_component();
    comp.state.insert(0, "# my change\n");
    comp.state.begin_apply();
    let server =
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 10\n";
    comp.state.apply_conflict(server.to_string());
    assert!(!comp.state.applying);
    assert!(comp.state.has_conflict());
    let conflict = comp.state.conflict.as_ref().unwrap();
    assert_eq!(conflict.server_yaml, server);
    assert!(conflict.message.contains("Deployment"));
    assert!(conflict.message.contains("nginx"));
}

#[test]
fn test_status_text_applying() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    comp.state.begin_apply();
    assert_eq!(comp.status_text(), "Applying...");
}

#[test]
fn test_status_text_error() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    comp.state.begin_apply();
    comp.state.apply_failure("timeout".to_string());
    assert_eq!(comp.status_text(), "Error: timeout");
}

#[test]
fn test_status_text_valid() {
    let comp = make_component();
    assert_eq!(comp.status_text(), "Valid YAML");
}

#[test]
fn test_status_text_invalid_yaml() {
    let mut state = EditorViewState::new("key: [invalid", "ConfigMap", "test", None, "1");
    state.validate();
    let comp = EditorViewComponent::new(state, Theme::dark());
    assert!(comp.status_text().starts_with("Invalid YAML:"));
}

// ========================================================================
// Conflict state handling
// ========================================================================

#[test]
fn test_accept_server_version() {
    let mut comp = make_component();
    comp.state.insert(0, "# my change\n");
    comp.state.begin_apply();
    comp.state.apply_conflict("server: yaml".to_string());

    comp.state.accept_server_version("server: yaml\n", "99999");

    assert!(!comp.state.has_conflict());
    assert_eq!(comp.state.text(), "server: yaml\n");
    assert_eq!(comp.state.original_yaml, "server: yaml\n");
    assert_eq!(comp.state.resource_version, "99999");
    assert!(!comp.state.is_dirty);
}

#[test]
fn test_dismiss_conflict() {
    let mut comp = make_component();
    comp.state.insert(0, "# my change\n");
    comp.state.begin_apply();
    comp.state.apply_conflict("server: yaml".to_string());

    comp.state.dismiss_conflict();
    assert!(!comp.state.has_conflict());
    // Local edits preserved
    assert!(comp.state.text().starts_with("# my change\n"));
}

#[test]
fn test_conflict_message_contains_resource_info() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    comp.state.begin_apply();
    comp.state.apply_conflict("server yaml".to_string());

    let conflict = comp.state.conflict.as_ref().unwrap();
    assert!(conflict.message.contains("Deployment"));
    assert!(conflict.message.contains("nginx"));
    assert!(conflict.message.contains("default"));
}

#[test]
fn test_conflict_message_cluster_scoped() {
    let mut state =
        EditorViewState::new("apiVersion: v1\nkind: Node\n", "Node", "node-1", None, "1");
    state.insert(0, "x");
    state.begin_apply();
    state.apply_conflict("server yaml".to_string());

    let conflict = state.conflict.as_ref().unwrap();
    assert!(conflict.message.contains("Node"));
    assert!(conflict.message.contains("node-1"));
    assert!(!conflict.message.contains("namespace"));
}

// ========================================================================
// Title includes dirty marker
// ========================================================================

#[test]
fn test_title_clean() {
    let comp = make_component();
    assert_eq!(comp.state.title(), "Deployment/nginx");
}

#[test]
fn test_title_dirty() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    assert_eq!(comp.state.title(), "Deployment/nginx *");
}

#[test]
fn test_title_dirty_contains_asterisk() {
    let mut comp = make_component();
    comp.state.insert(0, "x");
    let title = comp.state.title();
    assert!(title.contains('*'));
}

#[test]
fn test_title_clean_no_asterisk() {
    let comp = make_component();
    let title = comp.state.title();
    assert!(!title.contains('*'));
}

// ========================================================================
// Component construction tests
// ========================================================================

#[test]
fn test_component_new_dark_theme() {
    let state = make_state();
    let comp = EditorViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.line_count(), 7);
}

#[test]
fn test_component_new_light_theme() {
    let state = make_state();
    let comp = EditorViewComponent::new(state, Theme::light());
    assert_eq!(comp.state.line_count(), 7);
}

// ========================================================================
// can_apply logic
// ========================================================================

#[test]
fn test_can_apply_not_dirty() {
    let comp = make_component();
    assert!(!comp.state.can_apply());
}

#[test]
fn test_can_apply_dirty_and_valid() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    assert!(comp.state.can_apply());
}

#[test]
fn test_can_apply_while_applying() {
    let mut comp = make_component();
    comp.state.insert(0, "# mod\n");
    comp.state.applying = true;
    assert!(!comp.state.can_apply());
}

#[test]
fn test_can_apply_invalid_yaml() {
    let mut comp = make_component();
    comp.state.buffer = TextBuffer::from_str("key: [invalid");
    comp.state.is_dirty = true;
    comp.state.validate();
    assert!(!comp.state.can_apply());
}

// ========================================================================
// Full workflow integration test
// ========================================================================

#[test]
fn test_full_editor_workflow() {
    let mut comp = make_component();

    // 1. Initial state
    assert!(!comp.state.is_dirty);
    assert!(!comp.state.can_apply());
    assert_eq!(comp.state.title(), "Deployment/nginx");
    assert_eq!(comp.status_text(), "Valid YAML");

    // 2. Make edits
    comp.state.insert(0, "# modified\n");
    assert!(comp.state.is_dirty);
    assert!(comp.state.can_apply());
    assert!(comp.state.title().contains('*'));

    // 3. View diff
    comp.state.show_diff();
    assert_eq!(comp.state.mode, EditorMode::Diff);
    let diff = comp.state.compute_diff();
    assert!(diff.has_changes());
    assert!(diff.added_count > 0);

    // 4. Switch back and apply
    comp.state.show_edit();
    comp.state.begin_apply();
    assert!(comp.state.applying);
    assert!(!comp.state.can_apply());
    assert_eq!(comp.status_text(), "Applying...");

    // 5. Success
    comp.state.apply_success("67890");
    assert!(!comp.state.is_dirty);
    assert!(!comp.state.applying);
    assert_eq!(comp.state.resource_version, "67890");
    assert_eq!(comp.status_text(), "Valid YAML");
    assert!(!comp.state.title().contains('*'));
}

#[test]
fn test_full_conflict_workflow() {
    let mut comp = make_component();

    // 1. Edit
    comp.state.insert(0, "# my change\n");
    assert!(comp.state.can_apply());

    // 2. Apply fails with conflict
    comp.state.begin_apply();
    comp.state.apply_conflict("server: yaml\n".to_string());
    assert!(comp.state.has_conflict());
    assert!(!comp.state.applying);

    // 3. Accept server version
    comp.state.accept_server_version("server: yaml\n", "99999");
    assert!(!comp.state.has_conflict());
    assert!(!comp.state.is_dirty);
    assert_eq!(comp.state.text(), "server: yaml\n");
    assert_eq!(comp.state.resource_version, "99999");
}

#[test]
fn test_reset_clears_all_state() {
    let mut comp = make_component();
    comp.state.insert(0, "# modified\n");
    comp.state.show_diff();
    comp.state.apply_failure("error".to_string());

    comp.state.reset();

    assert_eq!(comp.state.text(), SAMPLE_YAML);
    assert!(!comp.state.is_dirty);
    assert!(comp.state.validation_error.is_none());
    assert!(comp.state.apply_error.is_none());
    assert!(comp.state.conflict.is_none());
    assert_eq!(comp.state.mode, EditorMode::Edit);
}
