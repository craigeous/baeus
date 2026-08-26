use std::path::{Path, PathBuf};

use crate::{PluginError, PluginManifest, PluginPermission};

/// Configuration for the plugin sandbox.
///
/// Defines the constraints that a plugin must operate within.
/// The sandbox enforces that plugins only access resources they
/// have declared permissions for.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Allowed permissions for the plugin
    pub allowed_permissions: Vec<PluginPermission>,
    /// Base directory for plugin libraries (loading restricted to this path)
    pub plugin_dir: PathBuf,
    /// Whether network access is permitted
    pub allow_network: bool,
    /// Allowed filesystem paths (empty = no filesystem access beyond plugin_dir)
    pub allowed_paths: Vec<PathBuf>,
}

impl SandboxConfig {
    /// Create a new SandboxConfig with the given plugin directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            allowed_permissions: Vec::new(),
            plugin_dir,
            allow_network: false,
            allowed_paths: Vec::new(),
        }
    }

    /// Create a SandboxConfig from a plugin manifest.
    ///
    /// The sandbox is configured based on the permissions declared
    /// in the manifest.
    pub fn from_manifest(manifest: &PluginManifest, plugin_dir: PathBuf) -> Self {
        let allow_network = manifest.has_permission(&PluginPermission::NetworkAccess);

        Self {
            allowed_permissions: manifest.permissions.clone(),
            plugin_dir,
            allow_network,
            allowed_paths: Vec::new(),
        }
    }

    /// Check if a specific permission is allowed by this sandbox config.
    pub fn is_permission_allowed(&self, permission: &PluginPermission) -> bool {
        self.allowed_permissions.contains(permission)
    }

    /// Check if a filesystem path is within the allowed paths.
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        // Plugin directory itself is always allowed
        if path.starts_with(&self.plugin_dir) {
            return true;
        }

        // Check against explicitly allowed paths
        self.allowed_paths.iter().any(|allowed| path.starts_with(allowed))
    }
}

/// A sandboxed wrapper around the plugin loader.
///
/// Validates plugin permissions against the manifest before allowing
/// operations, and restricts file loading to the sandboxed directory.
pub struct SandboxedLoader {
    /// Sandbox configuration
    config: SandboxConfig,
}

impl SandboxedLoader {
    /// Create a new SandboxedLoader with the given configuration.
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the sandbox configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Validate that a library path is within the sandboxed directory.
    pub fn validate_library_path(&self, path: &Path) -> Result<PathBuf, PluginError> {
        let full_path =
            if path.is_absolute() { path.to_path_buf() } else { self.config.plugin_dir.join(path) };

        // Normalize the path to resolve ".." components (without requiring the path to exist)
        let normalized = normalize_path(&full_path);

        if !normalized.starts_with(&self.config.plugin_dir) {
            return Err(PluginError::PermissionDenied(format!(
                "library path '{}' is outside the sandboxed directory '{}'",
                full_path.display(),
                self.config.plugin_dir.display()
            )));
        }

        Ok(normalized)
    }

    /// Validate that a plugin manifest declares only allowed permissions.
    ///
    /// This checks the manifest's requested permissions against the
    /// sandbox's allowed permissions. If the manifest requests a permission
    /// not in the allowed set, the validation fails.
    pub fn validate_manifest_permissions(
        &self,
        manifest: &PluginManifest,
    ) -> Result<(), PluginError> {
        for permission in &manifest.permissions {
            if !self.config.is_permission_allowed(permission) {
                return Err(PluginError::PermissionDenied(format!(
                    "plugin '{}' requests permission {:?} which is not allowed",
                    manifest.id, permission
                )));
            }
        }
        Ok(())
    }

    /// Validate that a plugin can perform a specific operation.
    pub fn check_permission(&self, permission: &PluginPermission) -> Result<(), PluginError> {
        if !self.config.is_permission_allowed(permission) {
            return Err(PluginError::PermissionDenied(format!(
                "permission {:?} is not allowed in this sandbox",
                permission
            )));
        }
        Ok(())
    }

    /// Validate that a filesystem path access is allowed.
    pub fn check_path_access(&self, path: &Path) -> Result<(), PluginError> {
        if !self.config.is_path_allowed(path) {
            return Err(PluginError::PermissionDenied(format!(
                "access to path '{}' is not allowed",
                path.display()
            )));
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            id: "io.example.sandboxed".to_string(),
            name: "Sandboxed Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A sandboxed plugin".to_string(),
            author: "Test".to_string(),
            min_app_version: "0.1.0".to_string(),
            permissions: vec![PluginPermission::ReadResources, PluginPermission::RegisterViews],
        }
    }

    // --- T129: Plugin sandbox tests ---

    #[test]
    fn test_sandbox_config_new() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir.clone());
        assert_eq!(config.plugin_dir, dir);
        assert!(!config.allow_network);
        assert!(config.allowed_permissions.is_empty());
        assert!(config.allowed_paths.is_empty());
    }

    #[test]
    fn test_sandbox_config_from_manifest() {
        let manifest = sample_manifest();
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::from_manifest(&manifest, dir);

        assert!(!config.allow_network);
        assert_eq!(config.allowed_permissions.len(), 2);
        assert!(config.is_permission_allowed(&PluginPermission::ReadResources));
        assert!(config.is_permission_allowed(&PluginPermission::RegisterViews));
        assert!(!config.is_permission_allowed(&PluginPermission::WriteResources));
    }

    #[test]
    fn test_sandbox_config_from_manifest_with_network() {
        let mut manifest = sample_manifest();
        manifest.permissions.push(PluginPermission::NetworkAccess);
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::from_manifest(&manifest, dir);

        assert!(config.allow_network);
        assert!(config.is_permission_allowed(&PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_sandbox_config_path_allowed_plugin_dir() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);

        assert!(config.is_path_allowed(Path::new("/tmp/plugins/my-plugin.dylib")));
        assert!(config.is_path_allowed(Path::new("/tmp/plugins/subdir/file")));
        assert!(!config.is_path_allowed(Path::new("/etc/passwd")));
        assert!(!config.is_path_allowed(Path::new("/tmp/other")));
    }

    #[test]
    fn test_sandbox_config_path_allowed_explicit() {
        let dir = PathBuf::from("/tmp/plugins");
        let mut config = SandboxConfig::new(dir);
        config.allowed_paths.push(PathBuf::from("/var/data"));

        assert!(config.is_path_allowed(Path::new("/var/data/file.txt")));
        assert!(config.is_path_allowed(Path::new("/tmp/plugins/file.dylib")));
        assert!(!config.is_path_allowed(Path::new("/etc/hosts")));
    }

    #[test]
    fn test_sandboxed_loader_validate_library_path_inside() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);
        let loader = SandboxedLoader::new(config);

        let result = loader.validate_library_path(Path::new("my-plugin.dylib"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/plugins/my-plugin.dylib"));
    }

    #[test]
    fn test_sandboxed_loader_validate_library_path_absolute_inside() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);
        let loader = SandboxedLoader::new(config);

        let result = loader.validate_library_path(Path::new("/tmp/plugins/my-plugin.dylib"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandboxed_loader_validate_library_path_outside() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);
        let loader = SandboxedLoader::new(config);

        let result = loader.validate_library_path(Path::new("/etc/malicious.dylib"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("outside the sandboxed directory"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_sandboxed_loader_validate_manifest_permissions_ok() {
        let manifest = sample_manifest();
        let config = SandboxConfig::from_manifest(&manifest, PathBuf::from("/tmp/plugins"));
        let loader = SandboxedLoader::new(config);

        let result = loader.validate_manifest_permissions(&manifest);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandboxed_loader_validate_manifest_permissions_rejected() {
        let manifest = sample_manifest();
        // Create sandbox that only allows ReadResources
        let mut config = SandboxConfig::new(PathBuf::from("/tmp/plugins"));
        config.allowed_permissions.push(PluginPermission::ReadResources);
        let loader = SandboxedLoader::new(config);

        // Manifest requests RegisterViews which is not allowed
        let result = loader.validate_manifest_permissions(&manifest);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("RegisterViews"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_sandboxed_loader_check_permission() {
        let manifest = sample_manifest();
        let config = SandboxConfig::from_manifest(&manifest, PathBuf::from("/tmp/plugins"));
        let loader = SandboxedLoader::new(config);

        assert!(loader.check_permission(&PluginPermission::ReadResources).is_ok());
        assert!(loader.check_permission(&PluginPermission::RegisterViews).is_ok());
        assert!(loader.check_permission(&PluginPermission::WriteResources).is_err());
    }

    #[test]
    fn test_sandboxed_loader_check_path_access() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);
        let loader = SandboxedLoader::new(config);

        assert!(loader.check_path_access(Path::new("/tmp/plugins/file.dylib")).is_ok());
        assert!(loader.check_path_access(Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn test_sandboxed_loader_path_traversal_blocked() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir);
        let loader = SandboxedLoader::new(config);

        // Attempt path traversal with relative path
        let result = loader.validate_library_path(Path::new("../../../etc/passwd"));
        // The path resolves to /tmp/plugins/../../../etc/passwd which doesn't start with /tmp/plugins
        assert!(result.is_err());
    }

    #[test]
    fn test_sandboxed_loader_config_reference() {
        let dir = PathBuf::from("/tmp/plugins");
        let config = SandboxConfig::new(dir.clone());
        let loader = SandboxedLoader::new(config);

        assert_eq!(loader.config().plugin_dir, dir);
    }

    #[test]
    fn test_sandbox_config_permission_check_all_variants() {
        let mut config = SandboxConfig::new(PathBuf::from("/tmp"));
        config.allowed_permissions = vec![
            PluginPermission::ReadResources,
            PluginPermission::WriteResources,
            PluginPermission::RegisterViews,
            PluginPermission::RegisterActions,
            PluginPermission::RegisterSidebar,
            PluginPermission::NetworkAccess,
        ];

        assert!(config.is_permission_allowed(&PluginPermission::ReadResources));
        assert!(config.is_permission_allowed(&PluginPermission::WriteResources));
        assert!(config.is_permission_allowed(&PluginPermission::RegisterViews));
        assert!(config.is_permission_allowed(&PluginPermission::RegisterActions));
        assert!(config.is_permission_allowed(&PluginPermission::RegisterSidebar));
        assert!(config.is_permission_allowed(&PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_sandbox_empty_config_denies_all() {
        let config = SandboxConfig::new(PathBuf::from("/tmp"));

        assert!(!config.is_permission_allowed(&PluginPermission::ReadResources));
        assert!(!config.is_permission_allowed(&PluginPermission::WriteResources));
        assert!(!config.is_permission_allowed(&PluginPermission::RegisterViews));
        assert!(!config.is_permission_allowed(&PluginPermission::RegisterActions));
        assert!(!config.is_permission_allowed(&PluginPermission::RegisterSidebar));
        assert!(!config.is_permission_allowed(&PluginPermission::NetworkAccess));
    }
}
