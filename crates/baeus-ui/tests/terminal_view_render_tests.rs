// T045: Render tests for TerminalViewComponent
// Tests are STATE-LEVEL (no actual GPUI window needed).

use baeus_ui::components::terminal_view::*;
use baeus_ui::components::terminal_view_component::*;
use baeus_ui::theme::Theme;
use gpui::{Keystroke, Modifiers};
use uuid::Uuid;

/// Helper: build a Keystroke for a single printable character key.
fn key(s: &str) -> Keystroke {
    Keystroke {
        modifiers: Modifiers::default(),
        key: s.to_string(),
        key_char: if s.len() == 1 { Some(s.to_string()) } else { None },
    }
}

/// Helper: build a Keystroke for a special key (enter, backspace, etc.)
fn special_key(name: &str) -> Keystroke {
    Keystroke { modifiers: Modifiers::default(), key: name.to_string(), key_char: None }
}

/// Helper: build a Keystroke with Ctrl modifier.
fn ctrl_key(ch: &str) -> Keystroke {
    Keystroke {
        modifiers: Modifiers { control: true, ..Default::default() },
        key: ch.to_string(),
        key_char: None,
    }
}

// ---------------------------------------------------------------------------
// Construction tests
// ---------------------------------------------------------------------------

#[test]
fn test_component_new_local_shell() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.title, "Shell");
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Disconnected);
}

#[test]
fn test_component_new_pod_exec() {
    let state = TerminalViewState::for_pod_exec(Uuid::new_v4(), "default", "nginx", Some("app"));
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.title, "nginx/app");
}

#[test]
fn test_component_emulator_size_matches_state() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let (rows, cols) = comp.grid_dimensions();
    assert_eq!(rows, 24);
    assert_eq!(cols, 80);
}

#[test]
fn test_component_custom_size() {
    let mut state = TerminalViewState::for_local_shell();
    state.resize(40, 120);
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let (rows, cols) = comp.grid_dimensions();
    assert_eq!(rows, 40);
    assert_eq!(cols, 120);
}

// ---------------------------------------------------------------------------
// Grid dimensions tests (T045: terminal grid rows x cols)
// ---------------------------------------------------------------------------

#[test]
fn test_grid_renders_correct_row_count() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let grid = comp.emulator.grid_rows();
    assert_eq!(grid.len(), 24);
}

#[test]
fn test_grid_renders_correct_col_count() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let grid = comp.emulator.grid_rows();
    assert_eq!(grid[0].len(), 80);
}

#[test]
fn test_grid_small_terminal() {
    let mut state = TerminalViewState::for_local_shell();
    state.resize(5, 10);
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let grid = comp.emulator.grid_rows();
    assert_eq!(grid.len(), 5);
    for row in &grid {
        assert_eq!(row.len(), 10);
    }
}

#[test]
fn test_grid_cells_default_to_space() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let cell = comp.emulator.cell_at(0, 0).unwrap();
    assert_eq!(cell.character, ' ');
}

// ---------------------------------------------------------------------------
// Cursor position tests (T045: cursor at correct position)
// ---------------------------------------------------------------------------

#[test]
fn test_cursor_initial_position() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    let (row, col) = comp.cursor_position();
    assert_eq!(row, 0);
    assert_eq!(col, 0);
}

#[test]
fn test_cursor_advances_after_output() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"Hello");
    let (row, col) = comp.cursor_position();
    assert_eq!(row, 0);
    assert_eq!(col, 5);
}

#[test]
fn test_cursor_wraps_at_end_of_line() {
    let mut state = TerminalViewState::for_local_shell();
    state.resize(5, 10);
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"0123456789X");
    let (row, col) = comp.cursor_position();
    assert_eq!(row, 1);
    assert_eq!(col, 1);
}

#[test]
fn test_cursor_after_newline() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"AB\nCD");
    let (row, col) = comp.cursor_position();
    assert_eq!(row, 1);
    assert_eq!(col, 2);
}

// ---------------------------------------------------------------------------
// Connection state indicator tests (T045)
// ---------------------------------------------------------------------------

#[test]
fn test_connection_state_disconnected() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Disconnected);
}

#[test]
fn test_connection_state_connecting() {
    let mut state = TerminalViewState::for_local_shell();
    state.connect();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Connecting);
}

#[test]
fn test_connection_state_connected() {
    let mut state = TerminalViewState::for_local_shell();
    state.set_connected();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Connected);
}

#[test]
fn test_connection_state_error() {
    let mut state = TerminalViewState::for_local_shell();
    state.set_error("timeout");
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Error("timeout".to_string()));
}

#[test]
fn test_connection_state_transitions() {
    let mut state = TerminalViewState::for_local_shell();
    state.connect();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());

    assert!(comp.state.is_connecting());
    comp.state.set_connected();
    assert!(comp.state.is_connected());
    comp.state.set_error("lost");
    assert!(!comp.state.is_connected());
    assert_eq!(comp.state.connection_state, TerminalConnectionState::Error("lost".to_string()));
}

// ---------------------------------------------------------------------------
// Display mode tests (T045)
// ---------------------------------------------------------------------------

#[test]
fn test_display_mode_default_inline() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.settings.display_mode, TerminalDisplayMode::Inline);
}

#[test]
fn test_display_mode_fullscreen() {
    let mut state = TerminalViewState::for_local_shell();
    state.set_display_mode(TerminalDisplayMode::Fullscreen);
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.settings.display_mode, TerminalDisplayMode::Fullscreen);
}

#[test]
fn test_display_mode_split() {
    let mut state = TerminalViewState::for_local_shell();
    state.set_display_mode(TerminalDisplayMode::Split);
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.settings.display_mode, TerminalDisplayMode::Split);
}

#[test]
fn test_display_mode_change_after_creation() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.set_display_mode(TerminalDisplayMode::Fullscreen);
    assert_eq!(comp.state.settings.display_mode, TerminalDisplayMode::Fullscreen);
    comp.state.set_display_mode(TerminalDisplayMode::Inline);
    assert_eq!(comp.state.settings.display_mode, TerminalDisplayMode::Inline);
}

// ---------------------------------------------------------------------------
// Font size tests (T045)
// ---------------------------------------------------------------------------

#[test]
fn test_font_size_default() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.settings.font_size, 13.0);
}

#[test]
fn test_font_size_change() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.set_font_size(18.0);
    assert_eq!(comp.state.settings.font_size, 18.0);
}

#[test]
fn test_font_size_clamped_low() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.set_font_size(2.0);
    assert_eq!(comp.state.settings.font_size, 8.0);
}

#[test]
fn test_font_size_clamped_high() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.set_font_size(100.0);
    assert_eq!(comp.state.settings.font_size, 32.0);
}

// ---------------------------------------------------------------------------
// Scrollback offset tests (T045)
// ---------------------------------------------------------------------------

#[test]
fn test_scrollback_initial_zero() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert!(!comp.state.is_scrolled_up());
    assert_eq!(comp.state.scrollback_offset, 0);
}

#[test]
fn test_scrollback_scroll_up() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.scroll_up(5);
    assert!(comp.state.is_scrolled_up());
    assert_eq!(comp.state.scrollback_offset, 5);
}

#[test]
fn test_scrollback_scroll_down() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.scroll_up(10);
    comp.state.scroll_down(3);
    assert_eq!(comp.state.scrollback_offset, 7);
}

#[test]
fn test_scrollback_scroll_to_bottom() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.scroll_up(20);
    comp.state.scroll_to_bottom();
    assert_eq!(comp.state.scrollback_offset, 0);
    assert!(!comp.state.is_scrolled_up());
}

// ---------------------------------------------------------------------------
// Bell state tests (T045)
// ---------------------------------------------------------------------------

#[test]
fn test_bell_initial_off() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert!(!comp.state.bell_ringing);
}

#[test]
fn test_bell_ring_and_clear() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.state.ring_bell();
    assert!(comp.state.bell_ringing);
    comp.state.clear_bell();
    assert!(!comp.state.bell_ringing);
}

// ---------------------------------------------------------------------------
// T050: Keyboard input tests
// ---------------------------------------------------------------------------

#[test]
fn test_handle_key_input_printable() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&key("a"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"a");
}

#[test]
fn test_handle_key_input_multiple() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&key("h"));
    comp.handle_keystroke(&key("e"));
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("o"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"hello");
}

#[test]
fn test_handle_key_input_enter() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&special_key("enter"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"\r");
}

#[test]
fn test_handle_key_input_backspace() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&special_key("backspace"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, &[0x7f]);
}

#[test]
fn test_handle_key_input_tab() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&special_key("tab"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"\t");
}

#[test]
fn test_handle_key_input_escape() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&special_key("escape"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, &[0x1b]);
}

#[test]
fn test_take_pending_input_clears_buffer() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&key("x"));
    let _ = comp.take_pending_input();
    let buf = comp.take_pending_input();
    assert!(buf.is_empty());
}

#[test]
fn test_has_pending_input() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    assert!(!comp.has_pending_input());
    comp.handle_keystroke(&key("a"));
    assert!(comp.has_pending_input());
    let _ = comp.take_pending_input();
    assert!(!comp.has_pending_input());
}

#[test]
fn test_handle_key_input_multi_byte_string() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    // Simulate typing "ls -la" character by character as GPUI would deliver
    for ch in "ls".chars() {
        comp.handle_keystroke(&key(&ch.to_string()));
    }
    comp.handle_keystroke(&special_key("space"));
    comp.handle_keystroke(&key("-"));
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("a"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"ls -la");
}

#[test]
fn test_handle_key_input_sequence() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("s"));
    comp.handle_keystroke(&special_key("enter"));
    let buf = comp.take_pending_input();
    assert_eq!(buf, b"ls\r");
}

// ---------------------------------------------------------------------------
// T051: Terminal output tests
// ---------------------------------------------------------------------------

#[test]
fn test_process_output_renders_text() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"Hello");
    let cell = comp.emulator.cell_at(0, 0).unwrap();
    assert_eq!(cell.character, 'H');
    let cell = comp.emulator.cell_at(0, 4).unwrap();
    assert_eq!(cell.character, 'o');
}

#[test]
fn test_process_output_updates_cursor() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"ABC");
    assert_eq!(comp.cursor_position(), (0, 3));
}

#[test]
fn test_process_output_newline() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"AB\nCD");
    assert_eq!(comp.emulator.cell_at(0, 0).unwrap().character, 'A');
    assert_eq!(comp.emulator.cell_at(1, 0).unwrap().character, 'C');
}

#[test]
fn test_process_output_syncs_title() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.emulator.set_title("new-title");
    // Trigger a process_output so it syncs.
    comp.process_output(b" ");
    assert_eq!(comp.state.title, "new-title");
}

#[test]
fn test_process_output_title_not_overwritten_if_empty() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.title, "Shell");
    comp.process_output(b"data");
    // Emulator title is "" so state title should remain "Shell".
    assert_eq!(comp.state.title, "Shell");
}

#[test]
fn test_process_output_multiple_calls() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"AB");
    comp.process_output(b"CD");
    assert_eq!(comp.emulator.cell_at(0, 0).unwrap().character, 'A');
    assert_eq!(comp.emulator.cell_at(0, 2).unwrap().character, 'C');
    assert_eq!(comp.cursor_position(), (0, 4));
}

// ---------------------------------------------------------------------------
// T051: Resize tests
// ---------------------------------------------------------------------------

#[test]
fn test_resize_updates_state_and_emulator() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.resize(40, 120);
    assert_eq!(comp.state.rows, 40);
    assert_eq!(comp.state.cols, 120);
    assert_eq!(comp.grid_dimensions(), (40, 120));
}

#[test]
fn test_resize_preserves_existing_content() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.process_output(b"Hi");
    comp.resize(30, 100);
    assert_eq!(comp.emulator.cell_at(0, 0).unwrap().character, 'H');
    assert_eq!(comp.emulator.cell_at(0, 1).unwrap().character, 'i');
}

#[test]
fn test_resize_shrink() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());
    comp.resize(5, 10);
    let (rows, cols) = comp.grid_dimensions();
    assert_eq!(rows, 5);
    assert_eq!(cols, 10);
    let grid = comp.emulator.grid_rows();
    assert_eq!(grid.len(), 5);
    assert_eq!(grid[0].len(), 10);
}

// ---------------------------------------------------------------------------
// Integration: input + output round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_input_output_roundtrip() {
    let state = TerminalViewState::for_local_shell();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());

    // Simulate typing "ls" and pressing Enter.
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("s"));
    comp.handle_keystroke(&special_key("enter"));

    let input = comp.take_pending_input();
    assert_eq!(input, b"ls\r");

    // Simulate receiving output.
    comp.process_output(b"file1  file2\n");
    assert_eq!(comp.emulator.cell_at(0, 0).unwrap().character, 'f');
}

#[test]
fn test_full_lifecycle() {
    let mut state =
        TerminalViewState::for_pod_exec(Uuid::new_v4(), "production", "web-server", Some("app"));
    state.connect();
    let mut comp = TerminalViewComponent::new(state, Theme::dark());

    // Connecting state
    assert!(comp.state.is_connecting());

    // Move to connected
    comp.state.set_connected();
    assert!(comp.state.is_connected());

    // Process some output
    comp.process_output(b"root@web-server:/# ");

    // Verify grid
    assert_eq!(comp.emulator.cell_at(0, 0).unwrap().character, 'r');
    assert_eq!(comp.emulator.cell_at(0, 4).unwrap().character, '@');

    // Type a command
    comp.handle_keystroke(&key("l"));
    comp.handle_keystroke(&key("s"));
    comp.handle_keystroke(&special_key("enter"));
    let pending = comp.take_pending_input();
    assert_eq!(pending, b"ls\r");

    // Resize
    comp.resize(50, 120);
    assert_eq!(comp.grid_dimensions(), (50, 120));

    // Scrollback
    comp.state.scroll_up(10);
    assert!(comp.state.is_scrolled_up());

    // Disconnect
    comp.state.disconnect();
    assert!(!comp.state.is_connected());
}

// ---------------------------------------------------------------------------
// Theme color mapping tests
// ---------------------------------------------------------------------------

#[test]
fn test_component_with_light_theme() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::light());
    // Just verify construction succeeds with light theme.
    assert_eq!(comp.grid_dimensions(), (24, 80));
}

#[test]
fn test_component_with_dark_theme() {
    let state = TerminalViewState::for_local_shell();
    let comp = TerminalViewComponent::new(state, Theme::dark());
    assert_eq!(comp.grid_dimensions(), (24, 80));
}
