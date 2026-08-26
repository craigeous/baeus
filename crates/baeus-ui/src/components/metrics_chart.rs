use gpui::{Context, ElementId, FontWeight, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// T365: MetricsAvailability — graceful degradation for absent metrics-server
// ---------------------------------------------------------------------------

/// Describes the availability state of the cluster's metrics-server.
///
/// When the metrics-server API returns a 404 or the connection is refused,
/// the UI should still render all non-metrics dashboard sections and display
/// a helpful panel with install instructions instead of a chart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MetricsAvailability {
    /// Metrics-server is reachable and returning data.
    Available,
    /// Metrics-server is not reachable. `message` contains a user-facing
    /// explanation (e.g. "metrics-server returned HTTP 404").
    Unavailable { message: String },
    /// We are currently probing the metrics-server endpoint.
    #[default]
    Loading,
}

impl MetricsAvailability {
    /// Returns `true` when metrics data can be displayed.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns `true` when we know for sure that metrics-server is absent.
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }

    /// Returns `true` while we are still checking.
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Construct the Unavailable variant from an HTTP status code or
    /// connection error description.
    pub fn from_error(error: &str) -> Self {
        Self::Unavailable { message: error.to_string() }
    }

    /// Human-readable header for the unavailable panel.
    pub fn header() -> &'static str {
        "Metrics server is not available"
    }

    /// Explanation paragraph shown when metrics-server is absent.
    pub fn explanation() -> &'static str {
        "CPU and memory metrics require metrics-server to be installed in your cluster."
    }

    /// kubectl command the user can run to install metrics-server.
    pub fn install_command() -> &'static str {
        "kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml"
    }
}

/// The type of metric being displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricKind {
    Cpu,
    Memory,
}

impl MetricKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::Cpu => "millicores",
            Self::Memory => "bytes",
        }
    }
}

/// A single data point in a metrics time series.
#[derive(Debug, Clone)]
pub struct MetricDataPoint {
    pub timestamp_secs: f64,
    pub value: f64,
}

/// A time series of metric data for a single entity (node or pod).
#[derive(Debug, Clone)]
pub struct MetricSeries {
    pub label: String,
    pub kind: MetricKind,
    pub data: Vec<MetricDataPoint>,
    pub capacity: Option<f64>,
}

impl MetricSeries {
    pub fn new(label: &str, kind: MetricKind) -> Self {
        Self { label: label.to_string(), kind, data: Vec::new(), capacity: None }
    }

    pub fn with_capacity(mut self, capacity: f64) -> Self {
        self.capacity = Some(capacity);
        self
    }

    pub fn push(&mut self, timestamp_secs: f64, value: f64) {
        self.data.push(MetricDataPoint { timestamp_secs, value });
    }

    pub fn latest_value(&self) -> Option<f64> {
        self.data.last().map(|d| d.value)
    }

    pub fn max_value(&self) -> Option<f64> {
        self.data.iter().map(|d| d.value).reduce(f64::max)
    }

    pub fn min_value(&self) -> Option<f64> {
        self.data.iter().map(|d| d.value).reduce(f64::min)
    }

    pub fn average_value(&self) -> Option<f64> {
        if self.data.is_empty() {
            return None;
        }
        let sum: f64 = self.data.iter().map(|d| d.value).sum();
        Some(sum / self.data.len() as f64)
    }

    pub fn usage_percent(&self) -> Option<f64> {
        match (self.latest_value(), self.capacity) {
            (Some(value), Some(cap)) if cap > 0.0 => Some((value / cap) * 100.0),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Chart display configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartStyle {
    Line,
    Area,
    Bar,
}

impl ChartStyle {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Area => "Area",
            Self::Bar => "Bar",
        }
    }

    /// Returns all chart style variants for iterating in the UI.
    pub fn all() -> &'static [ChartStyle] {
        &[ChartStyle::Line, ChartStyle::Area, ChartStyle::Bar]
    }
}

/// State for the metrics chart component.
#[derive(Debug)]
pub struct MetricsChartState {
    pub series: Vec<MetricSeries>,
    pub chart_style: ChartStyle,
    pub show_legend: bool,
    pub time_range_secs: u64,
    /// T365: Rich availability enum replacing the old `metrics_available: bool`.
    pub availability: MetricsAvailability,
    /// Kept for backward-compat: `true` iff `availability == Available`.
    pub metrics_available: bool,
    pub empty_state_message: Option<String>,
    pub loading: bool,
}

impl Default for MetricsChartState {
    fn default() -> Self {
        Self {
            series: Vec::new(),
            chart_style: ChartStyle::Area,
            show_legend: true,
            time_range_secs: 3600, // 1 hour
            availability: MetricsAvailability::Available,
            metrics_available: true,
            empty_state_message: None,
            loading: false,
        }
    }
}

impl MetricsChartState {
    pub fn new(style: ChartStyle) -> Self {
        Self { chart_style: style, ..Default::default() }
    }

    pub fn add_series(&mut self, series: MetricSeries) {
        self.series.push(series);
    }

    pub fn clear_series(&mut self) {
        self.series.clear();
    }

    pub fn set_time_range(&mut self, secs: u64) {
        self.time_range_secs = secs;
    }

    pub fn set_unavailable(&mut self, message: &str) {
        self.metrics_available = false;
        self.availability = MetricsAvailability::Unavailable { message: message.to_string() };
        self.empty_state_message = Some(message.to_string());
    }

    pub fn set_available(&mut self) {
        self.metrics_available = true;
        self.availability = MetricsAvailability::Available;
        self.empty_state_message = None;
    }

    /// T365: Set availability from a `MetricsAvailability` value directly.
    pub fn set_availability(&mut self, availability: MetricsAvailability) {
        self.metrics_available = availability.is_available();
        if let MetricsAvailability::Unavailable { ref message } = availability {
            self.empty_state_message = Some(message.clone());
        } else {
            self.empty_state_message = None;
        }
        self.availability = availability;
    }

    /// T365: Begin a metrics-server availability check (sets Loading state).
    pub fn begin_availability_check(&mut self) {
        self.availability = MetricsAvailability::Loading;
        self.loading = true;
    }

    pub fn has_data(&self) -> bool {
        self.series.iter().any(|s| !s.is_empty())
    }

    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// Returns a user-facing message describing why metrics are unavailable
    /// and guidance on how to set up metrics-server.
    pub fn setup_guidance(&self) -> &str {
        if self.metrics_available {
            ""
        } else {
            "Metrics server is not installed or unreachable. \
             To enable CPU and memory monitoring, install metrics-server: \
             kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml"
        }
    }

    /// Returns true if the chart should show the empty/unavailable state.
    pub fn should_show_empty_state(&self) -> bool {
        self.availability.is_unavailable() || (!self.has_data() && self.series.is_empty())
    }

    /// Returns the appropriate empty state title.
    pub fn empty_state_title(&self) -> &str {
        if self.availability.is_unavailable() {
            MetricsAvailability::header()
        } else {
            "No Metrics Data"
        }
    }

    // -- T069: Data loading integration --

    /// Mark the chart as loading data.
    pub fn begin_loading(&mut self) {
        self.loading = true;
    }

    /// Mark the chart as done loading.
    pub fn load_complete(&mut self) {
        self.loading = false;
    }

    /// Returns true if the chart is currently loading.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Push a data point to an existing series by label, or create a new
    /// series if none exists with that label.
    pub fn push_data_point(
        &mut self,
        series_label: &str,
        kind: MetricKind,
        timestamp: f64,
        value: f64,
    ) {
        if let Some(series) = self.series.iter_mut().find(|s| s.label == series_label) {
            series.push(timestamp, value);
        } else {
            let mut new_series = MetricSeries::new(series_label, kind);
            new_series.push(timestamp, value);
            self.series.push(new_series);
        }
    }
}

/// Format bytes as a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TiB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Format CPU millicores as a human-readable string.
pub fn format_cpu(millicores: u64) -> String {
    if millicores >= 1000 {
        format!("{:.1} cores", millicores as f64 / 1000.0)
    } else {
        format!("{millicores}m")
    }
}

// ---------------------------------------------------------------------------
// T067: MetricsChartComponent with impl Render
// T070: Graceful degradation for metrics unavailable state
// ---------------------------------------------------------------------------

/// Palette of colors for distinguishing series in the chart.
const SERIES_PALETTE: &[(u8, u8, u8)] = &[
    (96, 165, 250),  // blue
    (74, 222, 128),  // green
    (251, 191, 36),  // amber
    (248, 113, 113), // red
    (167, 139, 250), // purple
    (45, 212, 191),  // teal
];

/// Precomputed colors for the metrics chart.
#[allow(dead_code)]
struct ChartColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    warning: Rgba,
    info: Rgba,
}

/// Available time range presets (in seconds).
const TIME_RANGE_PRESETS: &[(u64, &str)] =
    &[(300, "5m"), (900, "15m"), (1800, "30m"), (3600, "1h"), (14400, "4h"), (86400, "24h")];

/// GPUI-renderable metrics chart component.
///
/// Wraps a `MetricsChartState` and a `Theme` to provide a full
/// metrics chart UI with legend, axis labels, series summaries,
/// style selector, and graceful degradation when metrics-server
/// is not available.
pub struct MetricsChartComponent {
    pub state: MetricsChartState,
    pub theme: Theme,
}

impl MetricsChartComponent {
    pub fn new(state: MetricsChartState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the palette color for a series index.
    pub fn series_color(index: usize) -> crate::theme::Color {
        let (r, g, b) = SERIES_PALETTE[index % SERIES_PALETTE.len()];
        crate::theme::Color::rgb(r, g, b)
    }

    /// Time range label for the current time_range_secs.
    pub fn time_range_label(&self) -> &'static str {
        TIME_RANGE_PRESETS
            .iter()
            .find(|(s, _)| *s == self.state.time_range_secs)
            .map(|(_, l)| *l)
            .unwrap_or("custom")
    }

    /// Compute maximum value across all series for Y-axis scaling.
    pub fn max_across_series(&self) -> f64 {
        self.state.series.iter().filter_map(|s| s.max_value()).reduce(f64::max).unwrap_or(100.0)
    }

    /// Format a value for display based on the series' MetricKind.
    pub fn format_value(kind: MetricKind, value: f64) -> String {
        match kind {
            MetricKind::Cpu => format_cpu(value as u64),
            MetricKind::Memory => format_bytes(value as u64),
        }
    }

    /// Compute Y-axis labels (5 ticks from 0 to max).
    pub fn y_axis_labels(&self) -> Vec<String> {
        let max = self.max_across_series();
        let kind = self.state.series.first().map(|s| s.kind).unwrap_or(MetricKind::Cpu);
        (0..=4)
            .map(|i| {
                let val = max * (i as f64 / 4.0);
                Self::format_value(kind, val)
            })
            .collect()
    }

    /// Compute X-axis time labels based on time_range_secs.
    pub fn x_axis_labels(&self) -> Vec<String> {
        let secs = self.state.time_range_secs;
        let step = secs / 4;
        (0..=4)
            .map(|i| {
                let offset = step * i;
                format_duration_label(offset)
            })
            .collect()
    }

    // -- Precomputed colors --

    fn colors(&self) -> ChartColors {
        ChartColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            info: self.theme.colors.info.to_gpui(),
        }
    }

    // -- Render helpers (each returns Div) --

    /// Chart header with style selector and time range.
    fn render_chart_header(&self, colors: &ChartColors) -> gpui::Div {
        let style_sel = self.render_style_selector(colors);
        let time_sel = self.render_time_range_selector(colors);

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border)
            .child(style_sel)
            .child(div().flex_grow())
            .child(time_sel)
    }

    /// Style selector pills (Line / Area / Bar).
    fn render_style_selector(&self, colors: &ChartColors) -> gpui::Div {
        let mut row = div().flex().flex_row().gap(px(4.0));

        for style in ChartStyle::all() {
            let active = *style == self.state.chart_style;
            let tc = if active { colors.accent } else { colors.text_muted };
            let bg = if active { colors.surface } else { colors.background };
            let id = format!("style-{}", style.label());
            let pill = div()
                .id(ElementId::Name(SharedString::from(id)))
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .bg(bg)
                .cursor_pointer()
                .text_xs()
                .text_color(tc)
                .child(SharedString::from(style.label()));
            row = row.child(pill);
        }

        row
    }

    /// Time range selector pills.
    fn render_time_range_selector(&self, colors: &ChartColors) -> gpui::Div {
        let mut row = div().flex().flex_row().gap(px(4.0));

        for (secs, label) in TIME_RANGE_PRESETS {
            let active = *secs == self.state.time_range_secs;
            let tc = if active { colors.accent } else { colors.text_muted };
            let id = format!("time-{label}");
            let pill = div()
                .id(ElementId::Name(SharedString::from(id)))
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .cursor_pointer()
                .text_xs()
                .text_color(tc)
                .child(SharedString::from(*label));
            row = row.child(pill);
        }

        row
    }

    /// Y-axis labels column.
    fn render_y_axis(&self, colors: &ChartColors) -> gpui::Div {
        let labels = self.y_axis_labels();
        let mut col = div().flex().flex_col().justify_between().pr_2().py_1();

        // Reverse so highest is at top.
        for lbl in labels.iter().rev() {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(lbl.clone())),
            );
        }

        col
    }

    /// X-axis time labels row.
    fn render_x_axis(&self, colors: &ChartColors) -> gpui::Div {
        let labels = self.x_axis_labels();
        let mut row = div().flex().flex_row().justify_between().pt_1().pl(px(40.0));

        for lbl in &labels {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(lbl.clone())),
            );
        }

        row
    }

    /// Main chart area with bar-indicator visualization for each series.
    fn render_chart_area(&self, colors: &ChartColors) -> gpui::Div {
        let max = self.max_across_series();
        let mut col = div().flex().flex_col().flex_1().gap(px(4.0)).py_1();

        for (i, series) in self.state.series.iter().enumerate() {
            let bar = self.render_series_bar(series, i, max, colors);
            col = col.child(bar);
        }

        col
    }

    /// Single series bar indicator showing current usage as a fill bar.
    fn render_series_bar(
        &self,
        series: &MetricSeries,
        index: usize,
        max_value: f64,
        colors: &ChartColors,
    ) -> gpui::Div {
        let sc = Self::series_color(index).to_gpui();
        let label = SharedString::from(series.label.clone());
        let current = series.latest_value().unwrap_or(0.0);
        let pct = if max_value > 0.0 { (current / max_value * 100.0).min(100.0) } else { 0.0 };
        let val_text = SharedString::from(Self::format_value(series.kind, current));

        let fill_width = pct as f32 * 2.0; // scale to px

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(div().w(px(80.0)).text_xs().text_color(sc).child(label))
            .child(
                div()
                    .flex_1()
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .bg(colors.surface)
                    .child(div().h_full().w(px(fill_width)).rounded(px(2.0)).bg(sc)),
            )
            .child(div().w(px(60.0)).text_xs().text_color(colors.text_secondary).child(val_text))
    }

    /// Legend showing series labels with color indicators.
    fn render_legend(&self, colors: &ChartColors) -> gpui::Div {
        if !self.state.show_legend || self.state.series.is_empty() {
            return div();
        }

        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(12.0))
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(colors.border);

        for (i, series) in self.state.series.iter().enumerate() {
            let sc = Self::series_color(i).to_gpui();
            let lbl = SharedString::from(series.label.clone());

            let item = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(div().w(px(10.0)).h(px(10.0)).rounded(px(2.0)).bg(sc))
                .child(div().text_xs().text_color(colors.text_secondary).child(lbl));

            row = row.child(item);
        }

        row
    }

    /// Series summary row: current value, usage %, min/max/avg.
    fn render_series_summary(&self, series: &MetricSeries, colors: &ChartColors) -> gpui::Div {
        let current = series
            .latest_value()
            .map(|v| Self::format_value(series.kind, v))
            .unwrap_or_else(|| "N/A".to_string());
        let min = series
            .min_value()
            .map(|v| Self::format_value(series.kind, v))
            .unwrap_or_else(|| "-".to_string());
        let max = series
            .max_value()
            .map(|v| Self::format_value(series.kind, v))
            .unwrap_or_else(|| "-".to_string());
        let avg = series
            .average_value()
            .map(|v| Self::format_value(series.kind, v))
            .unwrap_or_else(|| "-".to_string());
        let pct =
            series.usage_percent().map(|p| format!("{p:.1}%")).unwrap_or_else(|| "-".to_string());

        let label = SharedString::from(series.label.clone());

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .px_3()
            .py_1()
            .child(div().w(px(80.0)).text_xs().text_color(colors.text_primary).child(label))
            .child(self.summary_stat("Current", &current, colors))
            .child(self.summary_stat("Usage", &pct, colors))
            .child(self.summary_stat("Min", &min, colors))
            .child(self.summary_stat("Max", &max, colors))
            .child(self.summary_stat("Avg", &avg, colors))
    }

    /// A single stat label + value pair.
    fn summary_stat(&self, label: &str, value: &str, colors: &ChartColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(label.to_string())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(value.to_string())),
            )
    }

    /// Render all series summaries.
    fn render_series_summaries(&self, colors: &ChartColors) -> gpui::Div {
        let mut col = div().flex().flex_col().gap(px(2.0)).border_t_1().border_color(colors.border);

        for series in &self.state.series {
            col = col.child(self.render_series_summary(series, colors));
        }

        col
    }

    /// Empty state / metrics unavailable (T070 + T365: graceful degradation).
    fn render_empty_state(&self, colors: &ChartColors) -> gpui::Div {
        let title = SharedString::from(self.state.empty_state_title().to_string());

        let mut container = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .py(px(32.0))
            .gap(px(12.0))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text_secondary)
                    .child(title),
            );

        if self.state.availability.is_unavailable() {
            // T365: Detailed unavailable panel with explanation,
            // install command, and "Check Again" button.
            let explanation = SharedString::from(MetricsAvailability::explanation());
            let install_cmd = SharedString::from(MetricsAvailability::install_command());

            // Show the specific error message from the server if present.
            if let MetricsAvailability::Unavailable { ref message } = self.state.availability {
                if !message.is_empty() {
                    container = container.child(
                        div()
                            .max_w(px(480.0))
                            .text_xs()
                            .text_color(colors.warning)
                            .child(SharedString::from(message.clone())),
                    );
                }
            }

            container = container
                // Explanation paragraph
                .child(
                    div()
                        .max_w(px(480.0))
                        .text_sm()
                        .text_color(colors.text_muted)
                        .child(explanation),
                )
                // Install instructions label
                .child(
                    div()
                        .max_w(px(480.0))
                        .text_xs()
                        .text_color(colors.text_secondary)
                        .child(SharedString::from("Install metrics-server with:")),
                )
                // Command block
                .child(
                    div()
                        .max_w(px(480.0))
                        .px_3()
                        .py_2()
                        .rounded(px(4.0))
                        .bg(colors.surface)
                        .border_1()
                        .border_color(colors.border)
                        .text_xs()
                        .text_color(colors.text_primary)
                        .child(install_cmd),
                )
                // "Check Again" button
                .child(
                    div()
                        .id(ElementId::Name(SharedString::from("check-metrics-btn")))
                        .px_3()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(colors.accent)
                        .cursor_pointer()
                        .text_xs()
                        .text_color(crate::theme::Color::rgb(255, 255, 255).to_gpui())
                        .child(SharedString::from("Check Again")),
                );
        } else if let Some(ref msg) = self.state.empty_state_message {
            container = container.child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(msg.clone())),
            );
        }

        container
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &ChartColors) -> gpui::Div {
        div().flex().items_center().justify_center().flex_1().py(px(32.0)).child(
            div()
                .text_sm()
                .text_color(colors.text_muted)
                .child(SharedString::from("Loading metrics...")),
        )
    }
}

/// Format a duration in seconds to a human-readable label.
fn format_duration_label(secs: u64) -> String {
    if secs == 0 {
        "now".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

// ---------------------------------------------------------------------------
// impl Render
// ---------------------------------------------------------------------------

impl Render for MetricsChartComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors();

        let mut base = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .border_1()
            .border_color(colors.border);

        base = base.child(self.render_chart_header(&colors));

        if self.state.is_loading() || self.state.availability.is_loading() {
            base = base.child(self.render_loading(&colors));
        } else if self.state.should_show_empty_state() {
            base = base.child(self.render_empty_state(&colors));
        } else {
            let chart_row = div()
                .flex()
                .flex_row()
                .flex_1()
                .child(self.render_y_axis(&colors))
                .child(self.render_chart_area(&colors));

            base = base
                .child(chart_row)
                .child(self.render_x_axis(&colors))
                .child(self.render_series_summaries(&colors))
                .child(self.render_legend(&colors));
        }

        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_kind_label() {
        assert_eq!(MetricKind::Cpu.label(), "CPU");
        assert_eq!(MetricKind::Memory.label(), "Memory");
    }

    #[test]
    fn test_metric_kind_unit() {
        assert_eq!(MetricKind::Cpu.unit(), "millicores");
        assert_eq!(MetricKind::Memory.unit(), "bytes");
    }

    #[test]
    fn test_metric_series_new() {
        let series = MetricSeries::new("node-1", MetricKind::Cpu);
        assert_eq!(series.label, "node-1");
        assert_eq!(series.kind, MetricKind::Cpu);
        assert!(series.is_empty());
        assert_eq!(series.len(), 0);
        assert!(series.capacity.is_none());
    }

    #[test]
    fn test_metric_series_with_capacity() {
        let series = MetricSeries::new("node-1", MetricKind::Cpu).with_capacity(4000.0);
        assert_eq!(series.capacity, Some(4000.0));
    }

    #[test]
    fn test_metric_series_push_and_access() {
        let mut series = MetricSeries::new("node-1", MetricKind::Cpu);
        series.push(1000.0, 100.0);
        series.push(2000.0, 200.0);
        series.push(3000.0, 150.0);

        assert_eq!(series.len(), 3);
        assert!(!series.is_empty());
        assert_eq!(series.latest_value(), Some(150.0));
        assert_eq!(series.max_value(), Some(200.0));
        assert_eq!(series.min_value(), Some(100.0));
        assert_eq!(series.average_value(), Some(150.0));
    }

    #[test]
    fn test_metric_series_empty_stats() {
        let series = MetricSeries::new("empty", MetricKind::Memory);
        assert!(series.latest_value().is_none());
        assert!(series.max_value().is_none());
        assert!(series.min_value().is_none());
        assert!(series.average_value().is_none());
    }

    #[test]
    fn test_usage_percent() {
        let mut series = MetricSeries::new("node-1", MetricKind::Cpu).with_capacity(4000.0);
        series.push(1000.0, 1500.0);

        let pct = series.usage_percent().unwrap();
        assert!((pct - 37.5).abs() < 0.01);
    }

    #[test]
    fn test_usage_percent_no_capacity() {
        let mut series = MetricSeries::new("node-1", MetricKind::Cpu);
        series.push(1000.0, 1500.0);
        assert!(series.usage_percent().is_none());
    }

    #[test]
    fn test_usage_percent_zero_capacity() {
        let mut series = MetricSeries::new("node-1", MetricKind::Cpu).with_capacity(0.0);
        series.push(1000.0, 100.0);
        assert!(series.usage_percent().is_none());
    }

    #[test]
    fn test_metrics_chart_state_default() {
        let state = MetricsChartState::default();
        assert!(state.series.is_empty());
        assert_eq!(state.chart_style, ChartStyle::Area);
        assert!(state.show_legend);
        assert_eq!(state.time_range_secs, 3600);
        assert!(state.metrics_available);
        assert!(state.empty_state_message.is_none());
    }

    #[test]
    fn test_metrics_chart_add_clear_series() {
        let mut state = MetricsChartState::default();
        state.add_series(MetricSeries::new("cpu", MetricKind::Cpu));
        state.add_series(MetricSeries::new("mem", MetricKind::Memory));
        assert_eq!(state.series_count(), 2);

        state.clear_series();
        assert_eq!(state.series_count(), 0);
    }

    #[test]
    fn test_metrics_chart_has_data() {
        let mut state = MetricsChartState::default();
        assert!(!state.has_data());

        let mut series = MetricSeries::new("cpu", MetricKind::Cpu);
        series.push(1000.0, 100.0);
        state.add_series(series);
        assert!(state.has_data());
    }

    #[test]
    fn test_metrics_chart_unavailable() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("metrics-server is not installed");
        assert!(!state.metrics_available);
        assert_eq!(state.empty_state_message.as_deref(), Some("metrics-server is not installed"));

        state.set_available();
        assert!(state.metrics_available);
        assert!(state.empty_state_message.is_none());
    }

    // --- T103: metrics-server absence empty state ---

    #[test]
    fn test_setup_guidance_when_unavailable() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("metrics-server not installed");
        let guidance = state.setup_guidance();
        assert!(guidance.contains("metrics-server"));
        assert!(guidance.contains("kubectl apply"));
    }

    #[test]
    fn test_setup_guidance_when_available() {
        let state = MetricsChartState::default();
        assert_eq!(state.setup_guidance(), "");
    }

    #[test]
    fn test_should_show_empty_state_unavailable() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("not installed");
        assert!(state.should_show_empty_state());
    }

    #[test]
    fn test_should_show_empty_state_no_data() {
        let state = MetricsChartState::default();
        assert!(state.should_show_empty_state());
    }

    #[test]
    fn test_should_not_show_empty_state_with_data() {
        let mut state = MetricsChartState::default();
        let mut series = MetricSeries::new("cpu", MetricKind::Cpu);
        series.push(1000.0, 100.0);
        state.add_series(series);
        assert!(!state.should_show_empty_state());
    }

    #[test]
    fn test_empty_state_title_unavailable() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("not installed");
        assert_eq!(state.empty_state_title(), MetricsAvailability::header());
    }

    #[test]
    fn test_empty_state_title_no_data() {
        let state = MetricsChartState::default();
        assert_eq!(state.empty_state_title(), "No Metrics Data");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1_572_864), "1.5 MiB");
        assert_eq!(format_bytes(1_610_612_736), "1.5 GiB");
        assert_eq!(format_bytes(1_649_267_441_664), "1.5 TiB");
    }

    #[test]
    fn test_format_cpu() {
        assert_eq!(format_cpu(250), "250m");
        assert_eq!(format_cpu(999), "999m");
        assert_eq!(format_cpu(1000), "1.0 cores");
        assert_eq!(format_cpu(2500), "2.5 cores");
    }

    // --- T365: MetricsAvailability enum tests ---

    #[test]
    fn test_metrics_availability_default_is_loading() {
        let avail = MetricsAvailability::default();
        assert!(avail.is_loading());
        assert!(!avail.is_available());
        assert!(!avail.is_unavailable());
    }

    #[test]
    fn test_metrics_availability_available() {
        let avail = MetricsAvailability::Available;
        assert!(avail.is_available());
        assert!(!avail.is_unavailable());
        assert!(!avail.is_loading());
    }

    #[test]
    fn test_metrics_availability_unavailable() {
        let avail = MetricsAvailability::Unavailable { message: "HTTP 404 Not Found".to_string() };
        assert!(avail.is_unavailable());
        assert!(!avail.is_available());
        assert!(!avail.is_loading());
    }

    #[test]
    fn test_metrics_availability_loading() {
        let avail = MetricsAvailability::Loading;
        assert!(avail.is_loading());
        assert!(!avail.is_available());
        assert!(!avail.is_unavailable());
    }

    #[test]
    fn test_metrics_availability_from_error() {
        let avail = MetricsAvailability::from_error("connection refused");
        assert!(avail.is_unavailable());
        if let MetricsAvailability::Unavailable { message } = &avail {
            assert_eq!(message, "connection refused");
        } else {
            panic!("expected Unavailable variant");
        }
    }

    #[test]
    fn test_metrics_availability_static_strings() {
        assert_eq!(MetricsAvailability::header(), "Metrics server is not available");
        assert!(MetricsAvailability::explanation().contains("metrics-server"));
        assert!(MetricsAvailability::install_command().contains("kubectl apply"));
        assert!(MetricsAvailability::install_command().contains("components.yaml"));
    }

    #[test]
    fn test_metrics_availability_equality() {
        assert_eq!(MetricsAvailability::Available, MetricsAvailability::Available);
        assert_eq!(MetricsAvailability::Loading, MetricsAvailability::Loading);
        assert_eq!(
            MetricsAvailability::Unavailable { message: "x".to_string() },
            MetricsAvailability::Unavailable { message: "x".to_string() }
        );
        assert_ne!(MetricsAvailability::Available, MetricsAvailability::Loading);
        assert_ne!(
            MetricsAvailability::Unavailable { message: "a".to_string() },
            MetricsAvailability::Unavailable { message: "b".to_string() },
        );
    }

    // --- T365: MetricsChartState integration with MetricsAvailability ---

    #[test]
    fn test_chart_state_default_availability() {
        let state = MetricsChartState::default();
        assert!(state.availability.is_available());
        assert!(state.metrics_available);
    }

    #[test]
    fn test_set_unavailable_updates_availability_enum() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("HTTP 404");
        assert!(state.availability.is_unavailable());
        assert!(!state.metrics_available);
        if let MetricsAvailability::Unavailable { message } = &state.availability {
            assert_eq!(message, "HTTP 404");
        } else {
            panic!("expected Unavailable");
        }
    }

    #[test]
    fn test_set_available_restores_availability_enum() {
        let mut state = MetricsChartState::default();
        state.set_unavailable("gone");
        state.set_available();
        assert!(state.availability.is_available());
        assert!(state.metrics_available);
        assert!(state.empty_state_message.is_none());
    }

    #[test]
    fn test_set_availability_directly() {
        let mut state = MetricsChartState::default();

        state.set_availability(MetricsAvailability::Loading);
        assert!(state.availability.is_loading());
        assert!(!state.metrics_available);

        state.set_availability(MetricsAvailability::Unavailable { message: "503".to_string() });
        assert!(state.availability.is_unavailable());
        assert!(!state.metrics_available);
        assert_eq!(state.empty_state_message.as_deref(), Some("503"));

        state.set_availability(MetricsAvailability::Available);
        assert!(state.availability.is_available());
        assert!(state.metrics_available);
        assert!(state.empty_state_message.is_none());
    }

    #[test]
    fn test_begin_availability_check() {
        let mut state = MetricsChartState::default();
        state.begin_availability_check();
        assert!(state.availability.is_loading());
        assert!(state.loading);
    }

    #[test]
    fn test_should_show_empty_state_uses_availability_enum() {
        let mut state = MetricsChartState::default();
        // When Available but no data, should show empty state
        assert!(state.should_show_empty_state());

        // When Unavailable, should show empty state even if series exist
        let mut series = MetricSeries::new("cpu", MetricKind::Cpu);
        series.push(1.0, 100.0);
        state.add_series(series);
        assert!(!state.should_show_empty_state());

        state.set_availability(MetricsAvailability::Unavailable {
            message: "not found".to_string(),
        });
        assert!(state.should_show_empty_state());
    }

    #[test]
    fn test_empty_state_title_uses_header_when_unavailable() {
        let mut state = MetricsChartState::default();
        state.set_availability(MetricsAvailability::Unavailable { message: "err".to_string() });
        assert_eq!(state.empty_state_title(), "Metrics server is not available");
    }

    #[test]
    fn test_empty_state_title_no_data_when_available() {
        let state = MetricsChartState::default();
        assert_eq!(state.empty_state_title(), "No Metrics Data");
    }
}
