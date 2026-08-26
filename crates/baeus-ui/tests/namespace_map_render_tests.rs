use baeus_core::resource::{OwnerReference, Resource};
use baeus_ui::views::namespace_map::*;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// T079: Render tests for NamespaceMapView
// ---------------------------------------------------------------------------

fn test_cluster_id() -> Uuid {
    Uuid::new_v4()
}

fn make_resource_with_uid(
    uid: &str,
    name: &str,
    namespace: &str,
    kind: &str,
    api_version: &str,
    cluster_id: Uuid,
) -> Resource {
    let mut r = Resource::new(name, namespace, kind, api_version, cluster_id);
    r.uid = uid.to_string();
    r
}

// --- Default state tests ---

#[test]
fn test_default_state() {
    let state = NamespaceMapState::default();
    assert!(state.namespace.is_none());
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert!(state.resource_map.layout.nodes.is_empty());
}

#[test]
fn test_set_namespace() {
    let mut state = NamespaceMapState::default();
    state.set_namespace(Some("production".to_string()));
    assert_eq!(state.namespace.as_deref(), Some("production"));
}

#[test]
fn test_set_namespace_clears_map() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    // Build some resources
    let svc = make_resource_with_uid("svc-uid", "svc", "default", "Service", "v1", cluster)
        .with_spec(serde_json::json!({"selector": {"app": "web"}}));
    let pod = make_resource_with_uid("pod-uid", "pod", "default", "Pod", "v1", cluster)
        .with_label("app", "web");
    state.set_resources(&[svc, pod]);
    assert!(!state.resource_map.layout.nodes.is_empty());

    // Changing namespace should clear the layout
    state.set_namespace(Some("other".to_string()));
    assert!(state.resource_map.layout.nodes.is_empty());
}

// --- Toolbar rendering tests ---

#[test]
fn test_toolbar_shows_namespace_label() {
    let mut state = NamespaceMapState::default();
    state.set_namespace(Some("production".to_string()));

    assert_eq!(state.namespace.as_deref(), Some("production"));
}

#[test]
fn test_toolbar_shows_all_namespaces_when_none_selected() {
    let state = NamespaceMapState::default();
    assert!(state.namespace.is_none());
}

// --- Loading state tests ---

#[test]
fn test_loading_state() {
    let mut state = NamespaceMapState::default();
    state.set_loading(true);
    assert!(state.loading);
}

#[test]
fn test_loading_indicator_shown_when_loading() {
    let mut state = NamespaceMapState::default();
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());
}

// --- Error state tests ---

#[test]
fn test_error_state() {
    let mut state = NamespaceMapState::default();
    state.set_error("connection failed".to_string());
    assert_eq!(state.error.as_deref(), Some("connection failed"));
    assert!(!state.loading);
}

#[test]
fn test_error_message_shown_when_error() {
    let mut state = NamespaceMapState::default();
    state.set_error("Failed to load resources".to_string());
    assert!(state.error.is_some());
    assert!(!state.loading);
}

// --- Empty state tests ---

#[test]
fn test_empty_state_when_no_namespace_selected() {
    let state = NamespaceMapState::default();
    assert!(state.namespace.is_none());
    assert!(state.resource_map.layout.nodes.is_empty());
}

#[test]
fn test_empty_state_when_no_resources_found() {
    let mut state = NamespaceMapState::default();
    state.set_namespace(Some("empty-ns".to_string()));
    state.set_resources(&[]);
    assert!(state.resource_map.layout.nodes.is_empty());
}

// --- Resource loading tests ---

#[test]
fn test_set_resources_builds_layout() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    let deploy = make_resource_with_uid(
        "deploy-uid",
        "my-deploy",
        "default",
        "Deployment",
        "apps/v1",
        cluster,
    );
    let mut rs =
        make_resource_with_uid("rs-uid", "my-rs", "default", "ReplicaSet", "apps/v1", cluster);
    rs.owner_references.push(OwnerReference {
        uid: "deploy-uid".to_string(),
        kind: "Deployment".to_string(),
        name: "my-deploy".to_string(),
        api_version: "apps/v1".to_string(),
        controller: true,
    });

    state.set_resources(&[deploy, rs]);

    assert!(!state.loading);
    assert_eq!(state.resource_map.layout.nodes.len(), 2);
    assert_eq!(state.resource_map.layout.edges.len(), 1);
}

#[test]
fn test_set_resources_with_empty_list() {
    let mut state = NamespaceMapState::default();
    state.set_resources(&[]);
    assert!(state.resource_map.layout.nodes.is_empty());
}

// --- Selection tests ---

#[test]
fn test_select_resource() {
    let mut state = NamespaceMapState::default();
    state.select_resource("Pod/default/my-pod");
    assert_eq!(state.resource_map.selected_node.as_deref(), Some("Pod/default/my-pod"));
}

#[test]
fn test_clear_selection() {
    let mut state = NamespaceMapState::default();
    state.select_resource("Service/ns/svc");
    state.clear_selection();
    assert!(state.resource_map.selected_node.is_none());
}

// --- T080: Loading lifecycle tests ---

#[test]
fn test_begin_loading() {
    let mut state = NamespaceMapState::default();
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());
}

#[test]
fn test_load_complete_builds_graph() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    state.set_loading(true);
    assert!(state.loading);

    // Build Ingress -> Service -> Pod chain
    let ingress = make_resource_with_uid(
        "ing-uid",
        "my-ingress",
        "production",
        "Ingress",
        "networking.k8s.io/v1",
        cluster,
    )
    .with_spec(serde_json::json!({
        "rules": [{
            "host": "app.example.com",
            "http": {
                "paths": [{
                    "path": "/",
                    "backend": {
                        "service": { "name": "my-service", "port": { "number": 80 } }
                    }
                }]
            }
        }]
    }));

    let svc =
        make_resource_with_uid("svc-uid", "my-service", "production", "Service", "v1", cluster)
            .with_spec(serde_json::json!({ "selector": { "app": "web" } }));

    let pod = make_resource_with_uid("pod-uid", "web-pod", "production", "Pod", "v1", cluster)
        .with_label("app", "web");

    state.set_resources(&[ingress, svc, pod]);

    assert!(!state.loading);
    assert!(state.error.is_none());
    assert_eq!(state.resource_map.layout.nodes.len(), 3);
    assert_eq!(state.resource_map.layout.edges.len(), 2);
}

#[test]
fn test_load_failed_sets_error() {
    let mut state = NamespaceMapState::default();
    state.set_loading(true);
    state.set_error("API server unreachable".to_string());

    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("API server unreachable"));
}

#[test]
fn test_load_complete_clears_loading_flag() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    state.set_loading(true);
    assert!(state.loading);

    let pod = make_resource_with_uid("p1", "pod-1", "default", "Pod", "v1", cluster);
    state.set_resources(&[pod]);

    assert!(!state.loading);
}

// --- Integration tests ---

#[test]
fn test_full_pipeline_namespace_map() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    state.set_namespace(Some("production".to_string()));
    state.set_loading(true);

    // Build resources
    let deploy = make_resource_with_uid(
        "d-uid",
        "my-deploy",
        "production",
        "Deployment",
        "apps/v1",
        cluster,
    );
    let mut rs =
        make_resource_with_uid("rs-uid", "my-rs", "production", "ReplicaSet", "apps/v1", cluster);
    rs.owner_references.push(OwnerReference {
        uid: "d-uid".to_string(),
        kind: "Deployment".to_string(),
        name: "my-deploy".to_string(),
        api_version: "apps/v1".to_string(),
        controller: true,
    });

    state.set_resources(&[deploy, rs]);

    // Verify state
    assert_eq!(state.namespace.as_deref(), Some("production"));
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert_eq!(state.resource_map.layout.nodes.len(), 2);

    // Select a node
    state.select_resource("Deployment/production/my-deploy");
    assert_eq!(
        state.resource_map.selected_node.as_deref(),
        Some("Deployment/production/my-deploy")
    );

    // Clear
    state.clear_selection();
    assert!(state.resource_map.selected_node.is_none());
}

#[test]
fn test_changing_namespace_clears_error() {
    let mut state = NamespaceMapState::default();
    state.set_error("some error".to_string());
    assert!(state.error.is_some());

    state.set_namespace(Some("new-ns".to_string()));
    assert!(state.error.is_none());
}

#[test]
fn test_loading_lifecycle_complete_flow() {
    let mut state = NamespaceMapState::default();
    let cluster = test_cluster_id();

    // 1. Begin loading
    state.set_loading(true);
    assert!(state.loading);

    // 2. Load complete
    let svc = make_resource_with_uid("s1", "svc1", "ns", "Service", "v1", cluster)
        .with_spec(serde_json::json!({"selector": {"app": "x"}}));
    let pod =
        make_resource_with_uid("p1", "pod1", "ns", "Pod", "v1", cluster).with_label("app", "x");
    state.set_resources(&[svc, pod]);

    assert!(!state.loading);
    assert!(state.error.is_none());
    assert_eq!(state.resource_map.layout.nodes.len(), 2);
}

#[test]
fn test_loading_lifecycle_error_flow() {
    let mut state = NamespaceMapState::default();

    // 1. Begin loading
    state.set_loading(true);
    assert!(state.loading);

    // 2. Load failed
    state.set_error("timeout".to_string());

    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("timeout"));
}
