pub mod app_shell;
pub mod command_palette;
pub mod dock;
pub mod header;
pub mod indent_guides;
pub mod sidebar;
pub mod workspace;

use crate::icons::ResourceCategory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    ClusterList,
    Dashboard {
        cluster_context: String,
    },
    ResourceList {
        cluster_context: String,
        category: ResourceCategory,
        kind: String,
    },
    ResourceDetail {
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    },
    HelmReleases {
        cluster_context: String,
    },
    HelmInstall {
        cluster_context: String,
    },
    CrdBrowser {
        cluster_context: String,
    },
    NamespaceMap {
        cluster_context: String,
    },
    PluginManager {
        cluster_context: String,
    },
    ClusterSettings {
        cluster_context: String,
    },
    ClusterTopology {
        cluster_context: String,
    },
    Preferences,
}

impl NavigationTarget {
    /// Returns the display label for this target, prefixed with the cluster name
    /// for cluster-scoped targets (FR-057).
    pub fn label(&self) -> String {
        match self {
            Self::ClusterList => "Clusters".to_string(),
            Self::Dashboard { cluster_context } => format!("{cluster_context} - Overview"),
            Self::ResourceList { cluster_context, kind, .. } => {
                format!("{cluster_context} - {kind}")
            }
            Self::ResourceDetail { cluster_context, kind, name, .. } => {
                format!("{cluster_context} - {kind}/{name}")
            }
            Self::HelmReleases { cluster_context } => {
                format!("{cluster_context} - Helm Releases")
            }
            Self::HelmInstall { cluster_context } => {
                format!("{cluster_context} - Install Chart")
            }
            Self::CrdBrowser { cluster_context } => {
                format!("{cluster_context} - Custom Resources")
            }
            Self::NamespaceMap { cluster_context } => {
                format!("{cluster_context} - Resource Map")
            }
            Self::PluginManager { cluster_context } => {
                format!("{cluster_context} - Plugins")
            }
            Self::ClusterSettings { cluster_context } => {
                format!("Settings - {cluster_context}")
            }
            Self::ClusterTopology { cluster_context } => {
                format!("{cluster_context} - Topology")
            }
            Self::Preferences => "Preferences".to_string(),
        }
    }

    /// T362: Returns a unique key for error tracking in the `view_errors` map.
    /// Format examples:
    ///   - "dashboard:my-cluster"
    ///   - "resources:my-cluster:Pod"
    ///   - "detail:my-cluster:Pod:my-pod:default"
    ///   - "events:my-cluster"
    pub fn view_error_key(&self) -> String {
        match self {
            Self::ClusterList => "cluster-list".to_string(),
            Self::Dashboard { cluster_context } => {
                format!("dashboard:{cluster_context}")
            }
            Self::ResourceList { cluster_context, kind, .. } => {
                format!("resources:{cluster_context}:{kind}")
            }
            Self::ResourceDetail { cluster_context, kind, name, namespace } => {
                let ns = namespace.as_deref().unwrap_or("_");
                format!("detail:{cluster_context}:{kind}:{name}:{ns}")
            }
            Self::HelmReleases { cluster_context } => {
                format!("helm-releases:{cluster_context}")
            }
            Self::HelmInstall { cluster_context } => {
                format!("helm-install:{cluster_context}")
            }
            Self::CrdBrowser { cluster_context } => {
                format!("crd-browser:{cluster_context}")
            }
            Self::NamespaceMap { cluster_context } => {
                format!("namespace-map:{cluster_context}")
            }
            Self::PluginManager { cluster_context } => {
                format!("plugin-manager:{cluster_context}")
            }
            Self::ClusterSettings { cluster_context } => {
                format!("cluster-settings:{cluster_context}")
            }
            Self::ClusterTopology { cluster_context } => {
                format!("cluster-topology:{cluster_context}")
            }
            Self::Preferences => "preferences".to_string(),
        }
    }

    /// Returns the cluster context for cluster-scoped targets, or None for ClusterList.
    pub fn cluster_context(&self) -> Option<&str> {
        match self {
            Self::ClusterList | Self::Preferences => None,
            Self::Dashboard { cluster_context }
            | Self::ResourceList { cluster_context, .. }
            | Self::ResourceDetail { cluster_context, .. }
            | Self::HelmReleases { cluster_context }
            | Self::HelmInstall { cluster_context }
            | Self::CrdBrowser { cluster_context }
            | Self::NamespaceMap { cluster_context }
            | Self::PluginManager { cluster_context }
            | Self::ClusterSettings { cluster_context }
            | Self::ClusterTopology { cluster_context } => Some(cluster_context),
        }
    }
}

#[derive(Debug)]
pub struct AppLayout {
    pub sidebar_collapsed: bool,
    pub active_navigation: NavigationTarget,
}

impl Default for AppLayout {
    fn default() -> Self {
        Self { sidebar_collapsed: false, active_navigation: NavigationTarget::ClusterList }
    }
}

impl AppLayout {
    pub fn navigate(&mut self, target: NavigationTarget) {
        self.active_navigation = target;
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let layout = AppLayout::default();
        assert!(!layout.sidebar_collapsed);
        assert_eq!(layout.active_navigation, NavigationTarget::ClusterList);
    }

    #[test]
    fn test_navigation() {
        let mut layout = AppLayout::default();
        layout.navigate(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
        assert_eq!(
            layout.active_navigation,
            NavigationTarget::Dashboard { cluster_context: "prod".to_string() }
        );
    }

    #[test]
    fn test_toggle_sidebar() {
        let mut layout = AppLayout::default();
        assert!(!layout.sidebar_collapsed);

        layout.toggle_sidebar();
        assert!(layout.sidebar_collapsed);

        layout.toggle_sidebar();
        assert!(!layout.sidebar_collapsed);
    }

    // --- T300: Cluster-scoped NavigationTarget tests ---

    #[test]
    fn test_cluster_list_has_no_cluster_context() {
        let target = NavigationTarget::ClusterList;
        assert_eq!(target.cluster_context(), None);
        assert_eq!(target.label(), "Clusters");
    }

    #[test]
    fn test_dashboard_carries_cluster_context() {
        let target = NavigationTarget::Dashboard { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_resource_list_carries_cluster_context() {
        let target = NavigationTarget::ResourceList {
            cluster_context: "staging".to_string(),
            category: ResourceCategory::Workloads,
            kind: "Pod".to_string(),
        };
        assert_eq!(target.cluster_context(), Some("staging"));
    }

    #[test]
    fn test_resource_detail_carries_cluster_context() {
        let target = NavigationTarget::ResourceDetail {
            cluster_context: "dev".to_string(),
            kind: "Pod".to_string(),
            name: "nginx".to_string(),
            namespace: Some("default".to_string()),
        };
        assert_eq!(target.cluster_context(), Some("dev"));
    }

    #[test]
    fn test_helm_releases_carries_cluster_context() {
        let target = NavigationTarget::HelmReleases { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_crd_browser_carries_cluster_context() {
        let target = NavigationTarget::CrdBrowser { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_namespace_map_carries_cluster_context() {
        let target = NavigationTarget::NamespaceMap { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_helm_install_carries_cluster_context() {
        let target = NavigationTarget::HelmInstall { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_plugin_manager_carries_cluster_context() {
        let target = NavigationTarget::PluginManager { cluster_context: "prod".to_string() };
        assert_eq!(target.cluster_context(), Some("prod"));
    }

    #[test]
    fn test_label_cluster_prefixed_format_fr057() {
        // FR-057: Tabs display cluster name prefix "prod - Pods"
        assert_eq!(
            NavigationTarget::Dashboard { cluster_context: "prod".to_string() }.label(),
            "prod - Overview"
        );

        assert_eq!(
            NavigationTarget::ResourceList {
                cluster_context: "staging".to_string(),
                category: ResourceCategory::Workloads,
                kind: "Pods".to_string(),
            }
            .label(),
            "staging - Pods"
        );

        assert_eq!(
            NavigationTarget::ResourceDetail {
                cluster_context: "dev".to_string(),
                kind: "Pod".to_string(),
                name: "nginx".to_string(),
                namespace: Some("default".to_string()),
            }
            .label(),
            "dev - Pod/nginx"
        );

        assert_eq!(
            NavigationTarget::HelmReleases { cluster_context: "prod".to_string() }.label(),
            "prod - Helm Releases"
        );

        assert_eq!(
            NavigationTarget::HelmInstall { cluster_context: "prod".to_string() }.label(),
            "prod - Install Chart"
        );

        assert_eq!(
            NavigationTarget::CrdBrowser { cluster_context: "prod".to_string() }.label(),
            "prod - Custom Resources"
        );

        assert_eq!(
            NavigationTarget::NamespaceMap { cluster_context: "prod".to_string() }.label(),
            "prod - Resource Map"
        );

        assert_eq!(
            NavigationTarget::PluginManager { cluster_context: "prod".to_string() }.label(),
            "prod - Plugins"
        );
    }
}
