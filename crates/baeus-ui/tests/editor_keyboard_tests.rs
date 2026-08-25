// T056: Integration tests for editor keyboard input handling.
// Tests for cursor movement, text insertion/deletion, undo/redo via keyboard,
// selection, and read-only mode protection.

use baeus_ui::components::editor_view::{CursorDirection, EditorViewState, KeyModifiers};

const SAMPLE_YAML: &str =
    "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: nginx\nspec:\n  replicas: 3\n";

fn make_editor() -> EditorViewState {
    EditorViewState::new(SAMPLE_YAML, "Deployment", "nginx", Some("default".to_string()), "12345")
}

fn no_modifiers() -> KeyModifiers {
    KeyModifiers::default()
}

fn cmd() -> KeyModifiers {
    KeyModifiers { cmd: true, ..Default::default() }
}

fn cmd_shift() -> KeyModifiers {
    KeyModifiers { cmd: true, shift: true, ..Default::default() }
}

// === Typing printable characters ===

#[test]
fn test_type_character_inserts_at_cursor() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("x", no_modifiers());
    assert!(editor.text().starts_with('x'));
    assert_eq!(editor.cursor_position, 1);
}

#[test]
fn test_type_character_at_cursor_middle() {
    let mut editor = make_editor();
    // Place cursor after "apiVersion"
    editor.cursor_position = 10;
    editor.handle_key("X", no_modifiers());
    let text = editor.text();
    assert_eq!(&text[10..11], "X");
    assert_eq!(editor.cursor_position, 11);
}

#[test]
fn test_type_character_at_end_of_buffer() {
    let mut editor = make_editor();
    let len = editor.buffer.len_chars();
    editor.cursor_position = len;
    editor.handle_key("#", no_modifiers());
    assert!(editor.text().ends_with('#'));
    assert_eq!(editor.cursor_position, len + 1);
}

#[test]
fn test_type_multiple_characters_sequentially() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("a", no_modifiers());
    editor.handle_key("b", no_modifiers());
    editor.handle_key("c", no_modifiers());
    assert!(editor.text().starts_with("abc"));
    assert_eq!(editor.cursor_position, 3);
}

// === Backspace ===

#[test]
fn test_backspace_deletes_previous_character() {
    let mut editor = make_editor();
    editor.cursor_position = 1;
    editor.handle_key("backspace", no_modifiers());
    // 'a' from "apiVersion" should be removed
    assert!(editor.text().starts_with("piVersion"));
    assert_eq!(editor.cursor_position, 0);
}

#[test]
fn test_backspace_at_position_zero_does_nothing() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    let original = editor.text();
    editor.handle_key("backspace", no_modifiers());
    assert_eq!(editor.text(), original);
    assert_eq!(editor.cursor_position, 0);
}

// === Delete key ===

#[test]
fn test_delete_removes_next_character() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("delete", no_modifiers());
    // 'a' from "apiVersion" should be removed
    assert!(editor.text().starts_with("piVersion"));
    assert_eq!(editor.cursor_position, 0);
}

#[test]
fn test_delete_at_end_of_buffer_does_nothing() {
    let mut editor = make_editor();
    let len = editor.buffer.len_chars();
    editor.cursor_position = len;
    let original = editor.text();
    editor.handle_key("delete", no_modifiers());
    assert_eq!(editor.text(), original);
}

// === Enter key ===

#[test]
fn test_enter_inserts_newline() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("enter", no_modifiers());
    assert!(editor.text().starts_with('\n'));
    assert_eq!(editor.cursor_position, 1);
}

// === Arrow keys ===

#[test]
fn test_arrow_left_moves_cursor() {
    let mut editor = make_editor();
    editor.cursor_position = 5;
    editor.handle_key("left", no_modifiers());
    assert_eq!(editor.cursor_position, 4);
}

#[test]
fn test_arrow_left_at_zero_stays() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("left", no_modifiers());
    assert_eq!(editor.cursor_position, 0);
}

#[test]
fn test_arrow_right_moves_cursor() {
    let mut editor = make_editor();
    editor.cursor_position = 5;
    editor.handle_key("right", no_modifiers());
    assert_eq!(editor.cursor_position, 6);
}

#[test]
fn test_arrow_right_at_end_stays() {
    let mut editor = make_editor();
    let len = editor.buffer.len_chars();
    editor.cursor_position = len;
    editor.handle_key("right", no_modifiers());
    assert_eq!(editor.cursor_position, len);
}

#[test]
fn test_arrow_up_moves_to_previous_line() {
    let mut editor = make_editor();
    // Start of line 1 ("kind: Deployment\n")
    // Line 0 is "apiVersion: apps/v1\n" (20 chars)
    editor.cursor_position = 20; // start of line 1
    editor.handle_key("up", no_modifiers());
    assert_eq!(editor.cursor_position, 0); // start of line 0
}

#[test]
fn test_arrow_up_at_first_line_stays() {
    let mut editor = make_editor();
    editor.cursor_position = 5;
    editor.handle_key("up", no_modifiers());
    assert_eq!(editor.cursor_position, 5); // stays on line 0
}

#[test]
fn test_arrow_down_moves_to_next_line() {
    let mut editor = make_editor();
    editor.cursor_position = 5; // column 5 on line 0
    editor.handle_key("down", no_modifiers());
    // Line 1 starts at offset 20, col 5 => offset 25
    assert_eq!(editor.cursor_position, 25);
}

#[test]
fn test_arrow_down_clamps_column() {
    let mut editor = make_editor();
    // "apiVersion: apps/v1\n" is 19 chars before newline, col 18 is last char
    editor.cursor_position = 18; // col 18 on line 0
    editor.handle_key("down", no_modifiers());
    // Line 1 "kind: Deployment\n" has 17 chars, so col clamps to 16 (0-indexed)
    // Line 1 starts at 20, so 20 + 16 = 36
    assert_eq!(editor.cursor_position, 36);
}

// === Home/End ===

#[test]
fn test_home_moves_to_line_start() {
    let mut editor = make_editor();
    editor.cursor_position = 25; // middle of line 1
    editor.handle_key("home", no_modifiers());
    assert_eq!(editor.cursor_position, 20); // start of line 1
}

#[test]
fn test_end_moves_to_line_end() {
    let mut editor = make_editor();
    editor.cursor_position = 20; // start of line 1
    editor.handle_key("end", no_modifiers());
    // Line 1 is "kind: Deployment\n", 16 chars without newline
    // End position = 20 + 16 = 36
    assert_eq!(editor.cursor_position, 36);
}

// === Cmd+Z (Undo) / Cmd+Shift+Z (Redo) ===

#[test]
fn test_cmd_z_triggers_undo() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("x", no_modifiers());
    assert!(editor.is_dirty);

    let handled = editor.handle_key("z", cmd());
    assert!(handled);
    assert_eq!(editor.text(), SAMPLE_YAML);
}

#[test]
fn test_cmd_shift_z_triggers_redo() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.handle_key("x", no_modifiers());
    editor.handle_key("z", cmd()); // undo

    let handled = editor.handle_key("z", cmd_shift());
    assert!(handled);
    assert!(editor.text().starts_with('x'));
}

// === Cmd+A (Select All) ===

#[test]
fn test_cmd_a_selects_all() {
    let mut editor = make_editor();
    let len = editor.buffer.len_chars();
    let handled = editor.handle_key("a", cmd());
    assert!(handled);
    assert_eq!(editor.selection, Some((0, len)));
}

// === Read-only mode ===

#[test]
fn test_readonly_prevents_character_insert() {
    let mut editor = make_editor();
    editor.read_only = true;
    let original = editor.text();
    let handled = editor.handle_key("x", no_modifiers());
    assert!(!handled);
    assert_eq!(editor.text(), original);
}

#[test]
fn test_readonly_prevents_backspace() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 5;
    let original = editor.text();
    editor.handle_key("backspace", no_modifiers());
    assert_eq!(editor.text(), original);
}

#[test]
fn test_readonly_prevents_delete() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 0;
    let original = editor.text();
    editor.handle_key("delete", no_modifiers());
    assert_eq!(editor.text(), original);
}

#[test]
fn test_readonly_prevents_enter() {
    let mut editor = make_editor();
    editor.read_only = true;
    let original = editor.text();
    editor.handle_key("enter", no_modifiers());
    assert_eq!(editor.text(), original);
}

#[test]
fn test_readonly_allows_arrow_keys() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 5;
    let handled = editor.handle_key("left", no_modifiers());
    assert!(handled);
    assert_eq!(editor.cursor_position, 4);
}

#[test]
fn test_readonly_allows_home_end() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 5;
    let handled = editor.handle_key("home", no_modifiers());
    assert!(handled);
    assert_eq!(editor.cursor_position, 0);
}

// === Cursor position tracking ===

#[test]
fn test_cursor_starts_at_zero() {
    let editor = make_editor();
    assert_eq!(editor.cursor_position, 0);
}

#[test]
fn test_cursor_advances_after_insert() {
    let mut editor = make_editor();
    editor.insert_at_cursor("abc");
    assert_eq!(editor.cursor_position, 3);
}

#[test]
fn test_cursor_line_and_column() {
    let mut editor = make_editor();
    // "apiVersion: apps/v1\nkind: ..."
    // cursor at position 25 => line 1, col 5
    editor.cursor_position = 25;
    assert_eq!(editor.cursor_line(), 1);
    assert_eq!(editor.cursor_column(), 5);
}

#[test]
fn test_cursor_line_at_start() {
    let editor = make_editor();
    assert_eq!(editor.cursor_line(), 0);
    assert_eq!(editor.cursor_column(), 0);
}

// === insert_at_cursor ===

#[test]
fn test_insert_at_cursor_basic() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.insert_at_cursor("# ");
    assert!(editor.text().starts_with("# "));
    assert_eq!(editor.cursor_position, 2);
}

#[test]
fn test_insert_at_cursor_end_of_buffer() {
    let mut editor = make_editor();
    let len = editor.buffer.len_chars();
    editor.cursor_position = len;
    editor.insert_at_cursor("# end");
    assert!(editor.text().ends_with("# end"));
    assert_eq!(editor.cursor_position, len + 5);
}

#[test]
fn test_multi_character_paste() {
    let mut editor = make_editor();
    editor.cursor_position = 0;
    editor.insert_at_cursor("hello world\n");
    assert!(editor.text().starts_with("hello world\n"));
    assert_eq!(editor.cursor_position, 12);
}

// === move_cursor direct API ===

#[test]
fn test_move_cursor_left() {
    let mut editor = make_editor();
    editor.cursor_position = 10;
    editor.move_cursor(CursorDirection::Left);
    assert_eq!(editor.cursor_position, 9);
}

#[test]
fn test_move_cursor_right() {
    let mut editor = make_editor();
    editor.cursor_position = 10;
    editor.move_cursor(CursorDirection::Right);
    assert_eq!(editor.cursor_position, 11);
}

#[test]
fn test_move_cursor_clears_selection() {
    let mut editor = make_editor();
    editor.select_all();
    assert!(editor.selection.is_some());
    editor.move_cursor(CursorDirection::Right);
    assert!(editor.selection.is_none());
}

// === select_all ===

#[test]
fn test_select_all_on_empty_buffer() {
    let mut editor = EditorViewState::new("", "ConfigMap", "cm", None, "1");
    editor.select_all();
    assert_eq!(editor.selection, Some((0, 0)));
}

// === handle_key return value ===

#[test]
fn test_handle_key_returns_false_for_unknown_key() {
    let mut editor = make_editor();
    let handled = editor.handle_key("F1", no_modifiers());
    assert!(!handled);
}

#[test]
fn test_handle_key_returns_true_for_printable() {
    let mut editor = make_editor();
    let handled = editor.handle_key("a", no_modifiers());
    assert!(handled);
}

#[test]
fn test_handle_key_returns_true_for_arrow() {
    let mut editor = make_editor();
    editor.cursor_position = 5;
    let handled = editor.handle_key("left", no_modifiers());
    assert!(handled);
}

// === Cursor clamp on beyond-end ===

#[test]
fn test_insert_at_cursor_clamps_position() {
    let mut editor = make_editor();
    editor.cursor_position = 99999; // way beyond buffer length
    editor.insert_at_cursor("x");
    // Should insert at end and cursor should be at end+1
    assert!(editor.text().ends_with('x'));
}

// === Undo in read-only mode via handle_key ===

#[test]
fn test_cmd_z_noop_in_readonly() {
    let mut editor = make_editor();
    editor.insert(0, "x");
    editor.read_only = true;
    let handled = editor.handle_key("z", cmd());
    assert!(!handled); // undo is blocked in read-only
}

// === backspace_at_cursor direct API ===

#[test]
fn test_backspace_at_cursor_readonly_noop() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 5;
    editor.backspace_at_cursor();
    assert_eq!(editor.text(), SAMPLE_YAML);
    assert_eq!(editor.cursor_position, 5);
}

// === delete_at_cursor direct API ===

#[test]
fn test_delete_at_cursor_readonly_noop() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.cursor_position = 0;
    editor.delete_at_cursor();
    assert_eq!(editor.text(), SAMPLE_YAML);
}

// === insert_at_cursor in read-only ===

#[test]
fn test_insert_at_cursor_readonly_noop() {
    let mut editor = make_editor();
    editor.read_only = true;
    editor.insert_at_cursor("x");
    assert_eq!(editor.text(), SAMPLE_YAML);
}
