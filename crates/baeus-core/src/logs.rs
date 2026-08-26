use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: Option<DateTime<Utc>>,
    pub content: String,
    pub container_name: String,
    pub pod_name: String,
    pub source_color_index: usize,
}

#[derive(Debug, Clone)]
pub struct LogStreamConfig {
    pub cluster_id: Uuid,
    pub namespace: String,
    pub pod_name: String,
    pub container_name: Option<String>,
    pub follow: bool,
    pub tail_lines: Option<u64>,
    pub timestamps: bool,
    pub since_seconds: Option<i64>,
}

impl LogStreamConfig {
    pub fn new(cluster_id: Uuid, namespace: String, pod_name: String) -> Self {
        Self {
            cluster_id,
            namespace,
            pod_name,
            container_name: None,
            follow: true,
            tail_lines: Some(1000),
            timestamps: true,
            since_seconds: None,
        }
    }

    pub fn with_container(mut self, name: String) -> Self {
        self.container_name = Some(name);
        self
    }

    pub fn with_tail_lines(mut self, lines: u64) -> Self {
        self.tail_lines = Some(lines);
        self
    }

    pub fn with_follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    pub fn with_since_seconds(mut self, seconds: i64) -> Self {
        self.since_seconds = Some(seconds);
        self
    }

    pub fn with_timestamps(mut self, timestamps: bool) -> Self {
        self.timestamps = timestamps;
        self
    }

    pub fn is_multi_container(&self) -> bool {
        self.container_name.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStreamState {
    Idle,
    Streaming,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug)]
pub struct LogBuffer {
    lines: Vec<LogLine>,
    max_lines: usize,
    search_query: Option<String>,
}

impl LogBuffer {
    pub fn new(max_lines: usize) -> Self {
        Self { lines: Vec::new(), max_lines, search_query: None }
    }

    pub fn push(&mut self, line: LogLine) {
        self.lines.push(line);
        if self.lines.len() > self.max_lines {
            let excess = self.lines.len() - self.max_lines;
            self.lines.drain(..excess);
        }
    }

    pub fn lines(&self) -> &[LogLine] {
        &self.lines
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn set_search(&mut self, query: Option<String>) {
        self.search_query = query;
    }

    pub fn search_results(&self) -> Vec<usize> {
        let Some(ref query) = self.search_query else {
            return Vec::new();
        };
        let query_lower = query.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.content.to_lowercase().contains(&query_lower))
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn filtered_lines(&self) -> Vec<&LogLine> {
        if self.search_query.is_none() {
            return self.lines.iter().collect();
        }
        let indices = self.search_results();
        indices.iter().map(|&i| &self.lines[i]).collect()
    }
}

/// Manages multiple concurrent log streams with their configurations and states.
pub struct LogStreamManager {
    pub streams: Vec<(LogStreamConfig, LogStreamState)>,
}

impl LogStreamManager {
    pub fn new() -> Self {
        Self { streams: Vec::new() }
    }

    /// Adds a new stream and returns its index.
    pub fn add_stream(&mut self, config: LogStreamConfig) -> usize {
        let index = self.streams.len();
        self.streams.push((config, LogStreamState::Idle));
        index
    }

    /// Removes a stream by index. Returns true if the index was valid.
    pub fn remove_stream(&mut self, index: usize) -> bool {
        if index < self.streams.len() {
            self.streams.remove(index);
            true
        } else {
            false
        }
    }

    /// Returns the total number of tracked streams.
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Sets the state of a stream by index. Returns true if the index was valid.
    pub fn set_stream_state(&mut self, index: usize, state: LogStreamState) -> bool {
        if let Some(entry) = self.streams.get_mut(index) {
            entry.1 = state;
            true
        } else {
            false
        }
    }

    /// Returns the state of a stream by index, if it exists.
    pub fn stream_state(&self, index: usize) -> Option<&LogStreamState> {
        self.streams.get(index).map(|(_, state)| state)
    }

    /// Returns the count of streams currently in the `Streaming` state.
    pub fn active_streams(&self) -> usize {
        self.streams.iter().filter(|(_, state)| *state == LogStreamState::Streaming).count()
    }

    /// Sets all streams to the `Stopped` state.
    pub fn stop_all(&mut self) {
        for (_, state) in &mut self.streams {
            *state = LogStreamState::Stopped;
        }
    }

    /// Pauses all streams that are currently `Streaming`.
    pub fn pause_all(&mut self) {
        for (_, state) in &mut self.streams {
            if *state == LogStreamState::Streaming {
                *state = LogStreamState::Paused;
            }
        }
    }

    /// Resumes all streams that are currently `Paused` back to `Streaming`.
    pub fn resume_all(&mut self) {
        for (_, state) in &mut self.streams {
            if *state == LogStreamState::Paused {
                *state = LogStreamState::Streaming;
            }
        }
    }
}

impl Default for LogStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct MultiPodLogState {
    pub configs: Vec<LogStreamConfig>,
    pub buffer: LogBuffer,
    pub state: LogStreamState,
    color_counter: usize,
}

impl MultiPodLogState {
    pub fn new(max_lines: usize) -> Self {
        Self {
            configs: Vec::new(),
            buffer: LogBuffer::new(max_lines),
            state: LogStreamState::Idle,
            color_counter: 0,
        }
    }

    pub fn add_source(&mut self, config: LogStreamConfig) -> usize {
        let idx = self.color_counter;
        self.color_counter += 1;
        self.configs.push(config);
        idx
    }

    pub fn source_count(&self) -> usize {
        self.configs.len()
    }

    /// Pushes a log line into the buffer, setting its `source_color_index` to the
    /// color index corresponding to the given `source_index`.
    pub fn push_line(&mut self, source_index: usize, line: LogLine) {
        let mut line = line;
        line.source_color_index = source_index;
        self.buffer.push(line);
    }

    /// Clears the underlying log buffer.
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    /// Sets the overall aggregation state.
    pub fn set_state(&mut self, state: LogStreamState) {
        self.state = state;
    }

    /// Delegates search to the underlying buffer.
    pub fn search(&mut self, query: Option<String>) {
        self.buffer.set_search(query);
    }

    /// Returns the total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the filtered lines from the buffer (respects active search query).
    pub fn filtered_lines(&self) -> Vec<&LogLine> {
        self.buffer.filtered_lines()
    }

    /// Returns the indices and references to configs whose `pod_name` matches `pod_name`.
    pub fn configs_for_pod(&self, pod_name: &str) -> Vec<(usize, &LogStreamConfig)> {
        self.configs.iter().enumerate().filter(|(_, config)| config.pod_name == pod_name).collect()
    }

    /// Removes a source config by index. Returns true if the index was valid.
    pub fn remove_source(&mut self, index: usize) -> bool {
        if index < self.configs.len() {
            self.configs.remove(index);
            true
        } else {
            false
        }
    }
}

/// Parse a K8s log timestamp from the beginning of a log line.
/// K8s log format: `2026-03-10T12:00:00.000000000Z content here`
/// Returns the parsed timestamp if the line starts with a valid RFC3339 timestamp.
pub fn parse_k8s_log_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let space_idx = line.find(' ')?;
    DateTime::parse_from_rfc3339(&line[..space_idx]).ok().map(|dt| dt.with_timezone(&Utc))
}

/// Supported formats for exporting log lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogDownloadFormat {
    PlainText,
    Json,
    Csv,
}

/// Formats a slice of log lines for download in the specified format.
pub fn format_logs_for_download(lines: &[LogLine], format: LogDownloadFormat) -> String {
    match format {
        LogDownloadFormat::PlainText => lines
            .iter()
            .map(|line| {
                let ts = line.timestamp.map(|t| t.to_rfc3339()).unwrap_or_default();
                format!("{} [{}] [{}] {}", ts, line.pod_name, line.container_name, line.content)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        LogDownloadFormat::Json => {
            serde_json::to_string_pretty(lines).unwrap_or_else(|_| "[]".to_string())
        }
        LogDownloadFormat::Csv => {
            let mut output = String::from("timestamp,pod_name,container_name,content\n");
            for line in lines {
                let ts = line.timestamp.map(|t| t.to_rfc3339()).unwrap_or_default();
                // Escape CSV fields that may contain commas or quotes
                let escaped_content = line.content.replace('"', "\"\"");
                output.push_str(&format!(
                    "{},{},{},\"{}\"\n",
                    ts, line.pod_name, line.container_name, escaped_content
                ));
            }
            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    fn make_log_line(content: &str, container: &str) -> LogLine {
        LogLine {
            timestamp: Some(Utc::now()),
            content: content.to_string(),
            container_name: container.to_string(),
            pod_name: "test-pod".to_string(),
            source_color_index: 0,
        }
    }

    fn make_log_line_for_pod(content: &str, container: &str, pod: &str) -> LogLine {
        LogLine {
            timestamp: Some(Utc::now()),
            content: content.to_string(),
            container_name: container.to_string(),
            pod_name: pod.to_string(),
            source_color_index: 0,
        }
    }

    fn make_config(pod_name: &str) -> LogStreamConfig {
        LogStreamConfig::new(Uuid::new_v4(), "default".to_string(), pod_name.to_string())
    }

    // ========================================================================
    // T065: Existing tests (preserved)
    // ========================================================================

    #[test]
    fn test_log_buffer_push_and_len() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("line 1", "app"));
        buffer.push(make_log_line("line 2", "app"));

        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_log_buffer_max_lines_eviction() {
        let mut buffer = LogBuffer::new(3);
        for i in 0..5 {
            buffer.push(make_log_line(&format!("line {i}"), "app"));
        }

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.lines()[0].content, "line 2");
        assert_eq!(buffer.lines()[2].content, "line 4");
    }

    #[test]
    fn test_log_buffer_clear() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("test", "app"));
        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_log_buffer_search() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("INFO: server started", "app"));
        buffer.push(make_log_line("ERROR: connection failed", "app"));
        buffer.push(make_log_line("INFO: request received", "app"));
        buffer.push(make_log_line("ERROR: timeout", "app"));

        buffer.set_search(Some("ERROR".to_string()));
        let results = buffer.search_results();
        assert_eq!(results, vec![1, 3]);
    }

    #[test]
    fn test_log_buffer_search_case_insensitive() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("Error: something", "app"));
        buffer.push(make_log_line("error: another", "app"));

        buffer.set_search(Some("error".to_string()));
        assert_eq!(buffer.search_results().len(), 2);
    }

    #[test]
    fn test_log_buffer_no_search() {
        let buffer = LogBuffer::new(100);
        assert!(buffer.search_results().is_empty());
    }

    #[test]
    fn test_log_stream_config_builder() {
        let config =
            LogStreamConfig::new(Uuid::new_v4(), "default".to_string(), "nginx".to_string())
                .with_container("app".to_string())
                .with_tail_lines(500)
                .with_follow(false)
                .with_since_seconds(3600);

        assert_eq!(config.container_name.as_deref(), Some("app"));
        assert_eq!(config.tail_lines, Some(500));
        assert!(!config.follow);
        assert_eq!(config.since_seconds, Some(3600));
    }

    #[test]
    fn test_multi_pod_log_state() {
        let mut state = MultiPodLogState::new(1000);
        let idx1 = state.add_source(LogStreamConfig::new(
            Uuid::new_v4(),
            "default".to_string(),
            "pod-1".to_string(),
        ));
        let idx2 = state.add_source(LogStreamConfig::new(
            Uuid::new_v4(),
            "default".to_string(),
            "pod-2".to_string(),
        ));

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(state.source_count(), 2);
        assert_eq!(state.state, LogStreamState::Idle);
    }

    // ========================================================================
    // T065: LogLine creation and serialization
    // ========================================================================

    #[test]
    fn test_log_line_creation_with_timestamp() {
        let now = Utc::now();
        let line = LogLine {
            timestamp: Some(now),
            content: "hello world".to_string(),
            container_name: "nginx".to_string(),
            pod_name: "web-pod".to_string(),
            source_color_index: 3,
        };
        assert_eq!(line.timestamp, Some(now));
        assert_eq!(line.content, "hello world");
        assert_eq!(line.container_name, "nginx");
        assert_eq!(line.pod_name, "web-pod");
        assert_eq!(line.source_color_index, 3);
    }

    #[test]
    fn test_log_line_creation_without_timestamp() {
        let line = LogLine {
            timestamp: None,
            content: "no timestamp".to_string(),
            container_name: "app".to_string(),
            pod_name: "pod-1".to_string(),
            source_color_index: 0,
        };
        assert!(line.timestamp.is_none());
        assert_eq!(line.content, "no timestamp");
    }

    #[test]
    fn test_log_line_serialization_roundtrip() {
        let line = make_log_line("serialize me", "app");
        let json = serde_json::to_string(&line).expect("serialization should succeed");
        let deserialized: LogLine =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(deserialized.content, "serialize me");
        assert_eq!(deserialized.container_name, "app");
        assert_eq!(deserialized.pod_name, "test-pod");
        assert_eq!(deserialized.source_color_index, 0);
    }

    #[test]
    fn test_log_line_clone() {
        let line = make_log_line("clone me", "app");
        let cloned = line.clone();
        assert_eq!(cloned.content, line.content);
        assert_eq!(cloned.container_name, line.container_name);
    }

    // ========================================================================
    // T065: LogStreamConfig defaults and all builder methods
    // ========================================================================

    #[test]
    fn test_log_stream_config_defaults() {
        let id = Uuid::new_v4();
        let config = LogStreamConfig::new(id, "kube-system".to_string(), "coredns".to_string());
        assert_eq!(config.cluster_id, id);
        assert_eq!(config.namespace, "kube-system");
        assert_eq!(config.pod_name, "coredns");
        assert!(config.container_name.is_none());
        assert!(config.follow);
        assert_eq!(config.tail_lines, Some(1000));
        assert!(config.timestamps);
        assert!(config.since_seconds.is_none());
    }

    #[test]
    fn test_log_stream_config_with_timestamps() {
        let config = make_config("pod-1").with_timestamps(false);
        assert!(!config.timestamps);

        let config2 = make_config("pod-2").with_timestamps(true);
        assert!(config2.timestamps);
    }

    #[test]
    fn test_log_stream_config_is_multi_container_true_when_no_container() {
        let config = make_config("pod-1");
        assert!(config.is_multi_container());
    }

    #[test]
    fn test_log_stream_config_is_multi_container_false_when_container_set() {
        let config = make_config("pod-1").with_container("nginx".to_string());
        assert!(!config.is_multi_container());
    }

    #[test]
    fn test_log_stream_config_all_builder_methods_chained() {
        let config = make_config("pod-1")
            .with_container("sidecar".to_string())
            .with_tail_lines(200)
            .with_follow(false)
            .with_since_seconds(7200)
            .with_timestamps(false);

        assert_eq!(config.container_name.as_deref(), Some("sidecar"));
        assert_eq!(config.tail_lines, Some(200));
        assert!(!config.follow);
        assert_eq!(config.since_seconds, Some(7200));
        assert!(!config.timestamps);
        assert!(!config.is_multi_container());
    }

    // ========================================================================
    // T065: LogBuffer edge cases
    // ========================================================================

    #[test]
    fn test_log_buffer_empty_buffer() {
        let buffer = LogBuffer::new(100);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert!(buffer.lines().is_empty());
        assert!(buffer.filtered_lines().is_empty());
        assert!(buffer.search_results().is_empty());
    }

    #[test]
    fn test_log_buffer_single_line() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("only line", "app"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.lines()[0].content, "only line");
    }

    #[test]
    fn test_log_buffer_exactly_at_max_capacity() {
        let mut buffer = LogBuffer::new(3);
        buffer.push(make_log_line("a", "app"));
        buffer.push(make_log_line("b", "app"));
        buffer.push(make_log_line("c", "app"));

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.lines()[0].content, "a");
        assert_eq!(buffer.lines()[2].content, "c");
    }

    #[test]
    fn test_log_buffer_one_over_max_capacity() {
        let mut buffer = LogBuffer::new(3);
        buffer.push(make_log_line("a", "app"));
        buffer.push(make_log_line("b", "app"));
        buffer.push(make_log_line("c", "app"));
        buffer.push(make_log_line("d", "app"));

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.lines()[0].content, "b");
        assert_eq!(buffer.lines()[2].content, "d");
    }

    #[test]
    fn test_log_buffer_max_capacity_one() {
        let mut buffer = LogBuffer::new(1);
        buffer.push(make_log_line("first", "app"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.lines()[0].content, "first");

        buffer.push(make_log_line("second", "app"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.lines()[0].content, "second");
    }

    #[test]
    fn test_log_buffer_clear_then_push() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("before clear", "app"));
        buffer.clear();
        buffer.push(make_log_line("after clear", "app"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.lines()[0].content, "after clear");
    }

    // ========================================================================
    // T065: LogBuffer search edge cases
    // ========================================================================

    #[test]
    fn test_log_buffer_search_empty_query_string() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("hello", "app"));
        buffer.push(make_log_line("world", "app"));

        // Empty string matches everything (every string contains "")
        buffer.set_search(Some("".to_string()));
        assert_eq!(buffer.search_results().len(), 2);
    }

    #[test]
    fn test_log_buffer_search_no_matches() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("INFO: all good", "app"));
        buffer.push(make_log_line("DEBUG: trace", "app"));

        buffer.set_search(Some("FATAL".to_string()));
        assert!(buffer.search_results().is_empty());
    }

    #[test]
    fn test_log_buffer_search_partial_match() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("connection_timeout_error", "app"));
        buffer.push(make_log_line("timeout_warning", "app"));
        buffer.push(make_log_line("all_clear", "app"));

        buffer.set_search(Some("timeout".to_string()));
        let results = buffer.search_results();
        assert_eq!(results, vec![0, 1]);
    }

    #[test]
    fn test_log_buffer_search_regex_like_string_treated_as_literal() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("file.txt matched", "app"));
        buffer.push(make_log_line("file*txt no match", "app"));
        buffer.push(make_log_line("something else", "app"));

        // Regex-like pattern should be treated as a literal string
        buffer.set_search(Some("file.txt".to_string()));
        let results = buffer.search_results();
        assert_eq!(results, vec![0]);
    }

    #[test]
    fn test_log_buffer_search_special_characters() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("error [ERR-123]", "app"));
        buffer.push(make_log_line("info (ok)", "app"));

        buffer.set_search(Some("[ERR-123]".to_string()));
        let results = buffer.search_results();
        assert_eq!(results, vec![0]);
    }

    #[test]
    fn test_log_buffer_search_clearing_search() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("error", "app"));
        buffer.push(make_log_line("info", "app"));

        buffer.set_search(Some("error".to_string()));
        assert_eq!(buffer.search_results().len(), 1);

        buffer.set_search(None);
        assert!(buffer.search_results().is_empty());
    }

    // ========================================================================
    // T065: LogBuffer filtered_lines
    // ========================================================================

    #[test]
    fn test_log_buffer_filtered_lines_no_search_returns_all() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("line 1", "app"));
        buffer.push(make_log_line("line 2", "app"));
        buffer.push(make_log_line("line 3", "app"));

        let filtered = buffer.filtered_lines();
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].content, "line 1");
        assert_eq!(filtered[2].content, "line 3");
    }

    #[test]
    fn test_log_buffer_filtered_lines_with_search() {
        let mut buffer = LogBuffer::new(100);
        buffer.push(make_log_line("INFO: ok", "app"));
        buffer.push(make_log_line("ERROR: fail", "app"));
        buffer.push(make_log_line("INFO: fine", "app"));

        buffer.set_search(Some("ERROR".to_string()));
        let filtered = buffer.filtered_lines();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "ERROR: fail");
    }

    #[test]
    fn test_log_buffer_filtered_lines_empty_buffer_with_search() {
        let mut buffer = LogBuffer::new(100);
        buffer.set_search(Some("anything".to_string()));
        assert!(buffer.filtered_lines().is_empty());
    }

    // ========================================================================
    // T065: LogStreamState transitions
    // ========================================================================

    #[test]
    fn test_log_stream_state_equality() {
        assert_eq!(LogStreamState::Idle, LogStreamState::Idle);
        assert_ne!(LogStreamState::Idle, LogStreamState::Streaming);
        assert_ne!(LogStreamState::Paused, LogStreamState::Stopped);
    }

    #[test]
    fn test_log_stream_state_copy() {
        let state = LogStreamState::Streaming;
        let copied = state;
        assert_eq!(state, copied);
    }

    #[test]
    fn test_log_stream_state_all_variants_distinct() {
        let variants = [
            LogStreamState::Idle,
            LogStreamState::Streaming,
            LogStreamState::Paused,
            LogStreamState::Stopped,
            LogStreamState::Error,
        ];
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }
    }

    // ========================================================================
    // T065: MultiPodLogState with multiple sources and interleaved lines
    // ========================================================================

    #[test]
    fn test_multi_pod_log_state_interleaved_lines_with_color_indices() {
        let mut state = MultiPodLogState::new(1000);
        let idx1 = state.add_source(make_config("pod-1"));
        let idx2 = state.add_source(make_config("pod-2"));

        state.push_line(idx1, make_log_line_for_pod("from pod-1 a", "app", "pod-1"));
        state.push_line(idx2, make_log_line_for_pod("from pod-2 a", "sidecar", "pod-2"));
        state.push_line(idx1, make_log_line_for_pod("from pod-1 b", "app", "pod-1"));

        assert_eq!(state.line_count(), 3);
        let lines = state.buffer.lines();
        assert_eq!(lines[0].source_color_index, 0);
        assert_eq!(lines[1].source_color_index, 1);
        assert_eq!(lines[2].source_color_index, 0);
    }

    #[test]
    fn test_multi_pod_log_state_three_sources() {
        let mut state = MultiPodLogState::new(1000);
        let idx1 = state.add_source(make_config("pod-a"));
        let idx2 = state.add_source(make_config("pod-b"));
        let idx3 = state.add_source(make_config("pod-c"));

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 2);
        assert_eq!(state.source_count(), 3);
    }

    // ========================================================================
    // T068: LogStreamManager tests
    // ========================================================================

    #[test]
    fn test_log_stream_manager_new_is_empty() {
        let mgr = LogStreamManager::new();
        assert_eq!(mgr.stream_count(), 0);
        assert_eq!(mgr.active_streams(), 0);
    }

    #[test]
    fn test_log_stream_manager_add_stream_returns_index() {
        let mut mgr = LogStreamManager::new();
        let idx0 = mgr.add_stream(make_config("pod-1"));
        let idx1 = mgr.add_stream(make_config("pod-2"));
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(mgr.stream_count(), 2);
    }

    #[test]
    fn test_log_stream_manager_new_streams_start_idle() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Idle));
    }

    #[test]
    fn test_log_stream_manager_remove_valid_stream() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.add_stream(make_config("pod-2"));

        assert!(mgr.remove_stream(0));
        assert_eq!(mgr.stream_count(), 1);
        // After removing index 0, the remaining stream is now at index 0
        assert_eq!(mgr.streams[0].0.pod_name, "pod-2");
    }

    #[test]
    fn test_log_stream_manager_remove_invalid_stream() {
        let mut mgr = LogStreamManager::new();
        assert!(!mgr.remove_stream(0));
        assert!(!mgr.remove_stream(99));
    }

    #[test]
    fn test_log_stream_manager_set_stream_state() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));

        assert!(mgr.set_stream_state(0, LogStreamState::Streaming));
        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Streaming));
    }

    #[test]
    fn test_log_stream_manager_set_stream_state_invalid_index() {
        let mut mgr = LogStreamManager::new();
        assert!(!mgr.set_stream_state(0, LogStreamState::Streaming));
    }

    #[test]
    fn test_log_stream_manager_stream_state_invalid_index() {
        let mgr = LogStreamManager::new();
        assert!(mgr.stream_state(0).is_none());
    }

    #[test]
    fn test_log_stream_manager_active_streams() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.add_stream(make_config("pod-2"));
        mgr.add_stream(make_config("pod-3"));

        mgr.set_stream_state(0, LogStreamState::Streaming);
        mgr.set_stream_state(2, LogStreamState::Streaming);

        assert_eq!(mgr.active_streams(), 2);
    }

    #[test]
    fn test_log_stream_manager_stop_all() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.add_stream(make_config("pod-2"));
        mgr.set_stream_state(0, LogStreamState::Streaming);
        mgr.set_stream_state(1, LogStreamState::Paused);

        mgr.stop_all();

        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Stopped));
        assert_eq!(mgr.stream_state(1), Some(&LogStreamState::Stopped));
    }

    #[test]
    fn test_log_stream_manager_pause_all_only_pauses_streaming() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.add_stream(make_config("pod-2"));
        mgr.add_stream(make_config("pod-3"));
        mgr.set_stream_state(0, LogStreamState::Streaming);
        mgr.set_stream_state(1, LogStreamState::Idle);
        mgr.set_stream_state(2, LogStreamState::Streaming);

        mgr.pause_all();

        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Paused));
        assert_eq!(mgr.stream_state(1), Some(&LogStreamState::Idle)); // unchanged
        assert_eq!(mgr.stream_state(2), Some(&LogStreamState::Paused));
    }

    #[test]
    fn test_log_stream_manager_resume_all_only_resumes_paused() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.add_stream(make_config("pod-2"));
        mgr.add_stream(make_config("pod-3"));
        mgr.set_stream_state(0, LogStreamState::Paused);
        mgr.set_stream_state(1, LogStreamState::Stopped);
        mgr.set_stream_state(2, LogStreamState::Paused);

        mgr.resume_all();

        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Streaming));
        assert_eq!(mgr.stream_state(1), Some(&LogStreamState::Stopped)); // unchanged
        assert_eq!(mgr.stream_state(2), Some(&LogStreamState::Streaming));
    }

    #[test]
    fn test_log_stream_manager_pause_resume_cycle() {
        let mut mgr = LogStreamManager::new();
        mgr.add_stream(make_config("pod-1"));
        mgr.set_stream_state(0, LogStreamState::Streaming);

        mgr.pause_all();
        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Paused));
        assert_eq!(mgr.active_streams(), 0);

        mgr.resume_all();
        assert_eq!(mgr.stream_state(0), Some(&LogStreamState::Streaming));
        assert_eq!(mgr.active_streams(), 1);
    }

    #[test]
    fn test_log_stream_manager_default() {
        let mgr = LogStreamManager::default();
        assert_eq!(mgr.stream_count(), 0);
    }

    // ========================================================================
    // T069: MultiPodLogState extended methods
    // ========================================================================

    #[test]
    fn test_multi_pod_push_line_sets_color_index() {
        let mut state = MultiPodLogState::new(100);
        let idx = state.add_source(make_config("pod-1"));
        state.push_line(idx, make_log_line("hello", "app"));

        assert_eq!(state.line_count(), 1);
        assert_eq!(state.buffer.lines()[0].source_color_index, idx);
    }

    #[test]
    fn test_multi_pod_clear_buffer() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.push_line(0, make_log_line("data", "app"));
        assert_eq!(state.line_count(), 1);

        state.clear_buffer();
        assert_eq!(state.line_count(), 0);
    }

    #[test]
    fn test_multi_pod_set_state() {
        let mut state = MultiPodLogState::new(100);
        assert_eq!(state.state, LogStreamState::Idle);

        state.set_state(LogStreamState::Streaming);
        assert_eq!(state.state, LogStreamState::Streaming);

        state.set_state(LogStreamState::Error);
        assert_eq!(state.state, LogStreamState::Error);
    }

    #[test]
    fn test_multi_pod_search_delegates_to_buffer() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.push_line(0, make_log_line("ERROR: bad", "app"));
        state.push_line(0, make_log_line("INFO: ok", "app"));

        state.search(Some("ERROR".to_string()));
        let filtered = state.filtered_lines();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "ERROR: bad");
    }

    #[test]
    fn test_multi_pod_search_none_returns_all() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.push_line(0, make_log_line("a", "app"));
        state.push_line(0, make_log_line("b", "app"));

        state.search(None);
        assert_eq!(state.filtered_lines().len(), 2);
    }

    #[test]
    fn test_multi_pod_line_count() {
        let mut state = MultiPodLogState::new(100);
        assert_eq!(state.line_count(), 0);

        state.add_source(make_config("pod-1"));
        state.push_line(0, make_log_line("x", "app"));
        state.push_line(0, make_log_line("y", "app"));
        assert_eq!(state.line_count(), 2);
    }

    #[test]
    fn test_multi_pod_filtered_lines_no_search() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.push_line(0, make_log_line("a", "app"));
        state.push_line(0, make_log_line("b", "app"));

        let lines = state.filtered_lines();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_multi_pod_configs_for_pod() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.add_source(make_config("pod-2"));
        state.add_source(make_config("pod-1").with_container("sidecar".to_string()));

        let configs = state.configs_for_pod("pod-1");
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].0, 0);
        assert_eq!(configs[1].0, 2);
    }

    #[test]
    fn test_multi_pod_configs_for_pod_no_match() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));

        let configs = state.configs_for_pod("pod-99");
        assert!(configs.is_empty());
    }

    #[test]
    fn test_multi_pod_remove_source_valid() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        state.add_source(make_config("pod-2"));

        assert!(state.remove_source(0));
        assert_eq!(state.source_count(), 1);
        assert_eq!(state.configs[0].pod_name, "pod-2");
    }

    #[test]
    fn test_multi_pod_remove_source_invalid() {
        let mut state = MultiPodLogState::new(100);
        assert!(!state.remove_source(0));
        assert!(!state.remove_source(5));
    }

    #[test]
    fn test_multi_pod_remove_source_last() {
        let mut state = MultiPodLogState::new(100);
        state.add_source(make_config("pod-1"));
        assert!(state.remove_source(0));
        assert_eq!(state.source_count(), 0);
    }

    // ========================================================================
    // T069: LogDownloadFormat and format_logs_for_download
    // ========================================================================

    #[test]
    fn test_log_download_format_variants_distinct() {
        assert_ne!(LogDownloadFormat::PlainText, LogDownloadFormat::Json);
        assert_ne!(LogDownloadFormat::Json, LogDownloadFormat::Csv);
        assert_ne!(LogDownloadFormat::PlainText, LogDownloadFormat::Csv);
    }

    #[test]
    fn test_format_logs_plain_text_empty() {
        let result = format_logs_for_download(&[], LogDownloadFormat::PlainText);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_logs_plain_text() {
        let line = LogLine {
            timestamp: None,
            content: "hello world".to_string(),
            container_name: "app".to_string(),
            pod_name: "pod-1".to_string(),
            source_color_index: 0,
        };
        let result = format_logs_for_download(&[line], LogDownloadFormat::PlainText);
        assert!(result.contains("[pod-1]"));
        assert!(result.contains("[app]"));
        assert!(result.contains("hello world"));
    }

    #[test]
    fn test_format_logs_plain_text_multiple_lines() {
        let lines = vec![make_log_line("first", "app"), make_log_line("second", "app")];
        let result = format_logs_for_download(&lines, LogDownloadFormat::PlainText);
        let output_lines: Vec<&str> = result.lines().collect();
        assert_eq!(output_lines.len(), 2);
        assert!(output_lines[0].contains("first"));
        assert!(output_lines[1].contains("second"));
    }

    #[test]
    fn test_format_logs_json_empty() {
        let result = format_logs_for_download(&[], LogDownloadFormat::Json);
        assert_eq!(result.trim(), "[]");
    }

    #[test]
    fn test_format_logs_json_roundtrip() {
        let line = make_log_line("json test", "app");
        let result = format_logs_for_download(&[line], LogDownloadFormat::Json);
        let parsed: Vec<LogLine> =
            serde_json::from_str(&result).expect("JSON output should be valid");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].content, "json test");
    }

    #[test]
    fn test_format_logs_csv_header() {
        let result = format_logs_for_download(&[], LogDownloadFormat::Csv);
        assert!(result.starts_with("timestamp,pod_name,container_name,content\n"));
    }

    #[test]
    fn test_format_logs_csv_with_data() {
        let line = LogLine {
            timestamp: None,
            content: "csv test".to_string(),
            container_name: "app".to_string(),
            pod_name: "pod-1".to_string(),
            source_color_index: 0,
        };
        let result = format_logs_for_download(&[line], LogDownloadFormat::Csv);
        let csv_lines: Vec<&str> = result.lines().collect();
        assert_eq!(csv_lines.len(), 2); // header + 1 data row
        assert!(csv_lines[1].contains("pod-1"));
        assert!(csv_lines[1].contains("app"));
        assert!(csv_lines[1].contains("csv test"));
    }

    #[test]
    fn test_format_logs_csv_escapes_quotes() {
        let line = LogLine {
            timestamp: None,
            content: r#"said "hello""#.to_string(),
            container_name: "app".to_string(),
            pod_name: "pod-1".to_string(),
            source_color_index: 0,
        };
        let result = format_logs_for_download(&[line], LogDownloadFormat::Csv);
        // Double-quotes should be escaped as ""
        assert!(result.contains(r#"said ""hello"""#));
    }

    // ========================================================================
    // parse_k8s_log_timestamp tests
    // ========================================================================

    #[test]
    fn test_parse_k8s_log_timestamp_valid() {
        let line = "2026-03-10T12:00:00.000000000Z some log content here";
        let ts = parse_k8s_log_timestamp(line);
        assert!(ts.is_some());
        let ts = ts.unwrap();
        assert_eq!(ts.year(), 2026);
        assert_eq!(ts.month(), 3);
        assert_eq!(ts.day(), 10);
    }

    #[test]
    fn test_parse_k8s_log_timestamp_no_space() {
        let line = "2026-03-10T12:00:00Z";
        assert!(parse_k8s_log_timestamp(line).is_none());
    }

    #[test]
    fn test_parse_k8s_log_timestamp_not_rfc3339() {
        let line = "not-a-timestamp some content";
        assert!(parse_k8s_log_timestamp(line).is_none());
    }

    #[test]
    fn test_parse_k8s_log_timestamp_empty() {
        assert!(parse_k8s_log_timestamp("").is_none());
    }

    #[test]
    fn test_parse_k8s_log_timestamp_nanoseconds() {
        let line = "2024-01-15T08:30:45.123456789Z INFO Starting server";
        let ts = parse_k8s_log_timestamp(line);
        assert!(ts.is_some());
        assert_eq!(ts.unwrap().hour(), 8);
    }

    #[test]
    fn test_format_logs_plain_text_with_timestamp() {
        let now = Utc::now();
        let line = LogLine {
            timestamp: Some(now),
            content: "timestamped".to_string(),
            container_name: "app".to_string(),
            pod_name: "pod-1".to_string(),
            source_color_index: 0,
        };
        let result = format_logs_for_download(&[line], LogDownloadFormat::PlainText);
        // Should contain an RFC3339 timestamp
        assert!(result.contains(&now.to_rfc3339()));
        assert!(result.contains("timestamped"));
    }
}
