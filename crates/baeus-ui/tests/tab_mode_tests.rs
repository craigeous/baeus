// T352: Preview vs Fixed tab mode tests
//
// Tests for tab mode behavior using WorkspaceState. Validates regular tab
// creation (fixed/non-preview), tab reuse for same target, closing tabs,
// tab labels matching NavigationTarget::label(), and active tab tracking.
// (T354 will add `is_preview` field for preview tab support.)

use baeus_ui::icons::ResourceCategory;
use baeus_ui::layout::NavigationTarget;
use baeus_ui::layout::workspace::WorkspaceState;

const TEST_CLUSTER: &str = "test-cluster";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pod_list_target() -> NavigationTarget {
    NavigationTarget::ResourceList {
        cluster_context: TEST_CLUSTER.to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    }
}

fn deployment_list_target() -> NavigationTarget {
    NavigationTarget::ResourceList {
        cluster_context: TEST_CLUSTER.to_string(),
        category: ResourceCategory::Workloads,
        kind: "Deployment".to_string(),
    }
}

fn service_list_target() -> NavigationTarget {
    NavigationTarget::ResourceList {
        cluster_context: TEST_CLUSTER.to_string(),
        category: ResourceCategory::Network,
        kind: "Service".to_string(),
    }
}

fn events_target() -> NavigationTarget {
    NavigationTarget::ResourceList {
        cluster_context: TEST_CLUSTER.to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    }
}

fn helm_target() -> NavigationTarget {
    NavigationTarget::HelmReleases { cluster_context: TEST_CLUSTER.to_string() }
}

fn detail_target(kind: &str, name: &str) -> NavigationTarget {
    NavigationTarget::ResourceDetail {
        cluster_context: TEST_CLUSTER.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        namespace: Some("default".to_string()),
    }
}

// =========================================================================
// T352: Regular open_tab creates a fixed (non-preview) tab
// =========================================================================

#[test]
fn test_open_tab_creates_fixed_tab() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(pod_list_target());

    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert!(tab.closable, "regular tab should be closable");
    assert!(!tab.dirty, "new tab should not be dirty");
}

#[test]
fn test_open_tab_becomes_active() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(pod_list_target());

    assert_eq!(ws.active_tab_id, Some(id));
    assert!(ws.active_tab().is_some());
    assert_eq!(ws.active_tab().unwrap().id, id);
}

#[test]
fn test_open_multiple_tabs_last_is_active() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(pod_list_target());
    ws.open_tab(deployment_list_target());
    let third_id = ws.open_tab(events_target());

    assert_eq!(ws.tab_count(), 3);
    assert_eq!(ws.active_tab_id, Some(third_id));
}

#[test]
fn test_pinned_tab_is_not_closable() {
    let ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
    let dashboard = ws.active_tab().unwrap();
    assert!(!dashboard.closable);
}

// =========================================================================
// T352: Tab reuse — opening same target reuses existing tab
// =========================================================================

#[test]
fn test_open_same_target_reuses_tab() {
    let mut ws = WorkspaceState::default();
    let id1 = ws.open_tab(pod_list_target());
    let id2 = ws.open_tab(pod_list_target());

    assert_eq!(id1, id2, "opening the same target should return the same tab ID");
    assert_eq!(ws.tab_count(), 1, "should not create a duplicate tab");
}

#[test]
fn test_open_same_target_activates_existing() {
    let mut ws = WorkspaceState::default();
    let pod_id = ws.open_tab(pod_list_target());
    let deploy_id = ws.open_tab(deployment_list_target());

    assert_eq!(ws.active_tab_id, Some(deploy_id));

    // Re-open pod list, should activate the existing tab
    let reused_id = ws.open_tab(pod_list_target());
    assert_eq!(reused_id, pod_id);
    assert_eq!(ws.active_tab_id, Some(pod_id));
    assert_eq!(ws.tab_count(), 2);
}

#[test]
fn test_different_targets_create_different_tabs() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(pod_list_target());
    ws.open_tab(deployment_list_target());
    ws.open_tab(service_list_target());

    assert_eq!(ws.tab_count(), 3);
}

#[test]
fn test_same_kind_different_cluster_creates_new_tab() {
    let mut ws = WorkspaceState::default();
    let target_a = NavigationTarget::ResourceList {
        cluster_context: "cluster-a".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };
    let target_b = NavigationTarget::ResourceList {
        cluster_context: "cluster-b".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };

    ws.open_tab(target_a);
    ws.open_tab(target_b);

    assert_eq!(ws.tab_count(), 2, "same kind in different clusters should create separate tabs");
}

// =========================================================================
// T352: Closing tabs works correctly
// =========================================================================

#[test]
fn test_close_tab_removes_it() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(events_target());

    assert!(ws.close_tab(id));
    assert_eq!(ws.tab_count(), 0);
}

#[test]
fn test_close_tab_returns_false_for_pinned() {
    let ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
    let dashboard_id = ws.active_tab().unwrap().id;

    let mut ws = ws;
    assert!(!ws.close_tab(dashboard_id));
    assert_eq!(ws.tab_count(), 1);
}

#[test]
fn test_close_nonexistent_tab_returns_false() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(pod_list_target());

    let fake_id = uuid::Uuid::new_v4();
    assert!(!ws.close_tab(fake_id));
    assert_eq!(ws.tab_count(), 1);
}

#[test]
fn test_close_active_tab_activates_neighbor() {
    let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
    let events_id = ws.open_tab(events_target());
    let helm_id = ws.open_tab(helm_target());

    // Active is helm_target
    assert_eq!(ws.active_tab_id, Some(helm_id));

    // Close helm, should activate events
    ws.close_tab(helm_id);
    assert_eq!(ws.active_tab_id, Some(events_id));
}

#[test]
fn test_close_middle_tab_activates_correct_neighbor() {
    let mut ws = WorkspaceState::default();
    let pod_id = ws.open_tab(pod_list_target());
    let deploy_id = ws.open_tab(deployment_list_target());
    let service_id = ws.open_tab(service_list_target());

    // Activate middle tab
    ws.activate_tab(deploy_id);
    assert_eq!(ws.active_tab_id, Some(deploy_id));

    // Close middle tab
    ws.close_tab(deploy_id);
    // Should activate one of the remaining tabs
    assert!(ws.active_tab_id.is_some());
    let active = ws.active_tab_id.unwrap();
    assert!(active == pod_id || active == service_id);
}

#[test]
fn test_close_last_remaining_closable_tab() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(events_target());

    ws.close_tab(id);
    assert_eq!(ws.tab_count(), 0);
    assert!(ws.active_tab_id.is_none());
    assert!(ws.active_tab().is_none());
}

// =========================================================================
// T352: Tab labels match NavigationTarget::label()
// =========================================================================

#[test]
fn test_tab_label_matches_navigation_target_label() {
    let mut ws = WorkspaceState::default();
    let target = pod_list_target();
    let expected_label = target.label();
    let id = ws.open_tab(target);

    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, expected_label);
}

#[test]
fn test_tab_label_for_dashboard() {
    let ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
    let tab = ws.active_tab().unwrap();
    assert_eq!(tab.label, "test-cluster - Overview");
}

#[test]
fn test_tab_label_for_resource_list() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(deployment_list_target());
    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "test-cluster - Deployment");
}

#[test]
fn test_tab_label_for_resource_detail() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(detail_target("Pod", "nginx-abc"));
    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "test-cluster - Pod/nginx-abc");
}

#[test]
fn test_tab_label_for_events() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(events_target());
    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "test-cluster - Event");
}

#[test]
fn test_tab_label_for_helm_releases() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(helm_target());
    let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "test-cluster - Helm Releases");
}

#[test]
fn test_tab_labels_include_cluster_context() {
    let mut ws = WorkspaceState::default();

    let targets = vec![pod_list_target(), events_target(), helm_target()];

    for target in targets {
        let id = ws.open_tab(target);
        let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
        assert!(
            tab.label.starts_with(TEST_CLUSTER),
            "tab label '{}' should start with cluster context",
            tab.label
        );
    }
}

// =========================================================================
// T352: Active tab tracking when switching between tabs
// =========================================================================

#[test]
fn test_activate_tab_switches_active() {
    let mut ws = WorkspaceState::default();
    let pod_id = ws.open_tab(pod_list_target());
    let deploy_id = ws.open_tab(deployment_list_target());

    assert_eq!(ws.active_tab_id, Some(deploy_id));

    ws.activate_tab(pod_id);
    assert_eq!(ws.active_tab_id, Some(pod_id));
    assert_eq!(ws.active_tab().unwrap().id, pod_id);
}

#[test]
fn test_activate_nonexistent_tab_returns_false() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(pod_list_target());

    let fake_id = uuid::Uuid::new_v4();
    let result = ws.activate_tab(fake_id);
    assert!(!result);
}

#[test]
fn test_activate_already_active_tab_is_noop() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(pod_list_target());

    assert_eq!(ws.active_tab_id, Some(id));
    let result = ws.activate_tab(id);
    assert!(result);
    assert_eq!(ws.active_tab_id, Some(id));
}

#[test]
fn test_active_tab_returns_correct_target() {
    let mut ws = WorkspaceState::default();
    let target = events_target();
    ws.open_tab(target.clone());

    assert_eq!(ws.active_tab().unwrap().target, target);
}

#[test]
fn test_switching_between_multiple_tabs() {
    let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
    let dashboard_id = ws.active_tab().unwrap().id;
    let pod_id = ws.open_tab(pod_list_target());
    let deploy_id = ws.open_tab(deployment_list_target());
    let events_id = ws.open_tab(events_target());

    assert_eq!(ws.tab_count(), 4);

    // Switch to dashboard
    ws.activate_tab(dashboard_id);
    assert_eq!(ws.active_tab().unwrap().label, "test-cluster - Overview");

    // Switch to pods
    ws.activate_tab(pod_id);
    assert!(ws.active_tab().unwrap().label.contains("Pod"));

    // Switch to events
    ws.activate_tab(events_id);
    assert!(ws.active_tab().unwrap().label.contains("Event"));

    // Switch to deployments
    ws.activate_tab(deploy_id);
    assert!(ws.active_tab().unwrap().label.contains("Deployment"));
}

#[test]
fn test_tab_count_reflects_operations() {
    let mut ws = WorkspaceState::default();
    assert_eq!(ws.tab_count(), 0);

    ws.open_tab(pod_list_target());
    assert_eq!(ws.tab_count(), 1);

    let events_id = ws.open_tab(events_target());
    assert_eq!(ws.tab_count(), 2);

    // Reuse existing tab
    ws.open_tab(pod_list_target());
    assert_eq!(ws.tab_count(), 2);

    ws.close_tab(events_id);
    assert_eq!(ws.tab_count(), 1);
}

// =========================================================================
// T352: Dirty flag tracking on tabs
// =========================================================================

#[test]
fn test_mark_dirty_and_clean() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(pod_list_target());

    ws.mark_dirty(id);
    assert!(ws.tabs.iter().find(|t| t.id == id).unwrap().dirty);

    ws.mark_clean(id);
    assert!(!ws.tabs.iter().find(|t| t.id == id).unwrap().dirty);
}
