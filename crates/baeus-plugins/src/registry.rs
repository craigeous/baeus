use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Plugin, PluginError, PluginManifest, PluginState};

/// Registry that manages installed plugins.
///
/// The PluginRegistry maintains the catalog of known plugins, handles
/// installation, enabling/disabling, and uninstallation. It scans a
/// designated plugin directory for available plugin libraries.
pub struct PluginRegistry {
    /// Map of plugin ID to Plugin
    plugins: HashMap<String, Plugin>,
    /// Directory where plugin libraries are stored
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    /// Create a new PluginRegistry with the given plugin directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self { plugins: HashMap::new(), plugin_dir }
    }

    /// Returns the plugin directory path.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Returns the number of installed plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Scan the plugin directory for available plugin libraries.
    ///
    /// Returns a list of paths to .dylib/.so/.dll files found in the directory.
    pub fn scan_directory(&self) -> Result<Vec<PathBuf>, PluginError> {
        if !self.plugin_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = std::fs::read_dir(&self.plugin_dir).map_err(|e| {
            PluginError::LoadFailed(format!(
                "failed to read plugin directory '{}': {}",
                self.plugin_dir.display(),
                e
            ))
        })?;

        let mut libraries = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                PluginError::LoadFailed(format!("failed to read directory entry: {}", e))
            })?;
            let path = entry.path();
            if is_plugin_library(&path) {
                libraries.push(path);
            }
        }

        Ok(libraries)
    }

    /// Install a plugin from a manifest and library path.
    ///
    /// The plugin will be in the `Installed` state after this call.
    pub fn install(
        &mut self,
        manifest: PluginManifest,
        library_path: String,
    ) -> Result<(), PluginError> {
        if self.plugins.contains_key(&manifest.id) {
            return Err(PluginError::AlreadyInstalled(manifest.id));
        }

        let plugin = Plugin::new(manifest, library_path);
        self.plugins.insert(plugin.id.clone(), plugin);
        Ok(())
    }

    /// Enable a plugin by ID.
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        plugin.enable()
    }

    /// Disable a plugin by ID.
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        plugin.disable();
        Ok(())
    }

    /// Uninstall a plugin by ID.
    ///
    /// Removes the plugin from the registry. Does not delete the library file.
    pub fn uninstall(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if self.plugins.remove(plugin_id).is_none() {
            return Err(PluginError::NotFound(plugin_id.to_string()));
        }
        Ok(())
    }

    /// Get a reference to a plugin by ID.
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&Plugin> {
        self.plugins.get(plugin_id)
    }

    /// Get a mutable reference to a plugin by ID.
    pub fn get_plugin_mut(&mut self, plugin_id: &str) -> Option<&mut Plugin> {
        self.plugins.get_mut(plugin_id)
    }

    /// List all installed plugins.
    pub fn list_plugins(&self) -> Vec<&Plugin> {
        self.plugins.values().collect()
    }

    /// List plugins filtered by state.
    pub fn list_by_state(&self, state: &PluginState) -> Vec<&Plugin> {
        self.plugins.values().filter(|p| &p.state == state).collect()
    }

    /// List all enabled plugins.
    pub fn enabled_plugins(&self) -> Vec<&Plugin> {
        self.list_by_state(&PluginState::Enabled)
    }

    /// Returns true if a plugin with the given ID is installed.
    pub fn is_installed(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }
}

/// Check if a path looks like a plugin shared library.
fn is_plugin_library(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("dylib") | Some("so") | Some("dll"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginPermission;
    use std::fs;

    fn sample_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            name: format!("Plugin {}", id),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: "Test".to_string(),
            min_app_version: "0.1.0".to_string(),
            permissions: vec![PluginPermission::ReadResources],
        }
    }

    // --- T127: Plugin registry tests ---

    #[test]
    fn test_registry_new() {
        let dir = PathBuf::from("/tmp/baeus-registry-test");
        let registry = PluginRegistry::new(dir.clone());
        assert_eq!(registry.plugin_dir(), dir);
        assert_eq!(registry.plugin_count(), 0);
        assert!(registry.list_plugins().is_empty());
    }

    #[test]
    fn test_registry_install() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let manifest = sample_manifest("io.example.test");

        registry.install(manifest, "/plugins/test.dylib".to_string()).unwrap();
        assert_eq!(registry.plugin_count(), 1);
        assert!(registry.is_installed("io.example.test"));
    }

    #[test]
    fn test_registry_install_duplicate_rejected() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let manifest = sample_manifest("io.example.test");

        registry.install(manifest.clone(), "/plugins/test.dylib".to_string()).unwrap();
        let result = registry.install(manifest, "/plugins/test2.dylib".to_string());
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::AlreadyInstalled(id) => {
                assert_eq!(id, "io.example.test");
            }
            other => panic!("expected AlreadyInstalled, got {:?}", other),
        }
    }

    #[test]
    fn test_registry_enable() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry
            .install(sample_manifest("io.example.test"), "/plugins/test.dylib".to_string())
            .unwrap();

        registry.enable("io.example.test").unwrap();
        let plugin = registry.get_plugin("io.example.test").unwrap();
        assert_eq!(plugin.state, PluginState::Enabled);
    }

    #[test]
    fn test_registry_enable_not_found() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let result = registry.enable("nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::NotFound(id) => assert_eq!(id, "nonexistent"),
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_registry_disable() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry
            .install(sample_manifest("io.example.test"), "/plugins/test.dylib".to_string())
            .unwrap();
        registry.enable("io.example.test").unwrap();

        registry.disable("io.example.test").unwrap();
        let plugin = registry.get_plugin("io.example.test").unwrap();
        assert_eq!(plugin.state, PluginState::Disabled);
    }

    #[test]
    fn test_registry_disable_not_found() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let result = registry.disable("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_uninstall() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry
            .install(sample_manifest("io.example.test"), "/plugins/test.dylib".to_string())
            .unwrap();

        registry.uninstall("io.example.test").unwrap();
        assert_eq!(registry.plugin_count(), 0);
        assert!(!registry.is_installed("io.example.test"));
    }

    #[test]
    fn test_registry_uninstall_not_found() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let result = registry.uninstall("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_get_plugin() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry
            .install(sample_manifest("io.example.test"), "/plugins/test.dylib".to_string())
            .unwrap();

        let plugin = registry.get_plugin("io.example.test");
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().manifest.name, "Plugin io.example.test");

        let missing = registry.get_plugin("io.example.nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_registry_list_plugins() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry.install(sample_manifest("io.example.a"), "/plugins/a.dylib".to_string()).unwrap();
        registry.install(sample_manifest("io.example.b"), "/plugins/b.dylib".to_string()).unwrap();

        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 2);
    }

    #[test]
    fn test_registry_list_by_state() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry.install(sample_manifest("io.example.a"), "/plugins/a.dylib".to_string()).unwrap();
        registry.install(sample_manifest("io.example.b"), "/plugins/b.dylib".to_string()).unwrap();
        registry.install(sample_manifest("io.example.c"), "/plugins/c.dylib".to_string()).unwrap();

        registry.enable("io.example.a").unwrap();
        registry.enable("io.example.b").unwrap();

        let enabled = registry.enabled_plugins();
        assert_eq!(enabled.len(), 2);

        let installed = registry.list_by_state(&PluginState::Installed);
        assert_eq!(installed.len(), 1);
    }

    #[test]
    fn test_registry_scan_directory_empty() {
        let dir = std::env::temp_dir().join("baeus-registry-scan-empty");
        fs::create_dir_all(&dir).ok();

        let registry = PluginRegistry::new(dir.clone());
        let result = registry.scan_directory().unwrap();
        assert!(result.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_scan_directory_nonexistent() {
        let dir = PathBuf::from("/tmp/baeus-registry-scan-nonexistent-xyz");
        let registry = PluginRegistry::new(dir);
        let result = registry.scan_directory().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_registry_scan_directory_finds_libraries() {
        let dir = std::env::temp_dir().join("baeus-registry-scan-libs");
        fs::create_dir_all(&dir).ok();

        fs::write(dir.join("plugin1.dylib"), b"fake").ok();
        fs::write(dir.join("plugin2.so"), b"fake").ok();
        fs::write(dir.join("readme.txt"), b"text").ok();

        let registry = PluginRegistry::new(dir.clone());
        let result = registry.scan_directory().unwrap();
        assert_eq!(result.len(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_install_enable_disable_uninstall_lifecycle() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        let id = "io.example.lifecycle";

        // Install
        registry.install(sample_manifest(id), "/plugins/lifecycle.dylib".to_string()).unwrap();
        assert_eq!(registry.get_plugin(id).unwrap().state, PluginState::Installed);

        // Enable
        registry.enable(id).unwrap();
        assert_eq!(registry.get_plugin(id).unwrap().state, PluginState::Enabled);

        // Disable
        registry.disable(id).unwrap();
        assert_eq!(registry.get_plugin(id).unwrap().state, PluginState::Disabled);

        // Re-enable
        registry.enable(id).unwrap();
        assert_eq!(registry.get_plugin(id).unwrap().state, PluginState::Enabled);

        // Uninstall
        registry.uninstall(id).unwrap();
        assert!(!registry.is_installed(id));
    }

    #[test]
    fn test_registry_get_plugin_mut() {
        let mut registry = PluginRegistry::new(PathBuf::from("/tmp/test"));
        registry
            .install(sample_manifest("io.example.test"), "/plugins/test.dylib".to_string())
            .unwrap();

        let plugin = registry.get_plugin_mut("io.example.test").unwrap();
        plugin.set_error("test error".to_string());

        assert!(registry.get_plugin("io.example.test").unwrap().is_error());
    }
}
