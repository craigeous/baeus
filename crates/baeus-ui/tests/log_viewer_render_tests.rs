// T044: Render tests for LogViewer (state-level, no GPUI window needed).
//
// Verifies:
// - Log entries render with timestamps and level colors
// - Auto-scroll state tracking
// - Search highlighting marks the right entries
// - Container filter shows/hides entries
// - Stream state renders correct status indicator
// - Download format selection works
// - Settings toggles (timestamps, wrap, auto-scroll) work

use baeus_core::logs::{LogDownloadFormat, LogLine, LogStreamState};
use baeus_ui::components::log_viewer::{LogDownloadState, LogViewerState, LogViewerView};
use baeus_ui::theme::Theme;
use chrono::Utc;

fn make_line(content: &str, container: &str) -> LogLine {
    LogLine {
        timestamp: Some(Utc::now()),
        content: content.to_string(),
        container_name: container.to_string(),
        pod_name: "test-pod".to_string(),
        source_color_index: 0,
    }
}

fn make_line_with_color(content: &str, container: &str, color_idx: usize) -> LogLine {
    LogLine {
        timestamp: Some(Utc::now()),
        content: content.to_string(),
        container_name: container.to_string(),
        pod_name: "test-pod".to_string(),
        source_color_index: color_idx,
    }
}

fn make_view() -> LogViewerView {
    let state = LogViewerState::new(1000);
    LogViewerView::new(state, Theme::dark())
}

// ========================================================================
// Level color mapping
// ========================================================================

#[test]
fn test_error_line_uses_error_color() {
    let view = make_view();
    let line = make_line("ERROR: something broke", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.error);
}

#[test]
fn test_error_lowercase_uses_error_color() {
    let view = make_view();
    let line = make_line("error: something broke", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.error);
}

#[test]
fn test_warn_line_uses_warning_color() {
    let view = make_view();
    let line = make_line("WARN: disk usage high", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_warn_lowercase_uses_warning_color() {
    let view = make_view();
    let line = make_line("warning: check this", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_info_line_uses_info_color() {
    let view = make_view();
    let line = make_line("INFO: server started", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.info);
}

#[test]
fn test_info_lowercase_uses_info_color() {
    let view = make_view();
    let line = make_line("info: connected", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.info);
}

#[test]
fn test_plain_line_uses_primary_color() {
    let view = make_view();
    let line = make_line("just a plain message", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::dark().colors.text_primary);
}

#[test]
fn test_level_color_with_light_theme() {
    let state = LogViewerState::new(1000);
    let view = LogViewerView::new(state, Theme::light());
    let line = make_line("ERROR: fail", "app");
    let color = view.level_color_for_line(&line);
    assert_eq!(color, Theme::light().colors.error);
}

// ========================================================================
// Auto-scroll state tracking
// ========================================================================

#[test]
fn test_auto_scroll_on_by_default() {
    let view = make_view();
    assert!(view.state.settings.auto_scroll);
    assert!(!view.show_scroll_to_bottom_indicator());
}

#[test]
fn test_auto_scroll_off_shows_indicator() {
    let mut view = make_view();
    view.state.toggle_auto_scroll();
    assert!(!view.state.settings.auto_scroll);
    assert!(view.show_scroll_to_bottom_indicator());
}

#[test]
fn test_auto_scroll_toggle_back_hides_indicator() {
    let mut view = make_view();
    view.state.toggle_auto_scroll();
    view.state.toggle_auto_scroll();
    assert!(view.state.settings.auto_scroll);
    assert!(!view.show_scroll_to_bottom_indicator());
}

// ========================================================================
// Search highlighting
// ========================================================================

#[test]
fn test_search_match_on_matching_line() {
    let mut view = make_view();
    let line = make_line("ERROR: timeout", "app");
    view.state.push_line(line.clone());
    view.state.set_search(Some("ERROR".to_string()));
    assert!(view.line_matches_search(&line));
}

#[test]
fn test_search_no_match_on_non_matching_line() {
    let mut view = make_view();
    let line = make_line("INFO: all good", "app");
    view.state.push_line(line.clone());
    view.state.set_search(Some("ERROR".to_string()));
    assert!(!view.line_matches_search(&line));
}

#[test]
fn test_search_match_case_insensitive() {
    let mut view = make_view();
    let line = make_line("Error: mixed case", "app");
    view.state.push_line(line.clone());
    view.state.set_search(Some("error".to_string()));
    assert!(view.line_matches_search(&line));
}

#[test]
fn test_no_search_no_match() {
    let view = make_view();
    let line = make_line("anything", "app");
    assert!(!view.line_matches_search(&line));
}

#[test]
fn test_search_empty_query_matches_all() {
    let mut view = make_view();
    let line = make_line("anything", "app");
    view.state.set_search(Some("".to_string()));
    assert!(view.line_matches_search(&line));
}

#[test]
fn test_search_match_count_accurate() {
    let mut view = make_view();
    view.state.push_line(make_line("ERROR: a", "app"));
    view.state.push_line(make_line("INFO: b", "app"));
    view.state.push_line(make_line("ERROR: c", "app"));
    view.state.set_search(Some("ERROR".to_string()));
    assert_eq!(view.state.search_match_count, 2);
    assert_eq!(view.state.current_search_index, Some(0));
}

#[test]
fn test_search_navigation_updates_index() {
    let mut view = make_view();
    view.state.push_line(make_line("ERROR: a", "app"));
    view.state.push_line(make_line("ERROR: b", "app"));
    view.state.set_search(Some("ERROR".to_string()));
    assert_eq!(view.state.current_search_index, Some(0));

    view.state.next_search_match();
    assert_eq!(view.state.current_search_index, Some(1));

    view.state.next_search_match(); // wraps
    assert_eq!(view.state.current_search_index, Some(0));
}

// ========================================================================
// Container filter shows/hides entries
// ========================================================================

#[test]
fn test_no_filter_shows_all_lines() {
    let mut view = make_view();
    view.state.push_line(make_line("a", "app"));
    view.state.push_line(make_line("b", "sidecar"));
    assert_eq!(view.state.visible_lines().len(), 2);
}

#[test]
fn test_filter_hides_container() {
    let mut view = make_view();
    view.state.push_line(make_line("a", "app"));
    view.state.push_line(make_line("b", "sidecar"));
    view.state.push_line(make_line("c", "app"));

    view.state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    view.state.container_filter.as_mut().unwrap().toggle("sidecar");

    let visible = view.state.visible_lines();
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().all(|l| l.container_name == "app"));
}

#[test]
fn test_filter_show_all_restores() {
    let mut view = make_view();
    view.state.push_line(make_line("a", "app"));
    view.state.push_line(make_line("b", "sidecar"));

    view.state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    view.state.container_filter.as_mut().unwrap().toggle("sidecar");
    assert_eq!(view.state.visible_lines().len(), 1);

    view.state.container_filter.as_mut().unwrap().show_all();
    assert_eq!(view.state.visible_lines().len(), 2);
}

#[test]
fn test_filter_hide_all_shows_nothing() {
    let mut view = make_view();
    view.state.push_line(make_line("a", "app"));
    view.state.push_line(make_line("b", "sidecar"));

    view.state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    view.state.container_filter.as_mut().unwrap().hide_all();
    assert_eq!(view.state.visible_lines().len(), 0);
}

// ========================================================================
// Stream state renders correct status indicator
// ========================================================================

#[test]
fn test_idle_stream_state_label() {
    let view = make_view();
    assert_eq!(view.stream_state_label(), "Idle");
}

#[test]
fn test_streaming_state_label() {
    let mut view = make_view();
    view.start_streaming();
    assert_eq!(view.stream_state_label(), "Streaming");
}

#[test]
fn test_paused_state_label() {
    let mut view = make_view();
    view.start_streaming();
    view.pause_streaming();
    assert_eq!(view.stream_state_label(), "Paused");
}

#[test]
fn test_stopped_state_label() {
    let mut view = make_view();
    view.start_streaming();
    view.stop_streaming();
    assert_eq!(view.stream_state_label(), "Stopped");
}

#[test]
fn test_error_state_label() {
    let mut view = make_view();
    view.state.set_stream_state(LogStreamState::Error);
    assert_eq!(view.stream_state_label(), "Error");
}

#[test]
fn test_streaming_state_color_is_success() {
    let mut view = make_view();
    view.start_streaming();
    assert_eq!(view.stream_state_color(), Theme::dark().colors.success,);
}

#[test]
fn test_paused_state_color_is_warning() {
    let mut view = make_view();
    view.pause_streaming();
    assert_eq!(view.stream_state_color(), Theme::dark().colors.warning,);
}

#[test]
fn test_stopped_state_color_is_text_secondary() {
    let mut view = make_view();
    view.stop_streaming();
    assert_eq!(view.stream_state_color(), Theme::dark().colors.text_secondary,);
}

#[test]
fn test_error_state_color_is_error() {
    let mut view = make_view();
    view.state.set_stream_state(LogStreamState::Error);
    assert_eq!(view.stream_state_color(), Theme::dark().colors.error,);
}

#[test]
fn test_idle_state_color_is_text_muted() {
    let view = make_view();
    assert_eq!(view.stream_state_color(), Theme::dark().colors.text_muted,);
}

// ========================================================================
// Download format selection
// ========================================================================

#[test]
fn test_default_download_format_is_plain_text() {
    let view = make_view();
    assert!(matches!(view.state.download_format, LogDownloadFormat::PlainText));
}

#[test]
fn test_set_download_format_json() {
    let mut view = make_view();
    view.state.set_download_format(LogDownloadFormat::Json);
    assert!(matches!(view.state.download_format, LogDownloadFormat::Json));
}

#[test]
fn test_set_download_format_csv() {
    let mut view = make_view();
    view.state.set_download_format(LogDownloadFormat::Csv);
    assert!(matches!(view.state.download_format, LogDownloadFormat::Csv));
}

#[test]
fn test_download_state_starts_idle() {
    let view = make_view();
    assert_eq!(view.state.download_state, LogDownloadState::Idle);
}

#[test]
fn test_prepare_download_returns_content() {
    let mut view = make_view();
    view.state.push_line(make_line("hello world", "app"));
    let result = view.state.prepare_download();
    assert!(result.contains("hello world"));
}

#[test]
fn test_prepare_download_transitions_to_ready() {
    let mut view = make_view();
    view.state.push_line(make_line("test", "app"));
    view.state.prepare_download();
    assert!(matches!(view.state.download_state, LogDownloadState::Ready(_)));
}

// ========================================================================
// Settings toggles
// ========================================================================

#[test]
fn test_timestamps_on_by_default() {
    let view = make_view();
    assert!(view.state.settings.show_timestamps);
}

#[test]
fn test_toggle_timestamps_off() {
    let mut view = make_view();
    view.state.toggle_timestamps();
    assert!(!view.state.settings.show_timestamps);
}

#[test]
fn test_toggle_timestamps_back_on() {
    let mut view = make_view();
    view.state.toggle_timestamps();
    view.state.toggle_timestamps();
    assert!(view.state.settings.show_timestamps);
}

#[test]
fn test_wrap_off_by_default() {
    let view = make_view();
    assert!(!view.state.settings.wrap_lines);
}

#[test]
fn test_toggle_wrap_on() {
    let mut view = make_view();
    view.state.toggle_wrap_lines();
    assert!(view.state.settings.wrap_lines);
}

#[test]
fn test_toggle_auto_scroll_off() {
    let mut view = make_view();
    view.state.toggle_auto_scroll();
    assert!(!view.state.settings.auto_scroll);
}

// ========================================================================
// T047: Streaming integration methods
// ========================================================================

#[test]
fn test_start_streaming_sets_state() {
    let mut view = make_view();
    view.start_streaming();
    assert_eq!(view.state.stream_state, LogStreamState::Streaming);
}

#[test]
fn test_pause_streaming_sets_state() {
    let mut view = make_view();
    view.start_streaming();
    view.pause_streaming();
    assert_eq!(view.state.stream_state, LogStreamState::Paused);
}

#[test]
fn test_stop_streaming_sets_state() {
    let mut view = make_view();
    view.start_streaming();
    view.stop_streaming();
    assert_eq!(view.state.stream_state, LogStreamState::Stopped);
}

#[test]
fn test_push_line_via_view() {
    let mut view = make_view();
    view.push_line(make_line("pushed via view", "app"));
    assert_eq!(view.state.line_count(), 1);
    let lines = view.state.visible_lines();
    assert_eq!(lines[0].content, "pushed via view");
}

#[test]
fn test_streaming_then_push_lines() {
    let mut view = make_view();
    view.start_streaming();
    for i in 0..5 {
        view.push_line(make_line(&format!("line {i}"), "app"));
    }
    assert_eq!(view.state.line_count(), 5);
    assert!(view.state.is_streaming());
}

// ========================================================================
// T048: Container selector
// ========================================================================

#[test]
fn test_container_dropdown_closed_by_default() {
    let view = make_view();
    assert!(!view.container_dropdown_open);
}

#[test]
fn test_container_dropdown_can_be_opened() {
    let mut view = make_view();
    view.container_dropdown_open = true;
    assert!(view.container_dropdown_open);
}

#[test]
fn test_container_color_returns_distinct_colors() {
    let c0 = LogViewerView::container_color(0);
    let c1 = LogViewerView::container_color(1);
    let c2 = LogViewerView::container_color(2);
    assert_ne!(c0, c1);
    assert_ne!(c1, c2);
    assert_ne!(c0, c2);
}

#[test]
fn test_container_color_wraps_around() {
    let c0 = LogViewerView::container_color(0);
    let c8 = LogViewerView::container_color(8);
    assert_eq!(c0, c8);
}

#[test]
fn test_multi_container_visible_lines_with_filter() {
    let mut view = make_view();
    view.state.push_line(make_line_with_color("nginx log 1", "nginx", 0));
    view.state.push_line(make_line_with_color("istio log 1", "istio-proxy", 1));
    view.state.push_line(make_line_with_color("nginx log 2", "nginx", 0));

    view.state.set_container_filter(vec!["nginx".to_string(), "istio-proxy".to_string()]);

    // Both visible
    assert_eq!(view.state.visible_lines().len(), 3);

    // Toggle off istio-proxy
    view.state.container_filter.as_mut().unwrap().toggle("istio-proxy");
    let visible = view.state.visible_lines();
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().all(|l| l.container_name == "nginx"));
}

#[test]
fn test_container_selector_filter_count() {
    let mut view = make_view();
    view.state.set_container_filter(vec![
        "app".to_string(),
        "sidecar".to_string(),
        "init".to_string(),
    ]);
    let filter = view.state.container_filter.as_ref().unwrap();
    assert_eq!(filter.total_count(), 3);
    assert_eq!(filter.visible_count(), 3);
}

#[test]
fn test_container_selector_toggle_reduces_visible() {
    let mut view = make_view();
    view.state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    view.state.container_filter.as_mut().unwrap().toggle("sidecar");
    let filter = view.state.container_filter.as_ref().unwrap();
    assert_eq!(filter.visible_count(), 1);
    assert!(filter.is_visible("app"));
    assert!(!filter.is_visible("sidecar"));
}

#[test]
fn test_source_color_index_preserved_in_visible_lines() {
    let mut view = make_view();
    view.state.push_line(make_line_with_color("from app", "app", 0));
    view.state.push_line(make_line_with_color("from sidecar", "sidecar", 1));

    let lines = view.state.visible_lines();
    assert_eq!(lines[0].source_color_index, 0);
    assert_eq!(lines[1].source_color_index, 1);
}

// ========================================================================
// Full workflow test
// ========================================================================

#[test]
fn test_full_log_viewer_workflow() {
    let mut view = make_view();

    // Start streaming
    view.start_streaming();
    assert_eq!(view.stream_state_label(), "Streaming");

    // Set up containers
    view.state.set_container_filter(vec!["nginx".to_string(), "istio-proxy".to_string()]);

    // Push log lines
    view.push_line(make_line_with_color("INFO: nginx started", "nginx", 0));
    view.push_line(make_line_with_color("ERROR: istio failed", "istio-proxy", 1));
    view.push_line(make_line_with_color("WARN: nginx slow", "nginx", 0));

    assert_eq!(view.state.line_count(), 3);

    // Search for errors
    view.state.set_search(Some("ERROR".to_string()));
    assert_eq!(view.state.search_match_count, 1);

    // Verify level colors
    let error_line = make_line("ERROR: test", "app");
    let warn_line = make_line("WARN: test", "app");
    let info_line = make_line("INFO: test", "app");

    assert_eq!(view.level_color_for_line(&error_line), Theme::dark().colors.error,);
    assert_eq!(view.level_color_for_line(&warn_line), Theme::dark().colors.warning,);
    assert_eq!(view.level_color_for_line(&info_line), Theme::dark().colors.info,);

    // Pause
    view.pause_streaming();
    assert_eq!(view.stream_state_label(), "Paused");

    // Toggle settings
    view.state.toggle_timestamps();
    assert!(!view.state.settings.show_timestamps);

    // Set download format
    view.state.set_download_format(LogDownloadFormat::Json);
    assert!(matches!(view.state.download_format, LogDownloadFormat::Json,));

    // Stop
    view.stop_streaming();
    assert_eq!(view.stream_state_label(), "Stopped");
}

#[test]
fn test_view_new_defaults() {
    let view = make_view();
    assert_eq!(view.state.stream_state, LogStreamState::Idle);
    assert_eq!(view.state.line_count(), 0);
    assert!(view.state.search_query.is_none());
    assert!(view.state.container_filter.is_none());
    assert!(!view.container_dropdown_open);
    assert!(view.state.settings.show_timestamps);
    assert!(!view.state.settings.wrap_lines);
    assert!(view.state.settings.auto_scroll);
}
