// T059: Render tests for HelmReleasesView (state-level, no GPUI window needed).
//
// Verifies:
// - Status badge colors: Deployed=green, Failed=red, PendingInstall=yellow, etc.
// - Namespace filter reduces visible releases
// - Selected release tracking
// - Empty state when no releases
// - Loading state indicator
// - Error state displays error message
// - healthy_count / unhealthy_count badges
// - Action buttons present per release
// - Status label text

use baeus_helm::{HelmRelease, HelmReleaseStatus};
use baeus_ui::theme::Theme;
use baeus_ui::views::helm_releases::HelmReleasesViewComponent;
use baeus_ui::views::helm_releases::HelmReleasesViewState;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn sample_release(name: &str, namespace: &str, status: HelmReleaseStatus) -> HelmRelease {
    HelmRelease {
        name: name.to_string(),
        namespace: namespace.to_string(),
        chart_name: format!("{name}-chart"),
        chart_version: "1.0.0".to_string(),
        app_version: Some("1.0.0".to_string()),
        status,
        revision: 1,
        last_deployed: Utc::now(),
        values: json!({}),
        cluster_id: Uuid::new_v4(),
    }
}

fn sample_releases() -> Vec<HelmRelease> {
    vec![
        sample_release("nginx", "default", HelmReleaseStatus::Deployed),
        sample_release("redis", "default", HelmReleaseStatus::Deployed),
        sample_release("prometheus", "monitoring", HelmReleaseStatus::Deployed),
        sample_release("broken-app", "staging", HelmReleaseStatus::Failed),
        sample_release("upgrading-app", "staging", HelmReleaseStatus::PendingUpgrade),
    ]
}

fn make_component() -> HelmReleasesViewComponent {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());
    HelmReleasesViewComponent::new(state, Theme::dark())
}

fn make_empty_component() -> HelmReleasesViewComponent {
    let state = HelmReleasesViewState::default();
    HelmReleasesViewComponent::new(state, Theme::dark())
}

// ========================================================================
// Status badge colors
// ========================================================================

#[test]
fn test_deployed_status_is_success_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::Deployed);
    assert_eq!(color, Theme::dark().colors.success);
}

#[test]
fn test_failed_status_is_error_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::Failed);
    assert_eq!(color, Theme::dark().colors.error);
}

#[test]
fn test_pending_install_is_warning_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::PendingInstall);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_pending_upgrade_is_warning_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::PendingUpgrade);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_pending_rollback_is_warning_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::PendingRollback);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_uninstalling_is_muted_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::Uninstalling);
    assert_eq!(color, Theme::dark().colors.text_muted);
}

#[test]
fn test_superseded_is_muted_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::Superseded);
    assert_eq!(color, Theme::dark().colors.text_muted);
}

#[test]
fn test_unknown_is_secondary_color() {
    let comp = make_component();
    let color = comp.status_color(&HelmReleaseStatus::Unknown);
    assert_eq!(color, Theme::dark().colors.text_secondary);
}

#[test]
fn test_status_colors_with_light_theme() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());
    let comp = HelmReleasesViewComponent::new(state, Theme::light());
    assert_eq!(comp.status_color(&HelmReleaseStatus::Deployed), Theme::light().colors.success,);
    assert_eq!(comp.status_color(&HelmReleaseStatus::Failed), Theme::light().colors.error,);
}

// ========================================================================
// Status label text
// ========================================================================

#[test]
fn test_status_label_deployed() {
    assert_eq!(HelmReleasesViewComponent::status_label(&HelmReleaseStatus::Deployed), "Deployed",);
}

#[test]
fn test_status_label_failed() {
    assert_eq!(HelmReleasesViewComponent::status_label(&HelmReleaseStatus::Failed), "Failed",);
}

#[test]
fn test_status_label_pending_install() {
    assert_eq!(
        HelmReleasesViewComponent::status_label(&HelmReleaseStatus::PendingInstall),
        "Pending Install",
    );
}

#[test]
fn test_status_label_pending_upgrade() {
    assert_eq!(
        HelmReleasesViewComponent::status_label(&HelmReleaseStatus::PendingUpgrade),
        "Pending Upgrade",
    );
}

#[test]
fn test_status_label_pending_rollback() {
    assert_eq!(
        HelmReleasesViewComponent::status_label(&HelmReleaseStatus::PendingRollback),
        "Pending Rollback",
    );
}

#[test]
fn test_status_label_uninstalling() {
    assert_eq!(
        HelmReleasesViewComponent::status_label(&HelmReleaseStatus::Uninstalling),
        "Uninstalling",
    );
}

#[test]
fn test_status_label_superseded() {
    assert_eq!(
        HelmReleasesViewComponent::status_label(&HelmReleaseStatus::Superseded),
        "Superseded",
    );
}

#[test]
fn test_status_label_unknown() {
    assert_eq!(HelmReleasesViewComponent::status_label(&HelmReleaseStatus::Unknown), "Unknown",);
}

// ========================================================================
// Namespace filter reduces visible releases
// ========================================================================

#[test]
fn test_no_filter_shows_all() {
    let comp = make_component();
    assert_eq!(comp.state.filtered_releases().len(), 5);
}

#[test]
fn test_filter_by_default_namespace() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("default".to_string()));
    let filtered = comp.state.filtered_releases();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|r| r.namespace == "default"));
}

#[test]
fn test_filter_by_monitoring_namespace() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("monitoring".to_string()));
    let filtered = comp.state.filtered_releases();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "prometheus");
}

#[test]
fn test_filter_by_nonexistent_namespace() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("ghost".to_string()));
    assert!(comp.state.filtered_releases().is_empty());
}

#[test]
fn test_filter_clears_selection() {
    let mut comp = make_component();
    comp.state.select_release("nginx");
    assert!(comp.state.selected_release.is_some());
    comp.state.filter_by_namespace(Some("staging".to_string()));
    assert!(comp.state.selected_release.is_none());
}

// ========================================================================
// Selected release is highlighted
// ========================================================================

#[test]
fn test_no_selection_by_default() {
    let comp = make_component();
    assert!(comp.state.selected_release.is_none());
    assert!(comp.state.selected().is_none());
}

#[test]
fn test_select_release_updates_state() {
    let mut comp = make_component();
    comp.state.select_release("redis");
    assert_eq!(comp.state.selected_release.as_deref(), Some("redis"));
    let selected = comp.state.selected().unwrap();
    assert_eq!(selected.name, "redis");
}

#[test]
fn test_clear_selection() {
    let mut comp = make_component();
    comp.state.select_release("nginx");
    comp.state.clear_selection();
    assert!(comp.state.selected_release.is_none());
}

// ========================================================================
// Empty state shows "No Helm releases found"
// ========================================================================

#[test]
fn test_empty_state_no_releases() {
    let comp = make_empty_component();
    assert!(comp.state.releases.is_empty());
    assert!(comp.state.filtered_releases().is_empty());
    assert!(!comp.state.loading);
}

#[test]
fn test_empty_state_after_filtering() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("nonexistent".to_string()));
    assert!(comp.state.filtered_releases().is_empty());
}

// ========================================================================
// Loading state indicator
// ========================================================================

#[test]
fn test_loading_state_flag() {
    let mut state = HelmReleasesViewState::default();
    state.set_loading(true);
    let comp = HelmReleasesViewComponent::new(state, Theme::dark());
    assert!(comp.state.loading);
}

#[test]
fn test_loading_clears_error() {
    let mut state = HelmReleasesViewState::default();
    state.set_error("old error".to_string());
    state.set_loading(true);
    assert!(comp_from(state).state.error.is_none());
}

fn comp_from(state: HelmReleasesViewState) -> HelmReleasesViewComponent {
    HelmReleasesViewComponent::new(state, Theme::dark())
}

#[test]
fn test_loading_false_preserves_error() {
    let mut state = HelmReleasesViewState::default();
    state.set_error("kept".to_string());
    state.set_loading(false);
    assert!(state.error.is_some());
}

// ========================================================================
// Error state displays error message
// ========================================================================

#[test]
fn test_error_state() {
    let mut state = HelmReleasesViewState::default();
    state.set_error("cluster unreachable".to_string());
    let comp = HelmReleasesViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.error.as_deref(), Some("cluster unreachable"),);
    assert!(!comp.state.loading);
}

#[test]
fn test_error_cleared_on_set_releases() {
    let mut state = HelmReleasesViewState::default();
    state.set_error("connection lost".to_string());
    state.set_releases(sample_releases());
    assert!(state.error.is_none());
    assert!(!state.loading);
}

// ========================================================================
// healthy_count / unhealthy_count badges
// ========================================================================

#[test]
fn test_healthy_count_all_namespaces() {
    let comp = make_component();
    assert_eq!(comp.state.healthy_count(), 3);
}

#[test]
fn test_unhealthy_count_all_namespaces() {
    let comp = make_component();
    assert_eq!(comp.state.unhealthy_count(), 2);
}

#[test]
fn test_healthy_count_filtered_namespace() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("staging".to_string()));
    assert_eq!(comp.state.healthy_count(), 0);
    assert_eq!(comp.state.unhealthy_count(), 2);
}

#[test]
fn test_healthy_count_default_namespace() {
    let mut comp = make_component();
    comp.state.filter_by_namespace(Some("default".to_string()));
    assert_eq!(comp.state.healthy_count(), 2);
    assert_eq!(comp.state.unhealthy_count(), 0);
}

#[test]
fn test_healthy_count_empty_releases() {
    let comp = make_empty_component();
    assert_eq!(comp.state.healthy_count(), 0);
    assert_eq!(comp.state.unhealthy_count(), 0);
}

// ========================================================================
// Action buttons present per release
// ========================================================================

#[test]
fn test_releases_have_names_for_action_buttons() {
    let comp = make_component();
    let filtered = comp.state.filtered_releases();
    assert!(!filtered.is_empty());
    for release in &filtered {
        assert!(!release.name.is_empty());
    }
}

#[test]
fn test_all_statuses_have_labels() {
    let statuses = vec![
        HelmReleaseStatus::Deployed,
        HelmReleaseStatus::Failed,
        HelmReleaseStatus::PendingInstall,
        HelmReleaseStatus::PendingUpgrade,
        HelmReleaseStatus::PendingRollback,
        HelmReleaseStatus::Uninstalling,
        HelmReleaseStatus::Superseded,
        HelmReleaseStatus::Unknown,
    ];
    for s in statuses {
        let label = HelmReleasesViewComponent::status_label(&s);
        assert!(!label.is_empty());
    }
}

// ========================================================================
// Release table columns
// ========================================================================

#[test]
fn test_release_has_all_columns() {
    let comp = make_component();
    let releases = comp.state.filtered_releases();
    let r = releases[0];
    assert!(!r.name.is_empty());
    assert!(!r.namespace.is_empty());
    assert!(!r.chart_name.is_empty());
    assert!(!r.chart_version.is_empty());
    // status is always present (enum)
    // last_deployed is always present (DateTime)
}

#[test]
fn test_release_last_deployed_is_recent() {
    let comp = make_component();
    let releases = comp.state.filtered_releases();
    for r in &releases {
        let diff = Utc::now() - r.last_deployed;
        // Should have been created within the last minute
        assert!(diff.num_seconds() < 60);
    }
}

// ========================================================================
// Namespaces list
// ========================================================================

#[test]
fn test_namespaces_sorted_deduped() {
    let comp = make_component();
    let ns = comp.state.namespaces();
    assert_eq!(ns, vec!["default", "monitoring", "staging"]);
}

#[test]
fn test_namespaces_empty_when_no_releases() {
    let comp = make_empty_component();
    assert!(comp.state.namespaces().is_empty());
}

// ========================================================================
// Component construction
// ========================================================================

#[test]
fn test_component_new_dark_theme() {
    let state = HelmReleasesViewState::default();
    let comp = HelmReleasesViewComponent::new(state, Theme::dark());
    assert!(comp.state.releases.is_empty());
}

#[test]
fn test_component_new_light_theme() {
    let state = HelmReleasesViewState::default();
    let comp = HelmReleasesViewComponent::new(state, Theme::light());
    assert!(comp.state.releases.is_empty());
}

// ========================================================================
// Full workflow integration test
// ========================================================================

#[test]
fn test_full_releases_workflow() {
    let mut state = HelmReleasesViewState::default();

    // 1. Loading
    state.set_loading(true);
    assert!(state.loading);

    // 2. Receive releases
    state.set_releases(sample_releases());
    assert!(!state.loading);
    assert_eq!(state.releases.len(), 5);

    // 3. Create component
    let mut comp = HelmReleasesViewComponent::new(state, Theme::dark());

    // 4. Check healthy/unhealthy
    assert_eq!(comp.state.healthy_count(), 3);
    assert_eq!(comp.state.unhealthy_count(), 2);

    // 5. Filter to staging
    comp.state.filter_by_namespace(Some("staging".to_string()));
    assert_eq!(comp.state.filtered_releases().len(), 2);
    assert_eq!(comp.state.healthy_count(), 0);
    assert_eq!(comp.state.unhealthy_count(), 2);

    // 6. Select a release
    comp.state.select_release("broken-app");
    let selected = comp.state.selected().unwrap();
    assert_eq!(selected.name, "broken-app");
    assert_eq!(comp.status_color(&selected.status), Theme::dark().colors.error,);

    // 7. Clear filter
    comp.state.filter_by_namespace(None);
    assert_eq!(comp.state.filtered_releases().len(), 5);

    // 8. Error scenario
    comp.state.set_error("connection lost".to_string());
    assert!(comp.state.error.is_some());
    assert!(!comp.state.loading);
}
