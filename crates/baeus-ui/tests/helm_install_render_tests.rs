// T060: Render tests for HelmInstallView (state-level, no GPUI window needed).
//
// Verifies:
// - Search bar renders with query
// - Search results list renders chart entries
// - Selected chart shows detail panel (name, description, version, sources)
// - Values editor renders content
// - Values editor validation error shows
// - Install button disabled when !can_install()
// - Namespace field renders
// - Searching indicator
// - Installing progress indicator
// - Error state

use baeus_helm::charts::ChartEntry;
use baeus_ui::theme::Theme;
use baeus_ui::views::helm_install::{HelmInstallViewComponent, HelmInstallViewState};

fn sample_chart(name: &str, version: &str) -> ChartEntry {
    ChartEntry {
        name: name.to_string(),
        version: version.to_string(),
        app_version: Some("1.0.0".to_string()),
        description: Some(format!("{name} chart")),
        home: Some(format!("https://{name}.example.com")),
        sources: vec![format!("https://github.com/example/{name}")],
        urls: vec![format!("https://charts.example.com/{name}-{version}.tgz")],
    }
}

fn sample_chart_minimal(name: &str, version: &str) -> ChartEntry {
    ChartEntry {
        name: name.to_string(),
        version: version.to_string(),
        app_version: None,
        description: None,
        home: None,
        sources: vec![],
        urls: vec![],
    }
}

fn sample_search_results() -> Vec<ChartEntry> {
    vec![
        sample_chart("nginx", "15.4.0"),
        sample_chart("nginx", "15.3.0"),
        sample_chart("nginx-ingress", "4.8.3"),
    ]
}

fn make_component() -> HelmInstallViewComponent {
    let state = HelmInstallViewState::default();
    HelmInstallViewComponent::new(state, Theme::dark())
}

fn make_component_with_results() -> HelmInstallViewComponent {
    let mut state = HelmInstallViewState::default();
    state.search_query = "nginx".to_string();
    state.set_search_results(sample_search_results());
    HelmInstallViewComponent::new(state, Theme::dark())
}

fn make_component_with_selection() -> HelmInstallViewComponent {
    let mut state = HelmInstallViewState::default();
    state.search_query = "nginx".to_string();
    state.set_search_results(sample_search_results());
    state.select_chart(sample_chart("nginx", "15.4.0"));
    HelmInstallViewComponent::new(state, Theme::dark())
}

// ========================================================================
// Search bar renders with query
// ========================================================================

#[test]
fn test_empty_search_query() {
    let comp = make_component();
    assert!(comp.state.search_query.is_empty());
}

#[test]
fn test_search_query_set() {
    let mut comp = make_component();
    comp.state.search_query = "redis".to_string();
    assert_eq!(comp.state.search_query, "redis");
}

#[test]
fn test_search_query_after_results() {
    let comp = make_component_with_results();
    assert_eq!(comp.state.search_query, "nginx");
}

// ========================================================================
// Search results list renders chart entries
// ========================================================================

#[test]
fn test_no_results_initially() {
    let comp = make_component();
    assert!(comp.state.search_results.is_empty());
}

#[test]
fn test_results_populated() {
    let comp = make_component_with_results();
    assert_eq!(comp.state.search_results.len(), 3);
}

#[test]
fn test_results_have_names() {
    let comp = make_component_with_results();
    assert_eq!(comp.state.search_results[0].name, "nginx");
    assert_eq!(comp.state.search_results[2].name, "nginx-ingress");
}

#[test]
fn test_results_have_versions() {
    let comp = make_component_with_results();
    assert_eq!(comp.state.search_results[0].version, "15.4.0");
    assert_eq!(comp.state.search_results[1].version, "15.3.0");
}

#[test]
fn test_results_have_descriptions() {
    let comp = make_component_with_results();
    assert_eq!(comp.state.search_results[0].description.as_deref(), Some("nginx chart"),);
}

// ========================================================================
// Selected chart shows detail panel
// ========================================================================

#[test]
fn test_no_selected_chart_initially() {
    let comp = make_component();
    assert!(comp.state.selected_chart.is_none());
    assert!(comp.state.selected_chart_name().is_none());
}

#[test]
fn test_selected_chart_name() {
    let comp = make_component_with_selection();
    assert_eq!(comp.state.selected_chart_name(), Some("nginx"));
}

#[test]
fn test_selected_chart_has_version() {
    let comp = make_component_with_selection();
    assert_eq!(comp.state.selected_version.as_deref(), Some("15.4.0"));
}

#[test]
fn test_selected_chart_description() {
    let comp = make_component_with_selection();
    let chart = comp.state.selected_chart.as_ref().unwrap();
    assert_eq!(chart.description.as_deref(), Some("nginx chart"));
}

#[test]
fn test_selected_chart_home() {
    let comp = make_component_with_selection();
    let chart = comp.state.selected_chart.as_ref().unwrap();
    assert_eq!(chart.home.as_deref(), Some("https://nginx.example.com"));
}

#[test]
fn test_selected_chart_sources() {
    let comp = make_component_with_selection();
    let chart = comp.state.selected_chart.as_ref().unwrap();
    assert_eq!(chart.sources.len(), 1);
    assert_eq!(chart.sources[0], "https://github.com/example/nginx");
}

#[test]
fn test_selected_chart_version_override() {
    let mut comp = make_component_with_selection();
    comp.state.select_version("15.3.0");
    assert_eq!(comp.state.selected_version.as_deref(), Some("15.3.0"));
    // Chart reference unchanged
    assert_eq!(comp.state.selected_chart_name(), Some("nginx"));
}

#[test]
fn test_selected_chart_minimal_no_description() {
    let mut state = HelmInstallViewState::default();
    state.select_chart(sample_chart_minimal("bare", "0.1.0"));
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    let chart = comp.state.selected_chart.as_ref().unwrap();
    assert!(chart.description.is_none());
    assert!(chart.home.is_none());
    assert!(chart.sources.is_empty());
}

// ========================================================================
// Values editor renders content
// ========================================================================

#[test]
fn test_values_editor_default_empty() {
    let comp = make_component();
    assert!(comp.state.values_editor.content.is_empty());
    assert!(comp.state.values_editor.is_valid);
}

#[test]
fn test_values_editor_with_content() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("replicaCount: 3\nimage:\n  tag: latest");
    assert_eq!(comp.state.values_editor.content, "replicaCount: 3\nimage:\n  tag: latest",);
    assert!(comp.state.values_editor.is_valid);
}

#[test]
fn test_values_editor_validation_valid() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("service:\n  type: LoadBalancer");
    assert!(comp.state.values_editor.is_valid);
    assert!(comp.state.values_editor.validation_error.is_none());
    assert_eq!(comp.validation_label(), "Valid YAML");
}

// ========================================================================
// Values editor validation error shows
// ========================================================================

#[test]
fn test_values_editor_validation_error() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("key: [invalid");
    assert!(!comp.state.values_editor.is_valid);
    assert!(comp.state.values_editor.validation_error.is_some());
}

#[test]
fn test_values_editor_validation_label_on_error() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("key: [invalid");
    let label = comp.validation_label();
    // Should show the actual error message
    assert!(!label.is_empty());
    assert_ne!(label, "Valid YAML");
}

#[test]
fn test_values_editor_error_clears_on_fix() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("key: [invalid");
    assert!(!comp.state.values_editor.is_valid);

    comp.state.values_editor.set_content("key: fixed");
    assert!(comp.state.values_editor.is_valid);
    assert!(comp.state.values_editor.validation_error.is_none());
}

// ========================================================================
// Install button disabled when !can_install()
// ========================================================================

#[test]
fn test_install_button_disabled_no_chart() {
    let comp = make_component();
    assert!(!comp.install_button_enabled());
    assert!(!comp.state.can_install());
}

#[test]
fn test_install_button_enabled_with_selection() {
    let comp = make_component_with_selection();
    assert!(comp.install_button_enabled());
    assert!(comp.state.can_install());
}

#[test]
fn test_install_button_disabled_empty_namespace() {
    let mut comp = make_component_with_selection();
    comp.state.set_namespace("");
    assert!(!comp.install_button_enabled());
}

#[test]
fn test_install_button_disabled_invalid_values() {
    let mut comp = make_component_with_selection();
    comp.state.values_editor.set_content("key: [invalid");
    assert!(!comp.install_button_enabled());
}

#[test]
fn test_install_button_disabled_while_installing() {
    let mut comp = make_component_with_selection();
    comp.state.installing = true;
    assert!(!comp.install_button_enabled());
}

#[test]
fn test_install_button_disabled_no_version() {
    let mut comp = make_component_with_selection();
    comp.state.selected_version = None;
    assert!(!comp.install_button_enabled());
}

#[test]
fn test_install_button_label_default() {
    let comp = make_component();
    assert_eq!(comp.install_button_label(), "Install");
}

#[test]
fn test_install_button_label_installing() {
    let mut comp = make_component_with_selection();
    comp.state.installing = true;
    assert_eq!(comp.install_button_label(), "Installing...");
}

// ========================================================================
// Namespace field renders
// ========================================================================

#[test]
fn test_default_namespace() {
    let comp = make_component();
    assert_eq!(comp.state.namespace, "default");
}

#[test]
fn test_set_namespace() {
    let mut comp = make_component();
    comp.state.set_namespace("production");
    assert_eq!(comp.state.namespace, "production");
}

#[test]
fn test_empty_namespace() {
    let mut comp = make_component();
    comp.state.set_namespace("");
    assert_eq!(comp.state.namespace, "");
}

// ========================================================================
// Searching indicator
// ========================================================================

#[test]
fn test_not_searching_initially() {
    let comp = make_component();
    assert!(!comp.state.searching);
}

#[test]
fn test_searching_flag() {
    let mut state = HelmInstallViewState::default();
    state.searching = true;
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert!(comp.state.searching);
}

#[test]
fn test_searching_cleared_on_results() {
    let mut state = HelmInstallViewState::default();
    state.searching = true;
    state.set_search_results(sample_search_results());
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert!(!comp.state.searching);
}

// ========================================================================
// Installing progress indicator
// ========================================================================

#[test]
fn test_not_installing_initially() {
    let comp = make_component();
    assert!(!comp.state.installing);
}

#[test]
fn test_installing_flag() {
    let mut comp = make_component_with_selection();
    comp.state.installing = true;
    assert!(comp.state.installing);
    assert!(!comp.state.can_install());
}

#[test]
fn test_installing_label() {
    let mut comp = make_component_with_selection();
    comp.state.installing = true;
    assert_eq!(comp.install_button_label(), "Installing...");
}

// ========================================================================
// Error state
// ========================================================================

#[test]
fn test_no_error_initially() {
    let comp = make_component();
    assert!(comp.state.error.is_none());
}

#[test]
fn test_error_set() {
    let mut state = HelmInstallViewState::default();
    state.error = Some("search failed".to_string());
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.error.as_deref(), Some("search failed"));
}

#[test]
fn test_error_cleared_on_search_results() {
    let mut state = HelmInstallViewState::default();
    state.error = Some("old error".to_string());
    state.set_search_results(sample_search_results());
    assert!(state.error.is_none());
}

// ========================================================================
// Component construction
// ========================================================================

#[test]
fn test_component_new_dark() {
    let state = HelmInstallViewState::default();
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert!(comp.state.search_results.is_empty());
}

#[test]
fn test_component_new_light() {
    let state = HelmInstallViewState::default();
    let comp = HelmInstallViewComponent::new(state, Theme::light());
    assert!(comp.state.search_results.is_empty());
}

// ========================================================================
// Full workflow integration test
// ========================================================================

#[test]
fn test_full_install_workflow() {
    let mut state = HelmInstallViewState::default();

    // 1. Start search
    state.search_query = "nginx".to_string();
    state.searching = true;
    let comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert!(comp.state.searching);
    assert!(!comp.install_button_enabled());

    // 2. Get results
    let mut state = comp.state;
    state.set_search_results(sample_search_results());
    assert!(!state.searching);
    assert_eq!(state.search_results.len(), 3);

    // 3. Select chart
    state.select_chart(sample_chart("nginx", "15.4.0"));
    let mut comp = HelmInstallViewComponent::new(state, Theme::dark());
    assert!(comp.install_button_enabled());
    assert_eq!(comp.state.selected_chart_name(), Some("nginx"));
    assert_eq!(comp.state.selected_version.as_deref(), Some("15.4.0"));

    // 4. Edit values
    comp.state.values_editor.set_content("replicaCount: 3\nimage:\n  tag: stable");
    assert!(comp.state.values_editor.is_valid);
    assert!(comp.install_button_enabled());

    // 5. Set namespace
    comp.state.set_namespace("production");
    assert!(comp.install_button_enabled());

    // 6. Start install
    comp.state.installing = true;
    assert!(!comp.install_button_enabled());
    assert_eq!(comp.install_button_label(), "Installing...");

    // 7. Install complete
    comp.state.clear();
    assert!(!comp.install_button_enabled());
    assert_eq!(comp.state.namespace, "default");
    assert!(comp.state.search_results.is_empty());
    assert!(comp.state.selected_chart.is_none());
}

#[test]
fn test_values_validation_workflow() {
    let mut comp = make_component_with_selection();
    assert!(comp.install_button_enabled());
    assert_eq!(comp.validation_label(), "Valid YAML");

    // Set invalid YAML
    comp.state.values_editor.set_content("key: [broken");
    assert!(!comp.install_button_enabled());
    assert_ne!(comp.validation_label(), "Valid YAML");

    // Fix YAML
    comp.state.values_editor.set_content("key: value\nother: 42");
    assert!(comp.install_button_enabled());
    assert_eq!(comp.validation_label(), "Valid YAML");

    // Clear editor
    comp.state.values_editor.clear();
    assert!(comp.install_button_enabled());
    assert_eq!(comp.validation_label(), "Valid YAML");
}
