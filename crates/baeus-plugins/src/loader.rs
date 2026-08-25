use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

use crate::{APP_VERSION, PluginError, PluginManifest};

/// Trait that every Baeus plugin must implement.
///
/// This is the single entry point for plugin lifecycle and capability registration.
/// Plugin libraries export a `_baeus_plugin_create` function that returns a
/// `Box<dyn BaeusPlugin>`.
pub trait BaeusPlugin: Send + Sync {
    /// Returns the plugin name.
    fn name(&self) -> &str;

    /// Returns the plugin version string.
    fn version(&self) -> &str;

    /// Returns the plugin manifest.
    fn manifest(&self) -> PluginManifest;

    /// Called once when the plugin is loaded. Register capabilities here.
    fn on_load(&self) -> Result<(), PluginError>;

    /// Called when the plugin is unloaded. Clean up resources here.
    fn on_unload(&self) -> Result<(), PluginError>;
}

/// A loaded plugin library handle.
///
/// Wraps the raw library handle and the plugin instance created from it.
/// The library must outlive the plugin instance since the plugin's vtable
/// lives in the loaded library.
pub struct LoadedPlugin {
    /// The plugin instance
    pub plugin: Box<dyn BaeusPlugin>,
    /// The raw library handle - kept alive to prevent unloading
    _library: Library,
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin").field("plugin", &"<dyn BaeusPlugin>").finish()
    }
}

impl LoadedPlugin {
    /// Returns the plugin name.
    pub fn name(&self) -> &str {
        self.plugin.name()
    }

    /// Returns the plugin version.
    pub fn version(&self) -> &str {
        self.plugin.version()
    }

    /// Returns the plugin manifest.
    pub fn manifest(&self) -> PluginManifest {
        self.plugin.manifest()
    }
}

/// Loads .dylib plugin files from a sandboxed directory.
///
/// Uses `libloading` to dynamically load shared libraries and extract
/// plugin instances via the standard entry point function.
///
/// # Security
///
/// Loading dynamic libraries is inherently unsafe and grants the plugin full
/// process privileges. Mitigations in place:
/// - Path traversal prevention via `normalize_path()` (no filesystem access)
/// - Sandbox directory check (`starts_with` on normalized path)
/// - SemVer version compatibility check
///
/// **Not yet implemented** (documented as future work):
/// - Code signature verification (macOS `codesign`, Linux `minisign`)
/// - Explicit user consent/approval before loading new plugins
///
/// Plugins can be disabled entirely by setting `plugins_enabled = false`
/// in the application preferences.
pub struct PluginLoader {
    /// Base directory where plugins are loaded from
    plugin_dir: PathBuf,
    /// Whether plugin loading is enabled. When false, `load()` always returns an error.
    enabled: bool,
}

impl PluginLoader {
    /// Create a new PluginLoader that loads plugins from the given directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self { plugin_dir, enabled: true }
    }

    /// Create a new PluginLoader with plugin loading disabled.
    pub fn disabled(plugin_dir: PathBuf) -> Self {
        Self { plugin_dir, enabled: false }
    }

    /// Set whether plugin loading is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns whether plugin loading is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the plugin directory path.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Load a plugin from a .dylib file.
    ///
    /// The library must export a `_baeus_plugin_create` function with the signature:
    /// `fn() -> *mut dyn BaeusPlugin`
    ///
    /// # Safety
    /// Loading dynamic libraries is inherently unsafe. The library must be compiled
    /// against a compatible version of the BaeusPlugin trait.
    pub fn load(&self, library_path: &Path) -> Result<LoadedPlugin, PluginError> {
        if !self.enabled {
            return Err(PluginError::PermissionDenied(
                "plugin loading is disabled in preferences".to_string(),
            ));
        }

        // Verify the path is within the sandboxed plugin directory
        let full_path = if library_path.is_absolute() {
            library_path.to_path_buf()
        } else {
            self.plugin_dir.join(library_path)
        };

        // Normalize the path to resolve ".." components (prevents path traversal)
        let full_path = normalize_path(&full_path);

        if !full_path.starts_with(&self.plugin_dir) {
            return Err(PluginError::PermissionDenied(format!(
                "plugin path '{}' is outside the sandboxed directory '{}'",
                full_path.display(),
                self.plugin_dir.display()
            )));
        }

        if !full_path.exists() {
            return Err(PluginError::NotFound(format!(
                "plugin library not found: {}",
                full_path.display()
            )));
        }

        // Load the library
        let library = unsafe { Library::new(&full_path) }.map_err(|e| {
            PluginError::LoadFailed(format!(
                "failed to load library '{}': {}",
                full_path.display(),
                e
            ))
        })?;

        // Look up the plugin creation function
        let plugin = unsafe {
            let create_fn: Symbol<unsafe fn() -> *mut dyn BaeusPlugin> =
                library.get(b"_baeus_plugin_create").map_err(|e| {
                    PluginError::LoadFailed(format!(
                        "plugin '{}' missing _baeus_plugin_create entry point: {}",
                        full_path.display(),
                        e
                    ))
                })?;

            Box::from_raw(create_fn())
        };

        // Check version compatibility
        let manifest = plugin.manifest();
        check_version_compatibility(&manifest.min_app_version, APP_VERSION)?;

        Ok(LoadedPlugin { plugin, _library: library })
    }

    /// Scan the plugin directory for shared library files.
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
}

/// Check if the application version satisfies the plugin's minimum version requirement.
///
/// Uses simple SemVer major.minor.patch comparison. The app version must be
/// greater than or equal to the plugin's min_app_version, and must share the
/// same major version (breaking changes increment major).
pub fn check_version_compatibility(
    min_app_version: &str,
    app_version: &str,
) -> Result<(), PluginError> {
    let min_parts = parse_semver(min_app_version).map_err(|e| {
        PluginError::InvalidManifest(format!(
            "invalid min_app_version '{}': {}",
            min_app_version, e
        ))
    })?;
    let app_parts = parse_semver(app_version).map_err(|e| {
        PluginError::InternalError(format!("invalid app version '{}': {}", app_version, e))
    })?;

    // Major version must match (breaking changes)
    if min_parts.0 != app_parts.0 {
        return Err(PluginError::VersionMismatch(format!(
            "plugin requires app major version {}, but app is version {}",
            min_parts.0, app_version
        )));
    }

    // App version must be >= min_app_version
    if app_parts < min_parts {
        return Err(PluginError::VersionMismatch(format!(
            "plugin requires app version >= {}, but app is version {}",
            min_app_version, app_version
        )));
    }

    Ok(())
}

/// Parse a SemVer string into (major, minor, patch) tuple.
fn parse_semver(version: &str) -> Result<(u32, u32, u32), String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("expected 3 parts, got {}", parts.len()));
    }

    let major = parts[0].parse::<u32>().map_err(|e| format!("invalid major version: {}", e))?;
    let minor = parts[1].parse::<u32>().map_err(|e| format!("invalid minor version: {}", e))?;
    let patch = parts[2].parse::<u32>().map_err(|e| format!("invalid patch version: {}", e))?;

    Ok((major, minor, patch))
}

/// Normalize a path by resolving `.` and `..` components without
/// accessing the filesystem. This prevents path traversal attacks.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                // Pop the last component if possible
                if !components.is_empty() {
                    components.pop();
                }
            }
            Component::CurDir => {
                // Skip "." components
            }
            _ => {
                components.push(component);
            }
        }
    }

    components.iter().collect()
}

/// Check if a path looks like a plugin shared library.
fn is_plugin_library(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("dylib") => true, // macOS
        Some("so") => true,    // Linux
        Some("dll") => true,   // Windows
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- T123: Plugin loading/unloading tests ---

    #[test]
    fn test_plugin_loader_new() {
        let dir = PathBuf::from("/tmp/baeus-plugins-test");
        let loader = PluginLoader::new(dir.clone());
        assert_eq!(loader.plugin_dir(), dir);
    }

    #[test]
    fn test_plugin_loader_rejects_path_outside_sandbox() {
        let dir = PathBuf::from("/tmp/baeus-plugins-test");
        let loader = PluginLoader::new(dir);

        let result = loader.load(Path::new("/etc/malicious.dylib"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("outside the sandboxed directory"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_plugin_loader_rejects_nonexistent_file() {
        let dir = std::env::temp_dir().join("baeus-loader-test-nonexistent");
        fs::create_dir_all(&dir).ok();
        let loader = PluginLoader::new(dir.clone());

        let result = loader.load(Path::new("does-not-exist.dylib"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::NotFound(msg) => {
                assert!(msg.contains("not found"));
            }
            other => panic!("expected NotFound, got {:?}", other),
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_plugin_loader_rejects_invalid_library() {
        let dir = std::env::temp_dir().join("baeus-loader-test-invalid");
        fs::create_dir_all(&dir).ok();

        // Create a fake .dylib that isn't a valid shared library
        let fake_path = dir.join("fake.dylib");
        fs::write(&fake_path, b"not a real library").ok();

        let loader = PluginLoader::new(dir.clone());
        let result = loader.load(Path::new("fake.dylib"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::LoadFailed(msg) => {
                assert!(msg.contains("failed to load library"));
            }
            other => panic!("expected LoadFailed, got {:?}", other),
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_scan_directory_empty() {
        let dir = std::env::temp_dir().join("baeus-scan-test-empty");
        fs::create_dir_all(&dir).ok();

        let loader = PluginLoader::new(dir.clone());
        let result = loader.scan_directory().unwrap();
        assert!(result.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let dir = PathBuf::from("/tmp/baeus-scan-test-does-not-exist-xyz");
        let loader = PluginLoader::new(dir);
        let result = loader.scan_directory().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_directory_finds_dylibs() {
        let dir = std::env::temp_dir().join("baeus-scan-test-dylibs");
        fs::create_dir_all(&dir).ok();

        // Create some fake files
        fs::write(dir.join("plugin1.dylib"), b"fake").ok();
        fs::write(dir.join("plugin2.dylib"), b"fake").ok();
        fs::write(dir.join("readme.txt"), b"not a plugin").ok();
        fs::write(dir.join("data.json"), b"{}").ok();

        let loader = PluginLoader::new(dir.clone());
        let result = loader.scan_directory().unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.extension().unwrap() == "dylib"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_scan_directory_finds_so_files() {
        let dir = std::env::temp_dir().join("baeus-scan-test-so");
        fs::create_dir_all(&dir).ok();

        fs::write(dir.join("plugin.so"), b"fake").ok();
        fs::write(dir.join("plugin.dylib"), b"fake").ok();

        let loader = PluginLoader::new(dir.clone());
        let result = loader.scan_directory().unwrap();
        assert_eq!(result.len(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    // --- Version compatibility tests ---

    #[test]
    fn test_version_compatibility_exact_match() {
        assert!(check_version_compatibility("0.1.0", "0.1.0").is_ok());
    }

    #[test]
    fn test_version_compatibility_app_newer_minor() {
        assert!(check_version_compatibility("0.1.0", "0.2.0").is_ok());
    }

    #[test]
    fn test_version_compatibility_app_newer_patch() {
        assert!(check_version_compatibility("0.1.0", "0.1.5").is_ok());
    }

    #[test]
    fn test_version_compatibility_app_older() {
        let result = check_version_compatibility("0.3.0", "0.1.0");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::VersionMismatch(msg) => {
                assert!(msg.contains(">="));
            }
            other => panic!("expected VersionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_version_compatibility_major_version_mismatch() {
        let result = check_version_compatibility("1.0.0", "0.9.9");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::VersionMismatch(msg) => {
                assert!(msg.contains("major version"));
            }
            other => panic!("expected VersionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_version_compatibility_major_version_higher_app() {
        // App at major version 2, plugin wants 1 - incompatible (breaking changes)
        let result = check_version_compatibility("1.0.0", "2.0.0");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::VersionMismatch(msg) => {
                assert!(msg.contains("major version"));
            }
            other => panic!("expected VersionMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_version_compatibility_invalid_min_version() {
        let result = check_version_compatibility("not.a.version", "0.1.0");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InvalidManifest(msg) => {
                assert!(msg.contains("invalid min_app_version"));
            }
            other => panic!("expected InvalidManifest, got {:?}", other),
        }
    }

    #[test]
    fn test_version_compatibility_invalid_app_version() {
        let result = check_version_compatibility("0.1.0", "bad");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InternalError(msg) => {
                assert!(msg.contains("invalid app version"));
            }
            other => panic!("expected InternalError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_semver_valid() {
        assert_eq!(parse_semver("0.1.0"), Ok((0, 1, 0)));
        assert_eq!(parse_semver("1.2.3"), Ok((1, 2, 3)));
        assert_eq!(parse_semver("10.20.30"), Ok((10, 20, 30)));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert!(parse_semver("1.2").is_err());
        assert!(parse_semver("abc").is_err());
        assert!(parse_semver("1.2.3.4").is_err());
        assert!(parse_semver("a.b.c").is_err());
    }

    #[test]
    fn test_is_plugin_library() {
        assert!(is_plugin_library(Path::new("plugin.dylib")));
        assert!(is_plugin_library(Path::new("plugin.so")));
        assert!(is_plugin_library(Path::new("plugin.dll")));
        assert!(!is_plugin_library(Path::new("plugin.txt")));
        assert!(!is_plugin_library(Path::new("plugin.rs")));
        assert!(!is_plugin_library(Path::new("plugin")));
    }

    #[test]
    fn test_loaded_plugin_path_traversal_blocked() {
        let dir = std::env::temp_dir().join("baeus-loader-traversal");
        fs::create_dir_all(&dir).ok();

        let loader = PluginLoader::new(dir.clone());

        // Attempt path traversal
        let result = loader.load(Path::new("../../../etc/passwd"));
        assert!(result.is_err());

        fs::remove_dir_all(&dir).ok();
    }
}
