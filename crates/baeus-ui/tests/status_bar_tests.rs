// T353: Status Bar tests
//
// Tests for the data that would feed a status bar component.
// Since the status bar does not exist as a separate component yet,
// these tests verify ClusterStatus and SidebarState cluster tracking
// that would supply status bar information.

use baeus_ui::layout::sidebar::{ClusterStatus, SidebarState};

// =========================================================================
// T353: ClusterStatus enum variants
// =========================================================================

#[test]
fn test_cluster_status_connected_variant() {
    let status = ClusterStatus::Connected;
    assert_eq!(status, ClusterStatus::Connected);
}

#[test]
fn test_cluster_status_disconnected_variant() {
    let status = ClusterStatus::Disconnected;
    assert_eq!(status, ClusterStatus::Disconnected);
}

#[test]
fn test_cluster_status_connecting_variant() {
    let status = ClusterStatus::Connecting;
    assert_eq!(status, ClusterStatus::Connecting);
}

#[test]
fn test_cluster_status_error_variant() {
    let status = ClusterStatus::Error;
    assert_eq!(status, ClusterStatus::Error);
}

#[test]
fn test_cluster_status_all_variants_distinct() {
    let variants = [
        ClusterStatus::Connected,
        ClusterStatus::Disconnected,
        ClusterStatus::Connecting,
        ClusterStatus::Error,
    ];
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j]);
        }
    }
}

#[test]
fn test_cluster_status_clone() {
    let status = ClusterStatus::Connected;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_cluster_status_debug_format() {
    let status = ClusterStatus::Connected;
    let debug = format!("{:?}", status);
    assert!(debug.contains("Connected"));
}

// =========================================================================
// T353: Connected cluster's context_name can be retrieved
// =========================================================================

#[test]
fn test_connected_cluster_context_name() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod-us-east-1", "Production US East");

    // Set to connected
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.context_name, "prod-us-east-1");
    assert_eq!(cluster.status, ClusterStatus::Connected);
}

#[test]
fn test_cluster_context_name_via_selected_cluster() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("minikube", "Local Minikube");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

    state.select_cluster(id);
    let selected = state.selected_cluster().unwrap();
    assert_eq!(selected.context_name, "minikube");
}

#[test]
fn test_cluster_display_name_preserved() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("arn:aws:eks:us-east-1:123456:cluster/prod", "Production EKS");

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.display_name, "Production EKS");
    assert_eq!(cluster.context_name, "arn:aws:eks:us-east-1:123456:cluster/prod");
}

// =========================================================================
// T353: Status transitions: Disconnected -> Connecting -> Connected
// =========================================================================

#[test]
fn test_status_transition_disconnected_to_connecting() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("dev-cluster", "Dev");

    // Initial state is Disconnected
    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Disconnected);

    // Transition to Connecting
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connecting;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Connecting);
}

#[test]
fn test_status_transition_connecting_to_connected() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("staging", "Staging");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connecting;

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Connected);
}

#[test]
fn test_full_status_transition_lifecycle() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "Production");

    // Disconnected (initial)
    assert_eq!(
        state.clusters.iter().find(|c| c.id == id).unwrap().status,
        ClusterStatus::Disconnected
    );

    // -> Connecting
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connecting;
    assert_eq!(
        state.clusters.iter().find(|c| c.id == id).unwrap().status,
        ClusterStatus::Connecting
    );

    // -> Connected
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;
    assert_eq!(
        state.clusters.iter().find(|c| c.id == id).unwrap().status,
        ClusterStatus::Connected
    );
}

#[test]
fn test_status_transition_to_error() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "Production");

    // Connecting -> Error
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connecting;
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Error;

    assert_eq!(state.clusters.iter().find(|c| c.id == id).unwrap().status, ClusterStatus::Error);
}

#[test]
fn test_status_transition_error_to_disconnected() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "Production");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Error;
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Disconnected;

    assert_eq!(
        state.clusters.iter().find(|c| c.id == id).unwrap().status,
        ClusterStatus::Disconnected
    );
}

#[test]
fn test_connected_to_disconnected_transition() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "Production");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Disconnected;

    assert_eq!(
        state.clusters.iter().find(|c| c.id == id).unwrap().status,
        ClusterStatus::Disconnected
    );
}

// =========================================================================
// T353: Error status carries information
// =========================================================================

#[test]
fn test_error_status_is_distinguishable() {
    let status = ClusterStatus::Error;
    assert_ne!(status, ClusterStatus::Connected);
    assert_ne!(status, ClusterStatus::Connecting);
    assert_ne!(status, ClusterStatus::Disconnected);
}

#[test]
fn test_error_status_on_cluster_entry() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("broken-cluster", "Broken");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Error;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Error);
    // The context_name is still accessible even in error state
    assert_eq!(cluster.context_name, "broken-cluster");
    assert_eq!(cluster.display_name, "Broken");
}

// =========================================================================
// T353: Multiple clusters can have different statuses simultaneously
// =========================================================================

#[test]
fn test_multiple_clusters_different_statuses() {
    let mut state = SidebarState::default();
    let prod_id = state.add_cluster("prod", "Production");
    let staging_id = state.add_cluster("staging", "Staging");
    let dev_id = state.add_cluster("dev", "Development");
    let broken_id = state.add_cluster("broken", "Broken Cluster");

    // Set different statuses
    state.clusters.iter_mut().find(|c| c.id == prod_id).unwrap().status = ClusterStatus::Connected;
    state.clusters.iter_mut().find(|c| c.id == staging_id).unwrap().status =
        ClusterStatus::Connecting;
    // dev stays Disconnected (default)
    state.clusters.iter_mut().find(|c| c.id == broken_id).unwrap().status = ClusterStatus::Error;

    // Verify each cluster has its own status
    assert_eq!(
        state.clusters.iter().find(|c| c.id == prod_id).unwrap().status,
        ClusterStatus::Connected
    );
    assert_eq!(
        state.clusters.iter().find(|c| c.id == staging_id).unwrap().status,
        ClusterStatus::Connecting
    );
    assert_eq!(
        state.clusters.iter().find(|c| c.id == dev_id).unwrap().status,
        ClusterStatus::Disconnected
    );
    assert_eq!(
        state.clusters.iter().find(|c| c.id == broken_id).unwrap().status,
        ClusterStatus::Error
    );
}

#[test]
fn test_multiple_clusters_all_connected() {
    let mut state = SidebarState::default();
    let ids: Vec<_> = (0..5)
        .map(|i| state.add_cluster(&format!("cluster-{i}"), &format!("Cluster {i}")))
        .collect();

    for id in &ids {
        state.clusters.iter_mut().find(|c| c.id == *id).unwrap().status = ClusterStatus::Connected;
    }

    assert!(state.clusters.iter().all(|c| c.status == ClusterStatus::Connected));
}

#[test]
fn test_cluster_status_change_does_not_affect_others() {
    let mut state = SidebarState::default();
    let id_a = state.add_cluster("cluster-a", "Cluster A");
    let id_b = state.add_cluster("cluster-b", "Cluster B");

    state.clusters.iter_mut().find(|c| c.id == id_a).unwrap().status = ClusterStatus::Connected;

    // Changing A's status should not affect B
    assert_eq!(
        state.clusters.iter().find(|c| c.id == id_b).unwrap().status,
        ClusterStatus::Disconnected
    );

    state.clusters.iter_mut().find(|c| c.id == id_a).unwrap().status = ClusterStatus::Error;

    // B still unaffected
    assert_eq!(
        state.clusters.iter().find(|c| c.id == id_b).unwrap().status,
        ClusterStatus::Disconnected
    );
}

// =========================================================================
// T353: Status bar data retrieval patterns
// =========================================================================

#[test]
fn test_selected_cluster_status_for_status_bar() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "Production");

    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

    // Status bar would read from selected cluster
    let selected = state.selected_cluster().unwrap();
    assert_eq!(selected.status, ClusterStatus::Connected);
    assert_eq!(selected.context_name, "prod");
}

#[test]
fn test_count_clusters_by_status() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("c1", "C1");
    let id2 = state.add_cluster("c2", "C2");
    let id3 = state.add_cluster("c3", "C3");
    let _id4 = state.add_cluster("c4", "C4");

    state.clusters.iter_mut().find(|c| c.id == id1).unwrap().status = ClusterStatus::Connected;
    state.clusters.iter_mut().find(|c| c.id == id2).unwrap().status = ClusterStatus::Connected;
    state.clusters.iter_mut().find(|c| c.id == id3).unwrap().status = ClusterStatus::Error;
    // id4 remains Disconnected

    let connected_count =
        state.clusters.iter().filter(|c| c.status == ClusterStatus::Connected).count();
    let disconnected_count =
        state.clusters.iter().filter(|c| c.status == ClusterStatus::Disconnected).count();
    let error_count = state.clusters.iter().filter(|c| c.status == ClusterStatus::Error).count();

    assert_eq!(connected_count, 2);
    assert_eq!(disconnected_count, 1);
    assert_eq!(error_count, 1);
}

#[test]
fn test_find_cluster_by_context_for_status_bar() {
    let mut state = SidebarState::default();
    state.add_cluster("prod-us-east", "Prod US East");
    state.add_cluster("staging-eu-west", "Staging EU West");

    let id = state.find_cluster_id_by_context("prod-us-east");
    assert!(id.is_some());

    let cluster = state.clusters.iter().find(|c| c.id == id.unwrap()).unwrap();
    assert_eq!(cluster.context_name, "prod-us-east");
}

#[test]
fn test_find_cluster_by_context_not_found() {
    let state = SidebarState::default();
    let id = state.find_cluster_id_by_context("nonexistent");
    assert!(id.is_none());
}
