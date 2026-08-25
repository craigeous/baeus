use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

// --- T134: Keyboard Shortcut System ---
// These types will be wired into GPUI event handlers when the UI framework is integrated.

/// An action that can be triggered by a keyboard shortcut.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyAction {
    ToggleCommandPalette,
    NavigateToDashboard,
    NavigateToClusterList,
    NavigateToPods,
    NavigateToDeployments,
    NavigateToServices,
    NavigateToEvents,
    NavigateToHelmReleases,
    ToggleSidebar,
    NextTab,
    PrevTab,
    CloseTab,
    Refresh,
    FocusSearch,
}

/// Modifier keys that can be combined with a key press.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct KeyModifiers {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[allow(dead_code)]
impl KeyModifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn cmd() -> Self {
        Self { cmd: true, ..Self::default() }
    }

    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Self::default() }
    }

    pub fn cmd_shift() -> Self {
        Self { cmd: true, shift: true, ..Self::default() }
    }
}

/// A single key binding mapping a key + modifiers to an action.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub modifiers: KeyModifiers,
    pub action: KeyAction,
}

/// Configuration holding all keybindings with lookup support.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingConfig {
    pub bindings: Vec<KeyBinding>,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self::default_bindings()
    }
}

#[allow(dead_code)]
impl KeybindingConfig {
    /// Look up the action for a given key and modifier combination.
    /// Returns the first matching action, or `None` if no binding matches.
    pub fn find_action(&self, key: &str, modifiers: &KeyModifiers) -> Option<KeyAction> {
        self.bindings.iter().find(|b| b.key == key && b.modifiers == *modifiers).map(|b| b.action)
    }

    /// Return the default set of keybindings for the application.
    pub fn default_bindings() -> Self {
        Self {
            bindings: vec![
                KeyBinding {
                    key: "k".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::ToggleCommandPalette,
                },
                KeyBinding {
                    key: "1".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToDashboard,
                },
                KeyBinding {
                    key: "2".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToClusterList,
                },
                KeyBinding {
                    key: "3".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToPods,
                },
                KeyBinding {
                    key: "4".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToDeployments,
                },
                KeyBinding {
                    key: "5".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToServices,
                },
                KeyBinding {
                    key: "6".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToEvents,
                },
                KeyBinding {
                    key: "7".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToHelmReleases,
                },
                KeyBinding {
                    key: "b".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::ToggleSidebar,
                },
                KeyBinding {
                    key: "Tab".to_string(),
                    modifiers: KeyModifiers::ctrl(),
                    action: KeyAction::NextTab,
                },
                KeyBinding {
                    key: "Tab".to_string(),
                    modifiers: KeyModifiers { ctrl: true, shift: true, ..KeyModifiers::default() },
                    action: KeyAction::PrevTab,
                },
                KeyBinding {
                    key: "w".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::CloseTab,
                },
                KeyBinding {
                    key: "r".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::Refresh,
                },
                KeyBinding {
                    key: "f".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::FocusSearch,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// Per-cluster appearance overrides (custom icon color, custom icon image).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterAppearance {
    /// Override for the auto-generated palette color (RGB u32).
    pub custom_color: Option<u32>,
    /// Path to a custom icon image file.
    pub custom_icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub theme: Theme,
    pub default_namespace: Option<String>,
    pub favorite_clusters: Vec<String>,
    pub keybindings: BTreeMap<String, String>,
    pub log_line_limit: u32,
    pub font_size: f32,
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub kubeconfig_scan_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub terminal_shell_path: Option<String>,
    #[serde(default)]
    pub cluster_appearances: HashMap<String, ClusterAppearance>,
    #[serde(default)]
    pub default_aws_profile: Option<String>,
    #[serde(default)]
    pub cluster_aws_profiles: HashMap<String, String>,
    /// Saved EKS connections for reconnecting on app restart.
    /// Only metadata is persisted — never secrets (access keys, tokens).
    #[serde(default)]
    pub saved_eks_connections: Vec<SavedEksConnection>,
}

/// Persisted metadata for an EKS cluster connected via native AWS integration.
/// No secrets are stored — only enough to identify the cluster and re-authenticate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedEksConnection {
    pub cluster_name: String,
    pub cluster_arn: String,
    pub endpoint: String,
    pub region: String,
    pub certificate_authority_data: Option<String>,
    pub auth_method: SavedEksAuthMethod,
    /// SSO start URL (used for re-auth prompt).
    pub sso_start_url: Option<String>,
    /// SSO region (used for re-auth).
    pub sso_region: Option<String>,
    /// IAM role ARN to assume for this cluster (if any).
    #[serde(default)]
    pub role_arn: Option<String>,
}

/// Which auth method was used (for re-auth guidance, not credential storage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SavedEksAuthMethod {
    Sso,
    AccessKey,
    AssumeRole,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            default_namespace: None,
            favorite_clusters: Vec::new(),
            keybindings: BTreeMap::new(),
            log_line_limit: 10000,
            font_size: 13.0,
            sidebar_collapsed: false,
            kubeconfig_scan_dirs: Vec::new(),
            terminal_shell_path: None,
            cluster_appearances: HashMap::new(),
            default_aws_profile: None,
            cluster_aws_profiles: HashMap::new(),
            saved_eks_connections: Vec::new(),
        }
    }
}

impl UserPreferences {
    pub fn config_path() -> Result<PathBuf> {
        let baeus_dir =
            dirs::home_dir().context("Could not determine home directory")?.join(".baeus");
        Ok(baeus_dir.join("preferences.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            // Migrate from old location if it exists
            if let Some(old_path) = Self::legacy_config_path() {
                if old_path.exists() {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::copy(&old_path, &path).is_ok() {
                        let _ = std::fs::remove_file(&old_path);
                        tracing::info!(
                            "Migrated preferences from {} to {}",
                            old_path.display(),
                            path.display(),
                        );
                    }
                }
            }
            if !path.exists() {
                return Ok(Self::default());
            }
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read preferences from {}", path.display()))?;
        let mut prefs: Self =
            serde_json::from_str(&contents).with_context(|| "Failed to parse preferences JSON")?;
        prefs.sanitize_paths();
        Ok(prefs)
    }

    /// Legacy preferences path (~/Library/Application Support/baeus/preferences.json).
    fn legacy_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("baeus").join("preferences.json"))
    }

    /// Validate and remove unsafe paths from preferences (path traversal prevention).
    fn sanitize_paths(&mut self) {
        let home = dirs::home_dir();
        self.cluster_appearances.retain(|cluster, appearance| {
            if let Some(ref icon_path) = appearance.custom_icon_path {
                let path = std::path::Path::new(icon_path);
                // Reject relative paths and path traversal
                if !path.is_absolute() || icon_path.contains("..") {
                    tracing::warn!(
                        "Rejecting custom icon path for '{}': not absolute or contains '..'",
                        cluster,
                    );
                    return false;
                }
                // Must be under home directory
                if let Some(ref home) = home {
                    if let Ok(canonical) = path.canonicalize() {
                        if !canonical.starts_with(home) {
                            tracing::warn!(
                                "Rejecting custom icon path for '{}': outside home directory",
                                cluster,
                            );
                            return false;
                        }
                    }
                }
            }
            true
        });
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory {}", parent.display())
            })?;
            // Restrict directory permissions to owner-only on Unix.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        let contents =
            serde_json::to_string_pretty(self).context("Failed to serialize preferences")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write preferences to {}", path.display()))?;
        // Restrict file permissions to owner read/write only on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.clamp(10.0, 24.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = UserPreferences::default();

        assert_eq!(prefs.theme, Theme::System);
        assert!(prefs.default_namespace.is_none());
        assert!(prefs.favorite_clusters.is_empty());
        assert!(prefs.keybindings.is_empty());
        assert_eq!(prefs.log_line_limit, 10000);
        assert_eq!(prefs.font_size, 13.0);
        assert!(!prefs.sidebar_collapsed);
        assert!(prefs.kubeconfig_scan_dirs.is_empty());
    }

    #[test]
    fn test_theme_serialization() {
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"Light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"Dark\"");
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"System\"");
    }

    #[test]
    fn test_preferences_serialization_roundtrip() {
        let mut prefs = UserPreferences {
            theme: Theme::Dark,
            default_namespace: Some("kube-system".to_string()),
            log_line_limit: 5000,
            font_size: 16.0,
            sidebar_collapsed: true,
            ..Default::default()
        };
        prefs.favorite_clusters.push("prod-us-east".to_string());
        prefs.keybindings.insert("command_palette".to_string(), "Cmd+K".to_string());

        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let deserialized: UserPreferences = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.theme, Theme::Dark);
        assert_eq!(deserialized.default_namespace.as_deref(), Some("kube-system"));
        assert_eq!(deserialized.favorite_clusters, vec!["prod-us-east"]);
        assert_eq!(deserialized.log_line_limit, 5000);
        assert_eq!(deserialized.font_size, 16.0);
        assert!(deserialized.sidebar_collapsed);
    }

    #[test]
    fn test_font_size_clamping() {
        let mut prefs = UserPreferences::default();

        prefs.set_font_size(8.0);
        assert_eq!(prefs.font_size, 10.0);

        prefs.set_font_size(30.0);
        assert_eq!(prefs.font_size, 24.0);

        prefs.set_font_size(16.0);
        assert_eq!(prefs.font_size, 16.0);
    }

    #[test]
    fn test_config_path() {
        let path = UserPreferences::config_path().unwrap();
        assert!(path.to_string_lossy().contains("baeus"));
        assert!(path.to_string_lossy().ends_with("preferences.json"));
    }

    // --- T134: Keyboard Shortcut System ---

    #[test]
    fn test_key_modifiers_constructors() {
        let none = KeyModifiers::none();
        assert!(!none.cmd && !none.ctrl && !none.alt && !none.shift);

        let cmd = KeyModifiers::cmd();
        assert!(cmd.cmd && !cmd.ctrl && !cmd.alt && !cmd.shift);

        let ctrl = KeyModifiers::ctrl();
        assert!(!ctrl.cmd && ctrl.ctrl && !ctrl.alt && !ctrl.shift);

        let cmd_shift = KeyModifiers::cmd_shift();
        assert!(cmd_shift.cmd && !cmd_shift.ctrl && !cmd_shift.alt && cmd_shift.shift);
    }

    #[test]
    fn test_key_action_variants_are_distinct() {
        // Ensure all enum variants are distinct values
        let actions = vec![
            KeyAction::ToggleCommandPalette,
            KeyAction::NavigateToDashboard,
            KeyAction::NavigateToClusterList,
            KeyAction::NavigateToPods,
            KeyAction::NavigateToDeployments,
            KeyAction::NavigateToServices,
            KeyAction::NavigateToEvents,
            KeyAction::NavigateToHelmReleases,
            KeyAction::ToggleSidebar,
            KeyAction::NextTab,
            KeyAction::PrevTab,
            KeyAction::CloseTab,
            KeyAction::Refresh,
            KeyAction::FocusSearch,
        ];
        // All 14 variants should be unique
        for (i, a) in actions.iter().enumerate() {
            for (j, b) in actions.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn test_keybinding_config_default_bindings_not_empty() {
        let config = KeybindingConfig::default_bindings();
        assert!(!config.bindings.is_empty());
        // Should have exactly 14 default bindings (one per action)
        assert_eq!(config.bindings.len(), 14);
    }

    #[test]
    fn test_keybinding_config_find_action_cmd_k() {
        let config = KeybindingConfig::default_bindings();
        let action = config.find_action("k", &KeyModifiers::cmd());
        assert_eq!(action, Some(KeyAction::ToggleCommandPalette));
    }

    #[test]
    fn test_keybinding_config_find_action_navigation() {
        let config = KeybindingConfig::default_bindings();

        assert_eq!(
            config.find_action("1", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToDashboard)
        );
        assert_eq!(
            config.find_action("2", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToClusterList)
        );
        assert_eq!(config.find_action("3", &KeyModifiers::cmd()), Some(KeyAction::NavigateToPods));
        assert_eq!(
            config.find_action("4", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToDeployments)
        );
        assert_eq!(
            config.find_action("5", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToServices)
        );
        assert_eq!(
            config.find_action("6", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToEvents)
        );
        assert_eq!(
            config.find_action("7", &KeyModifiers::cmd()),
            Some(KeyAction::NavigateToHelmReleases)
        );
    }

    #[test]
    fn test_keybinding_config_find_action_sidebar_toggle() {
        let config = KeybindingConfig::default_bindings();
        assert_eq!(config.find_action("b", &KeyModifiers::cmd()), Some(KeyAction::ToggleSidebar));
    }

    #[test]
    fn test_keybinding_config_find_action_tab_switching() {
        let config = KeybindingConfig::default_bindings();

        assert_eq!(config.find_action("Tab", &KeyModifiers::ctrl()), Some(KeyAction::NextTab));

        let ctrl_shift = KeyModifiers { ctrl: true, shift: true, ..KeyModifiers::default() };
        assert_eq!(config.find_action("Tab", &ctrl_shift), Some(KeyAction::PrevTab));
    }

    #[test]
    fn test_keybinding_config_find_action_close_refresh_search() {
        let config = KeybindingConfig::default_bindings();

        assert_eq!(config.find_action("w", &KeyModifiers::cmd()), Some(KeyAction::CloseTab));
        assert_eq!(config.find_action("r", &KeyModifiers::cmd()), Some(KeyAction::Refresh));
        assert_eq!(config.find_action("f", &KeyModifiers::cmd()), Some(KeyAction::FocusSearch));
    }

    #[test]
    fn test_keybinding_config_find_action_no_match() {
        let config = KeybindingConfig::default_bindings();
        // No binding for plain "k" without modifiers
        assert_eq!(config.find_action("k", &KeyModifiers::none()), None);
        // No binding for "z" with cmd
        assert_eq!(config.find_action("z", &KeyModifiers::cmd()), None);
    }

    #[test]
    fn test_keybinding_config_default_trait() {
        let config = KeybindingConfig::default();
        assert_eq!(config.bindings.len(), 14);
        // Default trait should match default_bindings()
        let explicit = KeybindingConfig::default_bindings();
        assert_eq!(config.bindings.len(), explicit.bindings.len());
    }

    #[test]
    fn test_keybinding_serialization_roundtrip() {
        let config = KeybindingConfig::default_bindings();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: KeybindingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bindings.len(), config.bindings.len());
        // Verify first binding round-trips correctly
        assert_eq!(deserialized.bindings[0].key, "k");
        assert_eq!(deserialized.bindings[0].modifiers, KeyModifiers::cmd());
        assert_eq!(deserialized.bindings[0].action, KeyAction::ToggleCommandPalette);
    }

    #[test]
    fn test_key_binding_equality() {
        let b1 = KeyBinding {
            key: "k".to_string(),
            modifiers: KeyModifiers::cmd(),
            action: KeyAction::ToggleCommandPalette,
        };
        let b2 = KeyBinding {
            key: "k".to_string(),
            modifiers: KeyModifiers::cmd(),
            action: KeyAction::ToggleCommandPalette,
        };
        let b3 = KeyBinding {
            key: "j".to_string(),
            modifiers: KeyModifiers::cmd(),
            action: KeyAction::ToggleCommandPalette,
        };
        assert_eq!(b1, b2);
        assert_ne!(b1, b3);
    }

    #[test]
    fn test_kubeconfig_scan_dirs_serialization_roundtrip() {
        let prefs = UserPreferences {
            kubeconfig_scan_dirs: vec![
                PathBuf::from("/home/user/.kube"),
                PathBuf::from("/etc/kubernetes/configs"),
            ],
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let deserialized: UserPreferences = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.kubeconfig_scan_dirs.len(), 2);
        assert_eq!(deserialized.kubeconfig_scan_dirs[0], PathBuf::from("/home/user/.kube"));
        assert_eq!(deserialized.kubeconfig_scan_dirs[1], PathBuf::from("/etc/kubernetes/configs"));
    }

    #[test]
    fn test_kubeconfig_scan_dirs_missing_in_json_defaults_empty() {
        // Simulate loading old preferences JSON that doesn't have the field
        let json = r#"{"theme":"Dark","default_namespace":null,"favorite_clusters":[],"keybindings":{},"log_line_limit":10000,"font_size":13.0,"sidebar_collapsed":false}"#;
        let prefs: UserPreferences = serde_json::from_str(json).unwrap();
        assert!(prefs.kubeconfig_scan_dirs.is_empty());
    }

    #[test]
    fn test_keybinding_config_custom_bindings() {
        let config = KeybindingConfig {
            bindings: vec![KeyBinding {
                key: "p".to_string(),
                modifiers: KeyModifiers::cmd_shift(),
                action: KeyAction::ToggleCommandPalette,
            }],
        };
        // Custom binding should be found
        assert_eq!(
            config.find_action("p", &KeyModifiers::cmd_shift()),
            Some(KeyAction::ToggleCommandPalette)
        );
        // Default binding should NOT be found in custom config
        assert_eq!(config.find_action("k", &KeyModifiers::cmd()), None);
    }
}
