// T053: Log download/export integration tests.
// Tests format_logs_for_download, LogDownloadState, and LogViewerState::prepare_download().

use baeus_core::logs::{LogDownloadFormat, LogLine, format_logs_for_download};
use baeus_ui::components::log_viewer::{LogDownloadState, LogViewerState};
use chrono::Utc;

fn make_log_line(content: &str, container: &str, pod: &str) -> LogLine {
    LogLine {
        timestamp: Some(Utc::now()),
        content: content.to_string(),
        container_name: container.to_string(),
        pod_name: pod.to_string(),
        source_color_index: 0,
    }
}

fn make_log_line_no_timestamp(content: &str, container: &str, pod: &str) -> LogLine {
    LogLine {
        timestamp: None,
        content: content.to_string(),
        container_name: container.to_string(),
        pod_name: pod.to_string(),
        source_color_index: 0,
    }
}

// ========================================================================
// format_logs_for_download: PlainText
// ========================================================================

#[test]
fn test_format_plain_text_single_line() {
    let line = make_log_line("hello world", "app", "pod-1");
    let result = format_logs_for_download(&[line], LogDownloadFormat::PlainText);
    assert!(result.contains("[pod-1]"));
    assert!(result.contains("[app]"));
    assert!(result.contains("hello world"));
}

#[test]
fn test_format_plain_text_multiple_lines() {
    let lines = vec![
        make_log_line("line one", "app", "pod-1"),
        make_log_line("line two", "sidecar", "pod-1"),
    ];
    let result = format_logs_for_download(&lines, LogDownloadFormat::PlainText);
    let output_lines: Vec<&str> = result.lines().collect();
    assert_eq!(output_lines.len(), 2);
    assert!(output_lines[0].contains("line one"));
    assert!(output_lines[1].contains("line two"));
}

#[test]
fn test_format_plain_text_no_timestamp() {
    let line = make_log_line_no_timestamp("no ts", "app", "pod-1");
    let result = format_logs_for_download(&[line], LogDownloadFormat::PlainText);
    assert!(result.contains("no ts"));
    assert!(result.contains("[pod-1]"));
}

#[test]
fn test_format_plain_text_empty() {
    let result = format_logs_for_download(&[], LogDownloadFormat::PlainText);
    assert!(result.is_empty());
}

// ========================================================================
// format_logs_for_download: Json
// ========================================================================

#[test]
fn test_format_json_single_line() {
    let line = make_log_line("json test", "app", "pod-1");
    let result = format_logs_for_download(&[line], LogDownloadFormat::Json);
    let parsed: Vec<LogLine> = serde_json::from_str(&result).expect("Valid JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].content, "json test");
    assert_eq!(parsed[0].pod_name, "pod-1");
}

#[test]
fn test_format_json_empty() {
    let result = format_logs_for_download(&[], LogDownloadFormat::Json);
    assert_eq!(result.trim(), "[]");
}

#[test]
fn test_format_json_multiple_lines() {
    let lines =
        vec![make_log_line("first", "app", "pod-1"), make_log_line("second", "sidecar", "pod-2")];
    let result = format_logs_for_download(&lines, LogDownloadFormat::Json);
    let parsed: Vec<LogLine> = serde_json::from_str(&result).expect("Valid JSON");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].content, "first");
    assert_eq!(parsed[1].content, "second");
}

// ========================================================================
// format_logs_for_download: Csv
// ========================================================================

#[test]
fn test_format_csv_header_only_when_empty() {
    let result = format_logs_for_download(&[], LogDownloadFormat::Csv);
    assert!(result.starts_with("timestamp,pod_name,container_name,content\n"));
    // Header + no data rows
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn test_format_csv_with_single_line() {
    let line = make_log_line_no_timestamp("csv data", "app", "pod-1");
    let result = format_logs_for_download(&[line], LogDownloadFormat::Csv);
    let csv_lines: Vec<&str> = result.lines().collect();
    assert_eq!(csv_lines.len(), 2); // header + 1 data row
    assert!(csv_lines[0].starts_with("timestamp,"));
    assert!(csv_lines[1].contains("pod-1"));
    assert!(csv_lines[1].contains("app"));
    assert!(csv_lines[1].contains("csv data"));
}

#[test]
fn test_format_csv_escapes_quotes_in_content() {
    let line = LogLine {
        timestamp: None,
        content: r#"said "hello" there"#.to_string(),
        container_name: "app".to_string(),
        pod_name: "pod-1".to_string(),
        source_color_index: 0,
    };
    let result = format_logs_for_download(&[line], LogDownloadFormat::Csv);
    // Quotes should be doubled for CSV escaping
    assert!(result.contains(r#"said ""hello"" there"#));
}

#[test]
fn test_format_csv_multiple_lines() {
    let lines =
        vec![make_log_line("first", "app", "pod-1"), make_log_line("second", "sidecar", "pod-2")];
    let result = format_logs_for_download(&lines, LogDownloadFormat::Csv);
    let csv_lines: Vec<&str> = result.lines().collect();
    assert_eq!(csv_lines.len(), 3); // header + 2 data rows
}

// ========================================================================
// LogDownloadState
// ========================================================================

#[test]
fn test_download_state_idle_default() {
    let state = LogViewerState::new(1000);
    assert_eq!(state.download_state, LogDownloadState::Idle);
}

#[test]
fn test_download_state_variants_equality() {
    assert_eq!(LogDownloadState::Idle, LogDownloadState::Idle);
    assert_eq!(LogDownloadState::Preparing, LogDownloadState::Preparing);
    assert_ne!(LogDownloadState::Idle, LogDownloadState::Preparing);
    assert_ne!(LogDownloadState::Ready("a".to_string()), LogDownloadState::Ready("b".to_string()));
    assert_eq!(
        LogDownloadState::Error("err".to_string()),
        LogDownloadState::Error("err".to_string())
    );
}

#[test]
fn test_download_state_clone() {
    let state = LogDownloadState::Ready("content".to_string());
    let cloned = state.clone();
    assert_eq!(state, cloned);
}

// ========================================================================
// LogViewerState::prepare_download()
// ========================================================================

#[test]
fn test_prepare_download_empty_buffer_plain_text() {
    let mut state = LogViewerState::new(1000);
    let result = state.prepare_download();
    assert!(result.is_empty());
    assert!(matches!(state.download_state, LogDownloadState::Ready(ref s) if s.is_empty()));
}

#[test]
fn test_prepare_download_empty_buffer_csv() {
    let mut state = LogViewerState::new(1000);
    state.set_download_format(LogDownloadFormat::Csv);
    let result = state.prepare_download();
    // CSV should still have a header
    assert!(result.starts_with("timestamp,"));
}

#[test]
fn test_prepare_download_empty_buffer_json() {
    let mut state = LogViewerState::new(1000);
    state.set_download_format(LogDownloadFormat::Json);
    let result = state.prepare_download();
    assert_eq!(result.trim(), "[]");
}

#[test]
fn test_prepare_download_with_lines_plain_text() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("INFO: started", "app", "pod-1"));
    state.push_line(make_log_line("ERROR: failed", "app", "pod-1"));

    let result = state.prepare_download();
    assert!(result.contains("INFO: started"));
    assert!(result.contains("ERROR: failed"));
    assert!(matches!(state.download_state, LogDownloadState::Ready(_)));
}

#[test]
fn test_prepare_download_with_lines_json() {
    let mut state = LogViewerState::new(1000);
    state.set_download_format(LogDownloadFormat::Json);
    state.push_line(make_log_line("json line", "app", "pod-1"));

    let result = state.prepare_download();
    let parsed: Vec<LogLine> = serde_json::from_str(&result).expect("Valid JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].content, "json line");
}

#[test]
fn test_prepare_download_with_lines_csv() {
    let mut state = LogViewerState::new(1000);
    state.set_download_format(LogDownloadFormat::Csv);
    state.push_line(make_log_line("csv line", "app", "pod-1"));

    let result = state.prepare_download();
    let csv_lines: Vec<&str> = result.lines().collect();
    assert_eq!(csv_lines.len(), 2); // header + 1 data
    assert!(csv_lines[1].contains("csv line"));
}

#[test]
fn test_prepare_download_respects_container_filter() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("from app", "app", "pod-1"));
    state.push_line(make_log_line("from sidecar", "sidecar", "pod-1"));
    state.push_line(make_log_line("from app again", "app", "pod-1"));

    // Filter to show only "app" container
    state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
    state.container_filter.as_mut().unwrap().toggle("sidecar");

    let result = state.prepare_download();
    // Only 2 app lines should be in the output
    let output_lines: Vec<&str> = result.lines().collect();
    assert_eq!(output_lines.len(), 2);
    assert!(output_lines.iter().all(|l| l.contains("[app]")));
    assert!(!result.contains("from sidecar"));
}

#[test]
fn test_prepare_download_state_transitions_to_ready() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("test", "app", "pod-1"));

    assert_eq!(state.download_state, LogDownloadState::Idle);
    let content = state.prepare_download();
    match &state.download_state {
        LogDownloadState::Ready(s) => assert_eq!(s, &content),
        other => panic!("Expected Ready, got {:?}", other),
    }
}

#[test]
fn test_prepare_download_returns_same_content_as_state() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("check consistency", "app", "pod-1"));

    let result = state.prepare_download();
    if let LogDownloadState::Ready(ref stored) = state.download_state {
        assert_eq!(&result, stored);
    } else {
        panic!("Expected Ready state");
    }
}

#[test]
fn test_prepare_download_can_be_called_multiple_times() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("data", "app", "pod-1"));

    let first = state.prepare_download();
    let second = state.prepare_download();
    assert_eq!(first, second);
}

// ========================================================================
// Download state tracking in LogViewerState
// ========================================================================

#[test]
fn test_download_state_initial_is_idle() {
    let state = LogViewerState::new(500);
    assert_eq!(state.download_state, LogDownloadState::Idle);
}

#[test]
fn test_download_state_after_format_change() {
    let mut state = LogViewerState::new(1000);
    state.push_line(make_log_line("data", "app", "pod-1"));

    // Download as plain text
    state.prepare_download();
    assert!(matches!(state.download_state, LogDownloadState::Ready(_)));

    // Change format and download again
    state.set_download_format(LogDownloadFormat::Json);
    let json_result = state.prepare_download();
    if let LogDownloadState::Ready(ref stored) = state.download_state {
        assert_eq!(stored, &json_result);
        // Verify it is actually JSON
        let parsed: Vec<LogLine> = serde_json::from_str(stored).expect("Valid JSON");
        assert_eq!(parsed.len(), 1);
    } else {
        panic!("Expected Ready state");
    }
}

#[test]
fn test_download_state_error_variant() {
    // Test that the Error variant can be constructed and compared
    let err = LogDownloadState::Error("disk full".to_string());
    assert_eq!(err, LogDownloadState::Error("disk full".to_string()));
    assert_ne!(err, LogDownloadState::Idle);
}
