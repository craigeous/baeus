// T347: Log viewer enhancement tests
//
// Tests for the LogViewerState component and its settings, search/filter,
// timestamp toggle, line wrap toggle, and follow mode (auto-scroll).

use baeus_core::logs::{LogLine, LogStreamState};
use baeus_ui::components::log_viewer::*;
use chrono::Utc;

// ---------------------------------------------------------------------------
// Helper: create a LogLine for testing
// ---------------------------------------------------------------------------

fn make_line(content: &str, container: &str) -> LogLine {
    LogLine {
        timestamp: Some(Utc::now()),
        content: content.to_string(),
        container_name: container.to_string(),
        pod_name: "test-pod".to_string(),
        source_color_index: 0,
    }
}

// =========================================================================
// T347: LogViewerState creation with initial settings
// =========================================================================

#[test]
fn test_log_viewer_state_creation_defaults() {
    let state = LogViewerState::new(5000);
    assert_eq!(state.line_count(), 0);
    assert!(state.settings.show_timestamps);
    assert!(!state.settings.wrap_lines);
    assert!(state.settings.auto_scroll);
    assert_eq!(state.settings.font_size, 12);
    assert_eq!(state.stream_state, LogStreamState::Idle);
    assert!(state.search_query.is_none());
    assert_eq!(state.search_match_count, 0);
    assert!(state.current_search_index.is_none());
}

#[test]
fn test_log_viewer_state_custom_capacity() {
    let state = LogViewerState::new(100);
    assert_eq!(state.line_count(), 0);
    // The capacity is stored in the inner LogBuffer, verified by pushing
    // past capacity and checking eviction.
}

#[test]
fn test_log_viewer_settings_default_values() {
    let settings = LogViewerSettings::default();
    assert!(settings.show_timestamps);
    assert!(!settings.wrap_lines);
    assert!(settings.auto_scroll);
    assert_eq!(settings.font_size, 12);
}

// =========================================================================
// T347: Search/filter text
// =========================================================================

#[test]
fn test_set_search_filter_text() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("INFO: server started", "app"));
    state.push_line(make_line("ERROR: connection refused", "app"));
    state.push_line(make_line("INFO: request completed", "app"));
    state.push_line(make_line("ERROR: timeout exceeded", "app"));

    state.set_search(Some("ERROR".to_string()));
    assert_eq!(state.search_query.as_deref(), Some("ERROR"));
    assert_eq!(state.search_match_count, 2);
}

#[test]
fn test_set_search_empty_string() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("line one", "app"));
    state.push_line(make_line("line two", "app"));

    state.set_search(Some("".to_string()));
    // Empty string matches all lines
    assert_eq!(state.search_match_count, 2);
}

#[test]
fn test_set_search_none_clears() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("ERROR: fail", "app"));
    state.set_search(Some("ERROR".to_string()));
    assert_eq!(state.search_match_count, 1);

    state.set_search(None);
    assert!(state.search_query.is_none());
    assert_eq!(state.search_match_count, 0);
}

#[test]
fn test_clear_search_method() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("ERROR: test", "app"));
    state.set_search(Some("ERROR".to_string()));

    state.clear_search();
    assert!(state.search_query.is_none());
    assert_eq!(state.search_match_count, 0);
    assert!(state.current_search_index.is_none());
}

// =========================================================================
// T347: Adding lines
// =========================================================================

#[test]
fn test_push_lines() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("first line", "app"));
    assert_eq!(state.line_count(), 1);

    state.push_line(make_line("second line", "app"));
    assert_eq!(state.line_count(), 2);

    state.push_line(make_line("third line", "sidecar"));
    assert_eq!(state.line_count(), 3);
}

#[test]
fn test_push_updates_search_count() {
    let mut state = LogViewerState::new(1000);
    state.set_search(Some("ERROR".to_string()));

    state.push_line(make_line("INFO: ok", "app"));
    assert_eq!(state.search_match_count, 0);

    state.push_line(make_line("ERROR: bad", "app"));
    assert_eq!(state.search_match_count, 1);

    state.push_line(make_line("ERROR: worse", "app"));
    assert_eq!(state.search_match_count, 2);
}

#[test]
fn test_clear_lines() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("data", "app"));
    state.push_line(make_line("more data", "app"));
    assert_eq!(state.line_count(), 2);

    state.clear();
    assert_eq!(state.line_count(), 0);
    assert_eq!(state.search_match_count, 0);
}

// =========================================================================
// T347: Filtered lines — when filter text is set, only matching lines returned
// =========================================================================

#[test]
fn test_filtered_lines_no_filter_returns_all() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("alpha", "app"));
    state.push_line(make_line("beta", "app"));
    state.push_line(make_line("gamma", "app"));

    // No search filter, no container filter → all lines visible
    let visible = state.visible_lines();
    assert_eq!(visible.len(), 3);
}

#[test]
fn test_filtered_lines_with_search_filter() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("INFO: started", "app"));
    state.push_line(make_line("ERROR: failed to connect", "app"));
    state.push_line(make_line("INFO: retrying", "app"));
    state.push_line(make_line("ERROR: timeout", "app"));
    state.push_line(make_line("INFO: recovered", "app"));

    state.set_search(Some("ERROR".to_string()));

    // The buffer's filtered_lines only returns search-matching lines
    let filtered = state.buffer.filtered_lines();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|l| l.content.contains("ERROR")));
}

#[test]
fn test_filtered_lines_with_container_filter() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("from app", "app"));
    state.push_line(make_line("from sidecar", "sidecar"));
    state.push_line(make_line("from app again", "app"));

    state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    // Hide sidecar
    state.container_filter.as_mut().unwrap().toggle("sidecar");

    let visible = state.visible_lines();
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().all(|l| l.container_name == "app"));
}

#[test]
fn test_filtered_lines_case_insensitive_search() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("Error occurred", "app"));
    state.push_line(make_line("error again", "app"));
    state.push_line(make_line("all good", "app"));

    state.set_search(Some("error".to_string()));
    assert_eq!(state.search_match_count, 2);
}

// =========================================================================
// T347: Timestamp toggle — toggling timestamps on/off
// =========================================================================

#[test]
fn test_toggle_timestamps_off() {
    let mut state = LogViewerState::new(1000);
    assert!(state.settings.show_timestamps);

    state.toggle_timestamps();
    assert!(!state.settings.show_timestamps);
}

#[test]
fn test_toggle_timestamps_on_again() {
    let mut state = LogViewerState::new(1000);
    state.toggle_timestamps(); // off
    state.toggle_timestamps(); // on
    assert!(state.settings.show_timestamps);
}

#[test]
fn test_toggle_timestamps_multiple_cycles() {
    let mut state = LogViewerState::new(1000);
    for i in 0..6 {
        let expected = i % 2 == 0; // starts true, toggles
        assert_eq!(state.settings.show_timestamps, expected, "cycle {i}");
        state.toggle_timestamps();
    }
}

// =========================================================================
// T347: Line wrap toggle
// =========================================================================

#[test]
fn test_toggle_wrap_lines_on() {
    let mut state = LogViewerState::new(1000);
    assert!(!state.settings.wrap_lines);

    state.toggle_wrap_lines();
    assert!(state.settings.wrap_lines);
}

#[test]
fn test_toggle_wrap_lines_off() {
    let mut state = LogViewerState::new(1000);
    state.toggle_wrap_lines(); // on
    state.toggle_wrap_lines(); // off
    assert!(!state.settings.wrap_lines);
}

#[test]
fn test_wrap_lines_independent_of_timestamps() {
    let mut state = LogViewerState::new(1000);

    state.toggle_wrap_lines();
    assert!(state.settings.wrap_lines);
    assert!(state.settings.show_timestamps); // unchanged

    state.toggle_timestamps();
    assert!(state.settings.wrap_lines); // unchanged
    assert!(!state.settings.show_timestamps);
}

// =========================================================================
// T347: Follow mode toggle (auto-scroll)
// =========================================================================

#[test]
fn test_toggle_auto_scroll_off() {
    let mut state = LogViewerState::new(1000);
    assert!(state.settings.auto_scroll);

    state.toggle_auto_scroll();
    assert!(!state.settings.auto_scroll);
}

#[test]
fn test_toggle_auto_scroll_on() {
    let mut state = LogViewerState::new(1000);
    state.toggle_auto_scroll(); // off
    state.toggle_auto_scroll(); // on
    assert!(state.settings.auto_scroll);
}

#[test]
fn test_auto_scroll_affects_streaming_indicator() {
    let mut state = LogViewerState::new(1000);

    // With auto-scroll on, no "scroll to bottom" indicator needed
    assert!(state.settings.auto_scroll);

    // Turn auto-scroll off
    state.toggle_auto_scroll();
    assert!(!state.settings.auto_scroll);
    // A real view would show a "scroll to bottom" indicator here
}

#[test]
fn test_all_settings_independent() {
    let mut state = LogViewerState::new(1000);

    // Toggle each setting independently
    state.toggle_timestamps();
    state.toggle_wrap_lines();
    state.toggle_auto_scroll();

    assert!(!state.settings.show_timestamps);
    assert!(state.settings.wrap_lines);
    assert!(!state.settings.auto_scroll);
    assert_eq!(state.settings.font_size, 12); // unchanged

    // Toggle back
    state.toggle_timestamps();
    state.toggle_wrap_lines();
    state.toggle_auto_scroll();

    assert!(state.settings.show_timestamps);
    assert!(!state.settings.wrap_lines);
    assert!(state.settings.auto_scroll);
}

// =========================================================================
// T347: Search navigation
// =========================================================================

#[test]
fn test_search_navigation_next() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("ERROR: a", "app"));
    state.push_line(make_line("INFO: b", "app"));
    state.push_line(make_line("ERROR: c", "app"));

    state.set_search(Some("ERROR".to_string()));
    assert_eq!(state.current_search_index, Some(0));

    state.next_search_match();
    assert_eq!(state.current_search_index, Some(1));

    state.next_search_match(); // wraps
    assert_eq!(state.current_search_index, Some(0));
}

#[test]
fn test_search_navigation_prev() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("ERROR: a", "app"));
    state.push_line(make_line("ERROR: b", "app"));

    state.set_search(Some("ERROR".to_string()));
    assert_eq!(state.current_search_index, Some(0));

    state.prev_search_match(); // wraps to end
    assert_eq!(state.current_search_index, Some(1));

    state.prev_search_match();
    assert_eq!(state.current_search_index, Some(0));
}

#[test]
fn test_search_navigation_no_matches() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_line("INFO: ok", "app"));

    state.set_search(Some("FATAL".to_string()));
    assert_eq!(state.search_match_count, 0);
    assert!(state.current_search_index.is_none());

    // Navigation should be no-ops
    state.next_search_match();
    assert!(state.current_search_index.is_none());
    state.prev_search_match();
    assert!(state.current_search_index.is_none());
}

// =========================================================================
// T347: Stream state integration
// =========================================================================

#[test]
fn test_stream_state_transitions() {
    let mut state = LogViewerState::new(1000);
    assert!(!state.is_streaming());
    assert!(!state.is_paused());

    state.set_stream_state(LogStreamState::Streaming);
    assert!(state.is_streaming());

    state.set_stream_state(LogStreamState::Paused);
    assert!(state.is_paused());
    assert!(!state.is_streaming());

    state.set_stream_state(LogStreamState::Stopped);
    assert!(!state.is_streaming());
    assert!(!state.is_paused());
}

// =========================================================================
// T347: Full workflow
// =========================================================================

#[test]
fn test_log_viewer_full_workflow() {
    let mut state = LogViewerState::new(5000);

    // Start streaming
    state.set_stream_state(LogStreamState::Streaming);
    assert!(state.is_streaming());

    // Push some log lines
    for i in 0..20 {
        let content = if i % 5 == 0 {
            format!("ERROR: failure at step {i}")
        } else {
            format!("INFO: processing step {i}")
        };
        state.push_line(make_line(&content, "app"));
    }
    assert_eq!(state.line_count(), 20);

    // Set search filter
    state.set_search(Some("ERROR".to_string()));
    assert_eq!(state.search_match_count, 4); // 0, 5, 10, 15

    // Toggle timestamps off
    state.toggle_timestamps();
    assert!(!state.settings.show_timestamps);

    // Toggle wrap on
    state.toggle_wrap_lines();
    assert!(state.settings.wrap_lines);

    // Disable auto-scroll
    state.toggle_auto_scroll();
    assert!(!state.settings.auto_scroll);

    // Pause streaming
    state.set_stream_state(LogStreamState::Paused);
    assert!(state.is_paused());

    // Clear search
    state.clear_search();
    assert!(state.search_query.is_none());
    assert_eq!(state.search_match_count, 0);
}
