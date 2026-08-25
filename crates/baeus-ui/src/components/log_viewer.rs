// T073: Log viewer component state
// Auto-scrolling log display with timestamps, color-coded multi-container output,
// search highlighting, and download button.

use baeus_core::logs::{
    LogBuffer, LogDownloadFormat, LogLine, LogStreamState, format_logs_for_download,
};
use gpui::{
    Context, ElementId, Entity, Rgba, SharedString, Subscription, Window, deferred, div,
    prelude::*, px,
};
use gpui_component::Sizable;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::theme::Theme;

/// Strip ANSI escape sequences from a string to prevent terminal injection.
/// Removes CSI sequences (\x1b[...X), OSC sequences (\x1b]...\x07), and
/// other control sequences that could be used for UI spoofing.
fn strip_ansi_escapes(s: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
        // Match: ESC[ ... final byte, ESC] ... BEL, ESC followed by single char
        Regex::new(r"\x1b\[[0-9;]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b[A-Za-z]|\x1b\([A-Za-z]").unwrap()
    });

    ANSI_RE.replace_all(s, "").into_owned()
}

/// T348: Search mode for the log viewer.
/// In Highlight mode, all lines are shown with matches highlighted in accent color.
/// In Filter mode, only lines matching the search query are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogSearchMode {
    /// Show all lines; highlight matching lines in accent color.
    #[default]
    Highlight,
    /// Show only lines that match the search query.
    Filter,
}

impl LogSearchMode {
    /// Toggle between Highlight and Filter modes.
    pub fn toggle(&self) -> Self {
        match self {
            LogSearchMode::Highlight => LogSearchMode::Filter,
            LogSearchMode::Filter => LogSearchMode::Highlight,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            LogSearchMode::Highlight => "Highlight",
            LogSearchMode::Filter => "Filter",
        }
    }
}

/// Display settings for the log viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogViewerSettings {
    pub show_timestamps: bool,
    pub wrap_lines: bool,
    pub auto_scroll: bool,
    pub font_size: u8,
}

impl Default for LogViewerSettings {
    fn default() -> Self {
        Self { show_timestamps: true, wrap_lines: false, auto_scroll: true, font_size: 12 }
    }
}

/// Which containers are selected for viewing in a multi-container pod.
#[derive(Debug, Clone)]
pub struct ContainerFilter {
    /// All container names in the pod.
    pub all_containers: Vec<String>,
    /// Which containers are currently visible.
    pub visible: Vec<String>,
}

impl ContainerFilter {
    pub fn new(containers: Vec<String>) -> Self {
        let visible = containers.clone();
        Self { all_containers: containers, visible }
    }

    /// Toggle visibility of a container.
    pub fn toggle(&mut self, container: &str) {
        if let Some(pos) = self.visible.iter().position(|c| c == container) {
            self.visible.remove(pos);
        } else if self.all_containers.contains(&container.to_string()) {
            self.visible.push(container.to_string());
        }
    }

    /// Show all containers.
    pub fn show_all(&mut self) {
        self.visible = self.all_containers.clone();
    }

    /// Hide all containers.
    pub fn hide_all(&mut self) {
        self.visible.clear();
    }

    /// Returns true if a container is currently visible.
    pub fn is_visible(&self, container: &str) -> bool {
        self.visible.iter().any(|c| c == container)
    }

    /// Returns the number of visible containers.
    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    /// Returns the total number of containers.
    pub fn total_count(&self) -> usize {
        self.all_containers.len()
    }
}

/// Tracks the state of a log download/export operation.
#[derive(Debug, Clone, PartialEq)]
pub enum LogDownloadState {
    /// No download in progress.
    Idle,
    /// Formatting log lines for download.
    Preparing,
    /// Formatted content is ready to save.
    Ready(String),
    /// An error occurred during download preparation.
    Error(String),
}

/// State for the log viewer component.
#[derive(Debug)]
pub struct LogViewerState {
    pub buffer: LogBuffer,
    pub stream_state: LogStreamState,
    pub settings: LogViewerSettings,
    pub container_filter: Option<ContainerFilter>,
    pub search_query: Option<String>,
    pub search_match_count: usize,
    pub current_search_index: Option<usize>,
    pub scroll_offset: usize,
    pub download_format: LogDownloadFormat,
    pub download_state: LogDownloadState,
    /// T348: Search mode (Highlight shows all lines with matches highlighted,
    /// Filter shows only matching lines).
    pub search_mode: LogSearchMode,
    /// T350: Parent controller (kind, name) discovered via owner references.
    pub parent_controller: Option<(String, String)>,
    /// T351: When true, fetch logs from the previous container instance.
    pub previous_container: bool,
    /// Pod metadata for header display.
    pub namespace: String,
    pub pod_name: String,
    pub owner_kind: Option<String>,
    pub owner_name: Option<String>,
    /// Timestamp of the last fetched log line, for polling with sinceTime.
    pub last_fetch_time: Option<String>,
    /// When true, signals the polling loop to clear and re-fetch logs.
    pub needs_refetch: bool,
    /// Sibling pods from the same owner (for pod dropdown).
    pub sibling_pods: Vec<String>,
    /// When set, signals the polling loop to switch to a different pod.
    pub switch_to_pod: Option<String>,
}

impl LogViewerState {
    /// Creates a new log viewer with the given max line capacity.
    pub fn new(max_lines: usize) -> Self {
        Self {
            buffer: LogBuffer::new(max_lines),
            stream_state: LogStreamState::Idle,
            settings: LogViewerSettings::default(),
            container_filter: None,
            search_query: None,
            search_match_count: 0,
            current_search_index: None,
            scroll_offset: 0,
            download_format: LogDownloadFormat::PlainText,
            download_state: LogDownloadState::Idle,
            search_mode: LogSearchMode::default(),
            parent_controller: None,
            previous_container: false,
            namespace: String::new(),
            pod_name: String::new(),
            owner_kind: None,
            owner_name: None,
            last_fetch_time: None,
            needs_refetch: false,
            sibling_pods: Vec::new(),
            switch_to_pod: None,
        }
    }

    /// Push a log line, respecting the container filter.
    /// Strips ANSI escape sequences to prevent terminal injection via K8s logs.
    pub fn push_line(&mut self, mut line: LogLine) {
        line.content = strip_ansi_escapes(&line.content);
        self.buffer.push(line);
        // Update search count if active
        if self.search_query.is_some() {
            self.search_match_count = self.buffer.search_results().len();
        }
    }

    /// Set the stream state.
    pub fn set_stream_state(&mut self, state: LogStreamState) {
        self.stream_state = state;
    }

    /// Set the search query, updating match counts.
    pub fn set_search(&mut self, query: Option<String>) {
        self.search_query = query.clone();
        self.buffer.set_search(query);
        self.search_match_count = self.buffer.search_results().len();
        self.current_search_index = if self.search_match_count > 0 { Some(0) } else { None };
    }

    /// Navigate to the next search match.
    pub fn next_search_match(&mut self) {
        if self.search_match_count == 0 {
            return;
        }
        self.current_search_index =
            Some(self.current_search_index.map(|i| (i + 1) % self.search_match_count).unwrap_or(0));
    }

    /// Navigate to the previous search match.
    pub fn prev_search_match(&mut self) {
        if self.search_match_count == 0 {
            return;
        }
        self.current_search_index = Some(
            self.current_search_index
                .map(|i| if i == 0 { self.search_match_count - 1 } else { i - 1 })
                .unwrap_or(self.search_match_count - 1),
        );
    }

    /// Clear search.
    pub fn clear_search(&mut self) {
        self.set_search(None);
    }

    /// Set container filter for multi-container pods.
    pub fn set_container_filter(&mut self, containers: Vec<String>) {
        self.container_filter = Some(ContainerFilter::new(containers));
    }

    /// Get visible log lines, respecting container filter and search mode.
    ///
    /// In Filter mode (or when no search mode distinction matters), the buffer's
    /// `filtered_lines()` is used which already filters by the search query.
    /// In Highlight mode, all lines are returned (search matches are rendered
    /// with accent highlighting in the view layer).
    pub fn visible_lines(&self) -> Vec<&LogLine> {
        let lines = match self.search_mode {
            LogSearchMode::Filter => self.buffer.filtered_lines(),
            LogSearchMode::Highlight => self.buffer.lines().iter().collect(),
        };
        match &self.container_filter {
            Some(filter) => {
                lines.into_iter().filter(|line| filter.is_visible(&line.container_name)).collect()
            }
            None => lines,
        }
    }

    /// Total lines in buffer.
    pub fn line_count(&self) -> usize {
        self.buffer.len()
    }

    /// Clear all log lines.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.search_match_count = 0;
        self.current_search_index = None;
    }

    /// Toggle a display setting.
    pub fn toggle_timestamps(&mut self) {
        self.settings.show_timestamps = !self.settings.show_timestamps;
    }

    pub fn toggle_wrap_lines(&mut self) {
        self.settings.wrap_lines = !self.settings.wrap_lines;
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.settings.auto_scroll = !self.settings.auto_scroll;
    }

    /// T348: Toggle search mode between Highlight and Filter.
    pub fn toggle_search_mode(&mut self) {
        self.search_mode = self.search_mode.toggle();
    }

    /// T350: Set the parent controller (kind, name) discovered from owner references.
    pub fn set_parent_controller(&mut self, kind: String, name: String) {
        self.parent_controller = Some((kind, name));
    }

    /// T350: Clear the parent controller.
    pub fn clear_parent_controller(&mut self) {
        self.parent_controller = None;
    }

    /// T351: Toggle the previous container flag.
    pub fn toggle_previous_container(&mut self) {
        self.previous_container = !self.previous_container;
    }

    /// Set the download format.
    pub fn set_download_format(&mut self, format: LogDownloadFormat) {
        self.download_format = format;
    }

    /// Returns true if the stream is actively receiving logs.
    pub fn is_streaming(&self) -> bool {
        self.stream_state == LogStreamState::Streaming
    }

    /// Returns true if the stream is paused.
    pub fn is_paused(&self) -> bool {
        self.stream_state == LogStreamState::Paused
    }

    /// Prepare a download of the current visible log lines in the configured
    /// download format. Returns the formatted string ready to be saved to a file.
    /// Also transitions the download state through Preparing -> Ready.
    pub fn prepare_download(&mut self) -> String {
        self.download_state = LogDownloadState::Preparing;
        let visible: Vec<LogLine> = self.visible_lines().into_iter().cloned().collect();
        let formatted = format_logs_for_download(&visible, self.download_format);
        self.download_state = LogDownloadState::Ready(formatted.clone());
        formatted
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T046, T047, T048)
// ---------------------------------------------------------------------------

/// Palette of colors for source-colored container prefixes.
const CONTAINER_PALETTE: &[(u8, u8, u8)] = &[
    (96, 165, 250),  // blue
    (74, 222, 128),  // green
    (251, 191, 36),  // amber
    (248, 113, 113), // red
    (167, 139, 250), // purple
    (45, 212, 191),  // teal
    (251, 146, 60),  // orange
    (244, 114, 182), // pink
];

/// View wrapper for `LogViewerState` that holds a theme for rendering.
pub struct LogViewerView {
    pub state: LogViewerState,
    pub theme: Theme,
    /// Whether the container dropdown is open.
    pub container_dropdown_open: bool,
    /// Whether the pod dropdown is open.
    pub pod_dropdown_open: bool,
    /// The active container name for the log viewer.
    pub container_name: String,
    /// The cluster context string for this log viewer.
    pub cluster_context: String,
    /// Search input entity (created lazily on first render).
    search_input: Option<Entity<InputState>>,
    /// Subscription for search input change events.
    _search_subscription: Option<Subscription>,
}

impl LogViewerView {
    pub fn new(state: LogViewerState, theme: Theme) -> Self {
        Self {
            state,
            theme,
            container_dropdown_open: false,
            pod_dropdown_open: false,
            container_name: String::new(),
            cluster_context: String::new(),
            search_input: None,
            _search_subscription: None,
        }
    }

    /// Ensure the search input entity exists, creating it lazily.
    fn ensure_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search logs..."));
        let sub = cx.subscribe(&input, |this: &mut Self, entity, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let val = entity.read(cx).value().to_string();
                let query = if val.is_empty() { None } else { Some(val) };
                this.state.set_search(query);
                cx.notify();
            }
        });
        self.search_input = Some(input);
        self._search_subscription = Some(sub);
    }

    // --- T047: Streaming integration methods ---

    /// Sets state to Streaming.
    pub fn start_streaming(&mut self) {
        self.state.set_stream_state(LogStreamState::Streaming);
    }

    /// Sets state to Paused.
    pub fn pause_streaming(&mut self) {
        self.state.set_stream_state(LogStreamState::Paused);
    }

    /// Sets state to Stopped.
    pub fn stop_streaming(&mut self) {
        self.state.set_stream_state(LogStreamState::Stopped);
    }

    /// Pushes a log line, delegating to the inner state.
    pub fn push_line(&mut self, line: LogLine) {
        self.state.push_line(line);
    }

    // --- Helper: determine log level color ---

    /// Returns the theme color for a log line based on content.
    pub fn level_color_for_line(&self, line: &LogLine) -> crate::theme::Color {
        let lower = line.content.to_lowercase();
        if lower.contains("error") {
            self.theme.colors.error
        } else if lower.contains("warn") {
            self.theme.colors.warning
        } else if lower.contains("info") {
            self.theme.colors.info
        } else {
            self.theme.colors.text_primary
        }
    }

    /// Returns the palette color for a source_color_index.
    pub fn container_color(index: usize) -> crate::theme::Color {
        let (r, g, b) = CONTAINER_PALETTE[index % CONTAINER_PALETTE.len()];
        crate::theme::Color::rgb(r, g, b)
    }

    /// Display label for the current stream state.
    pub fn stream_state_label(&self) -> &'static str {
        match self.state.stream_state {
            LogStreamState::Idle => "Idle",
            LogStreamState::Streaming => "Streaming",
            LogStreamState::Paused => "Paused",
            LogStreamState::Stopped => "Stopped",
            LogStreamState::Error => "Error",
        }
    }

    /// Theme color for the current stream state.
    pub fn stream_state_color(&self) -> crate::theme::Color {
        match self.state.stream_state {
            LogStreamState::Idle => self.theme.colors.text_muted,
            LogStreamState::Streaming => self.theme.colors.success,
            LogStreamState::Paused => self.theme.colors.warning,
            LogStreamState::Stopped => self.theme.colors.text_secondary,
            LogStreamState::Error => self.theme.colors.error,
        }
    }

    /// Returns true if a log line matches the active search.
    pub fn line_matches_search(&self, line: &LogLine) -> bool {
        if let Some(ref query) = self.state.search_query {
            if query.is_empty() {
                return true;
            }
            line.content.to_lowercase().contains(&query.to_lowercase())
        } else {
            false
        }
    }

    /// True if auto-scroll is off (show indicator).
    pub fn show_scroll_to_bottom_indicator(&self) -> bool {
        !self.state.settings.auto_scroll
    }

    // --- Render helpers (each returns Div) ---

    /// Stream state indicator (colored dot + label).
    fn render_stream_indicator(&self) -> gpui::Div {
        let c = self.stream_state_color().to_gpui();
        let lbl = SharedString::from(self.stream_state_label());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(c))
            .child(div().text_xs().text_color(c).child(lbl))
    }

    /// Download format button.
    fn render_download_button(&self, colors: &LogViewerColors) -> gpui::Stateful<gpui::Div> {
        let txt = match self.state.download_format {
            LogDownloadFormat::PlainText => "Download TXT",
            LogDownloadFormat::Json => "Download JSON",
            LogDownloadFormat::Csv => "Download CSV",
        };
        div()
            .id("download-btn")
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_xs()
            .text_color(colors.text_primary)
            .child(SharedString::from(txt))
    }

    /// Container selector — dropdown that renders above all content via deferred().
    fn render_container_selector_interactive(
        &self,
        colors: &LogViewerColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let filter = match &self.state.container_filter {
            Some(f) => f,
            None => return div(),
        };
        let vis = filter.visible_count();
        let tot = filter.total_count();
        let summary = SharedString::from(format!("Containers: {vis}/{tot}"));
        let arrow = if self.container_dropdown_open { "\u{25B2}" } else { "\u{25BC}" };

        self.render_dropdown_widget(
            "container-selector",
            summary,
            arrow,
            self.container_dropdown_open,
            colors,
            cx,
            |this, _ev, _win, cx| {
                this.container_dropdown_open = !this.container_dropdown_open;
                this.pod_dropdown_open = false;
                cx.notify();
            },
            |this, colors, cx| {
                let f = this.state.container_filter.as_ref()?;
                let all_active = f.visible_count() == f.total_count();
                let all_c = if all_active { colors.accent } else { colors.text_secondary };
                let mut els: Vec<gpui::AnyElement> = Vec::new();
                els.push(
                    div()
                        .id("container-all")
                        .px_2()
                        .py_1()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(all_c)
                        .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                        .child(SharedString::from(if all_active {
                            "\u{2611} All"
                        } else {
                            "\u{2610} All"
                        }))
                        .on_click(cx.listener(|this, _ev, _win, cx| {
                            if let Some(ref mut f) = this.state.container_filter {
                                if f.visible_count() == f.total_count() {
                                    f.hide_all();
                                } else {
                                    f.show_all();
                                }
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                for (i, name) in f.all_containers.iter().enumerate() {
                    let iv = f.is_visible(name);
                    let pc = Self::container_color(i).to_gpui();
                    let ck = if iv { "\u{2611} " } else { "\u{2610} " };
                    let lb = SharedString::from(format!("{ck}{name}"));
                    let tc = if iv { colors.text_primary } else { colors.text_muted };
                    let cn = name.clone();
                    els.push(
                        this.render_container_item(&format!("c-{i}"), pc, lb, tc)
                            .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                            .on_click(cx.listener(move |this, _ev, _win, cx| {
                                if let Some(ref mut f) = this.state.container_filter {
                                    f.toggle(&cn);
                                }
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                Some(els)
            },
        )
    }

    /// Pod selector — dropdown of sibling pods from the same owner.
    fn render_pod_selector_interactive(
        &self,
        colors: &LogViewerColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if self.state.sibling_pods.len() <= 1 {
            if !self.state.pod_name.is_empty() {
                return self.render_meta_chip("Pod", &self.state.pod_name, colors);
            }
            return div();
        }
        let summary = SharedString::from(format!("Pod: {}", self.state.pod_name));
        let arrow = if self.pod_dropdown_open { "\u{25B2}" } else { "\u{25BC}" };
        self.render_dropdown_widget(
            "pod-selector",
            summary,
            arrow,
            self.pod_dropdown_open,
            colors,
            cx,
            |this, _ev, _win, cx| {
                this.pod_dropdown_open = !this.pod_dropdown_open;
                this.container_dropdown_open = false;
                cx.notify();
            },
            |this, colors, cx| {
                let mut els: Vec<gpui::AnyElement> = Vec::new();
                for (i, pn) in this.state.sibling_pods.iter().enumerate() {
                    let active = *pn == this.state.pod_name;
                    let tc = if active { colors.accent } else { colors.text_primary };
                    let prefix = if active { "\u{25CF} " } else { "  " };
                    let label = SharedString::from(format!("{prefix}{pn}"));
                    let pn_clone = pn.clone();
                    els.push(
                        div()
                            .id(ElementId::Name(SharedString::from(format!("pod-{i}"))))
                            .px_2()
                            .py_1()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(tc)
                            .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                            .child(label)
                            .on_click(cx.listener(move |this, _ev, _win, cx| {
                                if this.state.pod_name != pn_clone {
                                    this.state.pod_name = pn_clone.clone();
                                    this.state.clear();
                                    this.state.switch_to_pod = Some(pn_clone.clone());
                                    this.state.needs_refetch = true;
                                    this.state
                                        .set_stream_state(baeus_core::logs::LogStreamState::Idle);
                                }
                                this.pod_dropdown_open = false;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                Some(els)
            },
        )
    }

    /// Generic dropdown: toggle button + absolute/deferred floating panel.
    #[allow(clippy::too_many_arguments)]
    fn render_dropdown_widget(
        &self,
        id_prefix: &str,
        summary: SharedString,
        arrow: &str,
        is_open: bool,
        colors: &LogViewerColors,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        build_items: impl FnOnce(
            &Self,
            &LogViewerColors,
            &mut Context<Self>,
        ) -> Option<Vec<gpui::AnyElement>>,
    ) -> gpui::Div {
        let toggle_id = ElementId::Name(SharedString::from(format!("{id_prefix}-toggle")));
        let arrow_s = SharedString::from(arrow.to_string());

        let mut wrapper = div().relative();
        wrapper = wrapper.child(
            div()
                .id(toggle_id)
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .bg(colors.surface)
                .border_1()
                .border_color(colors.border)
                .cursor_pointer()
                .text_xs()
                .text_color(colors.text_primary)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(summary)
                        .child(div().text_xs().text_color(colors.text_muted).child(arrow_s)),
                )
                .on_click(cx.listener(on_toggle)),
        );
        if is_open {
            if let Some(els) = build_items(self, colors, cx) {
                let panel_id = ElementId::Name(SharedString::from(format!("{id_prefix}-panel")));
                let mut panel = div()
                    .id(panel_id)
                    .absolute()
                    .top(px(28.0))
                    .left_0()
                    .flex()
                    .flex_col()
                    .w(px(280.0))
                    .max_h(px(300.0))
                    .overflow_y_scroll()
                    .bg(colors.surface)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(4.0))
                    .shadow_lg()
                    .py_1()
                    .occlude();
                for el in els {
                    panel = panel.child(el);
                }
                wrapper = wrapper.child(deferred(panel));
            }
        }
        wrapper
    }

    /// Single container item row.
    fn render_container_item(
        &self,
        id: &str,
        dot_color: Rgba,
        label: SharedString,
        text_color: Rgba,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(ElementId::Name(SharedString::from(id.to_string())))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px_2()
            .py_1()
            .cursor_pointer()
            .child(div().w(px(6.0)).h(px(6.0)).rounded(px(3.0)).bg(dot_color))
            .child(div().text_xs().text_color(text_color).child(label))
    }

    /// Metadata label: non-interactive key:value display.
    fn render_meta_chip(&self, label: &str, value: &str, colors: &LogViewerColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(format!("{label}:"))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(value.to_string())),
            )
    }

    /// Checkbox-style toggle: [x] or [ ] with label.
    fn render_checkbox_toggle(
        &self,
        id: &str,
        label: &str,
        checked: bool,
        colors: &LogViewerColors,
    ) -> gpui::Stateful<gpui::Div> {
        let check_str = if checked { "\u{2611}" } else { "\u{2610}" };
        let tc = if checked { colors.accent } else { colors.text_muted };
        div()
            .id(ElementId::Name(SharedString::from(id.to_string())))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .cursor_pointer()
            .child(div().text_xs().text_color(tc).child(SharedString::from(check_str)))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(label.to_string())),
            )
    }

    /// Single log line.
    /// T348: In Highlight mode, matching lines get an accent-tinted background.
    /// In Filter mode (all visible lines already match), the selection bg is used.
    fn render_log_line(
        &self,
        line: &LogLine,
        index: usize,
        colors: &LogViewerColors,
    ) -> gpui::Stateful<gpui::Div> {
        let lc = self.level_color_for_line(line).to_gpui();
        let im = self.line_matches_search(line);
        let ids = format!("log-line-{index}");

        let mut row = div()
            .id(ElementId::Name(SharedString::from(ids)))
            .flex()
            .flex_row()
            .w_full()
            .px_2()
            .py(px(1.0));

        if im {
            match self.state.search_mode {
                LogSearchMode::Highlight => {
                    // Accent-colored highlight for matches in Highlight mode
                    row = row.bg(colors.highlight_accent);
                }
                LogSearchMode::Filter => {
                    // Selection background in Filter mode (all visible lines match)
                    row = row.bg(colors.selection);
                }
            }
        }

        // Container prefix
        if self.state.container_filter.is_some() {
            let pc = Self::container_color(line.source_color_index).to_gpui();
            let cl = SharedString::from(format!("[{}] ", line.container_name,));
            row = row.child(div().text_xs().text_color(pc).flex_shrink_0().mr_1().child(cl));
        }

        // Timestamp
        if self.state.settings.show_timestamps {
            let ts =
                line.timestamp.map(|t| t.format("%H:%M:%S%.3f").to_string()).unwrap_or_default();
            row = row.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .flex_shrink_0()
                    .mr_2()
                    .child(SharedString::from(ts)),
            );
        }

        // Content
        let ct = SharedString::from(line.content.clone());
        let mut content_div = div().text_xs().text_color(lc).flex_1().min_w(px(0.0)).child(ct);
        if !self.state.settings.wrap_lines {
            content_div = content_div.whitespace_nowrap();
        }
        row = row.child(content_div);

        row
    }

    /// Scrollable monospace log body.
    fn render_log_body(&self, colors: &LogViewerColors) -> gpui::Stateful<gpui::Div> {
        let visible = self.state.visible_lines();
        let wrap = self.state.settings.wrap_lines;
        let mut body = div()
            .id("log-viewer-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .font_family("Menlo")
            .text_xs()
            .w_full()
            .bg(colors.background);

        if !wrap {
            body = body.overflow_x_scroll();
        }

        if visible.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .py_4()
                    .text_color(colors.text_muted)
                    .child("No log lines"),
            );
        } else {
            for (i, line) in visible.iter().enumerate() {
                body = body.child(self.render_log_line(line, i, colors));
            }
        }

        body
    }

    /// Download status indicator.
    fn render_download_status(&self, colors: &LogViewerColors) -> gpui::Div {
        match &self.state.download_state {
            LogDownloadState::Idle => div(),
            LogDownloadState::Preparing => {
                div().text_xs().text_color(colors.text_muted).child("Preparing...")
            }
            LogDownloadState::Ready(_) => div().text_xs().text_color(colors.success).child("Ready"),
            LogDownloadState::Error(msg) => {
                let em = SharedString::from(msg.clone());
                div().text_xs().text_color(colors.error).child(em)
            }
        }
    }

    /// Header bar with real search input and next/prev buttons.
    fn render_header_with_search(
        &self,
        colors: &LogViewerColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border);

        // Stream state indicator
        row = row.child(self.render_stream_indicator());

        // Namespace chip
        if !self.state.namespace.is_empty() {
            row = row.child(self.render_meta_chip("Namespace", &self.state.namespace, colors));
        }

        // Owner chip
        if let Some(ref kind) = self.state.owner_kind {
            let name = self.state.owner_name.as_deref().unwrap_or("unknown");
            let val = format!("{kind}/{name}");
            row = row.child(self.render_meta_chip("Owner", &val, colors));
        }

        // Pod selector (dropdown if siblings exist, plain label otherwise)
        row = row.child(self.render_pod_selector_interactive(colors, cx));

        // Container selector
        row = row.child(self.render_container_selector_interactive(colors, cx));

        // Spacer
        row = row.child(div().flex_1());

        // Real search input
        if let Some(ref input_entity) = self.search_input {
            row = row
                .child(div().w(px(200.0)).child(Input::new(input_entity).appearance(true).small()));
        }

        // Match count + nav buttons
        if self.state.search_query.is_some() && self.state.search_match_count > 0 {
            let idx = self.state.current_search_index.map(|i| i + 1).unwrap_or(0);
            let cnt = self.state.search_match_count;
            row = row.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(format!("{idx}/{cnt}"))),
            );
            row = row.child(
                div()
                    .id("search-prev")
                    .cursor_pointer()
                    .text_xs()
                    .text_color(colors.text_primary)
                    .hover(|s| s.text_color(colors.accent))
                    .child("\u{25B2}") // ▲
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.state.prev_search_match();
                        cx.notify();
                    })),
            );
            row = row.child(
                div()
                    .id("search-next")
                    .cursor_pointer()
                    .text_xs()
                    .text_color(colors.text_primary)
                    .hover(|s| s.text_color(colors.accent))
                    .child("\u{25BC}") // ▼
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.state.next_search_match();
                        cx.notify();
                    })),
            );
        } else if self.state.search_query.is_some() {
            row = row.child(div().text_xs().text_color(colors.text_muted).child("0 matches"));
        }

        row
    }

    /// Footer with working click handlers for toggles.
    fn render_footer_with_handlers(
        &self,
        colors: &LogViewerColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let lines = self.state.buffer.lines();
        let first_ts = lines
            .first()
            .and_then(|l| l.timestamp)
            .map(|t| t.format("%-m/%-d/%Y, %-I:%M:%S %p").to_string());

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_1()
            .gap(px(12.0))
            .border_t_1()
            .border_color(colors.border);

        // Log start timestamp
        if let Some(ts) = first_ts {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(format!("Logs from {ts}"))),
            );
        }

        // Spacer
        row = row.child(div().flex_1());

        // Show timestamps toggle
        row = row.child(
            self.render_checkbox_toggle(
                "ft-timestamps",
                "Show timestamps",
                self.state.settings.show_timestamps,
                colors,
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.toggle_timestamps();
                cx.notify();
            })),
        );

        // Previous container toggle
        row = row.child(
            self.render_checkbox_toggle(
                "ft-previous",
                "Show previous terminated container",
                self.state.previous_container,
                colors,
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.toggle_previous_container();
                this.state.clear();
                this.state.needs_refetch = true;
                this.state.set_stream_state(baeus_core::logs::LogStreamState::Idle);
                cx.notify();
            })),
        );

        // Wrap lines toggle
        row = row.child(
            self.render_checkbox_toggle(
                "ft-wrap",
                "Wrap lines",
                self.state.settings.wrap_lines,
                colors,
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.toggle_wrap_lines();
                cx.notify();
            })),
        );

        // Auto-scroll toggle
        row = row.child(
            self.render_checkbox_toggle(
                "ft-follow",
                "Auto-scroll",
                self.state.settings.auto_scroll,
                colors,
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.toggle_auto_scroll();
                cx.notify();
            })),
        );

        // Search mode toggle
        row = row.child(
            self.render_checkbox_toggle(
                "ft-search-mode",
                self.state.search_mode.label(),
                self.state.search_mode == LogSearchMode::Filter,
                colors,
            )
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.toggle_search_mode();
                cx.notify();
            })),
        );

        // Download button
        row = row.child(self.render_download_button(colors).on_click(cx.listener(
            |this, _event, _window, cx| {
                let content = this.state.prepare_download();
                let ext = match this.state.download_format {
                    LogDownloadFormat::PlainText => "txt",
                    LogDownloadFormat::Json => "json",
                    LogDownloadFormat::Csv => "csv",
                };
                let pod =
                    if this.state.pod_name.is_empty() { "logs" } else { &this.state.pod_name };
                let filename = format!("{pod}-logs.{ext}");
                let dir = std::env::temp_dir().join("baeus-logs");
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(&filename);
                if std::fs::write(&path, &content).is_ok() {
                    let _ = std::process::Command::new("open").arg(&path).spawn();
                    this.state.download_state = LogDownloadState::Idle;
                } else {
                    this.state.download_state =
                        LogDownloadState::Error("Failed to write file".to_string());
                }
                cx.notify();
            },
        )));

        // Download status
        row = row.child(self.render_download_status(colors));

        row
    }
}

/// Precomputed colors for rendering the log viewer.
struct LogViewerColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    selection: Rgba,
    /// T348: accent-tinted background for search matches in Highlight mode.
    highlight_accent: Rgba,
}

impl Render for LogViewerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure search input exists
        self.ensure_search_input(window, cx);

        let colors = LogViewerColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            selection: self.theme.colors.selection.to_gpui(),
            highlight_accent: crate::theme::Color::rgba(
                self.theme.colors.accent.r,
                self.theme.colors.accent.g,
                self.theme.colors.accent.b,
                50, // semi-transparent accent overlay
            )
            .to_gpui(),
        };

        // Build header with real search input (needs cx)
        let header = self.render_header_with_search(&colors, cx);
        // Build footer with click handlers (needs cx)
        let footer = self.render_footer_with_handlers(&colors, cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .child(header)
            .child(self.render_log_body(&colors))
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_log_line(content: &str, container: &str) -> LogLine {
        LogLine {
            timestamp: Some(Utc::now()),
            content: content.to_string(),
            container_name: container.to_string(),
            pod_name: "test-pod".to_string(),
            source_color_index: 0,
        }
    }

    // --- LogViewerSettings tests ---

    #[test]
    fn test_default_settings() {
        let settings = LogViewerSettings::default();
        assert!(settings.show_timestamps);
        assert!(!settings.wrap_lines);
        assert!(settings.auto_scroll);
        assert_eq!(settings.font_size, 12);
    }

    // --- ContainerFilter tests ---

    #[test]
    fn test_container_filter_new() {
        let filter = ContainerFilter::new(vec!["app".to_string(), "sidecar".to_string()]);
        assert_eq!(filter.total_count(), 2);
        assert_eq!(filter.visible_count(), 2);
        assert!(filter.is_visible("app"));
        assert!(filter.is_visible("sidecar"));
    }

    #[test]
    fn test_container_filter_toggle() {
        let mut filter = ContainerFilter::new(vec!["app".to_string(), "sidecar".to_string()]);

        filter.toggle("app");
        assert!(!filter.is_visible("app"));
        assert!(filter.is_visible("sidecar"));
        assert_eq!(filter.visible_count(), 1);

        filter.toggle("app");
        assert!(filter.is_visible("app"));
        assert_eq!(filter.visible_count(), 2);
    }

    #[test]
    fn test_container_filter_toggle_nonexistent() {
        let mut filter = ContainerFilter::new(vec!["app".to_string()]);
        filter.toggle("ghost");
        assert_eq!(filter.visible_count(), 1);
    }

    #[test]
    fn test_container_filter_show_hide_all() {
        let mut filter = ContainerFilter::new(vec!["app".to_string(), "sidecar".to_string()]);

        filter.hide_all();
        assert_eq!(filter.visible_count(), 0);
        assert!(!filter.is_visible("app"));

        filter.show_all();
        assert_eq!(filter.visible_count(), 2);
        assert!(filter.is_visible("app"));
    }

    // --- LogViewerState tests ---

    #[test]
    fn test_log_viewer_new() {
        let state = LogViewerState::new(1000);
        assert_eq!(state.line_count(), 0);
        assert_eq!(state.stream_state, LogStreamState::Idle);
        assert!(state.search_query.is_none());
        assert_eq!(state.search_match_count, 0);
        assert!(state.current_search_index.is_none());
        assert!(state.container_filter.is_none());
        assert!(!state.is_streaming());
        assert!(!state.is_paused());
    }

    #[test]
    fn test_push_line() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("hello", "app"));
        state.push_line(make_log_line("world", "app"));
        assert_eq!(state.line_count(), 2);
    }

    #[test]
    fn test_stream_state() {
        let mut state = LogViewerState::new(1000);
        assert!(!state.is_streaming());

        state.set_stream_state(LogStreamState::Streaming);
        assert!(state.is_streaming());
        assert!(!state.is_paused());

        state.set_stream_state(LogStreamState::Paused);
        assert!(!state.is_streaming());
        assert!(state.is_paused());
    }

    #[test]
    fn test_search() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("INFO: started", "app"));
        state.push_line(make_log_line("ERROR: failed", "app"));
        state.push_line(make_log_line("INFO: recovered", "app"));
        state.push_line(make_log_line("ERROR: timeout", "app"));

        state.set_search(Some("ERROR".to_string()));
        assert_eq!(state.search_match_count, 2);
        assert_eq!(state.current_search_index, Some(0));
    }

    #[test]
    fn test_search_navigation() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("ERROR: a", "app"));
        state.push_line(make_log_line("ERROR: b", "app"));
        state.push_line(make_log_line("ERROR: c", "app"));

        state.set_search(Some("ERROR".to_string()));
        assert_eq!(state.current_search_index, Some(0));

        state.next_search_match();
        assert_eq!(state.current_search_index, Some(1));

        state.next_search_match();
        assert_eq!(state.current_search_index, Some(2));

        state.next_search_match(); // wraps
        assert_eq!(state.current_search_index, Some(0));

        state.prev_search_match(); // wraps back
        assert_eq!(state.current_search_index, Some(2));

        state.prev_search_match();
        assert_eq!(state.current_search_index, Some(1));
    }

    #[test]
    fn test_search_no_matches() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("hello", "app"));

        state.set_search(Some("NOTFOUND".to_string()));
        assert_eq!(state.search_match_count, 0);
        assert!(state.current_search_index.is_none());

        // Navigation should be no-ops
        state.next_search_match();
        assert!(state.current_search_index.is_none());
        state.prev_search_match();
        assert!(state.current_search_index.is_none());
    }

    #[test]
    fn test_clear_search() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("ERROR: test", "app"));
        state.set_search(Some("ERROR".to_string()));
        assert_eq!(state.search_match_count, 1);

        state.clear_search();
        assert!(state.search_query.is_none());
        assert_eq!(state.search_match_count, 0);
        assert!(state.current_search_index.is_none());
    }

    #[test]
    fn test_container_filter_visible_lines() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("from app", "app"));
        state.push_line(make_log_line("from sidecar", "sidecar"));
        state.push_line(make_log_line("from app again", "app"));

        // No filter: all visible
        assert_eq!(state.visible_lines().len(), 3);

        // Filter to app only
        state.set_container_filter(vec!["app".to_string(), "sidecar".to_string()]);
        state.container_filter.as_mut().unwrap().toggle("sidecar");

        let visible = state.visible_lines();
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|l| l.container_name == "app"));
    }

    #[test]
    fn test_clear() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("test", "app"));
        state.set_search(Some("test".to_string()));
        assert_eq!(state.line_count(), 1);
        assert_eq!(state.search_match_count, 1);

        state.clear();
        assert_eq!(state.line_count(), 0);
        assert_eq!(state.search_match_count, 0);
        assert!(state.current_search_index.is_none());
    }

    #[test]
    fn test_toggle_settings() {
        let mut state = LogViewerState::new(1000);
        assert!(state.settings.show_timestamps);
        state.toggle_timestamps();
        assert!(!state.settings.show_timestamps);

        assert!(!state.settings.wrap_lines);
        state.toggle_wrap_lines();
        assert!(state.settings.wrap_lines);

        assert!(state.settings.auto_scroll);
        state.toggle_auto_scroll();
        assert!(!state.settings.auto_scroll);
    }

    #[test]
    fn test_set_download_format() {
        let mut state = LogViewerState::new(1000);
        state.set_download_format(LogDownloadFormat::Json);
        assert!(matches!(state.download_format, LogDownloadFormat::Json));

        state.set_download_format(LogDownloadFormat::Csv);
        assert!(matches!(state.download_format, LogDownloadFormat::Csv));
    }

    #[test]
    fn test_push_updates_search_count() {
        let mut state = LogViewerState::new(1000);
        state.set_search(Some("ERROR".to_string()));
        assert_eq!(state.search_match_count, 0);

        state.push_line(make_log_line("INFO: ok", "app"));
        assert_eq!(state.search_match_count, 0);

        state.push_line(make_log_line("ERROR: failed", "app"));
        assert_eq!(state.search_match_count, 1);
    }

    #[test]
    fn test_full_workflow() {
        let mut state = LogViewerState::new(5000);

        // Start streaming
        state.set_stream_state(LogStreamState::Streaming);
        assert!(state.is_streaming());

        // Set up multi-container
        state.set_container_filter(vec!["nginx".to_string(), "istio-proxy".to_string()]);

        // Push some logs
        for i in 0..10 {
            state.push_line(make_log_line(
                &format!("request {i}"),
                if i % 2 == 0 { "nginx" } else { "istio-proxy" },
            ));
        }
        assert_eq!(state.line_count(), 10);
        assert_eq!(state.visible_lines().len(), 10);

        // Filter to nginx only
        state.container_filter.as_mut().unwrap().toggle("istio-proxy");
        assert_eq!(state.visible_lines().len(), 5);

        // Search
        state.set_search(Some("request 4".to_string()));
        assert_eq!(state.search_match_count, 1);

        // Pause
        state.set_stream_state(LogStreamState::Paused);
        assert!(state.is_paused());
    }

    // --- T348: LogSearchMode tests ---

    #[test]
    fn test_search_mode_default_is_highlight() {
        let state = LogViewerState::new(1000);
        assert_eq!(state.search_mode, LogSearchMode::Highlight);
    }

    #[test]
    fn test_search_mode_toggle() {
        let mut state = LogViewerState::new(1000);
        assert_eq!(state.search_mode, LogSearchMode::Highlight);

        state.toggle_search_mode();
        assert_eq!(state.search_mode, LogSearchMode::Filter);

        state.toggle_search_mode();
        assert_eq!(state.search_mode, LogSearchMode::Highlight);
    }

    #[test]
    fn test_search_mode_label() {
        assert_eq!(LogSearchMode::Highlight.label(), "Highlight");
        assert_eq!(LogSearchMode::Filter.label(), "Filter");
    }

    #[test]
    fn test_highlight_mode_shows_all_lines_with_search() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("INFO: started", "app"));
        state.push_line(make_log_line("ERROR: failed", "app"));
        state.push_line(make_log_line("INFO: recovered", "app"));

        // Default is Highlight mode
        state.set_search(Some("ERROR".to_string()));
        assert_eq!(state.search_match_count, 1);

        // In Highlight mode, all lines are visible
        let visible = state.visible_lines();
        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_filter_mode_shows_only_matching_lines() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("INFO: started", "app"));
        state.push_line(make_log_line("ERROR: failed", "app"));
        state.push_line(make_log_line("INFO: recovered", "app"));

        state.search_mode = LogSearchMode::Filter;
        state.set_search(Some("ERROR".to_string()));

        // In Filter mode, only matching lines are visible
        let visible = state.visible_lines();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].content.contains("ERROR"));
    }

    #[test]
    fn test_search_mode_switch_changes_visible_lines() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("INFO: ok", "app"));
        state.push_line(make_log_line("ERROR: bad", "app"));
        state.push_line(make_log_line("WARN: watch", "app"));

        state.set_search(Some("ERROR".to_string()));

        // Highlight mode: all 3 visible
        assert_eq!(state.visible_lines().len(), 3);

        // Switch to Filter mode: only 1 visible
        state.toggle_search_mode();
        assert_eq!(state.visible_lines().len(), 1);

        // Switch back to Highlight: all 3 visible again
        state.toggle_search_mode();
        assert_eq!(state.visible_lines().len(), 3);
    }

    #[test]
    fn test_highlight_mode_no_search_shows_all() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("a", "app"));
        state.push_line(make_log_line("b", "app"));

        // No search set, Highlight mode
        assert_eq!(state.visible_lines().len(), 2);
    }

    #[test]
    fn test_filter_mode_no_search_shows_all() {
        let mut state = LogViewerState::new(1000);
        state.push_line(make_log_line("a", "app"));
        state.push_line(make_log_line("b", "app"));

        state.search_mode = LogSearchMode::Filter;
        // No search set: filtered_lines returns all
        assert_eq!(state.visible_lines().len(), 2);
    }

    // --- T350: Parent controller tests ---

    #[test]
    fn test_parent_controller_default_none() {
        let state = LogViewerState::new(1000);
        assert!(state.parent_controller.is_none());
    }

    #[test]
    fn test_set_parent_controller() {
        let mut state = LogViewerState::new(1000);
        state.set_parent_controller("Deployment".to_string(), "nginx-deploy".to_string());

        assert_eq!(
            state.parent_controller,
            Some(("Deployment".to_string(), "nginx-deploy".to_string()))
        );
    }

    #[test]
    fn test_set_parent_controller_statefulset() {
        let mut state = LogViewerState::new(1000);
        state.set_parent_controller("StatefulSet".to_string(), "redis".to_string());

        let (kind, name) = state.parent_controller.as_ref().unwrap();
        assert_eq!(kind, "StatefulSet");
        assert_eq!(name, "redis");
    }

    #[test]
    fn test_clear_parent_controller() {
        let mut state = LogViewerState::new(1000);
        state.set_parent_controller("DaemonSet".to_string(), "fluentd".to_string());
        assert!(state.parent_controller.is_some());

        state.clear_parent_controller();
        assert!(state.parent_controller.is_none());
    }

    #[test]
    fn test_set_parent_controller_overwrites() {
        let mut state = LogViewerState::new(1000);
        state.set_parent_controller("Deployment".to_string(), "v1".to_string());
        state.set_parent_controller("StatefulSet".to_string(), "v2".to_string());

        let (kind, name) = state.parent_controller.as_ref().unwrap();
        assert_eq!(kind, "StatefulSet");
        assert_eq!(name, "v2");
    }

    // --- T351: Previous container tests ---

    #[test]
    fn test_previous_container_default_false() {
        let state = LogViewerState::new(1000);
        assert!(!state.previous_container);
    }

    #[test]
    fn test_toggle_previous_container() {
        let mut state = LogViewerState::new(1000);
        assert!(!state.previous_container);

        state.toggle_previous_container();
        assert!(state.previous_container);

        state.toggle_previous_container();
        assert!(!state.previous_container);
    }

    #[test]
    fn test_previous_container_independent_of_other_settings() {
        let mut state = LogViewerState::new(1000);
        state.toggle_previous_container();
        assert!(state.previous_container);

        // Other settings unchanged
        assert!(state.settings.show_timestamps);
        assert!(!state.settings.wrap_lines);
        assert!(state.settings.auto_scroll);
        assert_eq!(state.search_mode, LogSearchMode::Highlight);
    }
}
