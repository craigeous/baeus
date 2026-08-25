// T058: Integration tests for the YAML apply workflow.
// Tests for can_apply(), begin_apply(), apply_success/failure/conflict,
// accept_server_version, ApplyWorkflowState, and ResourceDetailState integration.

use baeus_editor::buffer::TextBuffer;
use baeus_ui::components::editor_view::{ApplyWorkflowState, EditorViewState};
use baeus_ui::views::resource_detail::ResourceDetailState;

const SAMPLE_YAML: &str =
    "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 3\n";

fn make_editor() -> EditorViewState {
    EditorViewState::new(SAMPLE_YAML, "Deployment", "nginx", Some("default".to_string()), "12345")
}

fn make_dirty_editor() -> EditorViewState {
    let mut editor = make_editor();
    editor.insert(0, "# modified\n");
    editor
}

// === can_apply() tests ===

#[test]
fn test_can_apply_true_when_dirty_valid_not_applying_not_readonly() {
    let editor = make_dirty_editor();
    assert!(editor.is_dirty);
    assert!(editor.is_valid());
    assert!(!editor.applying);
    assert!(!editor.read_only);
    assert!(editor.can_apply());
}

#[test]
fn test_can_apply_false_when_not_dirty() {
    let editor = make_editor();
    assert!(!editor.is_dirty);
    assert!(!editor.can_apply());
}

#[test]
fn test_can_apply_false_when_invalid_yaml() {
    let mut editor = make_editor();
    editor.buffer = TextBuffer::from_str("key: [invalid");
    editor.is_dirty = true;
    editor.validate();
    assert!(!editor.is_valid());
    assert!(!editor.can_apply());
}

#[test]
fn test_can_apply_false_when_applying() {
    let mut editor = make_dirty_editor();
    editor.applying = true;
    assert!(!editor.can_apply());
}

#[test]
fn test_can_apply_false_when_readonly() {
    let mut editor = make_dirty_editor();
    editor.read_only = true;
    assert!(!editor.can_apply());
}

// === begin_apply() ===

#[test]
fn test_begin_apply_sets_applying() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    assert!(editor.applying);
    assert!(editor.apply_error.is_none());
    assert!(editor.conflict.is_none());
}

#[test]
fn test_begin_apply_clears_previous_error() {
    let mut editor = make_dirty_editor();
    editor.apply_error = Some("previous error".to_string());
    editor.begin_apply();
    assert!(editor.apply_error.is_none());
}

#[test]
fn test_begin_apply_clears_previous_conflict() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());
    assert!(editor.has_conflict());

    // Begin apply again after conflict resolution
    editor.begin_apply();
    assert!(!editor.has_conflict());
    assert!(editor.applying);
}

// === apply_success() ===

#[test]
fn test_apply_success_resets_dirty() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_success("99999");
    assert!(!editor.is_dirty);
}

#[test]
fn test_apply_success_updates_original_yaml() {
    let mut editor = make_dirty_editor();
    let modified_text = editor.text();
    editor.begin_apply();
    editor.apply_success("99999");
    assert_eq!(editor.original_yaml, modified_text);
}

#[test]
fn test_apply_success_updates_resource_version() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_success("99999");
    assert_eq!(editor.resource_version, "99999");
}

#[test]
fn test_apply_success_clears_applying() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    assert!(editor.applying);
    editor.apply_success("99999");
    assert!(!editor.applying);
}

#[test]
fn test_apply_success_clears_error_and_conflict() {
    let mut editor = make_dirty_editor();
    editor.apply_error = Some("stale".to_string());
    editor.begin_apply();
    editor.apply_success("99999");
    assert!(editor.apply_error.is_none());
    assert!(editor.conflict.is_none());
}

// === apply_failure() ===

#[test]
fn test_apply_failure_sets_error() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_failure("server error".to_string());
    assert_eq!(editor.apply_error.as_deref(), Some("server error"));
}

#[test]
fn test_apply_failure_clears_applying() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_failure("error".to_string());
    assert!(!editor.applying);
}

#[test]
fn test_apply_failure_preserves_dirty() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_failure("error".to_string());
    assert!(editor.is_dirty);
}

// === apply_conflict() ===

#[test]
fn test_apply_conflict_creates_conflict_state() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: version\n".to_string());

    assert!(editor.has_conflict());
    let conflict = editor.conflict.as_ref().unwrap();
    assert_eq!(conflict.server_yaml, "server: version\n");
}

#[test]
fn test_apply_conflict_preserves_local_yaml() {
    let mut editor = make_dirty_editor();
    let local = editor.text();
    editor.begin_apply();
    editor.apply_conflict("server: version\n".to_string());

    let conflict = editor.conflict.as_ref().unwrap();
    assert_eq!(conflict.local_yaml, local);
}

#[test]
fn test_apply_conflict_message_contains_resource_info() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());

    let conflict = editor.conflict.as_ref().unwrap();
    assert!(conflict.message.contains("Deployment"));
    assert!(conflict.message.contains("nginx"));
    assert!(conflict.message.contains("default"));
}

#[test]
fn test_apply_conflict_clears_applying() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());
    assert!(!editor.applying);
}

// === accept_server_version() ===

#[test]
fn test_accept_server_replaces_buffer() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());

    editor.accept_server_version("server: yaml\n", "55555");
    assert_eq!(editor.text(), "server: yaml\n");
}

#[test]
fn test_accept_server_clears_conflict() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());
    assert!(editor.has_conflict());

    editor.accept_server_version("server: yaml\n", "55555");
    assert!(!editor.has_conflict());
}

#[test]
fn test_accept_server_updates_version() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());

    editor.accept_server_version("server: yaml\n", "55555");
    assert_eq!(editor.resource_version, "55555");
    assert_eq!(editor.original_yaml, "server: yaml\n");
    assert!(!editor.is_dirty);
}

#[test]
fn test_accept_server_resets_cursor() {
    let mut editor = make_dirty_editor();
    editor.cursor_position = 20;
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());

    editor.accept_server_version("server: yaml\n", "55555");
    assert_eq!(editor.cursor_position, 0);
    assert!(editor.selection.is_none());
}

// === Full apply workflow ===

#[test]
fn test_full_apply_workflow_success() {
    let mut editor = make_editor();

    // 1. Edit
    editor.insert(0, "# modified\n");
    assert!(editor.is_dirty);
    editor.validate();
    assert!(editor.can_apply());

    // 2. Begin apply
    editor.begin_apply();
    assert!(editor.applying);
    assert!(!editor.can_apply());

    // 3. Success
    editor.apply_success("67890");
    assert!(!editor.is_dirty);
    assert!(!editor.applying);
    assert_eq!(editor.resource_version, "67890");
}

#[test]
fn test_full_conflict_workflow() {
    let mut editor = make_editor();

    // 1. Edit
    editor.insert(0, "# my change\n");
    assert!(editor.can_apply());

    // 2. Begin apply
    editor.begin_apply();

    // 3. Conflict
    editor.apply_conflict("server: yaml\n".to_string());
    assert!(editor.has_conflict());
    assert!(!editor.applying);

    // 4. Accept server version
    editor.accept_server_version("server: yaml\n", "77777");
    assert!(!editor.has_conflict());
    assert_eq!(editor.text(), "server: yaml\n");
    assert_eq!(editor.resource_version, "77777");
    assert!(!editor.is_dirty);
}

#[test]
fn test_full_retry_workflow() {
    let mut editor = make_editor();

    // 1. Edit
    editor.insert(0, "# change\n");
    assert!(editor.can_apply());

    // 2. First apply attempt fails
    editor.begin_apply();
    editor.apply_failure("temporary error".to_string());
    assert_eq!(editor.apply_error.as_deref(), Some("temporary error"));
    assert!(editor.is_dirty);

    // 3. Edit again (content still modified)
    assert!(editor.can_apply());

    // 4. Second apply attempt succeeds
    editor.begin_apply();
    assert!(editor.apply_error.is_none()); // error cleared
    editor.apply_success("88888");
    assert!(!editor.is_dirty);
    assert_eq!(editor.resource_version, "88888");
}

// === ApplyWorkflowState ===

#[test]
fn test_workflow_state_idle() {
    let editor = make_editor();
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Idle);
}

#[test]
fn test_workflow_state_ready() {
    let editor = make_dirty_editor();
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Ready);
}

#[test]
fn test_workflow_state_applying() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Applying);
}

#[test]
fn test_workflow_state_failed() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_failure("err".to_string());
    // After failure, editor is still dirty+valid so it shows Ready
    // unless there's an apply_error, which takes priority
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Failed("err".to_string()));
}

#[test]
fn test_workflow_state_conflict() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Conflict);
}

#[test]
fn test_workflow_state_idle_after_success() {
    let mut editor = make_dirty_editor();
    editor.begin_apply();
    editor.apply_success("99999");
    // After success: not dirty, no error, no conflict => Idle
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Idle);
}

// === ResourceDetailState integration ===

#[test]
fn test_resource_detail_on_yaml_apply_success() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml(SAMPLE_YAML.to_string(), "1".to_string());
    state.open_yaml_editor();

    let editor = state.yaml_editor_mut().unwrap();
    editor.insert(0, "# modified\n");
    editor.begin_apply();

    let new_yaml = state.yaml_editor_ref().unwrap().text();
    state.on_yaml_apply_success(new_yaml.clone(), "2".to_string());

    assert_eq!(state.resource_version.as_deref(), Some("2"));
    assert_eq!(state.resource_yaml.as_deref(), Some(new_yaml.as_str()));
    assert!(!state.yaml_editor_ref().unwrap().is_dirty);
    assert!(!state.yaml_editor_ref().unwrap().applying);
}

#[test]
fn test_resource_detail_on_yaml_apply_failure() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml(SAMPLE_YAML.to_string(), "1".to_string());
    state.open_yaml_editor();

    let editor = state.yaml_editor_mut().unwrap();
    editor.insert(0, "# change\n");
    editor.begin_apply();

    state.on_yaml_apply_failure("forbidden".to_string());

    let editor = state.yaml_editor_ref().unwrap();
    assert!(!editor.applying);
    assert_eq!(editor.apply_error.as_deref(), Some("forbidden"));
    assert!(editor.is_dirty);
}

#[test]
fn test_resource_detail_on_yaml_apply_conflict() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml(SAMPLE_YAML.to_string(), "1".to_string());
    state.open_yaml_editor();

    let editor = state.yaml_editor_mut().unwrap();
    editor.insert(0, "# change\n");
    editor.begin_apply();

    state.on_yaml_apply_conflict("server: version\n".to_string());

    let editor = state.yaml_editor_ref().unwrap();
    assert!(!editor.applying);
    assert!(editor.has_conflict());
    let conflict = editor.conflict.as_ref().unwrap();
    assert_eq!(conflict.server_yaml, "server: version\n");
}

#[test]
fn test_resource_detail_full_apply_success_workflow() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml(SAMPLE_YAML.to_string(), "100".to_string());
    state.open_yaml_editor();

    // Edit
    {
        let editor = state.yaml_editor_mut().unwrap();
        editor.buffer =
            TextBuffer::from_str("apiVersion: apps/v1\nkind: Deployment\nspec:\n  replicas: 5\n");
        editor.is_dirty = true;
        editor.validate();
        assert!(editor.can_apply());
        editor.begin_apply();
    }

    // Success
    let new_yaml = state.yaml_editor_ref().unwrap().text();
    state.on_yaml_apply_success(new_yaml.clone(), "101".to_string());

    assert_eq!(state.resource_version.as_deref(), Some("101"));
    assert!(!state.yaml_editor_ref().unwrap().is_dirty);
}

#[test]
fn test_resource_detail_conflict_then_accept() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml(SAMPLE_YAML.to_string(), "100".to_string());
    state.open_yaml_editor();

    // Edit and begin apply
    {
        let editor = state.yaml_editor_mut().unwrap();
        editor.insert(0, "# changed\n");
        editor.begin_apply();
    }

    // Conflict
    let server_yaml = "server: updated\n";
    state.on_yaml_apply_conflict(server_yaml.to_string());
    assert!(state.yaml_editor_ref().unwrap().has_conflict());

    // Accept server version
    {
        let editor = state.yaml_editor_mut().unwrap();
        editor.accept_server_version(server_yaml, "200");
    }

    assert!(!state.yaml_editor_ref().unwrap().has_conflict());
    assert_eq!(state.yaml_editor_ref().unwrap().text(), server_yaml);
    assert_eq!(state.yaml_editor_ref().unwrap().resource_version, "200");
}

// === Additional edge cases ===

#[test]
fn test_can_apply_after_reset() {
    let mut editor = make_dirty_editor();
    assert!(editor.can_apply());
    editor.reset();
    assert!(!editor.can_apply()); // not dirty after reset
}

#[test]
fn test_apply_success_then_edit_again() {
    let mut editor = make_editor();
    editor.insert(0, "# first\n");
    editor.begin_apply();
    editor.apply_success("2");

    assert!(!editor.can_apply());
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Idle);

    // Edit again
    editor.insert(0, "# second\n");
    assert!(editor.can_apply());
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Ready);
}

#[test]
fn test_dismiss_conflict_preserves_local_edits() {
    let mut editor = make_dirty_editor();
    let local_text = editor.text();
    editor.begin_apply();
    editor.apply_conflict("server: yaml".to_string());

    editor.dismiss_conflict();
    assert!(!editor.has_conflict());
    assert_eq!(editor.text(), local_text);
    assert!(editor.is_dirty);
}

#[test]
fn test_workflow_state_transitions() {
    let mut editor = make_editor();

    // Idle -> Ready
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Idle);
    editor.insert(0, "# x\n");
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Ready);

    // Ready -> Applying
    editor.begin_apply();
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Applying);

    // Applying -> Idle (via success)
    editor.apply_success("2");
    assert_eq!(editor.apply_workflow_state(), ApplyWorkflowState::Idle);
}
