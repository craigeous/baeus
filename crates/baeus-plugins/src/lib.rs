pub mod api;
pub mod loader;
pub mod registry;
pub mod sandbox;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Plugin permission types that a plugin can request.
///
/// Each permission grants access to a specific capability.
/// Plugins must declare all required permissions in their manifest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    /// Can read K8s resources from connected clusters
    ReadResources,
    /// Can create/update/delete K8s resources
    WriteResources,
    /// Can add custom views/tabs to the UI
    RegisterViews,
    /// Can add actions to resource context menus
    RegisterActions,
    /// Can add items to the navigation sidebar
    RegisterSidebar,
    /// Can make network requests outside cluster endpoints
    NetworkAccess,
}

/// Plugin manifest containing metadata about a plugin.
///
/// Every plugin must provide a manifest that describes its identity,
/// version requirements, and requested permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique identifier (reverse-domain, e.g., "io.example.my-plugin")
    pub id: String,
    /// Display name
    pub name: String,
    /// SemVer version string
    pub version: String,
    /// Short description of what the plugin does
    pub description: String,
    /// Author name or organization
    pub author: String,
    /// Minimum Baeus version required (SemVer)
    pub min_app_version: String,
    /// Requested permissions
    pub permissions: Vec<PluginPermission>,
}

impl PluginManifest {
    /// Returns true if the manifest requests the given permission.
    pub fn has_permission(&self, permission: &PluginPermission) -> bool {
        self.permissions.contains(permission)
    }

    /// Returns true if the plugin requests network access (flagged as a warning).
    pub fn requests_network_access(&self) -> bool {
        self.has_permission(&PluginPermission::NetworkAccess)
    }

    /// Returns true if the plugin requests write access to K8s resources.
    pub fn requests_write_access(&self) -> bool {
        self.has_permission(&PluginPermission::WriteResources)
    }
}

/// Runtime state of a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin is installed but not yet activated
    Installed,
    /// Plugin is enabled and running
    Enabled,
    /// Plugin is disabled by the user
    Disabled,
    /// Plugin encountered an error
    Error(String),
}

impl PluginState {
    /// Returns true if the plugin is currently active and running.
    pub fn is_active(&self) -> bool {
        *self == PluginState::Enabled
    }

    /// Returns true if the plugin is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self, PluginState::Error(_))
    }
}

/// Errors that can occur during plugin operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// Failed to load the plugin library
    #[error("failed to load plugin: {0}")]
    LoadFailed(String),

    /// Plugin version is incompatible with the application
    #[error("version mismatch: {0}")]
    VersionMismatch(String),

    /// Plugin attempted an action it does not have permission for
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// The requested plugin was not found
    #[error("plugin not found: {0}")]
    NotFound(String),

    /// The requested resource was not found
    #[error("resource not found: {0}")]
    ResourceNotFound(String),

    /// Error communicating with the Kubernetes cluster
    #[error("cluster error: {0}")]
    ClusterError(String),

    /// Internal plugin error
    #[error("internal error: {0}")]
    InternalError(String),

    /// Plugin is already installed
    #[error("already installed: {0}")]
    AlreadyInstalled(String),

    /// Invalid plugin manifest
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}

/// A third-party extension package.
///
/// Represents a plugin that extends Baeus with custom views,
/// actions, and integrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    /// Unique plugin identifier
    pub id: String,
    /// Plugin manifest with metadata
    pub manifest: PluginManifest,
    /// Current runtime state
    pub state: PluginState,
    /// When the plugin was installed
    pub installed_at: DateTime<Utc>,
    /// Plugin-specific configuration
    pub config: Value,
    /// Path to the .dylib file
    pub library_path: String,
}

impl Plugin {
    /// Create a new plugin from a manifest and library path.
    pub fn new(manifest: PluginManifest, library_path: String) -> Self {
        Self {
            id: manifest.id.clone(),
            manifest,
            state: PluginState::Installed,
            installed_at: Utc::now(),
            config: Value::Object(serde_json::Map::new()),
            library_path,
        }
    }

    /// Returns true if the plugin is currently active and running.
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Returns true if the plugin is in an error state.
    pub fn is_error(&self) -> bool {
        self.state.is_error()
    }

    /// Enable the plugin.
    pub fn enable(&mut self) -> Result<(), PluginError> {
        match &self.state {
            PluginState::Installed | PluginState::Disabled => {
                self.state = PluginState::Enabled;
                Ok(())
            }
            PluginState::Enabled => Ok(()),
            PluginState::Error(msg) => {
                Err(PluginError::LoadFailed(format!("cannot enable plugin in error state: {msg}")))
            }
        }
    }

    /// Disable the plugin.
    pub fn disable(&mut self) {
        if self.state == PluginState::Enabled {
            self.state = PluginState::Disabled;
        }
    }

    /// Set the plugin into an error state.
    pub fn set_error(&mut self, message: String) {
        self.state = PluginState::Error(message);
    }
}

/// The current application version for compatibility checking.
pub const APP_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            id: "io.example.test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin for unit testing".to_string(),
            author: "Test Author".to_string(),
            min_app_version: "0.1.0".to_string(),
            permissions: vec![PluginPermission::ReadResources, PluginPermission::RegisterViews],
        }
    }

    fn sample_plugin() -> Plugin {
        Plugin::new(sample_manifest(), "/plugins/test-plugin.dylib".to_string())
    }

    // --- T122: Plugin type tests ---

    #[test]
    fn test_plugin_manifest_fields() {
        let manifest = sample_manifest();
        assert_eq!(manifest.id, "io.example.test-plugin");
        assert_eq!(manifest.name, "Test Plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin for unit testing");
        assert_eq!(manifest.author, "Test Author");
        assert_eq!(manifest.min_app_version, "0.1.0");
        assert_eq!(manifest.permissions.len(), 2);
    }

    #[test]
    fn test_plugin_manifest_has_permission() {
        let manifest = sample_manifest();
        assert!(manifest.has_permission(&PluginPermission::ReadResources));
        assert!(manifest.has_permission(&PluginPermission::RegisterViews));
        assert!(!manifest.has_permission(&PluginPermission::WriteResources));
        assert!(!manifest.has_permission(&PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_plugin_manifest_network_access_flag() {
        let mut manifest = sample_manifest();
        assert!(!manifest.requests_network_access());

        manifest.permissions.push(PluginPermission::NetworkAccess);
        assert!(manifest.requests_network_access());
    }

    #[test]
    fn test_plugin_manifest_write_access_flag() {
        let mut manifest = sample_manifest();
        assert!(!manifest.requests_write_access());

        manifest.permissions.push(PluginPermission::WriteResources);
        assert!(manifest.requests_write_access());
    }

    #[test]
    fn test_plugin_new_defaults() {
        let plugin = sample_plugin();
        assert_eq!(plugin.id, "io.example.test-plugin");
        assert_eq!(plugin.state, PluginState::Installed);
        assert_eq!(plugin.library_path, "/plugins/test-plugin.dylib");
        assert!(!plugin.is_active());
        assert!(!plugin.is_error());
        assert_eq!(plugin.config, json!({}));
    }

    #[test]
    fn test_plugin_enable() {
        let mut plugin = sample_plugin();
        assert_eq!(plugin.state, PluginState::Installed);

        plugin.enable().unwrap();
        assert_eq!(plugin.state, PluginState::Enabled);
        assert!(plugin.is_active());
    }

    #[test]
    fn test_plugin_enable_from_disabled() {
        let mut plugin = sample_plugin();
        plugin.state = PluginState::Disabled;

        plugin.enable().unwrap();
        assert_eq!(plugin.state, PluginState::Enabled);
    }

    #[test]
    fn test_plugin_enable_already_enabled() {
        let mut plugin = sample_plugin();
        plugin.enable().unwrap();

        // Enabling again should be a no-op
        plugin.enable().unwrap();
        assert_eq!(plugin.state, PluginState::Enabled);
    }

    #[test]
    fn test_plugin_enable_from_error_fails() {
        let mut plugin = sample_plugin();
        plugin.set_error("load failure".to_string());

        let result = plugin.enable();
        assert!(result.is_err());
        assert!(plugin.is_error());
    }

    #[test]
    fn test_plugin_disable() {
        let mut plugin = sample_plugin();
        plugin.enable().unwrap();
        assert!(plugin.is_active());

        plugin.disable();
        assert_eq!(plugin.state, PluginState::Disabled);
        assert!(!plugin.is_active());
    }

    #[test]
    fn test_plugin_disable_when_not_enabled() {
        let mut plugin = sample_plugin();
        // Disabling from Installed state should be a no-op
        plugin.disable();
        assert_eq!(plugin.state, PluginState::Installed);
    }

    #[test]
    fn test_plugin_set_error() {
        let mut plugin = sample_plugin();
        plugin.set_error("something went wrong".to_string());

        assert!(plugin.is_error());
        assert_eq!(plugin.state, PluginState::Error("something went wrong".to_string()));
    }

    #[test]
    fn test_plugin_state_is_active() {
        assert!(!PluginState::Installed.is_active());
        assert!(PluginState::Enabled.is_active());
        assert!(!PluginState::Disabled.is_active());
        assert!(!PluginState::Error("err".to_string()).is_active());
    }

    #[test]
    fn test_plugin_state_is_error() {
        assert!(!PluginState::Installed.is_error());
        assert!(!PluginState::Enabled.is_error());
        assert!(!PluginState::Disabled.is_error());
        assert!(PluginState::Error("err".to_string()).is_error());
    }

    #[test]
    fn test_plugin_permission_all_variants() {
        let permissions = [
            PluginPermission::ReadResources,
            PluginPermission::WriteResources,
            PluginPermission::RegisterViews,
            PluginPermission::RegisterActions,
            PluginPermission::RegisterSidebar,
            PluginPermission::NetworkAccess,
        ];
        assert_eq!(permissions.len(), 6);

        // Verify they are all distinct
        for (i, p) in permissions.iter().enumerate() {
            for (j, q) in permissions.iter().enumerate() {
                if i != j {
                    assert_ne!(p, q);
                }
            }
        }
    }

    #[test]
    fn test_plugin_error_display() {
        assert_eq!(
            PluginError::LoadFailed("bad lib".to_string()).to_string(),
            "failed to load plugin: bad lib"
        );
        assert_eq!(
            PluginError::VersionMismatch("too old".to_string()).to_string(),
            "version mismatch: too old"
        );
        assert_eq!(
            PluginError::PermissionDenied("no write".to_string()).to_string(),
            "permission denied: no write"
        );
        assert_eq!(
            PluginError::NotFound("missing".to_string()).to_string(),
            "plugin not found: missing"
        );
        assert_eq!(
            PluginError::ResourceNotFound("pod xyz".to_string()).to_string(),
            "resource not found: pod xyz"
        );
        assert_eq!(
            PluginError::ClusterError("timeout".to_string()).to_string(),
            "cluster error: timeout"
        );
        assert_eq!(
            PluginError::InternalError("crash".to_string()).to_string(),
            "internal error: crash"
        );
        assert_eq!(
            PluginError::AlreadyInstalled("dup".to_string()).to_string(),
            "already installed: dup"
        );
        assert_eq!(
            PluginError::InvalidManifest("bad".to_string()).to_string(),
            "invalid manifest: bad"
        );
    }

    #[test]
    fn test_plugin_serialization() {
        let plugin = sample_plugin();
        let json = serde_json::to_string(&plugin).unwrap();
        let deserialized: Plugin = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, plugin.id);
        assert_eq!(deserialized.manifest.name, plugin.manifest.name);
        assert_eq!(deserialized.manifest.version, plugin.manifest.version);
        assert_eq!(deserialized.state, plugin.state);
        assert_eq!(deserialized.library_path, plugin.library_path);
    }

    #[test]
    fn test_plugin_manifest_serialization() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, manifest.id);
        assert_eq!(deserialized.permissions, manifest.permissions);
    }

    #[test]
    fn test_plugin_permission_serialization() {
        let perm = PluginPermission::ReadResources;
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, "\"ReadResources\"");

        let deserialized: PluginPermission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PluginPermission::ReadResources);
    }

    #[test]
    fn test_plugin_state_serialization() {
        let installed = PluginState::Installed;
        let json = serde_json::to_string(&installed).unwrap();
        let deserialized: PluginState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, installed);

        let error = PluginState::Error("oops".to_string());
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: PluginState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, error);
    }

    #[test]
    fn test_plugin_manifest_empty_permissions() {
        let manifest = PluginManifest {
            id: "io.example.minimal".to_string(),
            name: "Minimal Plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "A plugin with no permissions".to_string(),
            author: "Tester".to_string(),
            min_app_version: "0.1.0".to_string(),
            permissions: vec![],
        };

        assert!(!manifest.has_permission(&PluginPermission::ReadResources));
        assert!(!manifest.requests_network_access());
        assert!(!manifest.requests_write_access());
    }

    #[test]
    fn test_plugin_config_can_be_set() {
        let mut plugin = sample_plugin();
        plugin.config = json!({"theme": "dark", "refresh_interval": 30});

        assert_eq!(plugin.config["theme"], "dark");
        assert_eq!(plugin.config["refresh_interval"], 30);
    }

    #[test]
    fn test_plugin_installed_at_is_set() {
        let before = Utc::now();
        let plugin = sample_plugin();
        let after = Utc::now();

        assert!(plugin.installed_at >= before);
        assert!(plugin.installed_at <= after);
    }
}
