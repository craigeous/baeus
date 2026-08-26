use baeus_core::crd::{CrdSchema, CrdScope};
use baeus_ui::theme::Theme;
use baeus_ui::views::crd_browser::{CrdBrowserState, CrdBrowserViewComponent};
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

fn sample_crd_with_versions(
    name: &str,
    group: &str,
    kind: &str,
    versions: Vec<&str>,
    scope: CrdScope,
) -> CrdSchema {
    CrdSchema {
        name: format!("{}.{group}", name.to_lowercase()),
        group: group.to_string(),
        kind: kind.to_string(),
        versions: versions.into_iter().map(|s| s.to_string()).collect(),
        scope,
        description: None,
        schema_properties: None,
        schema: None,
        cluster_id: Uuid::new_v4(),
    }
}

fn sample_crd_with_description(
    name: &str,
    group: &str,
    kind: &str,
    scope: CrdScope,
    description: &str,
) -> CrdSchema {
    CrdSchema {
        name: format!("{}.{group}", name.to_lowercase()),
        group: group.to_string(),
        kind: kind.to_string(),
        versions: vec!["v1".to_string()],
        scope,
        description: Some(description.to_string()),
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
        sample_crd("virtualmachines", "kubevirt.io", "VirtualMachine", CrdScope::Namespaced),
        sample_crd("ingressroutes", "traefik.containo.us", "IngressRoute", CrdScope::Namespaced),
    ]
}

// --- T074: Render tests ---

#[test]
fn test_renders_with_empty_state() {
    let state = CrdBrowserState::default();
    let theme = Theme::dark();
    let _component = CrdBrowserViewComponent::new(state, theme);
    // Component created successfully with empty state
}

#[test]
fn test_renders_with_crd_list() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let _component = CrdBrowserViewComponent::new(state, theme);
    // Component created with CRDs
}

#[test]
fn test_renders_crd_with_group_names() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    // Verify the state has the expected groups
    let groups = component.state.api_groups();
    assert!(groups.contains(&"cert-manager.io"));
    assert!(groups.contains(&"kubevirt.io"));
    assert!(groups.contains(&"traefik.containo.us"));
}

#[test]
fn test_renders_crd_with_kind_names() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    // Verify CRDs have kind names
    assert!(component.state.crds.iter().any(|c| c.kind == "Certificate"));
    assert!(component.state.crds.iter().any(|c| c.kind == "Issuer"));
    assert!(component.state.crds.iter().any(|c| c.kind == "ClusterIssuer"));
}

#[test]
fn test_renders_crd_with_versions() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd_with_versions(
        "certificates",
        "cert-manager.io",
        "Certificate",
        vec!["v1", "v1beta1", "v1alpha1"],
        CrdScope::Namespaced,
    );
    state.set_crds(vec![crd.clone()]);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let stored = &component.state.crds[0];
    assert_eq!(stored.versions.len(), 3);
    assert!(stored.supports_version("v1"));
    assert!(stored.supports_version("v1beta1"));
    assert!(stored.supports_version("v1alpha1"));
}

#[test]
fn test_renders_crd_with_scope() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let ns_count = component.state.namespaced_count();
    let cluster_count = component.state.cluster_scoped_count();

    // 4 namespaced: Certificate, Issuer, VirtualMachine, IngressRoute
    // 1 cluster: ClusterIssuer
    assert_eq!(ns_count, 4);
    assert_eq!(cluster_count, 1);
}

#[test]
fn test_group_filter_reduces_visible_crds() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let mut component = CrdBrowserViewComponent::new(state, theme);

    component.state.filter_by_group(Some("cert-manager.io".to_string()));
    let filtered = component.state.filtered_crds();
    assert_eq!(filtered.len(), 3);
    assert!(filtered.iter().all(|c| c.group == "cert-manager.io"));
}

#[test]
fn test_group_filter_none_shows_all() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    state.filter_by_group(Some("cert-manager.io".to_string()));
    let theme = Theme::dark();
    let mut component = CrdBrowserViewComponent::new(state, theme);

    component.state.filter_by_group(None);
    let filtered = component.state.filtered_crds();
    assert_eq!(filtered.len(), 5);
}

#[test]
fn test_selected_crd_shows_detail() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    state.select_crd("certificates.cert-manager.io");
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let selected = component.state.selected();
    assert!(selected.is_some());
    let crd = selected.unwrap();
    assert_eq!(crd.kind, "Certificate");
    assert_eq!(crd.group, "cert-manager.io");
}

#[test]
fn test_selected_crd_detail_has_full_info() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd_with_description(
        "certificates",
        "cert-manager.io",
        "Certificate",
        CrdScope::Namespaced,
        "Manages TLS certificates",
    );
    state.set_crds(vec![crd.clone()]);
    state.select_crd("certificates.cert-manager.io");
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let selected = component.state.selected().unwrap();
    assert_eq!(selected.name, "certificates.cert-manager.io");
    assert_eq!(selected.group, "cert-manager.io");
    assert_eq!(selected.kind, "Certificate");
    assert_eq!(selected.scope, CrdScope::Namespaced);
    assert_eq!(selected.description.as_deref(), Some("Manages TLS certificates"));
}

#[test]
fn test_empty_state_when_no_crds() {
    let state = CrdBrowserState::default();
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert!(component.state.crds.is_empty());
    assert!(!component.state.loading);
}

#[test]
fn test_loading_state_display() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert!(component.state.loading);
    assert!(component.state.error.is_none());
}

#[test]
fn test_error_state_display() {
    let mut state = CrdBrowserState::default();
    state.set_error("Failed to discover CRDs".to_string());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert!(!component.state.loading);
    assert_eq!(component.state.error.as_deref(), Some("Failed to discover CRDs"));
}

#[test]
fn test_api_group_dropdown_selector() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let groups = component.state.api_groups();
    assert_eq!(groups.len(), 3);
    assert!(groups.contains(&"cert-manager.io"));
    assert!(groups.contains(&"kubevirt.io"));
    assert!(groups.contains(&"traefik.containo.us"));
}

#[test]
fn test_crd_count_badge_total() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.state.total_count(), 5);
}

#[test]
fn test_crd_count_badge_namespaced() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.state.namespaced_count(), 4);
}

#[test]
fn test_crd_count_badge_cluster_scoped() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.state.cluster_scoped_count(), 1);
}

#[test]
fn test_count_badges_with_filter() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    state.filter_by_group(Some("cert-manager.io".to_string()));
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    // cert-manager.io has 2 namespaced and 1 cluster
    assert_eq!(component.state.filtered_count(), 3);
    assert_eq!(component.state.namespaced_count(), 2);
    assert_eq!(component.state.cluster_scoped_count(), 1);
}

#[test]
fn test_grouped_crds_by_api_group() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let grouped = component.state.grouped_crds();
    assert_eq!(grouped.len(), 3);
    assert_eq!(grouped["cert-manager.io"].len(), 3);
    assert_eq!(grouped["kubevirt.io"].len(), 1);
    assert_eq!(grouped["traefik.containo.us"].len(), 1);
}

#[test]
fn test_full_workflow_load_filter_select() {
    let mut state = CrdBrowserState::default();

    // Start loading
    state.set_loading(true);
    assert!(state.loading);

    // Load CRDs
    state.set_crds(sample_crds());
    assert!(!state.loading);
    assert_eq!(state.total_count(), 5);

    // Filter by group
    state.filter_by_group(Some("cert-manager.io".to_string()));
    assert_eq!(state.filtered_count(), 3);

    // Select a CRD
    state.select_crd("certificates.cert-manager.io");
    let selected = state.selected();
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().kind, "Certificate");

    // Create component
    let theme = Theme::dark();
    let _component = CrdBrowserViewComponent::new(state, theme);
}

#[test]
fn test_crd_entry_displays_multiple_versions() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd_with_versions(
        "certificates",
        "cert-manager.io",
        "Certificate",
        vec!["v1", "v1beta1", "v1alpha2", "v1alpha1"],
        CrdScope::Namespaced,
    );
    state.set_crds(vec![crd]);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let stored = &component.state.crds[0];
    let versions = stored.version_list();
    assert_eq!(versions.len(), 4);
    assert_eq!(versions, &["v1", "v1beta1", "v1alpha2", "v1alpha1"]);
}

#[test]
fn test_empty_state_no_crds_found() {
    let state = CrdBrowserState::default();
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert!(component.state.crds.is_empty());
    assert_eq!(component.state.total_count(), 0);
}

#[test]
fn test_loading_indicator_while_loading() {
    let mut state = CrdBrowserState::default();
    state.set_loading(true);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert!(component.state.loading);
    assert!(component.state.crds.is_empty());
}

#[test]
fn test_error_message_on_failure() {
    let mut state = CrdBrowserState::default();
    state.set_error("Connection timeout".to_string());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.state.error.as_deref(), Some("Connection timeout"));
}

#[test]
fn test_clear_filter_shows_all_crds() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    state.filter_by_group(Some("cert-manager.io".to_string()));
    assert_eq!(state.filtered_count(), 3);

    state.filter_by_group(None);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.state.filtered_count(), 5);
}

#[test]
fn test_selection_cleared_on_filter_change() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    state.select_crd("certificates.cert-manager.io");
    assert!(state.selected_crd.is_some());

    state.filter_by_group(Some("kubevirt.io".to_string()));
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    // Selection cleared when filter changes
    assert!(component.state.selected_crd.is_none());
}

#[test]
fn test_crd_entry_with_scope_badge_namespaced() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd("certificates", "cert-manager.io", "Certificate", CrdScope::Namespaced);
    state.set_crds(vec![crd]);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let stored = &component.state.crds[0];
    assert_eq!(stored.scope, CrdScope::Namespaced);
    assert!(stored.is_namespaced());
}

#[test]
fn test_crd_entry_with_scope_badge_cluster() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd("clusterissuers", "cert-manager.io", "ClusterIssuer", CrdScope::Cluster);
    state.set_crds(vec![crd]);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let stored = &component.state.crds[0];
    assert_eq!(stored.scope, CrdScope::Cluster);
    assert!(!stored.is_namespaced());
}

#[test]
fn test_detail_panel_shows_description() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd_with_description(
        "certificates",
        "cert-manager.io",
        "Certificate",
        CrdScope::Namespaced,
        "Manages X.509 certificates for TLS",
    );
    state.set_crds(vec![crd]);
    state.select_crd("certificates.cert-manager.io");
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let selected = component.state.selected().unwrap();
    assert_eq!(selected.description.as_deref(), Some("Manages X.509 certificates for TLS"));
}

#[test]
fn test_detail_panel_with_no_description() {
    let mut state = CrdBrowserState::default();
    let crd = sample_crd("certificates", "cert-manager.io", "Certificate", CrdScope::Namespaced);
    state.set_crds(vec![crd]);
    state.select_crd("certificates.cert-manager.io");
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let selected = component.state.selected().unwrap();
    assert!(selected.description.is_none());
}

#[test]
fn test_multiple_groups_renders_correctly() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let grouped = component.state.grouped_crds();
    assert_eq!(grouped.keys().len(), 3);
}

#[test]
fn test_single_group_renders_correctly() {
    let mut state = CrdBrowserState::default();
    let crds = vec![
        sample_crd("certificates", "cert-manager.io", "Certificate", CrdScope::Namespaced),
        sample_crd("issuers", "cert-manager.io", "Issuer", CrdScope::Namespaced),
    ];
    state.set_crds(crds);
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let grouped = component.state.grouped_crds();
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped["cert-manager.io"].len(), 2);
}

#[test]
fn test_crd_list_with_mixed_scopes() {
    let mut state = CrdBrowserState::default();
    state.set_crds(sample_crds());
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme);

    let ns_count = component.state.namespaced_count();
    let cluster_count = component.state.cluster_scoped_count();

    assert!(ns_count > 0);
    assert!(cluster_count > 0);
}

#[test]
fn test_theme_colors_applied() {
    let state = CrdBrowserState::default();
    let theme = Theme::dark();
    let component = CrdBrowserViewComponent::new(state, theme.clone());

    // Verify theme is stored
    assert_eq!(component.theme.mode, theme.mode);
}

#[test]
fn test_light_theme_applied() {
    let state = CrdBrowserState::default();
    let theme = Theme::light();
    let component = CrdBrowserViewComponent::new(state, theme);

    assert_eq!(component.theme.mode, baeus_ui::theme::ThemeMode::Light);
}
