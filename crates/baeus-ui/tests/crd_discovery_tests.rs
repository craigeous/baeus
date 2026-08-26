use baeus_core::crd::{CrdSchema, CrdScope};
use baeus_ui::views::crd_browser::CrdBrowserState;
use uuid::Uuid;

fn sample_crd(name: &str, group: &str, kind: &str, scope: CrdScope) -> CrdSchema {
    CrdSchema {
        name: format!("{}.{group}", name.to_lowercase()),
        group: group.to_string(),
        kind: kind.to_string(),
        versions: vec!["v1".to_string()],
        scope,
        description: None,
        schema_properties: None,
        schema: None,
        cluster_id: Uuid::new_v4(),
    }
}

fn sample_crds() -> Vec<CrdSchema> {
    vec![
        sample_crd("certificates", "cert-manager.io", "Certificate", CrdScope::Namespaced),
        sample_crd("issuers", "cert-manager.io", "Issuer", CrdScope::Namespaced),
        sample_crd("clusterissuers", "cert-manager.io", "ClusterIssuer", CrdScope::Cluster),
    ]
}

// --- T076: CRD discovery lifecycle tests ---

#[test]
fn test_begin_discovery_sets_loading() {
    let mut state = CrdBrowserState::default();
    assert!(!state.loading);

    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());
}

#[test]
fn test_begin_discovery_clears_error() {
    let mut state = CrdBrowserState::default();
    state.set_error("previous error".to_string());
    assert!(state.error.is_some());

    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());
}

#[test]
fn test_discovery_complete_sets_crds() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);

    state.set_crds(sample_crds());
    assert!(!state.loading);
    assert_eq!(state.crds.len(), 3);
    assert!(state.error.is_none());
}

#[test]
fn test_discovery_complete_clears_loading() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);
    assert!(state.loading);

    state.set_crds(sample_crds());
    assert!(!state.loading);
}

#[test]
fn test_discovery_complete_replaces_existing_crds() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    assert_eq!(state.crds.len(), 3);

    let new_crds = vec![sample_crd("single", "test.io", "Single", CrdScope::Namespaced)];
    state.set_crds(new_crds);
    assert_eq!(state.crds.len(), 1);
}

#[test]
fn test_discovery_failed_sets_error() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);

    state.set_error("Discovery failed: timeout".to_string());
    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("Discovery failed: timeout"));
}

#[test]
fn test_discovery_failed_clears_loading() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);
    assert!(state.loading);

    state.set_error("Connection refused".to_string());
    assert!(!state.loading);
}

#[test]
fn test_discovery_failed_preserves_existing_crds() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    assert_eq!(state.crds.len(), 3);

    state.set_error("Network error".to_string());
    assert_eq!(state.crds.len(), 3);
    assert!(state.error.is_some());
}

#[test]
fn test_full_discovery_success_workflow() {
    let mut state = CrdBrowserState::default();

    // Begin discovery
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());
    assert!(state.crds.is_empty());

    // Complete discovery
    state.set_crds(sample_crds());
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert_eq!(state.crds.len(), 3);
}

#[test]
fn test_full_discovery_failure_workflow() {
    let mut state = CrdBrowserState::default();

    // Begin discovery
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());

    // Discovery fails
    state.set_error("API server unreachable".to_string());
    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("API server unreachable"));
    assert!(state.crds.is_empty());
}

#[test]
fn test_retry_after_error() {
    let mut state = CrdBrowserState::default();

    // First discovery fails
    state.set_loading(true);
    state.set_error("First attempt failed".to_string());
    assert!(!state.loading);
    assert!(state.error.is_some());

    // Retry discovery
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());

    // Second attempt succeeds
    state.set_crds(sample_crds());
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert_eq!(state.crds.len(), 3);
}

#[test]
fn test_discovery_with_empty_result() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);

    state.set_crds(vec![]);
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert!(state.crds.is_empty());
}

#[test]
fn test_discovery_clears_previous_error() {
    let mut state = CrdBrowserState::default();
    state.set_error("Old error".to_string());
    assert!(state.error.is_some());

    state.set_crds(sample_crds());
    assert!(state.error.is_none());
}

#[test]
fn test_multiple_discovery_cycles() {
    let mut state = CrdBrowserState::default();

    // First discovery
    state.set_loading(true);
    state.set_crds(sample_crds());
    assert_eq!(state.crds.len(), 3);

    // Second discovery
    state.set_loading(true);
    let new_crds = vec![
        sample_crd("new1", "test.io", "New1", CrdScope::Namespaced),
        sample_crd("new2", "test.io", "New2", CrdScope::Cluster),
    ];
    state.set_crds(new_crds);
    assert_eq!(state.crds.len(), 2);

    // Third discovery
    state.set_loading(true);
    state.set_crds(vec![]);
    assert!(state.crds.is_empty());
}

#[test]
fn test_discovery_maintains_loading_state_consistency() {
    let mut state = CrdBrowserState::default();

    state.set_loading(true);
    assert!(state.loading);

    state.set_loading(false);
    assert!(!state.loading);

    state.set_loading(true);
    assert!(state.loading);

    state.set_crds(vec![]);
    assert!(!state.loading);
}
