// T081: Render tests for PluginManagerView (state-level, no GPUI window needed).
//
// Verifies:
// - Plugin cards render with name, version, description, author
// - Enable/disable toggle shows correct state per PluginState
// - Uninstall button present
// - Permission badges render (ReadResources, WriteResources, NetworkAccess, etc.)
// - Plugin count badges (total, enabled, error)
// - Selected plugin shows detail panel
// - Empty state when no plugins installed
// - Loading state display
// - Error state display
// - Plugin state transitions: Installed -> Enabled -> Disabled
// - Error state rendering with error message
// - Full workflow: load plugins -> select -> enable -> disable -> uninstall

use baeus_plugins::{Plugin, PluginManifest, PluginPermission, PluginState};
use baeus_ui::theme::Theme;
use baeus_ui::views::plugin_manager::{PluginManagerState, PluginManagerViewComponent};

fn sample_manifest(id: &str, name: &str) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        description: format!("A plugin for {name}"),
        author: "Test Author".to_string(),
        min_app_version: "0.1.0".to_string(),
        permissions: vec![PluginPermission::ReadResources],
    }
}

fn sample_plugin(id: &str, name: &str) -> Plugin {
    Plugin::new(sample_manifest(id, name), format!("/plugins/{id}.dylib"))
}

fn sample_plugin_with_permissions(
    id: &str,
    name: &str,
    permissions: Vec<PluginPermission>,
) -> Plugin {
    let mut manifest = sample_manifest(id, name);
    manifest.permissions = permissions;
    Plugin::new(manifest, format!("/plugins/{id}.dylib"))
}

fn sample_plugins() -> Vec<Plugin> {
    let mut p1 = sample_plugin("io.example.metrics", "Metrics Dashboard");
    p1.enable().unwrap();
    let mut p2 = sample_plugin("io.example.gitops", "GitOps Integration");
    p2.enable().unwrap();
    let p3 = sample_plugin("io.example.costview", "Cost Viewer");
    // p3 stays in Installed state

    vec![p1, p2, p3]
}

fn make_component() -> PluginManagerViewComponent {
    let mut state = PluginManagerState::default();
    state.set_plugins(sample_plugins());
    PluginManagerViewComponent::new(state, Theme::dark())
}

fn make_empty_component() -> PluginManagerViewComponent {
    let state = PluginManagerState::default();
    PluginManagerViewComponent::new(state, Theme::dark())
}

// ========================================================================
// State color mapping
// ========================================================================

#[test]
fn test_enabled_state_is_success_color() {
    let comp = make_component();
    let color = comp.state_color(&PluginState::Enabled);
    assert_eq!(color, Theme::dark().colors.success);
}

#[test]
fn test_disabled_state_is_muted_color() {
    let comp = make_component();
    let color = comp.state_color(&PluginState::Disabled);
    assert_eq!(color, Theme::dark().colors.text_muted);
}

#[test]
fn test_error_state_is_error_color() {
    let comp = make_component();
    let color = comp.state_color(&PluginState::Error("error".to_string()));
    assert_eq!(color, Theme::dark().colors.error);
}

#[test]
fn test_installed_state_is_accent_color() {
    let comp = make_component();
    let color = comp.state_color(&PluginState::Installed);
    assert_eq!(color, Theme::dark().colors.accent);
}

// ========================================================================
// State labels
// ========================================================================

#[test]
fn test_state_label_enabled() {
    assert_eq!(PluginManagerViewComponent::state_label(&PluginState::Enabled), "Enabled");
}

#[test]
fn test_state_label_disabled() {
    assert_eq!(PluginManagerViewComponent::state_label(&PluginState::Disabled), "Disabled");
}

#[test]
fn test_state_label_error() {
    assert_eq!(
        PluginManagerViewComponent::state_label(&PluginState::Error("err".to_string())),
        "Error"
    );
}

#[test]
fn test_state_label_installed() {
    assert_eq!(PluginManagerViewComponent::state_label(&PluginState::Installed), "Installed");
}

// ========================================================================
// Permission labels
// ========================================================================

#[test]
fn test_permission_label_read_resources() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::ReadResources),
        "Read Resources"
    );
}

#[test]
fn test_permission_label_write_resources() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::WriteResources),
        "Write Resources"
    );
}

#[test]
fn test_permission_label_register_views() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::RegisterViews),
        "Register Views"
    );
}

#[test]
fn test_permission_label_register_actions() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::RegisterActions),
        "Register Actions"
    );
}

#[test]
fn test_permission_label_register_sidebar() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::RegisterSidebar),
        "Register Sidebar"
    );
}

#[test]
fn test_permission_label_network_access() {
    assert_eq!(
        PluginManagerViewComponent::permission_label(&PluginPermission::NetworkAccess),
        "Network Access"
    );
}

// ========================================================================
// Component state
// ========================================================================

#[test]
fn test_component_with_empty_state() {
    let comp = make_empty_component();
    assert!(comp.state.plugins.is_empty());
    assert!(!comp.state.loading);
    assert!(comp.state.error.is_none());
}

#[test]
fn test_component_with_plugins() {
    let comp = make_component();
    assert_eq!(comp.state.plugins.len(), 3);
    assert_eq!(comp.state.enabled_count(), 2);
    assert_eq!(comp.state.error_count(), 0);
}

#[test]
fn test_component_loading_state() {
    let mut state = PluginManagerState::default();
    state.set_loading(true);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.loading);
}

#[test]
fn test_component_error_state() {
    let mut state = PluginManagerState::default();
    state.set_error("Failed to load plugins".to_string());
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.error.is_some());
}

// ========================================================================
// Plugin cards
// ========================================================================

#[test]
fn test_plugin_card_fields() {
    let comp = make_component();
    let plugin = &comp.state.plugins[0];
    assert_eq!(plugin.manifest.name, "Metrics Dashboard");
    assert_eq!(plugin.manifest.version, "1.0.0");
    assert_eq!(plugin.manifest.author, "Test Author");
    assert_eq!(plugin.manifest.description, "A plugin for Metrics Dashboard");
}

#[test]
fn test_enabled_plugin_state() {
    let comp = make_component();
    assert!(comp.state.plugins[0].is_active());
    assert!(comp.state.plugins[1].is_active());
}

#[test]
fn test_installed_plugin_state() {
    let comp = make_component();
    assert_eq!(comp.state.plugins[2].state, PluginState::Installed);
}

#[test]
fn test_disabled_plugin_state() {
    let mut state = PluginManagerState::default();
    let mut plugins = sample_plugins();
    plugins[0].disable();
    state.set_plugins(plugins);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.plugins[0].state, PluginState::Disabled);
}

#[test]
fn test_error_plugin_state() {
    let mut state = PluginManagerState::default();
    let mut plugins = sample_plugins();
    plugins[0].set_error("Failed to load library".to_string());
    state.set_plugins(plugins);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].is_error());
}

// ========================================================================
// Plugin counts
// ========================================================================

#[test]
fn test_plugin_count_badges() {
    let comp = make_component();
    assert_eq!(comp.state.plugins.len(), 3);
    assert_eq!(comp.state.enabled_count(), 2);
    assert_eq!(comp.state.error_count(), 0);
}

#[test]
fn test_plugin_count_with_errors() {
    let mut state = PluginManagerState::default();
    let mut plugins = sample_plugins();
    plugins[2].set_error("Load failed".to_string());
    state.set_plugins(plugins);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.plugins.len(), 3);
    assert_eq!(comp.state.enabled_count(), 2);
    assert_eq!(comp.state.error_count(), 1);
}

// ========================================================================
// Permissions
// ========================================================================

#[test]
fn test_plugin_with_multiple_permissions() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.multi",
        "Multi Permission",
        vec![
            PluginPermission::ReadResources,
            PluginPermission::WriteResources,
            PluginPermission::NetworkAccess,
        ],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.plugins[0].manifest.permissions.len(), 3);
}

#[test]
fn test_plugin_read_resources_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.reader",
        "Reader",
        vec![PluginPermission::ReadResources],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.has_permission(&PluginPermission::ReadResources));
}

#[test]
fn test_plugin_write_resources_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.writer",
        "Writer",
        vec![PluginPermission::WriteResources],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.has_permission(&PluginPermission::WriteResources));
}

#[test]
fn test_plugin_network_access_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.network",
        "Network",
        vec![PluginPermission::NetworkAccess],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.requests_network_access());
}

#[test]
fn test_plugin_register_views_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.views",
        "Views",
        vec![PluginPermission::RegisterViews],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.has_permission(&PluginPermission::RegisterViews));
}

#[test]
fn test_plugin_register_actions_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.actions",
        "Actions",
        vec![PluginPermission::RegisterActions],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.has_permission(&PluginPermission::RegisterActions));
}

#[test]
fn test_plugin_register_sidebar_permission() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.sidebar",
        "Sidebar",
        vec![PluginPermission::RegisterSidebar],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.plugins[0].manifest.has_permission(&PluginPermission::RegisterSidebar));
}

#[test]
fn test_plugin_all_permissions() {
    let mut state = PluginManagerState::default();
    let plugin = sample_plugin_with_permissions(
        "io.example.all",
        "All Permissions",
        vec![
            PluginPermission::ReadResources,
            PluginPermission::WriteResources,
            PluginPermission::RegisterViews,
            PluginPermission::RegisterActions,
            PluginPermission::RegisterSidebar,
            PluginPermission::NetworkAccess,
        ],
    );
    state.set_plugins(vec![plugin]);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.plugins[0].manifest.permissions.len(), 6);
}

// ========================================================================
// Selection
// ========================================================================

#[test]
fn test_selected_plugin() {
    let mut state = PluginManagerState::default();
    state.set_plugins(sample_plugins());
    state.select_plugin("io.example.metrics");
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert!(comp.state.selected().is_some());
    assert_eq!(comp.state.selected().unwrap().manifest.name, "Metrics Dashboard");
}

#[test]
fn test_no_selection() {
    let comp = make_component();
    assert!(comp.state.selected().is_none());
}

#[test]
fn test_detail_panel_fields() {
    let mut state = PluginManagerState::default();
    state.set_plugins(sample_plugins());
    state.select_plugin("io.example.metrics");
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    let selected = comp.state.selected().unwrap();
    assert_eq!(selected.manifest.description, "A plugin for Metrics Dashboard");
    assert!(selected.manifest.permissions.contains(&PluginPermission::ReadResources));
    assert!(!selected.library_path.is_empty());
}

// ========================================================================
// Themes
// ========================================================================

#[test]
fn test_component_with_light_theme() {
    let mut state = PluginManagerState::default();
    state.set_plugins(sample_plugins());
    let comp = PluginManagerViewComponent::new(state, Theme::light());
    assert_eq!(comp.theme, Theme::light());
}

#[test]
fn test_component_with_dark_theme() {
    let mut state = PluginManagerState::default();
    state.set_plugins(sample_plugins());
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.theme, Theme::dark());
}

// ========================================================================
// Multiple plugins different states
// ========================================================================

#[test]
fn test_multiple_plugins_different_states() {
    let mut state = PluginManagerState::default();
    let mut plugins = vec![
        sample_plugin("io.example.p1", "Plugin 1"),
        sample_plugin("io.example.p2", "Plugin 2"),
        sample_plugin("io.example.p3", "Plugin 3"),
        sample_plugin("io.example.p4", "Plugin 4"),
    ];
    plugins[0].enable().unwrap();
    plugins[1].state = PluginState::Installed;
    plugins[2].disable();
    plugins[3].set_error("Load error".to_string());

    state.set_plugins(plugins);
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.enabled_count(), 1);
    assert_eq!(comp.state.error_count(), 1);
}

// ========================================================================
// Full workflow
// ========================================================================

#[test]
fn test_full_workflow() {
    let mut state = PluginManagerState::default();

    // Start loading
    state.set_loading(true);
    let comp = PluginManagerViewComponent::new(state.clone(), Theme::dark());
    assert!(comp.state.loading);

    // Load plugins
    state.set_plugins(sample_plugins());
    let comp = PluginManagerViewComponent::new(state.clone(), Theme::dark());
    assert!(!comp.state.loading);
    assert_eq!(comp.state.plugins.len(), 3);

    // Select a plugin
    state.select_plugin("io.example.metrics");
    let comp = PluginManagerViewComponent::new(state.clone(), Theme::dark());
    assert!(comp.state.selected().is_some());

    // Disable the selected plugin
    state.disable("io.example.metrics").unwrap();
    let comp = PluginManagerViewComponent::new(state.clone(), Theme::dark());
    assert_eq!(comp.state.plugins[0].state, PluginState::Disabled);

    // Enable it again
    state.enable("io.example.metrics").unwrap();
    let comp = PluginManagerViewComponent::new(state.clone(), Theme::dark());
    assert!(comp.state.plugins[0].is_active());

    // Uninstall it
    state.uninstall("io.example.metrics").unwrap();
    let comp = PluginManagerViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.plugins.len(), 2);
    assert!(comp.state.selected().is_none());
}
