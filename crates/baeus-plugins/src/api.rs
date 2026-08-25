use std::collections::HashMap;

use uuid::Uuid;

use crate::{PluginError, PluginPermission};

/// Where a registered view should appear in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewLocation {
    /// Full-page tab in the workspace
    MainTab,
    /// Collapsible section in the sidebar
    SidebarSection,
    /// Additional tab in resource detail view
    ResourceDetail,
}

/// Registration info for a plugin-provided view.
#[derive(Debug, Clone)]
pub struct ViewRegistration {
    /// Unique view ID within this plugin
    pub id: String,
    /// Display label for tab/navigation
    pub label: String,
    /// Optional icon name from the Baeus icon set
    pub icon: Option<String>,
    /// Where the view appears
    pub location: ViewLocation,
    /// Plugin ID that registered this view
    pub plugin_id: String,
}

/// Registration info for a plugin-provided action.
#[derive(Debug, Clone)]
pub struct ActionRegistration {
    /// Unique action ID
    pub id: String,
    /// Display label in context menu
    pub label: String,
    /// Optional icon name
    pub icon: Option<String>,
    /// Which resource kinds this action applies to ("Pod", "Deployment", "*")
    pub resource_kinds: Vec<String>,
    /// Plugin ID that registered this action
    pub plugin_id: String,
}

/// Represents a Kubernetes resource returned to plugins.
///
/// A simplified view of a resource that plugins can read without
/// accessing internal application state directly.
#[derive(Debug, Clone)]
pub struct PluginResource {
    /// Resource UID from Kubernetes
    pub uid: String,
    /// Resource name
    pub name: String,
    /// Resource namespace (None for cluster-scoped)
    pub namespace: Option<String>,
    /// Resource kind (Pod, Deployment, etc.)
    pub kind: String,
    /// API version (v1, apps/v1, etc.)
    pub api_version: String,
    /// Resource labels
    pub labels: HashMap<String, String>,
}

/// The API surface available to plugins during activation and at runtime.
///
/// PluginContext provides controlled access to the Baeus application
/// capabilities. All operations are mediated by the plugin's declared
/// permissions.
pub struct PluginContext {
    /// The plugin ID this context belongs to
    plugin_id: String,
    /// Permissions granted to this plugin
    permissions: Vec<PluginPermission>,
    /// Registered views
    views: Vec<ViewRegistration>,
    /// Registered actions
    actions: Vec<ActionRegistration>,
    /// Simulated resources for testing/runtime
    resources: HashMap<Uuid, Vec<PluginResource>>,
}

impl PluginContext {
    /// Create a new PluginContext for the given plugin with the specified permissions.
    pub fn new(plugin_id: String, permissions: Vec<PluginPermission>) -> Self {
        Self {
            plugin_id,
            permissions,
            views: Vec::new(),
            actions: Vec::new(),
            resources: HashMap::new(),
        }
    }

    /// Returns the plugin ID this context belongs to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Check if this context has the given permission.
    pub fn has_permission(&self, permission: &PluginPermission) -> bool {
        self.permissions.contains(permission)
    }

    /// Register a custom view in the UI.
    ///
    /// Requires the `RegisterViews` permission.
    pub fn register_view(
        &mut self,
        id: String,
        label: String,
        icon: Option<String>,
        location: ViewLocation,
    ) -> Result<(), PluginError> {
        if !self.has_permission(&PluginPermission::RegisterViews) {
            return Err(PluginError::PermissionDenied(
                "RegisterViews permission required".to_string(),
            ));
        }

        // Check for duplicate view ID
        if self.views.iter().any(|v| v.id == id) {
            return Err(PluginError::InternalError(format!(
                "view with id '{}' already registered",
                id
            )));
        }

        self.views.push(ViewRegistration {
            id,
            label,
            icon,
            location,
            plugin_id: self.plugin_id.clone(),
        });

        Ok(())
    }

    /// Register a custom action for resource context menus.
    ///
    /// Requires the `RegisterActions` permission.
    pub fn register_action(
        &mut self,
        id: String,
        label: String,
        icon: Option<String>,
        resource_kinds: Vec<String>,
    ) -> Result<(), PluginError> {
        if !self.has_permission(&PluginPermission::RegisterActions) {
            return Err(PluginError::PermissionDenied(
                "RegisterActions permission required".to_string(),
            ));
        }

        // Check for duplicate action ID
        if self.actions.iter().any(|a| a.id == id) {
            return Err(PluginError::InternalError(format!(
                "action with id '{}' already registered",
                id
            )));
        }

        self.actions.push(ActionRegistration {
            id,
            label,
            icon,
            resource_kinds,
            plugin_id: self.plugin_id.clone(),
        });

        Ok(())
    }

    /// List resources of a given kind from a cluster.
    ///
    /// Requires the `ReadResources` permission.
    pub fn list_resources(
        &self,
        cluster_id: Uuid,
        kind: &str,
        _api_version: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<PluginResource>, PluginError> {
        if !self.has_permission(&PluginPermission::ReadResources) {
            return Err(PluginError::PermissionDenied(
                "ReadResources permission required".to_string(),
            ));
        }

        let resources = self
            .resources
            .get(&cluster_id)
            .map(|resources| {
                resources
                    .iter()
                    .filter(|r| r.kind == kind)
                    .filter(|r| match namespace {
                        Some(ns) => r.namespace.as_deref() == Some(ns),
                        None => true,
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        Ok(resources)
    }

    /// Get a specific resource by name.
    ///
    /// Requires the `ReadResources` permission.
    pub fn get_resource(
        &self,
        cluster_id: Uuid,
        kind: &str,
        _api_version: &str,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<PluginResource, PluginError> {
        if !self.has_permission(&PluginPermission::ReadResources) {
            return Err(PluginError::PermissionDenied(
                "ReadResources permission required".to_string(),
            ));
        }

        let resource = self
            .resources
            .get(&cluster_id)
            .and_then(|resources| {
                resources.iter().find(|r| {
                    r.kind == kind
                        && r.name == name
                        && match namespace {
                            Some(ns) => r.namespace.as_deref() == Some(ns),
                            None => r.namespace.is_none(),
                        }
                })
            })
            .cloned();

        resource.ok_or_else(|| PluginError::ResourceNotFound(format!("{}/{}", kind, name)))
    }

    /// Start watching resources of a given kind (returns a watch token).
    ///
    /// Requires the `ReadResources` permission.
    pub fn watch_resources(
        &self,
        _cluster_id: Uuid,
        _kind: &str,
        _api_version: &str,
        _namespace: Option<&str>,
    ) -> Result<WatchHandle, PluginError> {
        if !self.has_permission(&PluginPermission::ReadResources) {
            return Err(PluginError::PermissionDenied(
                "ReadResources permission required".to_string(),
            ));
        }

        Ok(WatchHandle { id: Uuid::new_v4(), active: true })
    }

    /// Delete or modify a resource (placeholder for future implementation).
    ///
    /// Requires the `WriteResources` permission.
    pub fn write_resource(
        &self,
        _cluster_id: Uuid,
        _kind: &str,
        _api_version: &str,
        _namespace: Option<&str>,
        _name: &str,
        _data: &serde_json::Value,
    ) -> Result<(), PluginError> {
        if !self.has_permission(&PluginPermission::WriteResources) {
            return Err(PluginError::PermissionDenied(
                "WriteResources permission required".to_string(),
            ));
        }
        // Actual implementation would delegate to kube client.
        Err(PluginError::InternalError("write_resource not yet implemented".to_string()))
    }

    /// Returns all registered views.
    pub fn views(&self) -> &[ViewRegistration] {
        &self.views
    }

    /// Returns all registered actions.
    pub fn actions(&self) -> &[ActionRegistration] {
        &self.actions
    }

    /// Returns actions that apply to a specific resource kind.
    pub fn actions_for_kind(&self, kind: &str) -> Vec<&ActionRegistration> {
        self.actions
            .iter()
            .filter(|a| a.resource_kinds.iter().any(|k| k == kind || k == "*"))
            .collect()
    }

    /// Add test resources (for testing purposes).
    pub fn add_test_resources(&mut self, cluster_id: Uuid, resources: Vec<PluginResource>) {
        self.resources.insert(cluster_id, resources);
    }
}

/// Handle for an active resource watch.
#[derive(Debug, Clone)]
pub struct WatchHandle {
    /// Unique watch identifier
    pub id: Uuid,
    /// Whether the watch is still active
    pub active: bool,
}

impl WatchHandle {
    /// Cancel the watch.
    pub fn cancel(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context_with_all_permissions() -> PluginContext {
        PluginContext::new(
            "io.example.test".to_string(),
            vec![
                PluginPermission::ReadResources,
                PluginPermission::WriteResources,
                PluginPermission::RegisterViews,
                PluginPermission::RegisterActions,
                PluginPermission::RegisterSidebar,
                PluginPermission::NetworkAccess,
            ],
        )
    }

    fn test_context_read_only() -> PluginContext {
        PluginContext::new("io.example.readonly".to_string(), vec![PluginPermission::ReadResources])
    }

    fn test_context_no_permissions() -> PluginContext {
        PluginContext::new("io.example.noperm".to_string(), vec![])
    }

    fn sample_resources() -> Vec<PluginResource> {
        vec![
            PluginResource {
                uid: "pod-1".to_string(),
                name: "nginx".to_string(),
                namespace: Some("default".to_string()),
                kind: "Pod".to_string(),
                api_version: "v1".to_string(),
                labels: HashMap::from([("app".to_string(), "nginx".to_string())]),
            },
            PluginResource {
                uid: "pod-2".to_string(),
                name: "redis".to_string(),
                namespace: Some("default".to_string()),
                kind: "Pod".to_string(),
                api_version: "v1".to_string(),
                labels: HashMap::new(),
            },
            PluginResource {
                uid: "svc-1".to_string(),
                name: "nginx-svc".to_string(),
                namespace: Some("default".to_string()),
                kind: "Service".to_string(),
                api_version: "v1".to_string(),
                labels: HashMap::new(),
            },
            PluginResource {
                uid: "pod-3".to_string(),
                name: "worker".to_string(),
                namespace: Some("production".to_string()),
                kind: "Pod".to_string(),
                api_version: "v1".to_string(),
                labels: HashMap::new(),
            },
        ]
    }

    // --- T124: PluginContext API tests ---

    #[test]
    fn test_plugin_context_new() {
        let ctx = test_context_with_all_permissions();
        assert_eq!(ctx.plugin_id(), "io.example.test");
        assert!(ctx.views().is_empty());
        assert!(ctx.actions().is_empty());
    }

    #[test]
    fn test_plugin_context_has_permission() {
        let ctx = test_context_with_all_permissions();
        assert!(ctx.has_permission(&PluginPermission::ReadResources));
        assert!(ctx.has_permission(&PluginPermission::RegisterViews));

        let readonly = test_context_read_only();
        assert!(readonly.has_permission(&PluginPermission::ReadResources));
        assert!(!readonly.has_permission(&PluginPermission::RegisterViews));
    }

    // --- register_view tests ---

    #[test]
    fn test_register_view_success() {
        let mut ctx = test_context_with_all_permissions();
        let result = ctx.register_view(
            "my-view".to_string(),
            "My View".to_string(),
            Some("custom-icon".to_string()),
            ViewLocation::MainTab,
        );
        assert!(result.is_ok());
        assert_eq!(ctx.views().len(), 1);
        assert_eq!(ctx.views()[0].id, "my-view");
        assert_eq!(ctx.views()[0].label, "My View");
        assert_eq!(ctx.views()[0].location, ViewLocation::MainTab);
        assert_eq!(ctx.views()[0].plugin_id, "io.example.test");
    }

    #[test]
    fn test_register_view_without_permission() {
        let mut ctx = test_context_no_permissions();
        let result = ctx.register_view(
            "my-view".to_string(),
            "My View".to_string(),
            None,
            ViewLocation::MainTab,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("RegisterViews"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_register_view_duplicate_id() {
        let mut ctx = test_context_with_all_permissions();
        ctx.register_view("dup-view".to_string(), "First".to_string(), None, ViewLocation::MainTab)
            .unwrap();

        let result = ctx.register_view(
            "dup-view".to_string(),
            "Second".to_string(),
            None,
            ViewLocation::SidebarSection,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InternalError(msg) => {
                assert!(msg.contains("already registered"));
            }
            other => panic!("expected InternalError, got {:?}", other),
        }
    }

    #[test]
    fn test_register_multiple_views() {
        let mut ctx = test_context_with_all_permissions();
        ctx.register_view("view-1".to_string(), "View 1".to_string(), None, ViewLocation::MainTab)
            .unwrap();
        ctx.register_view(
            "view-2".to_string(),
            "View 2".to_string(),
            None,
            ViewLocation::SidebarSection,
        )
        .unwrap();
        ctx.register_view(
            "view-3".to_string(),
            "View 3".to_string(),
            None,
            ViewLocation::ResourceDetail,
        )
        .unwrap();

        assert_eq!(ctx.views().len(), 3);
    }

    // --- register_action tests ---

    #[test]
    fn test_register_action_success() {
        let mut ctx = test_context_with_all_permissions();
        let result = ctx.register_action(
            "restart-pod".to_string(),
            "Restart Pod".to_string(),
            Some("restart-icon".to_string()),
            vec!["Pod".to_string()],
        );
        assert!(result.is_ok());
        assert_eq!(ctx.actions().len(), 1);
        assert_eq!(ctx.actions()[0].id, "restart-pod");
        assert_eq!(ctx.actions()[0].label, "Restart Pod");
        assert_eq!(ctx.actions()[0].resource_kinds, vec!["Pod"]);
    }

    #[test]
    fn test_register_action_without_permission() {
        let mut ctx = test_context_read_only();
        let result = ctx.register_action(
            "my-action".to_string(),
            "Action".to_string(),
            None,
            vec!["Pod".to_string()],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("RegisterActions"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_register_action_duplicate_id() {
        let mut ctx = test_context_with_all_permissions();
        ctx.register_action(
            "dup-action".to_string(),
            "First".to_string(),
            None,
            vec!["Pod".to_string()],
        )
        .unwrap();

        let result = ctx.register_action(
            "dup-action".to_string(),
            "Second".to_string(),
            None,
            vec!["Pod".to_string()],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_register_action_wildcard_kind() {
        let mut ctx = test_context_with_all_permissions();
        ctx.register_action(
            "global-action".to_string(),
            "Global".to_string(),
            None,
            vec!["*".to_string()],
        )
        .unwrap();

        let pod_actions = ctx.actions_for_kind("Pod");
        assert_eq!(pod_actions.len(), 1);

        let svc_actions = ctx.actions_for_kind("Service");
        assert_eq!(svc_actions.len(), 1);
    }

    #[test]
    fn test_actions_for_kind_filtering() {
        let mut ctx = test_context_with_all_permissions();
        ctx.register_action(
            "pod-action".to_string(),
            "Pod Action".to_string(),
            None,
            vec!["Pod".to_string()],
        )
        .unwrap();
        ctx.register_action(
            "deploy-action".to_string(),
            "Deploy Action".to_string(),
            None,
            vec!["Deployment".to_string()],
        )
        .unwrap();
        ctx.register_action(
            "multi-action".to_string(),
            "Multi".to_string(),
            None,
            vec!["Pod".to_string(), "Deployment".to_string()],
        )
        .unwrap();

        let pod_actions = ctx.actions_for_kind("Pod");
        assert_eq!(pod_actions.len(), 2); // pod-action + multi-action

        let deploy_actions = ctx.actions_for_kind("Deployment");
        assert_eq!(deploy_actions.len(), 2); // deploy-action + multi-action

        let svc_actions = ctx.actions_for_kind("Service");
        assert_eq!(svc_actions.len(), 0);
    }

    // --- list_resources tests ---

    #[test]
    fn test_list_resources_success() {
        let mut ctx = test_context_with_all_permissions();
        let cluster_id = Uuid::new_v4();
        ctx.add_test_resources(cluster_id, sample_resources());

        let pods = ctx.list_resources(cluster_id, "Pod", "v1", None).unwrap();
        assert_eq!(pods.len(), 3); // nginx, redis, worker
    }

    #[test]
    fn test_list_resources_with_namespace_filter() {
        let mut ctx = test_context_with_all_permissions();
        let cluster_id = Uuid::new_v4();
        ctx.add_test_resources(cluster_id, sample_resources());

        let pods = ctx.list_resources(cluster_id, "Pod", "v1", Some("default")).unwrap();
        assert_eq!(pods.len(), 2); // nginx, redis

        let pods = ctx.list_resources(cluster_id, "Pod", "v1", Some("production")).unwrap();
        assert_eq!(pods.len(), 1); // worker
    }

    #[test]
    fn test_list_resources_by_kind() {
        let mut ctx = test_context_with_all_permissions();
        let cluster_id = Uuid::new_v4();
        ctx.add_test_resources(cluster_id, sample_resources());

        let services = ctx.list_resources(cluster_id, "Service", "v1", None).unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "nginx-svc");
    }

    #[test]
    fn test_list_resources_without_permission() {
        let ctx = test_context_no_permissions();
        let result = ctx.list_resources(Uuid::new_v4(), "Pod", "v1", None);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(msg) => {
                assert!(msg.contains("ReadResources"));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_list_resources_unknown_cluster() {
        let ctx = test_context_with_all_permissions();
        let result = ctx.list_resources(Uuid::new_v4(), "Pod", "v1", None).unwrap();
        assert!(result.is_empty());
    }

    // --- get_resource tests ---

    #[test]
    fn test_get_resource_success() {
        let mut ctx = test_context_with_all_permissions();
        let cluster_id = Uuid::new_v4();
        ctx.add_test_resources(cluster_id, sample_resources());

        let pod = ctx.get_resource(cluster_id, "Pod", "v1", Some("default"), "nginx").unwrap();
        assert_eq!(pod.name, "nginx");
        assert_eq!(pod.uid, "pod-1");
    }

    #[test]
    fn test_get_resource_not_found() {
        let mut ctx = test_context_with_all_permissions();
        let cluster_id = Uuid::new_v4();
        ctx.add_test_resources(cluster_id, sample_resources());

        let result = ctx.get_resource(cluster_id, "Pod", "v1", Some("default"), "nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::ResourceNotFound(msg) => {
                assert!(msg.contains("Pod/nonexistent"));
            }
            other => panic!("expected ResourceNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_get_resource_without_permission() {
        let ctx = test_context_no_permissions();
        let result = ctx.get_resource(Uuid::new_v4(), "Pod", "v1", Some("default"), "nginx");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PermissionDenied(_) => {}
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    // --- watch_resources tests ---

    #[test]
    fn test_watch_resources_success() {
        let ctx = test_context_with_all_permissions();
        let handle = ctx.watch_resources(Uuid::new_v4(), "Pod", "v1", None).unwrap();
        assert!(handle.active);
    }

    #[test]
    fn test_watch_resources_without_permission() {
        let ctx = test_context_no_permissions();
        let result = ctx.watch_resources(Uuid::new_v4(), "Pod", "v1", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_watch_handle_cancel() {
        let ctx = test_context_with_all_permissions();
        let mut handle = ctx.watch_resources(Uuid::new_v4(), "Pod", "v1", None).unwrap();
        assert!(handle.active);
        handle.cancel();
        assert!(!handle.active);
    }

    // --- ViewLocation tests ---

    #[test]
    fn test_view_location_variants() {
        assert_ne!(ViewLocation::MainTab, ViewLocation::SidebarSection);
        assert_ne!(ViewLocation::MainTab, ViewLocation::ResourceDetail);
        assert_ne!(ViewLocation::SidebarSection, ViewLocation::ResourceDetail);
    }

    // --- PluginResource tests ---

    #[test]
    fn test_plugin_resource_fields() {
        let resource = PluginResource {
            uid: "test-uid".to_string(),
            name: "test-pod".to_string(),
            namespace: Some("default".to_string()),
            kind: "Pod".to_string(),
            api_version: "v1".to_string(),
            labels: HashMap::from([("app".to_string(), "test".to_string())]),
        };

        assert_eq!(resource.uid, "test-uid");
        assert_eq!(resource.name, "test-pod");
        assert_eq!(resource.namespace, Some("default".to_string()));
        assert_eq!(resource.kind, "Pod");
        assert_eq!(resource.labels.get("app"), Some(&"test".to_string()));
    }
}
