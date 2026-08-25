use baeus_plugins::{Plugin, PluginManifest, PluginPermission, PluginState};
use baeus_ui::views::plugin_manager::PluginManagerState;

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

// --- T083: Plugin operations tests ---

#[test]
fn test_full_lifecycle_install_to_uninstall() {
    let mut state = PluginManagerState::default();

    // Start with empty state
    assert_eq!(state.plugins.len(), 0);

    // Install a plugin
    let plugin = sample_plugin("io.example.test", "Test Plugin");
    state.install(plugin).unwrap();
    assert_eq!(state.plugins.len(), 1);
    assert_eq!(state.plugins[0].state, PluginState::Installed);

    // Enable it
    state.enable("io.example.test").unwrap();
    assert_eq!(state.plugins[0].state, PluginState::Enabled);
    assert_eq!(state.enabled_count(), 1);

    // Disable it
    state.disable("io.example.test").unwrap();
    assert_eq!(state.plugins[0].state, PluginState::Disabled);
    assert_eq!(state.enabled_count(), 0);

    // Enable again
    state.enable("io.example.test").unwrap();
    assert_eq!(state.plugins[0].state, PluginState::Enabled);

    // Uninstall it
    state.uninstall("io.example.test").unwrap();
    assert_eq!(state.plugins.len(), 0);
}

#[test]
fn test_install_multiple_plugins() {
    let mut state = PluginManagerState::default();

    state.install(sample_plugin("io.example.p1", "Plugin 1")).unwrap();
    state.install(sample_plugin("io.example.p2", "Plugin 2")).unwrap();
    state.install(sample_plugin("io.example.p3", "Plugin 3")).unwrap();

    assert_eq!(state.plugins.len(), 3);
}

#[test]
fn test_install_duplicate_returns_error() {
    let mut state = PluginManagerState::default();

    state.install(sample_plugin("io.example.dup", "Dup")).unwrap();
    let result = state.install(sample_plugin("io.example.dup", "Dup Again"));

    assert!(result.is_err());
    assert_eq!(state.plugins.len(), 1);
}

#[test]
fn test_enable_nonexistent_plugin_returns_error() {
    let mut state = PluginManagerState::default();
    let result = state.enable("io.example.missing");
    assert!(result.is_err());
}

#[test]
fn test_disable_nonexistent_plugin_returns_error() {
    let mut state = PluginManagerState::default();
    let result = state.disable("io.example.missing");
    assert!(result.is_err());
}

#[test]
fn test_uninstall_nonexistent_plugin_returns_error() {
    let mut state = PluginManagerState::default();
    let result = state.uninstall("io.example.missing");
    assert!(result.is_err());
}

#[test]
fn test_enable_plugin_in_error_state_fails() {
    let mut state = PluginManagerState::default();
    let mut plugin = sample_plugin("io.example.broken", "Broken");
    plugin.set_error("Load failure".to_string());
    state.install(plugin).unwrap();

    let result = state.enable("io.example.broken");
    assert!(result.is_err());
    assert!(state.plugins[0].is_error());
}

#[test]
fn test_disable_enabled_plugin() {
    let mut state = PluginManagerState::default();
    let mut plugin = sample_plugin("io.example.active", "Active");
    plugin.enable().unwrap();
    state.install(plugin).unwrap();

    assert!(state.plugins[0].is_active());

    state.disable("io.example.active").unwrap();
    assert_eq!(state.plugins[0].state, PluginState::Disabled);
}

#[test]
fn test_loading_state_transitions() {
    let mut state = PluginManagerState::default();

    // Begin loading
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());

    // Load complete
    state.set_plugins(vec![sample_plugin("io.example.test", "Test")]);
    assert!(!state.loading);
    assert_eq!(state.plugins.len(), 1);
}

#[test]
fn test_loading_with_error() {
    let mut state = PluginManagerState::default();

    // Begin loading
    state.set_loading(true);
    assert!(state.loading);

    // Load fails
    state.set_error("Failed to scan plugin directory".to_string());
    assert!(!state.loading);
    assert!(state.error.is_some());
    assert_eq!(state.plugins.len(), 0);
}

#[test]
fn test_error_cleared_on_successful_load() {
    let mut state = PluginManagerState::default();

    // Set error
    state.set_error("Previous error".to_string());
    assert!(state.error.is_some());

    // Successful load clears error
    state.set_plugins(vec![sample_plugin("io.example.test", "Test")]);
    assert!(state.error.is_none());
    assert_eq!(state.plugins.len(), 1);
}

#[test]
fn test_multiple_enable_disable_cycles() {
    let mut state = PluginManagerState::default();
    state.install(sample_plugin("io.example.toggle", "Toggle")).unwrap();

    for _ in 0..5 {
        state.enable("io.example.toggle").unwrap();
        assert!(state.plugins[0].is_active());

        state.disable("io.example.toggle").unwrap();
        assert_eq!(state.plugins[0].state, PluginState::Disabled);
    }
}

#[test]
fn test_uninstall_selected_plugin_clears_selection() {
    let mut state = PluginManagerState::default();
    state.install(sample_plugin("io.example.selected", "Selected")).unwrap();
    state.select_plugin("io.example.selected");
    assert!(state.selected().is_some());

    state.uninstall("io.example.selected").unwrap();
    assert!(state.selected().is_none());
}

#[test]
fn test_uninstall_other_plugin_preserves_selection() {
    let mut state = PluginManagerState::default();
    state.install(sample_plugin("io.example.p1", "P1")).unwrap();
    state.install(sample_plugin("io.example.p2", "P2")).unwrap();
    state.select_plugin("io.example.p1");

    state.uninstall("io.example.p2").unwrap();
    assert_eq!(state.selected().unwrap().id, "io.example.p1");
}

#[test]
fn test_enable_already_enabled_is_noop() {
    let mut state = PluginManagerState::default();
    let mut plugin = sample_plugin("io.example.active", "Active");
    plugin.enable().unwrap();
    state.install(plugin).unwrap();

    assert!(state.plugins[0].is_active());
    state.enable("io.example.active").unwrap();
    assert!(state.plugins[0].is_active());
}
