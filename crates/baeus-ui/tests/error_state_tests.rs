// T361: Error state rendering tests
//
// Tests covering DashboardState and ResourceTableState error, loading,
// and degraded state machines.

use baeus_ui::components::resource_table::{ColumnDef, ResourceTableState, TableRow};
use baeus_ui::views::dashboard::{DashboardEvent, DashboardState, NodeHealth, PodSummary};
use chrono::{TimeZone, Utc};
use uuid::Uuid;

// ===========================================================================
// Helpers
// ===========================================================================

fn sample_pod_summary() -> PodSummary {
    PodSummary::new(8, 1, 1, 2)
}

fn empty_pod_summary() -> PodSummary {
    PodSummary::new(0, 0, 0, 0)
}

fn sample_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            id: "name".to_string(),
            label: "Name".to_string(),
            sortable: true,
            width_weight: 2.0,
        },
        ColumnDef {
            id: "status".to_string(),
            label: "Status".to_string(),
            sortable: true,
            width_weight: 1.0,
        },
    ]
}

fn sample_row(uid: &str, name: &str, status: &str) -> TableRow {
    TableRow {
        uid: uid.to_string(),
        cells: vec![name.to_string(), status.to_string()],
        selected: false,
        kind: "Pod".to_string(),
        name: name.to_string(),
        namespace: Some("default".to_string()),
        container_statuses: Vec::new(),
        conditions: Vec::new(),
    }
}

fn populated_dashboard() -> DashboardState {
    let mut state = DashboardState::new("prod-cluster", 3, sample_pod_summary());
    state.nodes = vec![
        NodeHealth::new("node-1", true),
        NodeHealth::new("node-2", true),
        NodeHealth::new("node-3", false),
    ];
    state.namespaces = vec!["default".to_string(), "kube-system".to_string()];
    state.recent_events = vec![DashboardEvent::new(
        "Scheduled",
        "Pod scheduled on node-1",
        Utc.with_ymd_and_hms(2026, 2, 24, 10, 0, 0).unwrap(),
        false,
    )];
    state
}

// ===========================================================================
// 1. Dashboard error state: with_error constructor
// ===========================================================================

#[test]
fn test_with_error_sets_error_message() {
    let state = DashboardState::with_error("cluster", "connection error");
    assert_eq!(state.error.as_deref(), Some("connection error"));
}

#[test]
fn test_with_error_has_data_returns_false() {
    let state = DashboardState::with_error("cluster", "connection error");
    assert!(!state.has_data());
}

#[test]
fn test_with_error_is_degraded_returns_false() {
    // No prior data, so not degraded -- just errored
    let state = DashboardState::with_error("cluster", "connection error");
    assert!(!state.is_degraded());
}

#[test]
fn test_with_error_loading_is_false() {
    let state = DashboardState::with_error("cluster", "connection error");
    assert!(!state.loading);
}

#[test]
fn test_with_error_cluster_name_preserved() {
    let state = DashboardState::with_error("my-cluster", "timeout");
    assert_eq!(state.cluster_name, "my-cluster");
}

#[test]
fn test_with_error_empty_state_message() {
    let state = DashboardState::with_error("cluster", "anything");
    assert!(state.empty_state_message().contains("Unable to connect"));
}

#[test]
fn test_with_error_node_count_zero() {
    let state = DashboardState::with_error("cluster", "err");
    assert_eq!(state.node_count, 0);
    assert!(state.nodes.is_empty());
}

#[test]
fn test_with_error_pod_summary_zero() {
    let state = DashboardState::with_error("cluster", "err");
    assert_eq!(state.pod_summary.total, 0);
}

// ===========================================================================
// 2. Dashboard degraded state: data loaded then error set
// ===========================================================================

#[test]
fn test_degraded_state_after_data_then_error() {
    let mut state = populated_dashboard();
    assert!(!state.is_degraded());

    state.set_error("metrics endpoint unreachable".to_string());
    assert!(state.is_degraded());
}

#[test]
fn test_degraded_state_preserves_node_count() {
    let mut state = populated_dashboard();
    state.set_error("partial failure".to_string());
    assert_eq!(state.node_count, 3);
}

#[test]
fn test_degraded_state_preserves_pod_summary() {
    let mut state = populated_dashboard();
    let original_total = state.pod_summary.total;
    state.set_error("partial failure".to_string());
    assert_eq!(state.pod_summary.total, original_total);
}

#[test]
fn test_degraded_state_preserves_nodes_list() {
    let mut state = populated_dashboard();
    state.set_error("partial failure".to_string());
    assert_eq!(state.nodes.len(), 3);
}

#[test]
fn test_degraded_state_preserves_events() {
    let mut state = populated_dashboard();
    state.set_error("partial failure".to_string());
    assert_eq!(state.recent_events.len(), 1);
}

#[test]
fn test_degraded_state_error_message_stored() {
    let mut state = populated_dashboard();
    state.set_error("API server 503".to_string());
    assert_eq!(state.error.as_deref(), Some("API server 503"));
}

#[test]
fn test_degraded_with_only_pods_no_nodes() {
    // is_degraded checks node_count > 0 || pod_summary.total > 0
    let mut state = DashboardState::new("test", 0, sample_pod_summary());
    state.set_error("err".to_string());
    // pod_summary.total > 0 so should be degraded
    assert!(state.is_degraded());
}

#[test]
fn test_not_degraded_when_no_data_at_all() {
    let mut state = DashboardState::new("test", 0, empty_pod_summary());
    state.set_error("err".to_string());
    assert!(!state.is_degraded());
}

// ===========================================================================
// 3. ResourceTable with empty rows (no data state)
// ===========================================================================

#[test]
fn test_resource_table_empty_rows_visible_slice_empty() {
    let state = ResourceTableState::new(sample_columns(), 20);
    assert!(state.visible_slice().is_empty());
}

#[test]
fn test_resource_table_empty_rows_filtered_rows_empty() {
    let state = ResourceTableState::new(sample_columns(), 20);
    assert!(state.filtered_rows().is_empty());
}

#[test]
fn test_resource_table_empty_rows_total_filtered_count_zero() {
    let state = ResourceTableState::new(sample_columns(), 20);
    assert_eq!(state.total_filtered_count(), 0);
}

#[test]
fn test_resource_table_empty_rows_selected_row_is_none() {
    let state = ResourceTableState::new(sample_columns(), 20);
    assert!(state.selected_row().is_none());
}

#[test]
fn test_resource_table_empty_after_set_rows_empty_vec() {
    let mut state = ResourceTableState::new(sample_columns(), 20);
    state.set_rows(vec![sample_row("1", "pod-a", "Running"), sample_row("2", "pod-b", "Pending")]);
    assert_eq!(state.total_filtered_count(), 2);

    // Clear all rows
    state.set_rows(vec![]);
    assert_eq!(state.total_filtered_count(), 0);
    assert!(state.visible_slice().is_empty());
}

#[test]
fn test_resource_table_scroll_offset_resets_on_empty() {
    let mut state = ResourceTableState::new(sample_columns(), 20);
    state.set_rows(vec![sample_row("1", "pod-a", "Running"), sample_row("2", "pod-b", "Pending")]);
    state.scroll_to(1);
    assert_eq!(state.scroll_offset, 1);

    state.set_rows(vec![]);
    assert_eq!(state.scroll_offset, 0);
}

// ===========================================================================
// 4. Error clearing
// ===========================================================================

#[test]
fn test_clear_error_removes_error() {
    let mut state = DashboardState::with_error("cluster", "connection refused");
    assert!(state.error.is_some());
    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_clear_error_on_degraded_state_restores_normal() {
    let mut state = populated_dashboard();
    state.set_error("temporary failure".to_string());
    assert!(state.is_degraded());

    state.clear_error();
    assert!(!state.is_degraded());
    assert!(state.has_data());
}

#[test]
fn test_clear_error_when_no_error_is_noop() {
    let mut state = populated_dashboard();
    assert!(state.error.is_none());
    state.clear_error();
    assert!(state.error.is_none());
    assert!(state.has_data());
}

#[test]
fn test_clear_error_preserves_cluster_data() {
    let mut state = populated_dashboard();
    state.set_error("err".to_string());
    state.clear_error();
    assert_eq!(state.node_count, 3);
    assert_eq!(state.nodes.len(), 3);
    assert_eq!(state.pod_summary.total, 12);
    assert_eq!(state.namespaces.len(), 2);
}

// ===========================================================================
// 5. Loading state transitions
// ===========================================================================

// Loading -> Loaded
#[test]
fn test_loading_to_loaded_transition() {
    let id = Uuid::new_v4();
    let mut state = DashboardState::loading("cluster", id);
    assert!(state.loading);
    assert!(!state.has_data());

    // Simulate data arrival
    state.node_count = 3;
    state.pod_summary = sample_pod_summary();
    state.set_loaded();

    assert!(!state.loading);
    assert!(state.has_data());
    assert!(state.error.is_none());
}

// Loading -> Error
#[test]
fn test_loading_to_error_transition() {
    let id = Uuid::new_v4();
    let mut state = DashboardState::loading("cluster", id);
    assert!(state.loading);

    state.set_error("connection timed out".to_string());
    assert!(!state.loading);
    assert!(state.error.is_some());
    assert!(!state.has_data());
}

// Loaded -> Loading (e.g., cluster switch)
#[test]
fn test_loaded_to_loading_transition() {
    let mut state = populated_dashboard();
    assert!(state.has_data());

    state.set_loading();
    assert!(state.loading);
    assert!(state.error.is_none());
    // Note: set_loading clears error but doesn't clear data
}

// Loaded -> Error (degraded)
#[test]
fn test_loaded_to_error_becomes_degraded() {
    let mut state = populated_dashboard();
    assert!(state.has_data());
    assert!(!state.is_degraded());

    state.set_error("partial API failure".to_string());
    assert!(state.is_degraded());
    assert!(!state.loading);
}

// Error -> Loading (retry)
#[test]
fn test_error_to_loading_on_retry() {
    let mut state = DashboardState::with_error("cluster", "timeout");
    assert!(state.error.is_some());

    state.set_loading();
    assert!(state.loading);
    assert!(state.error.is_none());
}

// Error -> Loaded (successful retry)
#[test]
fn test_error_to_loaded_after_retry() {
    let mut state = DashboardState::with_error("cluster", "timeout");
    state.set_loading();
    state.node_count = 2;
    state.pod_summary = PodSummary::new(5, 0, 0, 0);
    state.set_loaded();

    assert!(!state.loading);
    assert!(state.error.is_none());
    assert!(state.has_data());
}

// Degraded -> Clear error -> Normal
#[test]
fn test_degraded_to_normal_via_clear_error() {
    let mut state = populated_dashboard();
    state.set_error("metrics down".to_string());
    assert!(state.is_degraded());

    state.clear_error();
    assert!(!state.is_degraded());
    assert!(state.has_data());
}

// Loading -> Loading (double set_loading is idempotent)
#[test]
fn test_double_set_loading_is_idempotent() {
    let mut state = populated_dashboard();
    state.set_loading();
    assert!(state.loading);
    state.set_loading();
    assert!(state.loading);
    assert!(state.error.is_none());
}

// Error -> Error (set_error overwrites previous error)
#[test]
fn test_set_error_overwrites_previous_error() {
    let mut state = DashboardState::with_error("cluster", "first error");
    state.set_error("second error".to_string());
    assert_eq!(state.error.as_deref(), Some("second error"));
}

// ===========================================================================
// 6. Additional edge cases
// ===========================================================================

#[test]
fn test_loading_constructor_sets_cluster_id() {
    let id = Uuid::new_v4();
    let state = DashboardState::loading("my-cluster", id);
    assert_eq!(state.cluster_id, Some(id));
    assert_eq!(state.cluster_name, "my-cluster");
}

#[test]
fn test_with_error_cluster_id_is_none() {
    let state = DashboardState::with_error("cluster", "err");
    assert!(state.cluster_id.is_none());
}

#[test]
fn test_has_data_requires_no_loading_no_error_and_data() {
    // No loading, no error, but no data either
    let state = DashboardState::new("empty", 0, empty_pod_summary());
    assert!(!state.has_data());

    // Has nodes but is loading
    let mut loading_state = DashboardState::new("loading", 5, sample_pod_summary());
    loading_state.loading = true;
    assert!(!loading_state.has_data());

    // Has nodes but has error
    let error_state = DashboardState::with_error("errored", "err");
    assert!(!error_state.has_data());

    // Has nodes, no loading, no error
    let good_state = DashboardState::new("good", 5, sample_pod_summary());
    assert!(good_state.has_data());
}

#[test]
fn test_empty_state_message_priority_loading_over_error() {
    // If somehow both loading and error are set, loading takes priority
    let mut state = DashboardState::with_error("cluster", "err");
    state.loading = true;
    assert_eq!(state.empty_state_message(), "Loading cluster data...");
}

#[test]
fn test_resource_table_filter_on_empty_table_returns_empty() {
    let mut state = ResourceTableState::new(sample_columns(), 20);
    state.filter_text = "anything".to_string();
    assert!(state.filtered_rows().is_empty());
    assert_eq!(state.total_filtered_count(), 0);
}

#[test]
fn test_resource_table_scroll_to_on_empty_clamps_to_zero() {
    let mut state = ResourceTableState::new(sample_columns(), 20);
    state.scroll_to(100);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_set_error_clears_loading_flag() {
    let mut state = DashboardState::loading("cluster", Uuid::new_v4());
    assert!(state.loading);
    state.set_error("failed".to_string());
    assert!(!state.loading);
    assert!(state.error.is_some());
}

#[test]
fn test_set_loading_clears_error() {
    let mut state = DashboardState::with_error("cluster", "some error");
    assert!(state.error.is_some());
    state.set_loading();
    assert!(state.error.is_none());
    assert!(state.loading);
}
