use crate::cluster::AuthMethod;
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct KubeContext {
    pub name: String,
    pub cluster_name: String,
    pub user_name: String,
    pub namespace: Option<String>,
    pub api_server_url: Option<String>,
    pub auth_method: AuthMethod,
}

#[derive(Debug)]
pub struct KubeconfigLoader {
    contexts: Vec<KubeContext>,
    current_context: Option<String>,
}

impl KubeconfigLoader {
    pub fn load_default() -> Result<Self> {
        let path = default_kubeconfig_path()?;
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &std::path::Path) -> Result<Self> {
        // Warn if kubeconfig file has overly permissive Unix permissions.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                let mode = metadata.permissions().mode();
                if mode & 0o077 != 0 {
                    tracing::warn!(
                        "Kubeconfig '{}' has overly permissive permissions ({:o}), \
                         should be 0600 to protect credentials",
                        path.display(),
                        mode & 0o777,
                    );
                }
            }
        }

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read kubeconfig from {}", path.display()))?;
        Self::load_from_str(&contents)
    }

    pub fn load_from_str(yaml: &str) -> Result<Self> {
        let config: serde_json::Value =
            serde_yaml_ng::from_str(yaml).context("Failed to parse kubeconfig YAML")?;

        let current_context =
            config.get("current-context").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut contexts = Vec::new();

        let context_entries =
            config.get("contexts").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        let clusters_array =
            config.get("clusters").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        let users_array =
            config.get("users").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        for ctx_entry in &context_entries {
            let name = ctx_entry.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ctx = ctx_entry.get("context").cloned().unwrap_or_default();

            let cluster_name =
                ctx.get("cluster").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let user_name = ctx.get("user").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let namespace = ctx.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string());

            let api_server_url = clusters_array
                .iter()
                .find(|c| c.get("name").and_then(|v| v.as_str()) == Some(&cluster_name))
                .and_then(|c| c.get("cluster"))
                .and_then(|c| c.get("server"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let auth_method = detect_auth_method(&users_array, &user_name);

            contexts.push(KubeContext {
                name,
                cluster_name,
                user_name,
                namespace,
                api_server_url,
                auth_method,
            });
        }

        Ok(Self { contexts, current_context })
    }

    pub fn contexts(&self) -> &[KubeContext] {
        &self.contexts
    }

    pub fn current_context(&self) -> Option<&str> {
        self.current_context.as_deref()
    }

    pub fn find_context(&self, name: &str) -> Option<&KubeContext> {
        self.contexts.iter().find(|c| c.name == name)
    }

    pub fn context_names(&self) -> Vec<&str> {
        self.contexts.iter().map(|c| c.name.as_str()).collect()
    }
}

fn detect_auth_method(users: &[serde_json::Value], user_name: &str) -> AuthMethod {
    let user = users
        .iter()
        .find(|u| u.get("name").and_then(|v| v.as_str()) == Some(user_name))
        .and_then(|u| u.get("user"));

    let Some(user) = user else {
        return AuthMethod::Token;
    };

    if user.get("exec").is_some() {
        AuthMethod::ExecPlugin
    } else if user.get("auth-provider").is_some() {
        AuthMethod::OIDC
    } else if user.get("client-certificate").is_some()
        || user.get("client-certificate-data").is_some()
    {
        AuthMethod::Certificate
    } else {
        AuthMethod::Token
    }
}

// ---------------------------------------------------------------------------
// Kubeconfig Discovery (FR-043 through FR-046)
// ---------------------------------------------------------------------------

/// Discovers kubeconfig files from the default path and additional user-configured directories.
#[derive(Debug)]
pub struct KubeconfigDiscovery {
    scan_paths: Vec<PathBuf>,
}

impl Default for KubeconfigDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl KubeconfigDiscovery {
    /// Create a new discovery starting with the default kubeconfig path(s).
    /// Handles `KUBECONFIG` env var with colon-separated paths per K8s spec.
    /// Also scans `~/.kube/` directory recursively for additional kubeconfig files.
    pub fn new() -> Self {
        let mut paths = Vec::new();
        // KUBECONFIG env var supports colon-separated (Unix) or semicolon-separated
        // (Windows) lists of paths per K8s spec.
        if let Ok(kubeconfig_env) = std::env::var("KUBECONFIG") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for segment in kubeconfig_env.split(separator) {
                let trimmed = segment.trim();
                if !trimmed.is_empty() {
                    paths.push(PathBuf::from(trimmed));
                }
            }
        }
        // If KUBECONFIG wasn't set or was empty, fall back to default
        if paths.is_empty() {
            if let Ok(default) = default_kubeconfig_path() {
                paths.push(default);
            }
        }
        // Also scan ~/.kube/ directory recursively for additional kubeconfigs
        if let Some(home) = dirs::home_dir() {
            let kube_dir = home.join(".kube");
            if kube_dir.is_dir() {
                paths.push(kube_dir);
            }
        }
        Self { scan_paths: paths }
    }

    /// Add additional directories to scan for kubeconfig files.
    /// Paths are canonicalized and restricted to the user's home directory tree.
    pub fn with_additional_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        let home = dirs::home_dir();
        // If we cannot determine the home directory, reject all additional scan
        // directories to prevent a compromised preferences file from reading
        // arbitrary filesystem locations.
        if home.is_none() {
            tracing::warn!("Cannot determine home directory; rejecting all additional scan dirs");
            return self;
        }
        let home = home.unwrap();
        for dir in dirs {
            // Canonicalize to resolve symlinks / ".." and validate the path exists.
            let canonical = match dir.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Skipping scan dir '{}': {}", dir.display(), e,);
                    continue;
                }
            };
            // Restrict to paths under the user's home directory to prevent
            // a compromised preferences.json from reading arbitrary locations.
            if !canonical.starts_with(&home) {
                tracing::warn!("Skipping scan dir '{}': outside home directory", dir.display(),);
                continue;
            }
            self.scan_paths.push(canonical);
        }
        self
    }

    /// Discover all kubeconfig file paths from configured scan paths.
    /// - If a path is a file, check if it's a valid kubeconfig.
    /// - If a path is a directory, recursively scan it (depth 3).
    pub fn discover(&self) -> Result<Vec<PathBuf>> {
        let mut results = Vec::new();

        for path in &self.scan_paths {
            if path.is_file() {
                if is_kubeconfig_file(path).unwrap_or(false) {
                    results.push(path.clone());
                }
            } else if path.is_dir() {
                if let Ok(found) = scan_directory_for_kubeconfigs(path) {
                    results.extend(found);
                }
            }
        }

        // Deduplicate by canonical path
        results.sort();
        results.dedup();
        Ok(results)
    }

    /// Discover and load all kubeconfig files, returning path + loader pairs.
    pub fn load_all(&self) -> Result<Vec<(PathBuf, KubeconfigLoader)>> {
        let paths = self.discover()?;
        let mut loaded = Vec::new();

        for path in paths {
            match KubeconfigLoader::load_from_path(&path) {
                Ok(loader) => loaded.push((path, loader)),
                Err(e) => {
                    tracing::warn!("Skipping invalid kubeconfig {}: {}", path.display(), e);
                }
            }
        }

        Ok(loaded)
    }
}

/// Check if a file looks like a valid kubeconfig using a YAML heuristic.
/// Returns true if the file contains `kind: Config` or all of `clusters`, `contexts`, `apiVersion`.
/// Skips files larger than 1 MB to avoid excessive memory usage on non-kubeconfig files.
pub fn is_kubeconfig_file(path: &std::path::Path) -> Result<bool> {
    // Guard: skip oversized files (kubeconfigs are typically <100KB).
    const MAX_KUBECONFIG_SIZE: u64 = 1_048_576; // 1 MB
    let metadata =
        std::fs::metadata(path).with_context(|| format!("Failed to stat {}", path.display()))?;
    if metadata.len() > MAX_KUBECONFIG_SIZE {
        return Ok(false);
    }

    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // Quick heuristic: check for kubeconfig markers without full YAML parse
    let has_kind_config = contents.contains("kind: Config") || contents.contains("kind:Config");
    let has_clusters = contents.contains("clusters:");
    let has_contexts = contents.contains("contexts:");
    let has_api_version = contents.contains("apiVersion:");

    Ok(has_kind_config || (has_clusters && has_contexts && has_api_version))
}

/// Recursively scan a directory for kubeconfig files, up to `max_depth` levels.
/// Skips hidden directories (starting with '.').
pub fn scan_directory_for_kubeconfigs(dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    scan_directory_recursive(dir, 5)
}

fn scan_directory_recursive(dir: &std::path::Path, remaining_depth: u32) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();

    if remaining_depth == 0 {
        return Ok(results);
    }

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }

        // Use symlink_metadata to avoid following symlinks, which could cause
        // cycles or read files outside the intended scan scope.
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            if let Ok(found) = scan_directory_recursive(&path, remaining_depth - 1) {
                results.extend(found);
            }
        } else if metadata.is_file() {
            // Check files with no extension, .yaml, .yml, or named "config"
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_candidate = ext.is_empty()
                || ext == "yaml"
                || ext == "yml"
                || ext == "conf"
                || name == "config";

            if is_candidate && is_kubeconfig_file(&path).unwrap_or(false) {
                results.push(path);
            }
        }
    }

    Ok(results)
}

pub fn default_kubeconfig_path() -> Result<PathBuf> {
    // Note: KUBECONFIG with multiple colon-separated paths is handled in
    // KubeconfigDiscovery::new(). This function returns the single default path.
    if let Ok(val) = std::env::var("KUBECONFIG") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        // Return just the first path for callers that expect a single file
        if let Some(first) = val.split(separator).next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }
    }
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".kube").join("config"))
}

// ---------------------------------------------------------------------------
// Kubeconfig Diff (T328b — FR-076)
// ---------------------------------------------------------------------------

/// Represents the difference between two kubeconfig snapshots.
#[derive(Debug, Clone, Default)]
pub struct KubeconfigDiff {
    /// Context names that were added (present in new, absent in old).
    pub added: Vec<String>,
    /// Context names that were removed (present in old, absent in new).
    pub removed: Vec<String>,
    /// Context names where the cluster or user changed.
    pub modified: Vec<String>,
}

impl KubeconfigDiff {
    /// Returns true if no changes were detected.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Compare two snapshots of kubeconfig contexts and produce a diff.
pub fn diff_contexts(old: &[KubeContext], new: &[KubeContext]) -> KubeconfigDiff {
    use std::collections::HashMap;

    let old_map: HashMap<&str, &KubeContext> = old.iter().map(|c| (c.name.as_str(), c)).collect();
    let new_map: HashMap<&str, &KubeContext> = new.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut diff = KubeconfigDiff::default();

    // Added contexts: in new but not old.
    for name in new_map.keys() {
        if !old_map.contains_key(name) {
            diff.added.push(name.to_string());
        }
    }

    // Removed contexts: in old but not new.
    for name in old_map.keys() {
        if !new_map.contains_key(name) {
            diff.removed.push(name.to_string());
        }
    }

    // Modified contexts: same name but different cluster/user/api_server.
    for (name, old_ctx) in &old_map {
        if let Some(new_ctx) = new_map.get(name) {
            let changed = old_ctx.cluster_name != new_ctx.cluster_name
                || old_ctx.user_name != new_ctx.user_name
                || old_ctx.api_server_url != new_ctx.api_server_url
                || old_ctx.namespace != new_ctx.namespace;
            if changed {
                diff.modified.push(name.to_string());
            }
        }
    }

    // Sort for deterministic output.
    diff.added.sort();
    diff.removed.sort();
    diff.modified.sort();
    diff
}

// ---------------------------------------------------------------------------
// Kubeconfig Directory Watcher (T328b — FR-076)
// ---------------------------------------------------------------------------

/// Watches kubeconfig scan directories for filesystem changes.
///
/// When a file is created, modified, or removed in any watched directory,
/// the watcher re-runs discovery, diffs against the previous snapshot,
/// and sends the diff through a channel.
pub struct KubeconfigWatcher {
    /// The underlying notify watcher handle. Dropping stops watching.
    _watcher: notify::RecommendedWatcher,
}

impl KubeconfigWatcher {
    /// Start watching the given scan paths for kubeconfig changes.
    ///
    /// `sender` receives [`KubeconfigDiff`] values whenever the set of
    /// discovered contexts changes. The watcher runs on a background thread
    /// managed by the `notify` crate.
    ///
    /// The returned struct must be kept alive — dropping it stops the watch.
    pub fn start(
        scan_paths: Vec<PathBuf>,
        sender: tokio::sync::mpsc::UnboundedSender<KubeconfigDiff>,
    ) -> Result<Self> {
        use notify::{EventKind, RecursiveMode, Watcher};
        use std::sync::{Arc, Mutex};

        // Take an initial snapshot of contexts.
        let discovery = KubeconfigDiscovery { scan_paths: scan_paths.clone() };
        let initial = Self::snapshot(&discovery);
        let prev_snapshot = Arc::new(Mutex::new(initial));

        let paths_for_handler = scan_paths.clone();
        let mut watcher = notify::recommended_watcher(
            move |event_result: std::result::Result<notify::Event, notify::Error>| {
                let Ok(event) = event_result else { return };

                // Only react to create/modify/remove events.
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
                    _ => return,
                }

                // Re-run discovery.
                let disc = KubeconfigDiscovery { scan_paths: paths_for_handler.clone() };
                let new_snapshot = Self::snapshot(&disc);

                // Diff against previous.
                let mut prev = prev_snapshot.lock().unwrap_or_else(|e| e.into_inner());
                let diff = diff_contexts(&prev, &new_snapshot);
                if !diff.is_empty() {
                    let _ = sender.send(diff);
                }
                *prev = new_snapshot;
            },
        )?;

        // Watch each scan path.
        for path in &scan_paths {
            if path.exists() {
                let mode = if path.is_dir() {
                    RecursiveMode::Recursive
                } else {
                    // Watch the parent directory for file-level changes.
                    RecursiveMode::NonRecursive
                };
                let watch_path = if path.is_file() { path.parent().unwrap_or(path) } else { path };
                watcher.watch(watch_path, mode)?;
            }
        }

        Ok(Self { _watcher: watcher })
    }

    /// Collect all context names from a discovery scan.
    fn snapshot(discovery: &KubeconfigDiscovery) -> Vec<KubeContext> {
        discovery
            .load_all()
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(_path, loader)| loader.into_contexts())
            .collect()
    }
}

impl KubeconfigLoader {
    /// Consume the loader and return its contexts.
    pub fn into_contexts(self) -> Vec<KubeContext> {
        self.contexts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: prod-context
clusters:
- name: prod-cluster
  cluster:
    server: https://prod.example.com:6443
    certificate-authority-data: LS0tLS1CRUdJTg==
- name: dev-cluster
  cluster:
    server: https://dev.example.com:6443
contexts:
- name: prod-context
  context:
    cluster: prod-cluster
    user: prod-user
    namespace: production
- name: dev-context
  context:
    cluster: dev-cluster
    user: dev-user
users:
- name: prod-user
  user:
    client-certificate-data: LS0tLS1CRUdJTg==
    client-key-data: LS0tLS1CRUdJTg==
- name: dev-user
  user:
    token: test-synthetic-token-not-real
"#;

    const OIDC_KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: oidc-context
clusters:
- name: oidc-cluster
  cluster:
    server: https://oidc.example.com:6443
contexts:
- name: oidc-context
  context:
    cluster: oidc-cluster
    user: oidc-user
users:
- name: oidc-user
  user:
    auth-provider:
      name: oidc
      config:
        idp-issuer-url: https://accounts.google.com
"#;

    const EXEC_KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: eks-context
clusters:
- name: eks-cluster
  cluster:
    server: https://eks.amazonaws.com
contexts:
- name: eks-context
  context:
    cluster: eks-cluster
    user: eks-user
users:
- name: eks-user
  user:
    exec:
      apiVersion: client.authentication.k8s.io/v1beta1
      command: aws-iam-authenticator
      args:
        - token
        - -i
        - my-cluster
"#;

    #[test]
    fn test_parse_kubeconfig_contexts() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();

        assert_eq!(loader.contexts().len(), 2);
        assert_eq!(loader.current_context(), Some("prod-context"));
    }

    #[test]
    fn test_context_names() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();
        let names = loader.context_names();

        assert!(names.contains(&"prod-context"));
        assert!(names.contains(&"dev-context"));
    }

    #[test]
    fn test_find_context() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();

        let prod = loader.find_context("prod-context").unwrap();
        assert_eq!(prod.cluster_name, "prod-cluster");
        assert_eq!(prod.user_name, "prod-user");
        assert_eq!(prod.namespace.as_deref(), Some("production"));
        assert_eq!(prod.api_server_url.as_deref(), Some("https://prod.example.com:6443"));

        let dev = loader.find_context("dev-context").unwrap();
        assert_eq!(dev.cluster_name, "dev-cluster");
        assert!(dev.namespace.is_none());

        assert!(loader.find_context("nonexistent").is_none());
    }

    #[test]
    fn test_certificate_auth_detection() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();
        let prod = loader.find_context("prod-context").unwrap();
        assert_eq!(prod.auth_method, AuthMethod::Certificate);
    }

    #[test]
    fn test_token_auth_detection() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();
        let dev = loader.find_context("dev-context").unwrap();
        assert_eq!(dev.auth_method, AuthMethod::Token);
    }

    #[test]
    fn test_oidc_auth_detection() {
        let loader = KubeconfigLoader::load_from_str(OIDC_KUBECONFIG).unwrap();
        let ctx = loader.find_context("oidc-context").unwrap();
        assert_eq!(ctx.auth_method, AuthMethod::OIDC);
    }

    #[test]
    fn test_exec_plugin_auth_detection() {
        let loader = KubeconfigLoader::load_from_str(EXEC_KUBECONFIG).unwrap();
        let ctx = loader.find_context("eks-context").unwrap();
        assert_eq!(ctx.auth_method, AuthMethod::ExecPlugin);
    }

    #[test]
    fn test_empty_kubeconfig() {
        let yaml = "apiVersion: v1\nkind: Config\n";
        let loader = KubeconfigLoader::load_from_str(yaml).unwrap();
        assert!(loader.contexts().is_empty());
        assert!(loader.current_context().is_none());
    }

    #[test]
    fn test_default_kubeconfig_path() {
        let path = default_kubeconfig_path().unwrap();
        assert!(path.to_string_lossy().contains(".kube"));
        assert!(path.to_string_lossy().ends_with("config"));
    }

    #[test]
    fn test_invalid_yaml_returns_error() {
        let result = KubeconfigLoader::load_from_str("{{invalid yaml");
        assert!(result.is_err());
    }

    // --- T203: KubeconfigDiscovery tests ---

    #[test]
    fn test_is_kubeconfig_file_with_kind_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "apiVersion: v1\nkind: Config\nclusters: []\ncontexts: []\n")
            .unwrap();
        assert!(is_kubeconfig_file(&path).unwrap());
    }

    #[test]
    fn test_is_kubeconfig_file_with_markers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubeconfig");
        std::fs::write(&path, "apiVersion: v1\nclusters:\n- name: test\ncontexts:\n- name: ctx\n")
            .unwrap();
        assert!(is_kubeconfig_file(&path).unwrap());
    }

    #[test]
    fn test_is_kubeconfig_file_not_kubeconfig() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("random.yaml");
        std::fs::write(&path, "foo: bar\nbaz: 42\n").unwrap();
        assert!(!is_kubeconfig_file(&path).unwrap());
    }

    #[test]
    fn test_scan_directory_finds_kubeconfigs() {
        let dir = tempfile::tempdir().unwrap();

        // Valid kubeconfig
        let config = dir.path().join("config");
        std::fs::write(&config, SAMPLE_KUBECONFIG).unwrap();

        // Non-kubeconfig yaml
        let other = dir.path().join("other.yaml");
        std::fs::write(&other, "not: a kubeconfig\n").unwrap();

        // Nested valid kubeconfig
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        let nested = sub.join("staging.yaml");
        std::fs::write(&nested, SAMPLE_KUBECONFIG).unwrap();

        let found = scan_directory_for_kubeconfigs(dir.path()).unwrap();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_scan_directory_skips_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".hidden");
        std::fs::create_dir(&hidden).unwrap();
        let config = hidden.join("config");
        std::fs::write(&config, SAMPLE_KUBECONFIG).unwrap();

        let found = scan_directory_for_kubeconfigs(dir.path()).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn test_scan_directory_respects_depth() {
        let dir = tempfile::tempdir().unwrap();
        // remaining_depth=5 means: scan dir (5), l1 (4), l2 (3), l3 (2), l4 (1), l5 (0=stop)
        // So files inside l4 are found, but l5 is never entered.
        let l1 = dir.path().join("l1");
        let l2 = l1.join("l2");
        let l3 = l2.join("l3");
        let l4 = l3.join("l4");
        let l5 = l4.join("l5");
        std::fs::create_dir_all(&l5).unwrap();

        // Put kubeconfig at depth 4 (l1/l2/l3/l4/config) — should be found
        std::fs::write(l4.join("config"), SAMPLE_KUBECONFIG).unwrap();
        // Put kubeconfig at depth 5 (l1/l2/l3/l4/l5/config) — should NOT be found (depth exhausted)
        std::fs::write(l5.join("config"), SAMPLE_KUBECONFIG).unwrap();

        let found = scan_directory_for_kubeconfigs(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
        // Use path components check instead of string contains for cross-platform compatibility.
        let path_str = found[0].to_string_lossy();
        assert!(path_str.contains("l4") && path_str.contains("config"));
    }

    #[test]
    fn test_discovery_new_has_default_path() {
        let discovery = KubeconfigDiscovery::new();
        assert!(!discovery.scan_paths.is_empty());
    }

    #[test]
    fn test_discovery_new_includes_kube_directory() {
        // KubeconfigDiscovery::new() should include ~/.kube/ as a directory scan path
        // (in addition to the default config file).
        let discovery = KubeconfigDiscovery::new();
        if let Some(home) = dirs::home_dir() {
            let kube_dir = home.join(".kube");
            if kube_dir.is_dir() {
                assert!(
                    discovery.scan_paths.contains(&kube_dir),
                    "scan_paths should include ~/.kube/ directory: {:?}",
                    discovery.scan_paths,
                );
            }
        }
    }

    #[test]
    fn test_scan_finds_nested_kubeconfigs() {
        let dir = tempfile::tempdir().unwrap();
        // Simulate a ~/.kube-like structure with nested kubeconfigs
        let clusters_dir = dir.path().join("clusters").join("prod");
        std::fs::create_dir_all(&clusters_dir).unwrap();
        std::fs::write(dir.path().join("config"), SAMPLE_KUBECONFIG).unwrap();
        std::fs::write(clusters_dir.join("config"), EXEC_KUBECONFIG).unwrap();

        let found = scan_directory_for_kubeconfigs(dir.path()).unwrap();
        assert_eq!(found.len(), 2, "Should find both config and clusters/prod/config");
    }

    #[test]
    fn test_discovery_deduplicates_file_and_dir() {
        // When a file path and its parent dir are both in scan_paths,
        // the file should only appear once in results.
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, SAMPLE_KUBECONFIG).unwrap();

        let discovery =
            KubeconfigDiscovery { scan_paths: vec![config.clone(), dir.path().to_path_buf()] };
        let found = discovery.discover().unwrap();
        // The file should appear exactly once (dedup by sort+dedup)
        let config_count = found.iter().filter(|p| p.ends_with("config")).count();
        assert_eq!(config_count, 1, "config should appear once after dedup");
    }

    #[test]
    fn test_discovery_with_additional_dirs() {
        // Use a temp dir under $HOME so it passes the home-directory security check.
        // If $HOME is unavailable, with_additional_dirs rejects all dirs (by design),
        // so we just verify no panic occurs.
        let home = dirs::home_dir();
        if let Some(ref home) = home {
            let test_dir = home.join(".kube");
            // .kube is very likely to exist; if it does, it should be accepted.
            if test_dir.exists() {
                let base_count = KubeconfigDiscovery::new().scan_paths.len();
                let discovery = KubeconfigDiscovery::new().with_additional_dirs(vec![test_dir]);
                assert!(discovery.scan_paths.len() >= base_count);
            }
        } else {
            // When HOME is unavailable, all additional dirs should be rejected.
            let discovery = KubeconfigDiscovery::new()
                .with_additional_dirs(vec![PathBuf::from("/tmp/extra-kube")]);
            // Should not have grown beyond defaults
            let base_count = KubeconfigDiscovery::new().scan_paths.len();
            assert_eq!(discovery.scan_paths.len(), base_count);
        }
    }

    #[test]
    fn test_discovery_load_all_with_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, SAMPLE_KUBECONFIG).unwrap();

        let discovery = KubeconfigDiscovery { scan_paths: vec![config] };
        let loaded = discovery.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1.contexts().len(), 2);
    }

    // --- T328b: KubeconfigDiff tests ---

    fn make_context(name: &str, cluster: &str, user: &str, server: Option<&str>) -> KubeContext {
        KubeContext {
            name: name.to_string(),
            cluster_name: cluster.to_string(),
            user_name: user.to_string(),
            namespace: None,
            api_server_url: server.map(|s| s.to_string()),
            auth_method: AuthMethod::Token,
        }
    }

    #[test]
    fn test_diff_no_changes() {
        let old = vec![
            make_context("prod", "prod-cluster", "admin", Some("https://prod:6443")),
            make_context("dev", "dev-cluster", "dev-user", Some("https://dev:6443")),
        ];
        let new = old.clone();
        let diff = diff_contexts(&old, &new);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_diff_added_contexts() {
        let old = vec![make_context("prod", "pc", "pu", None)];
        let new =
            vec![make_context("prod", "pc", "pu", None), make_context("staging", "sc", "su", None)];
        let diff = diff_contexts(&old, &new);
        assert_eq!(diff.added, vec!["staging"]);
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_removed_contexts() {
        let old =
            vec![make_context("prod", "pc", "pu", None), make_context("dev", "dc", "du", None)];
        let new = vec![make_context("prod", "pc", "pu", None)];
        let diff = diff_contexts(&old, &new);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["dev"]);
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_modified_cluster_name() {
        let old = vec![make_context("prod", "old-cluster", "pu", None)];
        let new = vec![make_context("prod", "new-cluster", "pu", None)];
        let diff = diff_contexts(&old, &new);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified, vec!["prod"]);
    }

    #[test]
    fn test_diff_modified_server_url() {
        let old = vec![make_context("prod", "pc", "pu", Some("https://old:6443"))];
        let new = vec![make_context("prod", "pc", "pu", Some("https://new:6443"))];
        let diff = diff_contexts(&old, &new);
        assert_eq!(diff.modified, vec!["prod"]);
    }

    #[test]
    fn test_diff_modified_user() {
        let old = vec![make_context("prod", "pc", "old-user", None)];
        let new = vec![make_context("prod", "pc", "new-user", None)];
        let diff = diff_contexts(&old, &new);
        assert_eq!(diff.modified, vec!["prod"]);
    }

    #[test]
    fn test_diff_modified_namespace() {
        let old = vec![make_context("prod", "pc", "pu", None)];
        let mut new_ctx = make_context("prod", "pc", "pu", None);
        new_ctx.namespace = Some("kube-system".to_string());
        let diff = diff_contexts(&old, &[new_ctx]);
        assert_eq!(diff.modified, vec!["prod"]);
    }

    #[test]
    fn test_diff_complex_changes() {
        let old = vec![
            make_context("alpha", "ac", "au", None),
            make_context("beta", "bc", "bu", None),
            make_context("gamma", "gc", "gu", None),
        ];
        let new = vec![
            make_context("alpha", "ac", "au", None),    // unchanged
            make_context("beta", "bc-new", "bu", None), // modified
            make_context("delta", "dc", "du", None),    // added
                                                        // gamma removed
        ];
        let diff = diff_contexts(&old, &new);
        assert_eq!(diff.added, vec!["delta"]);
        assert_eq!(diff.removed, vec!["gamma"]);
        assert_eq!(diff.modified, vec!["beta"]);
    }

    #[test]
    fn test_diff_empty_to_full() {
        let old: Vec<KubeContext> = vec![];
        let new = vec![make_context("prod", "pc", "pu", None)];
        let diff = diff_contexts(&old, &new);
        assert_eq!(diff.added, vec!["prod"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_full_to_empty() {
        let old = vec![make_context("prod", "pc", "pu", None)];
        let new: Vec<KubeContext> = vec![];
        let diff = diff_contexts(&old, &new);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["prod"]);
    }

    #[test]
    fn test_diff_is_empty_method() {
        let diff = KubeconfigDiff::default();
        assert!(diff.is_empty());

        let diff_with_added = KubeconfigDiff { added: vec!["x".into()], ..Default::default() };
        assert!(!diff_with_added.is_empty());
    }

    #[test]
    fn test_into_contexts() {
        let loader = KubeconfigLoader::load_from_str(SAMPLE_KUBECONFIG).unwrap();
        let contexts = loader.into_contexts();
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].name, "prod-context");
        assert_eq!(contexts[1].name, "dev-context");
    }

    #[test]
    fn test_watcher_start_with_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, SAMPLE_KUBECONFIG).unwrap();

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let watcher = KubeconfigWatcher::start(vec![dir.path().to_path_buf()], tx);
        assert!(watcher.is_ok());
        // Watcher is dropped here, stopping the watch.
    }

    #[test]
    fn test_watcher_start_nonexistent_path_still_ok() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        // Nonexistent paths are silently skipped (they just don't get watched).
        let watcher = KubeconfigWatcher::start(vec![PathBuf::from("/nonexistent/path")], tx);
        assert!(watcher.is_ok());
    }
}
