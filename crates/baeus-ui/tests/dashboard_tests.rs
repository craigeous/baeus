// Tests extracted from crates/baeus-ui/src/views/dashboard.rs

use baeus_ui::components::metrics_chart::MetricsAvailability;
use baeus_ui::views::dashboard::*;
use chrono::{TimeZone, Utc};
use uuid::Uuid;

fn sample_pod_summary() -> PodSummary {
    PodSummary::new(8, 1, 1, 2)
}

fn sample_events() -> Vec<DashboardEvent> {
    vec![
        DashboardEvent::new(
            "Scheduled",
            "Successfully assigned default/nginx-abc to node-1",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 0, 0).unwrap(),
            false,
        ),
        DashboardEvent::new(
            "BackOff",
            "Back-off restarting failed container",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 5, 0).unwrap(),
            true,
        ),
    ]
}

#[test]
fn test_pod_summary_new() {
    let summary = PodSummary::new(10, 2, 1, 3);
    assert_eq!(summary.running, 10);
    assert_eq!(summary.pending, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.succeeded, 3);
    assert_eq!(summary.total, 16);
}

#[test]
fn test_pod_summary_total_is_sum() {
    let summary = PodSummary::new(0, 0, 0, 0);
    assert_eq!(summary.total, 0);

    let summary = PodSummary::new(5, 3, 2, 1);
    assert_eq!(summary.total, 11);
}

#[test]
fn test_dashboard_event_new() {
    let ts = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
    let event = DashboardEvent::new("Pulled", "Container image pulled", ts, false);
    assert_eq!(event.reason, "Pulled");
    assert_eq!(event.message, "Container image pulled");
    assert_eq!(event.timestamp, ts);
    assert!(!event.is_warning);
}

#[test]
fn test_dashboard_event_warning() {
    let ts = Utc::now();
    let event = DashboardEvent::new("FailedMount", "Mount failed", ts, true);
    assert!(event.is_warning);
}

#[test]
fn test_dashboard_state_new() {
    let summary = sample_pod_summary();
    let state = DashboardState::new("prod-cluster", 5, summary.clone());
    assert_eq!(state.cluster_name, "prod-cluster");
    assert_eq!(state.node_count, 5);
    assert_eq!(state.pod_summary, summary);
    assert!(state.namespaces.is_empty());
    assert!(state.recent_events.is_empty());
}

#[test]
fn test_healthy_node_percentage_with_nodes() {
    let state = DashboardState::new("test", 3, sample_pod_summary());
    assert!((state.healthy_node_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_healthy_node_percentage_zero_nodes() {
    let state = DashboardState::new("test", 0, sample_pod_summary());
    assert!((state.healthy_node_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_pod_health_percentage() {
    // running=8, pending=1, failed=1, succeeded=2, total=12
    // healthy = running + succeeded = 10
    // percentage = 10/12 * 100 = 83.333...
    let state = DashboardState::new("test", 3, sample_pod_summary());
    let pct = state.pod_health_percentage();
    assert!((pct - 83.333_333_333_333_33).abs() < 0.01);
}

#[test]
fn test_pod_health_percentage_all_running() {
    let summary = PodSummary::new(10, 0, 0, 0);
    let state = DashboardState::new("test", 1, summary);
    assert!((state.pod_health_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_pod_health_percentage_all_failed() {
    let summary = PodSummary::new(0, 0, 5, 0);
    let state = DashboardState::new("test", 1, summary);
    assert!((state.pod_health_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_pod_health_percentage_zero_pods() {
    let summary = PodSummary::new(0, 0, 0, 0);
    let state = DashboardState::new("test", 1, summary);
    assert!((state.pod_health_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_pod_health_percentage_mixed() {
    // 5 running, 5 succeeded out of 20 total => 50%
    let summary = PodSummary::new(5, 5, 5, 5);
    let state = DashboardState::new("test", 1, summary);
    assert!((state.pod_health_percentage() - 50.0).abs() < f64::EPSILON);
}

// --- T048: Enhanced dashboard tests ---

#[test]
fn test_node_health() {
    let node = NodeHealth::new("node-1", true).with_role("control-plane");
    assert!(node.ready);
    assert_eq!(node.roles, vec!["control-plane"]);
}

#[test]
fn test_healthy_node_percentage_with_node_health() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.nodes = vec![
        NodeHealth::new("node-1", true),
        NodeHealth::new("node-2", true),
        NodeHealth::new("node-3", false),
    ];
    // 2 healthy out of 3 = 66.67%
    assert!((state.healthy_node_percentage() - 66.666_666_666_666_67).abs() < 0.01);
}

#[test]
fn test_healthy_unhealthy_node_counts() {
    let mut state = DashboardState::new("test", 4, sample_pod_summary());
    state.nodes = vec![
        NodeHealth::new("node-1", true),
        NodeHealth::new("node-2", true),
        NodeHealth::new("node-3", false),
        NodeHealth::new("node-4", true),
    ];
    assert_eq!(state.healthy_node_count(), 3);
    assert_eq!(state.unhealthy_node_count(), 1);
}

#[test]
fn test_dashboard_loading_state() {
    let state = DashboardState::loading("prod", Uuid::new_v4());
    assert!(state.loading);
    assert!(state.error.is_none());
    assert!(!state.has_data());
}

#[test]
fn test_dashboard_error_state() {
    let state = DashboardState::with_error("prod", "connection refused");
    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("connection refused"));
    assert!(!state.has_data());
}

#[test]
fn test_dashboard_has_data() {
    let state = DashboardState::new("test", 3, sample_pod_summary());
    assert!(state.has_data());
}

#[test]
fn test_warning_event_count() {
    let mut state = DashboardState::new("test", 1, sample_pod_summary());
    state.recent_events = sample_events();
    assert_eq!(state.warning_event_count(), 1);
}

#[test]
fn test_dashboard_with_namespaces_and_events() {
    let mut state = DashboardState::new("test", 2, sample_pod_summary());
    state.namespaces =
        vec!["default".to_string(), "kube-system".to_string(), "monitoring".to_string()];
    state.recent_events = sample_events();

    assert_eq!(state.namespaces.len(), 3);
    assert_eq!(state.recent_events.len(), 2);
    assert!(state.recent_events[1].is_warning);
    assert!(!state.recent_events[0].is_warning);
}

// --- T051: Error handling and graceful degradation ---

#[test]
fn test_empty_state_message_loading() {
    let state = DashboardState::loading("test", Uuid::new_v4());
    assert_eq!(state.empty_state_message(), "Loading cluster data...");
}

#[test]
fn test_empty_state_message_error() {
    let state = DashboardState::with_error("test", "timeout");
    assert!(state.empty_state_message().contains("Unable to connect"));
}

#[test]
fn test_empty_state_message_no_data() {
    let state = DashboardState::new("test", 0, PodSummary::new(0, 0, 0, 0));
    assert!(state.empty_state_message().contains("Select a cluster"));
}

#[test]
fn test_degraded_mode() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    assert!(!state.is_degraded());

    state.set_error("metrics unavailable".to_string());
    assert!(state.is_degraded()); // has data but also has error
}

#[test]
fn test_clear_error() {
    let mut state = DashboardState::with_error("test", "timeout");
    assert!(state.error.is_some());

    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_loading_lifecycle() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    assert!(!state.loading);

    state.set_loading();
    assert!(state.loading);
    assert!(state.error.is_none());

    state.set_loaded();
    assert!(!state.loading);
}

// --- T101: Metrics integration tests ---

#[test]
fn test_set_metrics_available() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    assert!(!state.has_metrics());

    state.set_metrics_available(true, true);
    assert!(state.has_metrics());
    assert!(state.cpu_metrics_available);
    assert!(state.memory_metrics_available);
}

#[test]
fn test_has_metrics_cpu_only() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_available(true, false);
    assert!(state.has_metrics());
}

#[test]
fn test_has_metrics_memory_only() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_available(false, true);
    assert!(state.has_metrics());
}

#[test]
fn test_has_metrics_neither() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_available(false, false);
    assert!(!state.has_metrics());
}

#[test]
fn test_metrics_unavailable_message() {
    let state = DashboardState::new("test", 3, sample_pod_summary());
    let msg = state.metrics_unavailable_message();
    assert!(msg.contains("metrics-server"));
    assert!(msg.contains("not installed"));
}

#[test]
fn test_pod_summary_serialization() {
    let summary = PodSummary::new(5, 2, 1, 3);
    let json = serde_json::to_string(&summary).unwrap();
    let deserialized: PodSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(summary, deserialized);
}

#[test]
fn test_dashboard_event_serialization() {
    let ts = Utc.with_ymd_and_hms(2026, 2, 24, 12, 0, 0).unwrap();
    let event = DashboardEvent::new("Pulled", "Image pulled successfully", ts, false);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: DashboardEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}

// --- T021: Render tests for DashboardView ---

/// Build a fully populated dashboard state simulating real cluster data.
fn render_scenario_dashboard() -> DashboardState {
    let pod_summary = PodSummary::new(42, 3, 2, 10);
    let mut state = DashboardState::new("production-east", 5, pod_summary);
    state.cluster_id = Some(Uuid::new_v4());
    state.nodes = vec![
        NodeHealth::new("node-1", true).with_role("control-plane").with_role("etcd"),
        NodeHealth::new("node-2", true).with_role("control-plane"),
        NodeHealth::new("node-3", true).with_role("worker"),
        NodeHealth::new("node-4", true).with_role("worker"),
        NodeHealth::new("node-5", false).with_role("worker"),
    ];
    state.namespaces = vec![
        "default".to_string(),
        "kube-system".to_string(),
        "monitoring".to_string(),
        "ingress-nginx".to_string(),
    ];
    state.recent_events = vec![
        DashboardEvent::new(
            "Scheduled",
            "Successfully assigned default/nginx-pod to node-3",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 0, 0).unwrap(),
            false,
        ),
        DashboardEvent::new(
            "Pulled",
            "Container image \"nginx:1.25\" already present on machine",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 0, 5).unwrap(),
            false,
        ),
        DashboardEvent::new(
            "BackOff",
            "Back-off restarting failed container redis in pod redis-cache-0",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 5, 0).unwrap(),
            true,
        ),
        DashboardEvent::new(
            "FailedScheduling",
            "0/5 nodes are available: insufficient memory",
            Utc.with_ymd_and_hms(2026, 2, 24, 10, 10, 0).unwrap(),
            true,
        ),
    ];
    state.set_metrics_available(true, true);
    state
}

#[test]
fn test_render_node_health_grid() {
    let state = render_scenario_dashboard();

    // The node health grid should display all 5 nodes
    assert_eq!(state.nodes.len(), 5);
    assert_eq!(state.node_count, 5);

    // Verify each node exposes name, ready status, and roles for rendering
    assert_eq!(state.nodes[0].name, "node-1");
    assert!(state.nodes[0].ready);
    assert_eq!(state.nodes[0].roles, vec!["control-plane", "etcd"]);

    assert_eq!(state.nodes[4].name, "node-5");
    assert!(!state.nodes[4].ready);
    assert_eq!(state.nodes[4].roles, vec!["worker"]);

    // Health summary for the grid header
    assert_eq!(state.healthy_node_count(), 4);
    assert_eq!(state.unhealthy_node_count(), 1);
    assert!((state.healthy_node_percentage() - 80.0).abs() < 0.01);
}

#[test]
fn test_render_pod_summary_cards() {
    let state = render_scenario_dashboard();

    // Pod summary cards should display counts for each status category
    assert_eq!(state.pod_summary.running, 42);
    assert_eq!(state.pod_summary.pending, 3);
    assert_eq!(state.pod_summary.failed, 2);
    assert_eq!(state.pod_summary.succeeded, 10);
    assert_eq!(state.pod_summary.total, 57);

    // The health percentage card should show the overall pod health
    // healthy = running(42) + succeeded(10) = 52 out of 57
    let expected_pct = (52.0 / 57.0) * 100.0;
    assert!((state.pod_health_percentage() - expected_pct).abs() < 0.01);
}

#[test]
fn test_render_event_feed() {
    let state = render_scenario_dashboard();

    // The event feed should render 4 events
    assert_eq!(state.recent_events.len(), 4);

    // Each event should have reason, message, timestamp, and warning flag
    let first = &state.recent_events[0];
    assert_eq!(first.reason, "Scheduled");
    assert!(!first.message.is_empty());
    assert!(!first.is_warning);

    // Warning events should be visually distinguished
    assert_eq!(state.warning_event_count(), 2);
    assert!(state.recent_events[2].is_warning);
    assert!(state.recent_events[3].is_warning);

    // Events should have chronological timestamps
    for window in state.recent_events.windows(2) {
        assert!(
            window[0].timestamp <= window[1].timestamp,
            "Events should be in chronological order for the feed"
        );
    }
}

#[test]
fn test_render_event_feed_filtered() {
    let state = render_scenario_dashboard();
    // filtered_events returns all events when no namespace filter is set
    let filtered = state.filtered_events();
    assert_eq!(filtered.len(), 4);
}

#[test]
fn test_render_dashboard_cluster_header() {
    let state = render_scenario_dashboard();

    // Dashboard header should display the cluster name and ID
    assert_eq!(state.cluster_name, "production-east");
    assert!(state.cluster_id.is_some());

    // Namespace selector data should be available
    assert_eq!(state.namespaces.len(), 4);
    assert!(state.namespaces.contains(&"kube-system".to_string()));
}

#[test]
fn test_render_dashboard_metrics_section() {
    let state = render_scenario_dashboard();

    // Metrics charts should be visible when metrics are available
    assert!(state.has_metrics());
    assert!(state.cpu_metrics_available);
    assert!(state.memory_metrics_available);
}

#[test]
fn test_render_dashboard_metrics_unavailable() {
    let mut state = render_scenario_dashboard();
    state.set_metrics_available(false, false);

    assert!(!state.has_metrics());
    let msg = state.metrics_unavailable_message();
    assert!(msg.contains("metrics-server"));
}

#[test]
fn test_render_dashboard_has_data_flag() {
    let state = render_scenario_dashboard();
    // A fully populated dashboard should report that it has data
    assert!(state.has_data());
    assert!(!state.loading);
    assert!(state.error.is_none());
}

#[test]
fn test_render_dashboard_loading_state_shows_spinner() {
    let id = Uuid::new_v4();
    let state = DashboardState::loading("prod-cluster", id);

    assert!(state.loading);
    assert!(!state.has_data());
    assert_eq!(state.empty_state_message(), "Loading cluster data...");
    assert_eq!(state.cluster_name, "prod-cluster");
    assert_eq!(state.cluster_id, Some(id));

    // All data sections should be empty during loading
    assert!(state.nodes.is_empty());
    assert_eq!(state.pod_summary.total, 0);
    assert!(state.recent_events.is_empty());
    assert!(state.namespaces.is_empty());
}

#[test]
fn test_render_dashboard_error_state_shows_message() {
    let state = DashboardState::with_error("broken-cluster", "TLS handshake failed");

    assert!(!state.has_data());
    assert_eq!(state.error.as_deref(), Some("TLS handshake failed"));
    assert!(state.empty_state_message().contains("Unable to connect"));
}

#[test]
fn test_render_dashboard_degraded_shows_partial_data() {
    let mut state = render_scenario_dashboard();
    state.set_error("metrics endpoint unreachable".to_string());

    // Even with error, data is still present (degraded mode)
    assert!(state.is_degraded());
    assert!(state.error.is_some());
    assert_eq!(state.node_count, 5);
    assert!(state.pod_summary.total > 0);
}

#[test]
fn test_render_node_health_conditions_ok_flag() {
    // The conditions_ok flag is used to show a secondary indicator on node cards
    let healthy = NodeHealth::new("node-ok", true);
    assert!(healthy.conditions_ok);

    let unhealthy = NodeHealth::new("node-bad", false);
    assert!(!unhealthy.conditions_ok);
}

#[test]
fn test_render_dashboard_all_nodes_healthy() {
    let mut state = DashboardState::new("healthy-cluster", 3, PodSummary::new(10, 0, 0, 0));
    state.nodes = vec![
        NodeHealth::new("node-1", true),
        NodeHealth::new("node-2", true),
        NodeHealth::new("node-3", true),
    ];

    assert_eq!(state.healthy_node_count(), 3);
    assert_eq!(state.unhealthy_node_count(), 0);
    assert!((state.healthy_node_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_render_dashboard_all_nodes_unhealthy() {
    let mut state = DashboardState::new("broken-cluster", 2, PodSummary::new(0, 0, 5, 0));
    state.nodes = vec![NodeHealth::new("node-1", false), NodeHealth::new("node-2", false)];

    assert_eq!(state.healthy_node_count(), 0);
    assert_eq!(state.unhealthy_node_count(), 2);
    assert!((state.healthy_node_percentage() - 0.0).abs() < f64::EPSILON);
}

// --- T365: Graceful degradation for metrics when metrics-server is unavailable ---

#[test]
fn test_dashboard_state_initial_metrics_availability_is_loading() {
    let state = DashboardState::new("test", 3, sample_pod_summary());
    assert!(state.metrics_availability.is_loading());
}

#[test]
fn test_dashboard_loading_state_metrics_availability_is_loading() {
    let state = DashboardState::loading("test", Uuid::new_v4());
    assert!(state.metrics_availability.is_loading());
}

#[test]
fn test_dashboard_error_state_metrics_availability_is_unavailable() {
    let state = DashboardState::with_error("test", "bad");
    assert!(state.metrics_availability.is_unavailable());
}

#[test]
fn test_set_metrics_available_updates_availability() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_available(true, true);
    assert!(state.metrics_availability.is_available());
    assert!(state.has_metrics());
}

#[test]
fn test_set_metrics_available_false_sets_unavailable() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_available(false, false);
    assert!(state.metrics_availability.is_unavailable());
    assert!(!state.has_metrics());
}

#[test]
fn test_set_metrics_availability_directly() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());

    state.set_metrics_availability(MetricsAvailability::Unavailable {
        message: "HTTP 404".to_string(),
    });
    assert!(state.metrics_availability.is_unavailable());
    assert!(!state.cpu_metrics_available);
    assert!(!state.memory_metrics_available);
    assert!(!state.has_metrics());

    state.set_metrics_availability(MetricsAvailability::Available);
    assert!(state.metrics_availability.is_available());
    assert!(state.cpu_metrics_available);
    assert!(state.memory_metrics_available);
    assert!(state.has_metrics());
}

#[test]
fn test_set_metrics_availability_loading() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());
    state.set_metrics_availability(MetricsAvailability::Loading);
    assert!(state.metrics_availability.is_loading());
    assert!(!state.has_metrics());
}

#[test]
fn test_dashboard_renders_rest_when_metrics_unavailable() {
    // The key invariant: nodes, pods, events, and namespaces data should
    // remain fully intact even when metrics-server is unavailable.
    let mut state = render_scenario_dashboard();
    state.set_metrics_availability(MetricsAvailability::Unavailable {
        message: "metrics-server returned HTTP 404".to_string(),
    });

    // Dashboard should still report that it has data (nodes + pods).
    assert!(state.has_data());

    // All non-metrics sections should be unaffected.
    assert_eq!(state.nodes.len(), 5);
    assert_eq!(state.pod_summary.total, 57);
    assert_eq!(state.recent_events.len(), 4);
    assert_eq!(state.namespaces.len(), 4);

    // Metrics specifically should be unavailable.
    assert!(!state.has_metrics());
    assert!(state.metrics_availability.is_unavailable());
}

#[test]
fn test_metrics_availability_transitions() {
    let mut state = DashboardState::new("test", 3, sample_pod_summary());

    // Starts as Loading
    assert!(state.metrics_availability.is_loading());

    // Transitions to Unavailable (e.g. 404)
    state.set_metrics_availability(MetricsAvailability::from_error(
        "the server could not find the requested resource",
    ));
    assert!(state.metrics_availability.is_unavailable());

    // User clicks "Check Again" -> Loading
    state.set_metrics_availability(MetricsAvailability::Loading);
    assert!(state.metrics_availability.is_loading());

    // Server now responds -> Available
    state.set_metrics_availability(MetricsAvailability::Available);
    assert!(state.metrics_availability.is_available());
    assert!(state.has_metrics());
}

#[test]
fn test_metrics_availability_install_command_contains_url() {
    let cmd = MetricsAvailability::install_command();
    assert!(cmd.contains("github.com/kubernetes-sigs/metrics-server"));
    assert!(cmd.contains("components.yaml"));
}

#[test]
fn test_metrics_availability_explanation_mentions_metrics_server() {
    let explanation = MetricsAvailability::explanation();
    assert!(explanation.contains("metrics-server"));
    assert!(explanation.contains("CPU"));
    assert!(explanation.contains("memory"));
}
