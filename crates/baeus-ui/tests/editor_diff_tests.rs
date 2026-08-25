// T057: Integration tests for diff view mode in the editor.
// Tests for computing diffs, diff summaries, mode switching,
// line counts, and diff result accuracy.

use baeus_editor::buffer::TextBuffer;
use baeus_editor::diff::DiffLineKind;
use baeus_ui::components::editor_view::{EditorMode, EditorViewState};

const SAMPLE_YAML: &str =
    "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 3\n";

fn make_editor() -> EditorViewState {
    EditorViewState::new(SAMPLE_YAML, "Deployment", "nginx", Some("default".to_string()), "12345")
}

// === Switching to Diff mode computes diff correctly ===

#[test]
fn test_switch_to_diff_mode() {
    let mut editor = make_editor();
    editor.show_diff();
    assert_eq!(editor.mode, EditorMode::Diff);
}

#[test]
fn test_diff_mode_computes_correct_diff_after_edit() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    editor.show_diff();

    let diff = editor.compute_diff();
    assert!(diff.has_changes());
    assert_eq!(diff.added_count, 1);
    assert_eq!(diff.removed_count, 1);
}

// === Added lines count ===

#[test]
fn test_added_lines_count() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\n  labels:\n    app: nginx\nspec:\n  replicas: 3\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert_eq!(diff.added_count, 2); // "  labels:" and "    app: nginx"
}

// === Removed lines count ===

#[test]
fn test_removed_lines_count() {
    let mut editor = make_editor();
    // Remove the "spec:" and "  replicas: 3" lines
    editor.buffer =
        TextBuffer::from_str("apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\n");
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert_eq!(diff.removed_count, 2); // "spec:" and "  replicas: 3"
}

// === Unchanged lines preserved ===

#[test]
fn test_unchanged_lines_preserved() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    // 5 lines unchanged (apiVersion, kind, metadata, name, spec), 1 removed, 1 added
    assert_eq!(diff.unchanged_count, 5);
}

// === Diff with no changes ===

#[test]
fn test_diff_no_changes() {
    let editor = make_editor();
    let diff = editor.compute_diff();
    assert!(!diff.has_changes());
    assert_eq!(diff.added_count, 0);
    assert_eq!(diff.removed_count, 0);
    assert_eq!(diff.unchanged_count, 6); // all 6 lines unchanged
}

#[test]
fn test_diff_summary_no_changes() {
    let editor = make_editor();
    assert_eq!(editor.diff_summary(), "No changes");
}

// === Diff with completely different content ===

#[test]
fn test_diff_completely_different() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str("completely:\n  different: content\n");
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert!(diff.has_changes());
    assert_eq!(diff.removed_count, 6);
    assert_eq!(diff.added_count, 2);
    assert_eq!(diff.unchanged_count, 0);
}

// === DiffLine line numbers are correct ===

#[test]
fn test_diff_line_numbers_unchanged() {
    let editor = make_editor();
    let diff = editor.compute_diff();
    // All unchanged: old_line_number and new_line_number should match
    for (i, line) in diff.lines.iter().enumerate() {
        assert_eq!(line.kind, DiffLineKind::Unchanged);
        assert_eq!(line.old_line_number, Some(i + 1));
        assert_eq!(line.new_line_number, Some(i + 1));
    }
}

#[test]
fn test_diff_line_numbers_with_changes() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();

    // Check that removed lines have old_line_number but no new_line_number
    let removed: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).collect();
    for r in &removed {
        assert!(r.old_line_number.is_some());
        assert!(r.new_line_number.is_none());
    }

    // Check that added lines have new_line_number but no old_line_number
    let added: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Added).collect();
    for a in &added {
        assert!(a.old_line_number.is_none());
        assert!(a.new_line_number.is_some());
    }
}

// === Switching back to Edit preserves buffer ===

#[test]
fn test_switch_diff_then_edit_preserves_content() {
    let mut editor = make_editor();
    let modified = "apiVersion: apps/v1\nkind: StatefulSet\n";
    editor.buffer = TextBuffer::from_str(modified);
    editor.is_dirty = true;

    editor.show_diff();
    assert_eq!(editor.mode, EditorMode::Diff);
    assert_eq!(editor.text(), modified);

    editor.show_edit();
    assert_eq!(editor.mode, EditorMode::Edit);
    assert_eq!(editor.text(), modified);
}

// === Summary string ===

#[test]
fn test_diff_summary_with_additions_only() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\n  labels:\n    app: nginx\nspec:\n  replicas: 3\n",
    );
    editor.is_dirty = true;
    let summary = editor.diff_summary();
    assert!(summary.starts_with('+'));
    assert!(summary.contains("-0"));
}

#[test]
fn test_diff_summary_with_removals_only() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str("apiVersion: apps/v1\nkind: Deployment\n");
    editor.is_dirty = true;
    let summary = editor.diff_summary();
    assert!(summary.contains("+0"));
    assert!(summary.contains('-'));
}

#[test]
fn test_diff_summary_mixed_changes() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let summary = editor.diff_summary();
    assert_eq!(summary, "+1 -1");
}

// === DiffResult total_lines ===

#[test]
fn test_diff_total_lines_no_changes() {
    let editor = make_editor();
    let diff = editor.compute_diff();
    assert_eq!(diff.total_lines(), 6);
}

#[test]
fn test_diff_total_lines_with_changes() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    // 5 unchanged + 1 removed + 1 added = 7
    assert_eq!(diff.total_lines(), 7);
}

// === Diff content correctness ===

#[test]
fn test_diff_removed_line_content() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    let removed: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].content, "  replicas: 3");
}

#[test]
fn test_diff_added_line_content() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 5\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    let added: Vec<_> = diff.lines.iter().filter(|l| l.kind == DiffLineKind::Added).collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].content, "  replicas: 5");
}

// === Diff after insert then undo ===

#[test]
fn test_diff_after_undo_shows_no_changes() {
    let mut editor = make_editor();
    editor.insert(0, "# comment\n");
    assert!(editor.compute_diff().has_changes());

    editor.undo();
    assert!(!editor.compute_diff().has_changes());
    assert_eq!(editor.diff_summary(), "No changes");
}

// === Multiple rounds of edits ===

#[test]
fn test_diff_multiple_edits() {
    let mut editor = make_editor();
    // Add a line and modify another
    editor.buffer = TextBuffer::from_str(
        "apiVersion: apps/v2\nkind: Deployment\nmetadata:\n  name: nginx\n  labels:\n    app: nginx\nspec:\n  replicas: 3\n",
    );
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert!(diff.has_changes());
    // "apiVersion: apps/v1" removed + "apiVersion: apps/v2" added
    // Two label lines added
    assert!(diff.added_count >= 3);
    assert!(diff.removed_count >= 1);
}

// === Empty buffer diff ===

#[test]
fn test_diff_empty_buffer() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str("");
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert!(diff.has_changes());
    assert_eq!(diff.removed_count, 6);
    assert_eq!(diff.added_count, 0);
}

// === Diff from empty original ===

#[test]
fn test_diff_from_empty_original() {
    let mut editor = EditorViewState::new("", "ConfigMap", "cm", None, "1");
    editor.buffer = TextBuffer::from_str("key: value\n");
    editor.is_dirty = true;
    let diff = editor.compute_diff();
    assert!(diff.has_changes());
    assert_eq!(diff.added_count, 1);
    assert_eq!(diff.removed_count, 0);
}
