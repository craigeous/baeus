use chrono::{DateTime, Utc};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::components::metrics_chart::MetricsAvailability;
use crate::theme::Theme;

/// T357: Time range selector options for dashboard metrics charts (FR-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimeRange {
    #[default]
    OneHour,
    SixHours,
    TwelveHours,
    TwentyFourHours,
    SevenDays,
    ThirtyDays,
    SixtyDays,
}

impl TimeRange {
    pub fn label(&self) -> &'static str {
        match self {
            TimeRange::OneHour => "1h",
            TimeRange::SixHours => "6h",
            TimeRange::TwelveHours => "12h",
            TimeRange::TwentyFourHours => "24h",
            TimeRange::SevenDays => "7d",
            TimeRange::ThirtyDays => "30d",
            TimeRange::SixtyDays => "60d",
        }
    }

    pub fn all() -> &'static [TimeRange] {
        &[
            TimeRange::OneHour,
            TimeRange::SixHours,
            TimeRange::TwelveHours,
            TimeRange::TwentyFourHours,
            TimeRange::SevenDays,
            TimeRange::ThirtyDays,
            TimeRange::SixtyDays,
        ]
    }

    /// Returns the duration in seconds for this time range.
    pub fn duration_secs(&self) -> u64 {
        match self {
            TimeRange::OneHour => 3600,
            TimeRange::SixHours => 6 * 3600,
            TimeRange::TwelveHours => 12 * 3600,
            TimeRange::TwentyFourHours => 24 * 3600,
            TimeRange::SevenDays => 7 * 24 * 3600,
            TimeRange::ThirtyDays => 30 * 24 * 3600,
            TimeRange::SixtyDays => 60 * 24 * 3600,
        }
    }
}

/// T358: A cluster issue detected from warning events or unhealthy nodes (FR-066).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterIssue {
    pub severity: IssueSeverity,
    pub source: String,
    pub message: String,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Severity level for cluster issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Critical,
}

impl ClusterIssue {
    pub fn warning(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            source: source.into(),
            message: message.into(),
            timestamp: None,
        }
    }

    pub fn critical(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Critical,
            source: source.into(),
            message: message.into(),
            timestamp: None,
        }
    }

    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }
}

/// Summary counts of pod statuses within a cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodSummary {
    pub running: u32,
    pub pending: u32,
    pub failed: u32,
    pub succeeded: u32,
    pub total: u32,
}

impl PodSummary {
    pub fn new(running: u32, pending: u32, failed: u32, succeeded: u32) -> Self {
        Self { running, pending, failed, succeeded, total: running + pending + failed + succeeded }
    }
}

/// Represents a single event displayed on the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardEvent {
    pub reason: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub is_warning: bool,
    pub namespace: Option<String>,
    pub involved_object_kind: Option<String>,
    pub involved_object_name: Option<String>,
    pub source: Option<String>,
    pub count: u32,
    pub last_seen: Option<DateTime<Utc>>,
}

impl DashboardEvent {
    pub fn new(
        reason: impl Into<String>,
        message: impl Into<String>,
        timestamp: DateTime<Utc>,
        is_warning: bool,
    ) -> Self {
        Self {
            reason: reason.into(),
            message: message.into(),
            timestamp,
            is_warning,
            namespace: None,
            involved_object_kind: None,
            involved_object_name: None,
            source: None,
            count: 1,
            last_seen: None,
        }
    }

    /// Create a fully populated event with all fields.
    #[allow(clippy::too_many_arguments)]
    pub fn with_details(
        reason: impl Into<String>,
        message: impl Into<String>,
        timestamp: DateTime<Utc>,
        is_warning: bool,
        namespace: Option<String>,
        involved_object_kind: Option<String>,
        involved_object_name: Option<String>,
        source: Option<String>,
        count: u32,
        last_seen: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            reason: reason.into(),
            message: message.into(),
            timestamp,
            is_warning,
            namespace,
            involved_object_kind,
            involved_object_name,
            source,
            count,
            last_seen,
        }
    }

    /// Display string for the involved object, e.g. "Pod/nginx-abc".
    pub fn involved_object_display(&self) -> String {
        match (&self.involved_object_kind, &self.involved_object_name) {
            (Some(kind), Some(name)) => format!("{kind}/{name}"),
            (Some(kind), None) => kind.clone(),
            (None, Some(name)) => name.clone(),
            (None, None) => "—".to_string(),
        }
    }
}

/// Convert a `DateTime<Utc>` to a human-readable age string (e.g., "5m", "2h", "3d").
pub fn human_age_from_datetime(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);
    let secs = duration.num_seconds();
    if secs < 0 {
        return "0s".to_string();
    }
    if secs < 60 {
        return format!("{}s", secs);
    }
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m", mins);
    }
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h", hours);
    }
    let days = duration.num_days();
    format!("{}d", days)
}

/// Represents a node's health status in the dashboard grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeHealth {
    pub name: String,
    pub ready: bool,
    pub roles: Vec<String>,
    pub conditions_ok: bool,
}

impl NodeHealth {
    pub fn new(name: impl Into<String>, ready: bool) -> Self {
        Self { name: name.into(), ready, roles: Vec::new(), conditions_ok: ready }
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }
}

/// Resource counts per kind for the dashboard overview.
#[derive(Debug, Clone, Default)]
pub struct ResourceCounts {
    pub pods: u32,
    pub deployments: u32,
    pub daemonsets: u32,
    pub statefulsets: u32,
    pub replicasets: u32,
    pub jobs: u32,
    pub cronjobs: u32,
}

/// State for the cluster dashboard view, providing an overview of cluster health.
#[derive(Debug, Clone)]
pub struct DashboardState {
    pub cluster_id: Option<Uuid>,
    pub cluster_name: String,
    pub node_count: u32,
    pub nodes: Vec<NodeHealth>,
    pub pod_summary: PodSummary,
    pub namespaces: Vec<String>,
    pub recent_events: Vec<DashboardEvent>,
    pub active_namespace_filter: Option<String>,
    pub cpu_metrics_available: bool,
    pub memory_metrics_available: bool,
    /// T365: Structured metrics-server availability state for graceful degradation.
    pub metrics_availability: MetricsAvailability,
    pub loading: bool,
    pub error: Option<String>,
    /// T357: Selected time range for metrics charts (FR-023).
    pub selected_time_range: TimeRange,
    /// Resource counts for the dashboard overview circles.
    pub resource_counts: ResourceCounts,
    /// CPU usage as percentage (0.0–100.0), None if metrics unavailable.
    pub cpu_usage_percent: Option<f32>,
    /// Memory usage as percentage (0.0–100.0), None if metrics unavailable.
    pub memory_usage_percent: Option<f32>,
    /// Total allocatable CPU cores across all nodes.
    pub cpu_capacity: Option<f64>,
    /// Total allocatable memory bytes across all nodes.
    pub memory_capacity: Option<f64>,
    /// Current aggregate CPU usage across all nodes.
    pub cpu_used: Option<f64>,
    /// Current aggregate memory usage across all nodes.
    pub memory_used: Option<f64>,
}

impl DashboardState {
    pub fn new(cluster_name: impl Into<String>, node_count: u32, pod_summary: PodSummary) -> Self {
        Self {
            cluster_id: None,
            cluster_name: cluster_name.into(),
            node_count,
            nodes: Vec::new(),
            pod_summary,
            namespaces: Vec::new(),
            recent_events: Vec::new(),
            active_namespace_filter: None,
            cpu_metrics_available: false,
            memory_metrics_available: false,
            metrics_availability: MetricsAvailability::Loading,
            loading: false,
            error: None,
            selected_time_range: TimeRange::default(),
            resource_counts: ResourceCounts::default(),
            cpu_usage_percent: None,
            memory_usage_percent: None,
            cpu_capacity: None,
            memory_capacity: None,
            cpu_used: None,
            memory_used: None,
        }
    }

    /// Create an empty loading state for a cluster.
    pub fn loading(cluster_name: impl Into<String>, cluster_id: Uuid) -> Self {
        Self {
            cluster_id: Some(cluster_id),
            cluster_name: cluster_name.into(),
            node_count: 0,
            nodes: Vec::new(),
            pod_summary: PodSummary::new(0, 0, 0, 0),
            namespaces: Vec::new(),
            recent_events: Vec::new(),
            active_namespace_filter: None,
            cpu_metrics_available: false,
            memory_metrics_available: false,
            metrics_availability: MetricsAvailability::Loading,
            loading: true,
            error: None,
            selected_time_range: TimeRange::default(),
            resource_counts: ResourceCounts::default(),
            cpu_usage_percent: None,
            memory_usage_percent: None,
            cpu_capacity: None,
            memory_capacity: None,
            cpu_used: None,
            memory_used: None,
        }
    }

    /// Create an error state for the dashboard.
    pub fn with_error(cluster_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            cluster_id: None,
            cluster_name: cluster_name.into(),
            node_count: 0,
            nodes: Vec::new(),
            pod_summary: PodSummary::new(0, 0, 0, 0),
            namespaces: Vec::new(),
            recent_events: Vec::new(),
            active_namespace_filter: None,
            cpu_metrics_available: false,
            memory_metrics_available: false,
            metrics_availability: MetricsAvailability::Unavailable {
                message: "Cluster error".to_string(),
            },
            loading: false,
            error: Some(error.into()),
            selected_time_range: TimeRange::default(),
            resource_counts: ResourceCounts::default(),
            cpu_usage_percent: None,
            memory_usage_percent: None,
            cpu_capacity: None,
            memory_capacity: None,
            cpu_used: None,
            memory_used: None,
        }
    }

    /// Returns the percentage of nodes that are healthy (Ready condition true).
    pub fn healthy_node_percentage(&self) -> f64 {
        if self.nodes.is_empty() {
            if self.node_count == 0 {
                return 0.0;
            }
            return 100.0;
        }
        let healthy = self.nodes.iter().filter(|n| n.ready).count() as f64;
        (healthy / self.nodes.len() as f64) * 100.0
    }

    /// Returns the percentage of pods that are in a healthy state (running or succeeded).
    pub fn pod_health_percentage(&self) -> f64 {
        if self.pod_summary.total == 0 {
            return 0.0;
        }
        let healthy = self.pod_summary.running + self.pod_summary.succeeded;
        (healthy as f64 / self.pod_summary.total as f64) * 100.0
    }

    /// Returns events filtered by the active namespace, or all events if no filter.
    pub fn filtered_events(&self) -> Vec<&DashboardEvent> {
        self.recent_events.iter().collect()
    }

    /// Count of healthy nodes.
    pub fn healthy_node_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.ready).count()
    }

    /// Count of unhealthy nodes.
    pub fn unhealthy_node_count(&self) -> usize {
        self.nodes.iter().filter(|n| !n.ready).count()
    }

    /// Count of warning events.
    pub fn warning_event_count(&self) -> usize {
        self.recent_events.iter().filter(|e| e.is_warning).count()
    }

    /// Whether the dashboard has any data loaded.
    pub fn has_data(&self) -> bool {
        !self.loading && self.error.is_none() && (self.node_count > 0 || self.pod_summary.total > 0)
    }

    /// Returns the appropriate empty state message.
    pub fn empty_state_message(&self) -> &str {
        if self.loading {
            "Loading cluster data..."
        } else if self.error.is_some() {
            "Unable to connect to the cluster. Check your kubeconfig and network connectivity."
        } else {
            "No data available. Select a cluster to view its dashboard."
        }
    }

    /// Whether the dashboard should show partial data with degraded indicators.
    /// This is true when some data is available but errors occurred loading other parts.
    pub fn is_degraded(&self) -> bool {
        self.error.is_some() && (self.node_count > 0 || self.pod_summary.total > 0)
    }

    /// Clear the error state (e.g., after a successful reconnection).
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Set loading state (e.g., when switching clusters).
    pub fn set_loading(&mut self) {
        self.loading = true;
        self.error = None;
    }

    /// Set error state with data preserved (degraded mode).
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
    }

    /// Mark loading complete.
    pub fn set_loaded(&mut self) {
        self.loading = false;
    }

    // --- T101: Metrics integration ---

    /// Set metrics availability for the dashboard.
    pub fn set_metrics_available(&mut self, cpu: bool, memory: bool) {
        self.cpu_metrics_available = cpu;
        self.memory_metrics_available = memory;
        // Keep the structured availability in sync.
        if cpu || memory {
            self.metrics_availability = MetricsAvailability::Available;
        } else {
            self.metrics_availability = MetricsAvailability::Unavailable {
                message: "metrics-server is not reachable".to_string(),
            };
        }
    }

    /// T365: Set the structured metrics availability directly.
    pub fn set_metrics_availability(&mut self, availability: MetricsAvailability) {
        self.cpu_metrics_available = availability.is_available();
        self.memory_metrics_available = availability.is_available();
        self.metrics_availability = availability;
    }

    /// Returns true if any metrics are available for display.
    pub fn has_metrics(&self) -> bool {
        self.cpu_metrics_available || self.memory_metrics_available
    }

    /// Returns a message to display when metrics are unavailable.
    pub fn metrics_unavailable_message(&self) -> &'static str {
        "Metrics server is not installed or not accessible. Install metrics-server to see CPU and memory usage charts."
    }

    // --- T357: Time range selector ---

    /// Set the selected time range for metrics charts.
    pub fn set_time_range(&mut self, range: TimeRange) {
        self.selected_time_range = range;
    }

    // --- T358: Issues section ---

    /// Compute cluster issues from warning events and unhealthy nodes (FR-066).
    pub fn issues(&self) -> Vec<ClusterIssue> {
        let mut issues = Vec::new();

        // Unhealthy nodes → Critical issues
        for node in &self.nodes {
            if !node.ready {
                issues.push(ClusterIssue::critical(
                    format!("Node/{}", node.name),
                    format!("Node {} is NotReady", node.name),
                ));
            }
            if !node.conditions_ok && node.ready {
                issues.push(ClusterIssue::warning(
                    format!("Node/{}", node.name),
                    format!("Node {} has degraded conditions", node.name),
                ));
            }
        }

        // Warning events → Warning issues
        for event in &self.recent_events {
            if event.is_warning {
                issues.push(
                    ClusterIssue::warning(event.reason.clone(), event.message.clone())
                        .with_timestamp(event.timestamp),
                );
            }
        }

        issues
    }

    /// Returns the count of critical issues.
    pub fn critical_issue_count(&self) -> usize {
        self.issues().iter().filter(|i| i.severity == IssueSeverity::Critical).count()
    }

    /// Returns the count of warning issues.
    pub fn warning_issue_count(&self) -> usize {
        self.issues().iter().filter(|i| i.severity == IssueSeverity::Warning).count()
    }
}

// ---------------------------------------------------------------------------
// DashboardView — GPUI renderable view
// ---------------------------------------------------------------------------

/// The main dashboard view that renders cluster health overview.
pub struct DashboardView {
    pub state: DashboardState,
    pub theme: Theme,
}

impl DashboardView {
    pub fn new(state: DashboardState, theme: Theme) -> Self {
        Self { state, theme }
    }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bg = self.theme.colors.background.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let text = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let text_muted = self.theme.colors.text_muted.to_gpui();
        let success = self.theme.colors.success.to_gpui();
        let warning = self.theme.colors.warning.to_gpui();
        let error = self.theme.colors.error.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();

        // If no data, show empty/loading/error state
        if !self.state.has_data() {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .bg(bg)
                .text_color(text)
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(text_muted)
                        .child(self.state.empty_state_message().to_string()),
                )
                .into_any_element();
        }

        // Main dashboard layout: vertical flex with sections
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg)
            .text_color(text)
            .overflow_hidden()
            .p_4()
            .gap_4();

        // Cluster title
        root = root.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child(self.state.cluster_name.clone()),
                )
                .when(self.state.is_degraded(), |el| {
                    el.child(div().text_xs().text_color(warning).child("Degraded"))
                }),
        );

        // --- Section A: Node Health Grid ---
        root = root.child(self.render_node_health_section(
            text,
            text_secondary,
            text_muted,
            surface,
            border,
            success,
            error,
        ));

        // --- Section B: Pod Summary ---
        root = root.child(self.render_pod_summary_section(
            text,
            text_secondary,
            surface,
            border,
            success,
            warning,
            error,
            accent,
        ));

        // --- Section C: Metrics Charts with Time Range Selector (T357) ---
        root = root.child(self.render_metrics_section(
            text,
            text_secondary,
            text_muted,
            surface,
            border,
            accent,
        ));

        // --- Section D: Issues Feed (T358) ---
        root = root.child(self.render_issues_section(
            text,
            text_secondary,
            text_muted,
            surface,
            border,
            warning,
            error,
        ));

        // --- Section E: Namespace List ---
        root = root.child(self.render_namespace_section(
            text,
            text_secondary,
            text_muted,
            surface,
            border,
        ));

        // --- Section F: Recent Events Feed ---
        root = root.child(self.render_events_section(
            text,
            text_secondary,
            text_muted,
            surface,
            border,
            warning,
            error,
        ));

        root.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// DashboardView rendering helpers
// ---------------------------------------------------------------------------

impl DashboardView {
    /// Section header with a label and optional subtitle.
    fn section_header(label: &str, text: Rgba, text_secondary: Rgba) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .pb_2()
            .child(
                div().font_weight(FontWeight::SEMIBOLD).text_color(text).child(label.to_string()),
            )
            .child(div().text_xs().text_color(text_secondary))
    }

    /// Render the node health grid section.
    #[allow(clippy::too_many_arguments)]
    fn render_node_health_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        text_muted: Rgba,
        surface: Rgba,
        border: Rgba,
        success: Rgba,
        error: Rgba,
    ) -> Div {
        let mut section = div().flex().flex_col().gap_2();

        section = section.child(Self::section_header("Node Health", text, text_secondary));

        if self.state.nodes.is_empty() {
            // Show node count summary when detailed node info is not available
            section = section.child(
                div()
                    .text_sm()
                    .text_color(text_muted)
                    .child(format!("{} node(s) reported", self.state.node_count)),
            );
        } else {
            // Grid of node cards
            let mut grid = div().flex().flex_row().flex_wrap().gap_2().overflow_hidden();

            for node in &self.state.nodes {
                let status_color = if node.ready { success } else { error };
                let status_label = if node.ready { "Ready" } else { "NotReady" };
                let conditions_label =
                    if node.conditions_ok { "Conditions OK" } else { "Conditions Degraded" };

                let roles_text = if node.roles.is_empty() {
                    "worker".to_string()
                } else {
                    node.roles.join(", ")
                };

                let card = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .min_w(px(160.0))
                    // Node name
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_sm()
                            .text_color(text)
                            .child(node.name.clone()),
                    )
                    // Roles
                    .child(div().text_xs().text_color(text_muted).child(roles_text))
                    // Status row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(status_color)
                                    .child(status_label.to_string()),
                            ),
                    )
                    // Conditions
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .child(conditions_label.to_string()),
                    );

                grid = grid.child(card);
            }

            section = section.child(grid);
        }

        section
    }

    /// Render the pod summary section with status count cards.
    #[allow(clippy::too_many_arguments)]
    fn render_pod_summary_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
        success: Rgba,
        warning: Rgba,
        error: Rgba,
        accent: Rgba,
    ) -> Div {
        let mut section = div().flex().flex_col().gap_2();

        section = section.child(Self::section_header("Pod Summary", text, text_secondary));

        let pods = &self.state.pod_summary;

        let pod_cards: Vec<(&str, u32, Rgba)> = vec![
            ("Running", pods.running, success),
            ("Pending", pods.pending, warning),
            ("Failed", pods.failed, error),
            ("Succeeded", pods.succeeded, accent),
        ];

        let mut cards_row = div().flex().flex_row().flex_wrap().gap_2().overflow_hidden();

        for (label, count, color) in pod_cards {
            let card = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .p_3()
                .rounded_md()
                .bg(surface)
                .border_1()
                .border_color(border)
                .min_w(px(100.0))
                // Count
                .child(
                    div().font_weight(FontWeight::BOLD).text_color(color).child(count.to_string()),
                )
                // Label
                .child(div().text_xs().text_color(text_secondary).child(label.to_string()));

            cards_row = cards_row.child(card);
        }

        // Total card
        let total_card = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_3()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border)
            .min_w(px(100.0))
            .child(
                div().font_weight(FontWeight::BOLD).text_color(text).child(pods.total.to_string()),
            )
            .child(div().text_xs().text_color(text_secondary).child("Total".to_string()));

        cards_row = cards_row.child(total_card);
        section = section.child(cards_row);

        section
    }

    /// Render the namespace list section.
    fn render_namespace_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        text_muted: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        let mut section = div().flex().flex_col().gap_2();

        section = section.child(Self::section_header("Namespaces", text, text_secondary));

        if self.state.namespaces.is_empty() {
            section = section.child(
                div()
                    .text_sm()
                    .text_color(text_muted)
                    .child("No namespaces discovered".to_string()),
            );
        } else {
            let mut list = div().flex().flex_row().flex_wrap().gap_1().overflow_hidden();

            for ns in &self.state.namespaces {
                let chip = div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(text_secondary)
                    .child(ns.clone());

                list = list.child(chip);
            }

            section = section.child(list);
        }

        section
    }

    /// Render the recent events feed section.
    #[allow(clippy::too_many_arguments)]
    fn render_events_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        text_muted: Rgba,
        surface: Rgba,
        border: Rgba,
        warning: Rgba,
        error: Rgba,
    ) -> Div {
        let mut section = div().flex().flex_col().gap_2();

        section = section.child(Self::section_header("Recent Events", text, text_secondary));

        let events = self.state.filtered_events();

        if events.is_empty() {
            section = section.child(
                div().text_sm().text_color(text_muted).child("No recent events".to_string()),
            );
        } else {
            let mut event_list = div().flex().flex_col().gap_1().overflow_hidden();

            for event in events {
                let type_color = if event.is_warning { warning } else { text_secondary };
                let type_label = if event.is_warning { "Warning" } else { "Normal" };
                let timestamp_str = event.timestamp.format("%H:%M:%S").to_string();

                let row = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    // Event type badge
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(type_color)
                            .min_w(px(56.0))
                            .child(type_label.to_string()),
                    )
                    // Reason
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text)
                            .min_w(px(80.0))
                            .child(event.reason.clone()),
                    )
                    // Message (flex to fill remaining space)
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(text_secondary)
                            .overflow_hidden()
                            .child(event.message.clone()),
                    )
                    // Timestamp
                    .child(
                        div().text_xs().text_color(text_muted).flex_shrink_0().child(timestamp_str),
                    );

                // Add a left accent border for warnings
                let row = if event.is_warning { row.border_l_2().border_color(error) } else { row };

                event_list = event_list.child(row);
            }

            section = section.child(event_list);
        }

        section
    }

    /// T357: Render the metrics charts section with a time-range selector (FR-023).
    #[allow(clippy::too_many_arguments)]
    fn render_metrics_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        text_muted: Rgba,
        surface: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let mut section = div().flex().flex_col().gap_2();

        // Header row with "Metrics" label and time range buttons
        let mut header = div().flex().flex_row().items_center().gap_2().pb_2();

        header =
            header.child(div().font_weight(FontWeight::SEMIBOLD).text_color(text).child("Metrics"));

        // Spacer to push time range selector right
        header = header.child(div().flex_1());

        // Time range button row
        let mut range_row = div().flex().flex_row().gap_1();

        for &range in TimeRange::all() {
            let is_selected = self.state.selected_time_range == range;
            let label = range.label();

            let btn = div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .when(is_selected, |el| el.bg(accent).text_color(gpui::rgb(0xFFFFFF)))
                .when(!is_selected, |el| {
                    el.bg(surface).border_1().border_color(border).text_color(text_secondary)
                })
                .child(label);

            range_row = range_row.child(btn);
        }

        header = header.child(range_row);
        section = section.child(header);

        // T365: Metrics chart placeholders with graceful degradation
        if self.state.metrics_availability.is_loading() {
            section = section.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_4()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .text_sm()
                    .text_color(text_muted)
                    .child("Checking metrics-server availability..."),
            );
        } else if self.state.metrics_availability.is_unavailable() {
            // Detailed unavailable panel with install instructions
            let mut panel = div()
                .flex()
                .flex_col()
                .items_center()
                .p_4()
                .gap_3()
                .rounded_md()
                .bg(surface)
                .border_1()
                .border_color(border);

            // Header
            panel = panel.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .child(MetricsAvailability::header()),
            );

            // Server-specific error message
            if let MetricsAvailability::Unavailable { ref message } =
                self.state.metrics_availability
            {
                if !message.is_empty() {
                    let warning_color = self.theme.colors.warning.to_gpui();
                    panel = panel
                        .child(div().text_xs().text_color(warning_color).child(message.clone()));
                }
            }

            // Explanation
            panel = panel.child(
                div().text_sm().text_color(text_muted).child(MetricsAvailability::explanation()),
            );

            // Install instructions label
            panel = panel.child(
                div().text_xs().text_color(text_secondary).child("Install metrics-server with:"),
            );

            // Command block
            panel = panel.child(
                div()
                    .max_w(px(560.0))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.colors.background.to_gpui())
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(text)
                    .child(MetricsAvailability::install_command()),
            );

            // "Check Again" button
            panel = panel.child(
                div()
                    .id(ElementId::Name(SharedString::from("check-metrics-btn")))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(accent)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(gpui::rgb(0xFFFFFF))
                    .child("Check Again"),
            );

            section = section.child(panel);
        } else if !self.state.has_metrics() {
            section = section.child(
                div()
                    .text_sm()
                    .text_color(text_muted)
                    .p_3()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .child(self.state.metrics_unavailable_message()),
            );
        } else {
            // CPU chart placeholder
            let mut charts = div().flex().flex_row().flex_wrap().gap_3();

            let chart_names = ["CPU Usage", "Memory Usage", "Pod Count"];
            for name in chart_names {
                let chart = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(200.0))
                    .h(px(150.0))
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .p_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(text_muted)
                            .child(format!(
                                "Chart ({} range)",
                                self.state.selected_time_range.label()
                            )),
                    );

                charts = charts.child(chart);
            }

            section = section.child(charts);
        }

        section
    }

    /// T358: Render the issues section (FR-066).
    /// Shows warning events and unhealthy node conditions.
    #[allow(clippy::too_many_arguments)]
    fn render_issues_section(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        text_muted: Rgba,
        surface: Rgba,
        border: Rgba,
        warning: Rgba,
        error: Rgba,
    ) -> Div {
        let issues = self.state.issues();

        let mut section = div().flex().flex_col().gap_2();

        // Header with issue count
        let count_label = if issues.is_empty() {
            "No issues".to_string()
        } else {
            format!("{} issue(s)", issues.len())
        };

        section = section.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .pb_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(text).child("Issues"))
                .child(
                    div()
                        .text_xs()
                        .text_color(if issues.is_empty() { text_secondary } else { warning })
                        .child(count_label),
                ),
        );

        if issues.is_empty() {
            section =
                section.child(div().text_sm().text_color(text_muted).child("No issues detected"));
        } else {
            let mut issue_list = div().flex().flex_col().gap_1().overflow_hidden();

            for issue in &issues {
                let (severity_color, severity_label) = match issue.severity {
                    IssueSeverity::Critical => (error, "CRITICAL"),
                    IssueSeverity::Warning => (warning, "WARNING"),
                };

                let mut row = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .border_l_2()
                    .border_color(severity_color)
                    .overflow_hidden()
                    // Severity badge
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(severity_color)
                            .min_w(px(64.0))
                            .child(severity_label),
                    )
                    // Source
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text)
                            .min_w(px(100.0))
                            .child(issue.source.clone()),
                    )
                    // Message
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(text_secondary)
                            .overflow_hidden()
                            .child(issue.message.clone()),
                    );

                // Timestamp if present
                if let Some(ts) = issue.timestamp {
                    row = row.child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .flex_shrink_0()
                            .child(ts.format("%H:%M:%S").to_string()),
                    );
                }

                issue_list = issue_list.child(row);
            }

            section = section.child(issue_list);
        }

        section
    }
}
