use crate::components::resource_table::{
    ColumnDef, ResourceTableState, columns_for_kind as table_columns_for_kind,
};
use crate::components::search_bar::SearchBarState;
use crate::theme::Theme;
use baeus_core::rbac::{RbacCache, RbacVerb};
use gpui::{Context, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

/// A quick action that can be performed on a resource from the list view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickAction {
    Scale { current_replicas: u32, desired_replicas: u32 },
    Restart,
    Delete,
    Cordon,
    Uncordon,
    ViewLogs,
    Exec,
    EditYaml,
}

impl QuickAction {
    /// Returns a human-readable label for this action.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Scale { .. } => "Scale",
            Self::Restart => "Restart",
            Self::Delete => "Delete",
            Self::Cordon => "Cordon",
            Self::Uncordon => "Uncordon",
            Self::ViewLogs => "View Logs",
            Self::Exec => "Exec",
            Self::EditYaml => "Edit YAML",
        }
    }

    /// Returns true if the action is destructive (i.e., deletes a resource).
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Delete)
    }

    /// Returns true if the action requires user confirmation before execution.
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            Self::Scale { .. } | Self::Restart | Self::Delete | Self::Cordon | Self::Uncordon
        )
    }
}

/// Returns the available quick actions for a given resource kind.
pub fn actions_for_kind(kind: &str) -> Vec<QuickAction> {
    match kind {
        "Pod" => vec![QuickAction::ViewLogs, QuickAction::Exec, QuickAction::Delete],
        "Deployment" | "StatefulSet" => vec![
            QuickAction::Scale { current_replicas: 0, desired_replicas: 0 },
            QuickAction::Restart,
            QuickAction::EditYaml,
            QuickAction::Delete,
        ],
        "DaemonSet" => vec![QuickAction::Restart, QuickAction::EditYaml, QuickAction::Delete],
        "ReplicaSet" | "Job" => vec![QuickAction::Delete],
        "CronJob" => vec![QuickAction::EditYaml, QuickAction::Delete],
        "Node" => vec![QuickAction::Cordon, QuickAction::Uncordon],
        "Service" | "Ingress" | "ConfigMap" | "Secret" | "NetworkPolicy" => {
            vec![QuickAction::EditYaml, QuickAction::Delete]
        }
        _ => vec![QuickAction::EditYaml, QuickAction::Delete],
    }
}

/// The set of workload resource kinds.
const WORKLOAD_KINDS: &[&str] =
    &["Pod", "Deployment", "StatefulSet", "DaemonSet", "ReplicaSet", "Job", "CronJob"];

// ---------------------------------------------------------------------------
// T062: Action Execution Types
// ---------------------------------------------------------------------------

/// Status of an action that has been submitted for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionStatus {
    /// Action is awaiting user confirmation (destructive actions).
    PendingConfirmation,
    /// User confirmed, action is being executed.
    Executing,
    /// Action completed successfully.
    Completed { message: String },
    /// Action failed.
    Failed { error: String },
}

/// A fully-specified action request ready for execution.
#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub resource_uid: String,
    pub resource_name: String,
    pub resource_namespace: Option<String>,
    pub kind: String,
    pub action: QuickAction,
    pub status: ActionStatus,
}

// ---------------------------------------------------------------------------
// T064: RBAC Integration Helpers
// ---------------------------------------------------------------------------

/// Maps a Kubernetes Kind to its plural resource name.
pub fn kind_to_plural(kind: &str) -> &str {
    match kind {
        "Pod" => "pods",
        "Deployment" => "deployments",
        "StatefulSet" => "statefulsets",
        "DaemonSet" => "daemonsets",
        "ReplicaSet" => "replicasets",
        "Job" => "jobs",
        "CronJob" => "cronjobs",
        "Node" => "nodes",
        "Service" => "services",
        "Ingress" => "ingresses",
        "ConfigMap" => "configmaps",
        "Secret" => "secrets",
        "NetworkPolicy" => "networkpolicies",
        "Namespace" => "namespaces",
        "ServiceAccount" => "serviceaccounts",
        "PersistentVolumeClaim" => "persistentvolumeclaims",
        "PersistentVolume" => "persistentvolumes",
        other => {
            // Best-effort: lowercase and append "s"
            // Note: this leaks into a static-lifetime approximation, but
            // callers needing the owned form should use `kind_to_plural_owned`.
            // For the common fallback we just return the lowered + "s" via a
            // leak-free approach: callers will get an owned String from
            // `resource_for_action` which handles this.
            // For this function we return a best-effort static str for known
            // kinds and panic-free fallback for unknown.
            // We use a simple convention: unknown kinds get an empty fallback;
            // `resource_for_action` handles the owned case.
            _ = other;
            "unknown"
        }
    }
}

/// Maps a Kubernetes Kind to its plural resource name (owned).
fn kind_to_plural_owned(kind: &str) -> String {
    let known = kind_to_plural(kind);
    if known != "unknown" {
        return known.to_string();
    }
    // Fallback: lowercase + "s"
    format!("{}s", kind.to_lowercase())
}

/// Maps a `QuickAction` to the required RBAC verb.
pub fn verb_for_action(action: &QuickAction) -> RbacVerb {
    match action {
        QuickAction::Delete => RbacVerb::Delete,
        QuickAction::Scale { .. } => RbacVerb::Update,
        QuickAction::Restart => RbacVerb::Update,
        QuickAction::Cordon => RbacVerb::Patch,
        QuickAction::Uncordon => RbacVerb::Patch,
        QuickAction::EditYaml => RbacVerb::Update,
        QuickAction::ViewLogs => RbacVerb::Get,
        QuickAction::Exec => RbacVerb::Create,
    }
}

/// Maps a kind + action to the RBAC resource string.
///
/// For most actions this is the plural form of the kind (e.g. "pods",
/// "deployments"). For `ViewLogs` this returns "pods/log" and for `Exec`
/// this returns "pods/exec" (sub-resources).
pub fn resource_for_action(kind: &str, action: &QuickAction) -> String {
    match action {
        QuickAction::ViewLogs => "pods/log".to_string(),
        QuickAction::Exec => "pods/exec".to_string(),
        QuickAction::Cordon | QuickAction::Uncordon => "nodes".to_string(),
        _ => kind_to_plural_owned(kind),
    }
}

/// Maps a Kubernetes Kind to its API group ("" for core resources, "apps"
/// for Deployment/StatefulSet/DaemonSet/ReplicaSet, etc.).
pub fn api_group_for_kind(kind: &str) -> &str {
    match kind {
        // Core API (v1)
        "Pod"
        | "Service"
        | "ConfigMap"
        | "Secret"
        | "Namespace"
        | "Node"
        | "PersistentVolume"
        | "PersistentVolumeClaim"
        | "ServiceAccount" => "",
        // apps group
        "Deployment" | "StatefulSet" | "DaemonSet" | "ReplicaSet" => "apps",
        // batch group
        "Job" | "CronJob" => "batch",
        // networking group
        "Ingress" | "NetworkPolicy" => "networking.k8s.io",
        // fallback: empty string for unknown kinds
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// T104-T105: Column configurations for Network and Storage resource kinds
// ---------------------------------------------------------------------------

/// Returns the default column definitions for a given resource kind.
///
/// This provides kind-specific column layouts for the resource table.
/// Network resources (Services, Ingresses, NetworkPolicies, Endpoints)
/// and Storage resources (PV, PVC, StorageClasses) have tailored columns
/// that surface the most important fields at a glance.
pub fn columns_for_kind(kind: &str) -> Vec<ColumnDef> {
    match kind {
        // ---- Network resources (T104) ----
        "Service" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "type".to_string(),
                label: "Type".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "cluster_ip".to_string(),
                label: "Cluster IP".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "external_ip".to_string(),
                label: "External IP".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "ports".to_string(),
                label: "Ports".to_string(),
                sortable: false,
                width_weight: 1.5,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
        "Ingress" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "hosts".to_string(),
                label: "Hosts".to_string(),
                sortable: false,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "address".to_string(),
                label: "Address".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "ports".to_string(),
                label: "Ports".to_string(),
                sortable: false,
                width_weight: 0.8,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
        "NetworkPolicy" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "pod_selector".to_string(),
                label: "Pod Selector".to_string(),
                sortable: false,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
        "Endpoints" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "endpoints".to_string(),
                label: "Endpoints".to_string(),
                sortable: false,
                width_weight: 3.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],

        // ---- Storage resources (T105) ----
        "PersistentVolume" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "capacity".to_string(),
                label: "Capacity".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "access_modes".to_string(),
                label: "Access Modes".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "reclaim_policy".to_string(),
                label: "Reclaim Policy".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "status".to_string(),
                label: "Status".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
            ColumnDef {
                id: "claim".to_string(),
                label: "Claim".to_string(),
                sortable: true,
                width_weight: 1.5,
            },
            ColumnDef {
                id: "storage_class".to_string(),
                label: "Storage Class".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
        "PersistentVolumeClaim" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "status".to_string(),
                label: "Status".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
            ColumnDef {
                id: "volume".to_string(),
                label: "Volume".to_string(),
                sortable: true,
                width_weight: 1.5,
            },
            ColumnDef {
                id: "capacity".to_string(),
                label: "Capacity".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "access_modes".to_string(),
                label: "Access Modes".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "storage_class".to_string(),
                label: "Storage Class".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
        "StorageClass" => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "provisioner".to_string(),
                label: "Provisioner".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "reclaim_policy".to_string(),
                label: "Reclaim Policy".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "volume_binding_mode".to_string(),
                label: "Volume Binding Mode".to_string(),
                sortable: true,
                width_weight: 1.5,
            },
            ColumnDef {
                id: "allow_expansion".to_string(),
                label: "Allow Expansion".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],

        // ---- Default: generic columns for any resource kind ----
        _ => vec![
            ColumnDef {
                id: "name".to_string(),
                label: "Name".to_string(),
                sortable: true,
                width_weight: 2.0,
            },
            ColumnDef {
                id: "namespace".to_string(),
                label: "Namespace".to_string(),
                sortable: true,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "status".to_string(),
                label: "Status".to_string(),
                sortable: false,
                width_weight: 1.0,
            },
            ColumnDef {
                id: "age".to_string(),
                label: "Age".to_string(),
                sortable: true,
                width_weight: 0.8,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// T064a: Create Resource State
// ---------------------------------------------------------------------------

/// State for the "Create Resource" dialog/workflow.
#[derive(Debug)]
pub struct CreateResourceState {
    pub kind: String,
    pub api_version: String,
    pub namespace: Option<String>,
    pub yaml_template: String,
    pub modified_yaml: Option<String>,
    pub validation_error: Option<String>,
    pub submitting: bool,
}

impl CreateResourceState {
    /// Creates state with a default YAML template for the kind.
    pub fn new(kind: &str, api_version: &str, namespace: Option<&str>) -> Self {
        let yaml_template = Self::default_template(kind, api_version, namespace);
        Self {
            kind: kind.to_string(),
            api_version: api_version.to_string(),
            namespace: namespace.map(|s| s.to_string()),
            yaml_template,
            modified_yaml: None,
            validation_error: None,
            submitting: false,
        }
    }

    /// Generates a minimal YAML template for common resource kinds.
    pub fn default_template(kind: &str, api_version: &str, namespace: Option<&str>) -> String {
        let mut lines = Vec::new();
        lines.push(format!("apiVersion: {api_version}"));
        lines.push(format!("kind: {kind}"));
        lines.push("metadata:".to_string());
        lines.push(format!("  name: my-{}", kind.to_lowercase()));
        if let Some(ns) = namespace {
            lines.push(format!("  namespace: {ns}"));
        }

        match kind {
            "Pod" => {
                lines.push("spec:".to_string());
                lines.push("  containers:".to_string());
                lines.push("    - name: main".to_string());
                lines.push("      image: nginx:latest".to_string());
            }
            "Deployment" => {
                lines.push("spec:".to_string());
                lines.push("  replicas: 1".to_string());
                lines.push("  selector:".to_string());
                lines.push("    matchLabels:".to_string());
                lines.push("      app: my-deployment".to_string());
                lines.push("  template:".to_string());
                lines.push("    metadata:".to_string());
                lines.push("      labels:".to_string());
                lines.push("        app: my-deployment".to_string());
                lines.push("    spec:".to_string());
                lines.push("      containers:".to_string());
                lines.push("        - name: main".to_string());
                lines.push("          image: nginx:latest".to_string());
            }
            "Service" => {
                lines.push("spec:".to_string());
                lines.push("  selector:".to_string());
                lines.push("    app: my-service".to_string());
                lines.push("  ports:".to_string());
                lines.push("    - port: 80".to_string());
                lines.push("      targetPort: 80".to_string());
            }
            "ConfigMap" => {
                lines.push("data:".to_string());
                lines.push("  key: value".to_string());
            }
            "Secret" => {
                lines.push("type: Opaque".to_string());
                lines.push("data:".to_string());
                lines.push("  key: dmFsdWU=".to_string());
            }
            _ => {
                lines.push("spec: {}".to_string());
            }
        }

        lines.join("\n")
    }

    /// Updates the modified YAML.
    pub fn set_yaml(&mut self, yaml: &str) {
        self.modified_yaml = Some(yaml.to_string());
    }

    /// Validates that the YAML is parseable. Returns `true` if valid.
    pub fn validate(&mut self) -> bool {
        let yaml_to_check = self.modified_yaml.as_deref().unwrap_or(&self.yaml_template);

        match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml_to_check) {
            Ok(_) => {
                self.validation_error = None;
                true
            }
            Err(e) => {
                self.validation_error = Some(e.to_string());
                false
            }
        }
    }

    /// Sets the submitting flag.
    pub fn set_submitting(&mut self, submitting: bool) {
        self.submitting = submitting;
    }

    /// Sets a validation error message.
    pub fn set_validation_error(&mut self, error: &str) {
        self.validation_error = Some(error.to_string());
    }

    /// Clears the validation error.
    pub fn clear_validation_error(&mut self) {
        self.validation_error = None;
    }
}

// ---------------------------------------------------------------------------
// ResourceListState
// ---------------------------------------------------------------------------

/// Cluster-scoped resource kinds that should not show a namespace filter.
const CLUSTER_SCOPED_KINDS: &[&str] = &[
    "Node",
    "Namespace",
    "PersistentVolume",
    "StorageClass",
    "ClusterRole",
    "ClusterRoleBinding",
    "PodSecurityPolicy",
];

/// State for the resource list view, managing the display of any K8s resource
/// kind in a table format with quick actions.
#[derive(Debug)]
pub struct ResourceListState {
    pub kind: String,
    pub api_version: String,
    pub namespace_filter: Option<String>,
    /// T339: Multi-namespace filtering (FR-034, FR-073).
    /// When non-empty, only resources in these namespaces are shown.
    /// When empty, all namespaces are shown.
    pub selected_namespaces: Vec<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub selected_resource_uid: Option<String>,
    pub pending_action: Option<(String, QuickAction)>,
    /// The current action request (T062).
    pub current_action_request: Option<ActionRequest>,
    /// The create-resource dialog state (T064a).
    pub show_create_dialog: Option<CreateResourceState>,
}

impl ResourceListState {
    /// Creates a new resource list state for the given kind and API version.
    pub fn new(kind: &str, api_version: &str) -> Self {
        Self {
            kind: kind.to_string(),
            api_version: api_version.to_string(),
            namespace_filter: None,
            selected_namespaces: Vec::new(),
            loading: false,
            error: None,
            selected_resource_uid: None,
            pending_action: None,
            current_action_request: None,
            show_create_dialog: None,
        }
    }

    /// Sets the namespace filter. Pass `None` to show all namespaces.
    pub fn set_namespace_filter(&mut self, ns: Option<String>) {
        self.namespace_filter = ns;
    }

    // --- T339: Multi-namespace filtering (FR-034, FR-073) ---

    /// Sets the selected namespaces for multi-namespace filtering.
    pub fn set_selected_namespaces(&mut self, namespaces: Vec<String>) {
        self.selected_namespaces = namespaces;
    }

    /// Adds a namespace to the multi-namespace filter.
    pub fn add_namespace(&mut self, ns: &str) {
        if !self.selected_namespaces.contains(&ns.to_string()) {
            self.selected_namespaces.push(ns.to_string());
        }
    }

    /// Removes a namespace from the multi-namespace filter.
    pub fn remove_namespace(&mut self, ns: &str) {
        self.selected_namespaces.retain(|n| n != ns);
    }

    /// Clears the multi-namespace filter (show all namespaces).
    pub fn clear_namespace_filter(&mut self) {
        self.selected_namespaces.clear();
    }

    /// Returns `true` if this resource kind is cluster-scoped
    /// (should not show a namespace filter).
    pub fn is_cluster_scoped(&self) -> bool {
        CLUSTER_SCOPED_KINDS.contains(&self.kind.as_str())
    }

    /// Returns `true` if the namespace filter is active (non-empty
    /// and the resource kind is namespace-scoped).
    pub fn has_namespace_filter(&self) -> bool {
        !self.selected_namespaces.is_empty() && !self.is_cluster_scoped()
    }

    /// Filters a list of table rows by the currently selected namespaces.
    ///
    /// For cluster-scoped resources or when no namespaces are selected,
    /// all rows are returned.
    pub fn filter_by_namespaces<'a>(
        &self,
        rows: &'a [crate::components::resource_table::TableRow],
    ) -> Vec<&'a crate::components::resource_table::TableRow> {
        if self.is_cluster_scoped() || self.selected_namespaces.is_empty() {
            return rows.iter().collect();
        }
        rows.iter()
            .filter(|row| match &row.namespace {
                Some(ns) => self.selected_namespaces.contains(ns),
                None => false,
            })
            .collect()
    }

    /// Sets the loading state.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Sets an error message.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clears the current error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Selects a resource by its UID.
    pub fn select_resource(&mut self, uid: &str) {
        self.selected_resource_uid = Some(uid.to_string());
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.selected_resource_uid = None;
    }

    /// Requests an action on a resource, storing it as a pending action.
    pub fn request_action(&mut self, uid: &str, action: QuickAction) {
        self.pending_action = Some((uid.to_string(), action));
    }

    /// Consumes and returns the pending action, if any.
    pub fn take_pending_action(&mut self) -> Option<(String, QuickAction)> {
        self.pending_action.take()
    }

    /// Returns the available quick actions for this resource's kind.
    pub fn available_actions(&self) -> Vec<QuickAction> {
        actions_for_kind(&self.kind)
    }

    /// Returns true if this resource kind is a workload kind (Pod, Deployment,
    /// StatefulSet, DaemonSet, ReplicaSet, Job, CronJob).
    pub fn is_workload_kind(&self) -> bool {
        WORKLOAD_KINDS.contains(&self.kind.as_str())
    }

    // --- T062: Action Execution Methods ---

    /// Creates an `ActionRequest` and stores it as the current action.
    ///
    /// Actions that `requires_confirmation()` start as `PendingConfirmation`;
    /// others start as `Executing`.
    pub fn submit_action(
        &mut self,
        resource_uid: &str,
        resource_name: &str,
        namespace: Option<&str>,
        kind: &str,
        action: QuickAction,
    ) -> &ActionRequest {
        let status = if action.requires_confirmation() {
            ActionStatus::PendingConfirmation
        } else {
            ActionStatus::Executing
        };

        self.current_action_request = Some(ActionRequest {
            resource_uid: resource_uid.to_string(),
            resource_name: resource_name.to_string(),
            resource_namespace: namespace.map(|s| s.to_string()),
            kind: kind.to_string(),
            action,
            status,
        });

        self.current_action_request.as_ref().unwrap()
    }

    /// Moves the current action from `PendingConfirmation` to `Executing`.
    pub fn confirm_action(&mut self) {
        if let Some(ref mut req) = self.current_action_request {
            if req.status == ActionStatus::PendingConfirmation {
                req.status = ActionStatus::Executing;
            }
        }
    }

    /// Clears the current action request (cancellation).
    pub fn cancel_action(&mut self) {
        self.current_action_request = None;
    }

    /// Marks the current action as `Completed` with the given message.
    pub fn complete_action(&mut self, message: &str) {
        if let Some(ref mut req) = self.current_action_request {
            req.status = ActionStatus::Completed { message: message.to_string() };
        }
    }

    /// Marks the current action as `Failed` with the given error.
    pub fn fail_action(&mut self, error: &str) {
        if let Some(ref mut req) = self.current_action_request {
            req.status = ActionStatus::Failed { error: error.to_string() };
        }
    }

    /// Returns a reference to the current action request, if any.
    pub fn current_action(&self) -> Option<&ActionRequest> {
        self.current_action_request.as_ref()
    }

    /// Returns `true` if there is an action awaiting user confirmation.
    pub fn has_pending_confirmation(&self) -> bool {
        matches!(
            self.current_action_request,
            Some(ActionRequest { status: ActionStatus::PendingConfirmation, .. })
        )
    }

    // --- T064: RBAC-aware action filtering ---

    /// Returns only the actions the user has permission for, according to the
    /// RBAC cache. Actions whose permission is not cached are **included** (we
    /// assume allowed until proven otherwise -- optimistic).
    pub fn filtered_actions(
        &self,
        rbac_cache: &RbacCache,
        namespace: Option<&str>,
    ) -> Vec<QuickAction> {
        let all_actions = self.available_actions();
        let api_group = api_group_for_kind(&self.kind);

        all_actions
            .into_iter()
            .filter(|action| {
                let verb = verb_for_action(action);
                let resource = resource_for_action(&self.kind, action);

                // Determine the api_group for the resource check. For
                // sub-resources like pods/log and pods/exec the api_group
                // is "" (core). For cordon/uncordon on nodes it is also "".
                let action_api_group = match action {
                    QuickAction::ViewLogs | QuickAction::Exec => "",
                    QuickAction::Cordon | QuickAction::Uncordon => "",
                    _ => api_group,
                };

                match rbac_cache.is_allowed(verb, &resource, action_api_group, namespace) {
                    Some(true) => true,   // explicitly allowed
                    Some(false) => false, // explicitly denied
                    None => true,         // not cached, assume allowed
                }
            })
            .collect()
    }

    // --- T064a: Create Resource Dialog ---

    /// Opens the create-resource dialog with a default template for the
    /// current kind and API version.
    pub fn open_create_dialog(&mut self) {
        self.show_create_dialog = Some(CreateResourceState::new(
            &self.kind,
            &self.api_version,
            self.namespace_filter.as_deref(),
        ));
    }

    /// Closes the create-resource dialog.
    pub fn close_create_dialog(&mut self) {
        self.show_create_dialog = None;
    }

    /// Returns a reference to the create-resource dialog state, if open.
    pub fn create_dialog(&self) -> Option<&CreateResourceState> {
        self.show_create_dialog.as_ref()
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// View wrapper for `ResourceListState` that holds a theme and embedded
/// table + search bar state for rendering.
pub struct ResourceListView {
    pub state: ResourceListState,
    pub table_state: ResourceTableState,
    pub search_state: SearchBarState,
    pub theme: Theme,
}

impl ResourceListView {
    /// Creates a new view with default table columns for the kind.
    ///
    /// T337: Uses `columns_for_kind` from `resource_table` to get the
    /// correct per-resource columns.
    pub fn new(state: ResourceListState, theme: Theme) -> Self {
        let columns = table_columns_for_kind(&state.kind);
        let table_state = ResourceTableState::new(columns, 20);
        let search_state = SearchBarState::new();
        Self { state, table_state, search_state, theme }
    }

    /// Returns the title text for the toolbar.
    pub fn toolbar_title(&self) -> String {
        format!("{}s", self.state.kind)
    }

    /// Returns whether the view is in a loading state.
    pub fn is_loading(&self) -> bool {
        self.state.loading
    }

    /// Returns whether the view has an error.
    pub fn has_error(&self) -> bool {
        self.state.error.is_some()
    }

    /// Returns whether the table has any visible rows.
    pub fn has_rows(&self) -> bool {
        !self.table_state.rows.is_empty()
    }

    /// Render the toolbar with search bar and action buttons.
    fn render_toolbar(&self, colors: &ListColors) -> gpui::Div {
        let title = SharedString::from(self.toolbar_title());

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_4()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_base()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.text_primary)
                    .child(title),
            )
            .child(self.render_search_placeholder(colors))
            .child(self.render_action_buttons(colors))
    }

    /// Render a search placeholder in the toolbar.
    fn render_search_placeholder(&self, colors: &ListColors) -> gpui::Div {
        let placeholder = SharedString::from("Filter...");
        div()
            .flex_1()
            .px_3()
            .py_1()
            .rounded(px(6.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .text_color(colors.text_muted)
            .child(placeholder)
    }

    /// Render action buttons (refresh, create).
    fn render_action_buttons(&self, colors: &ListColors) -> gpui::Div {
        let refresh_btn = div()
            .id("refresh-btn")
            .px_3()
            .py_1()
            .rounded(px(6.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_sm()
            .text_color(colors.text_primary)
            .child("Refresh");

        let create_btn = div()
            .id("create-btn")
            .px_3()
            .py_1()
            .rounded(px(6.0))
            .bg(colors.accent)
            .cursor_pointer()
            .text_sm()
            .text_color(colors.button_text)
            .child("Create");

        div().flex().gap(px(4.0)).child(refresh_btn).child(create_btn)
    }

    /// Render the loading state indicator.
    fn render_loading(&self, colors: &ListColors) -> gpui::Div {
        div()
            .flex()
            .justify_center()
            .py_8()
            .text_sm()
            .text_color(colors.text_muted)
            .child("Loading resources...")
    }

    /// Render the error state.
    fn render_error(&self, colors: &ListColors) -> gpui::Div {
        let error_msg = self.state.error.as_deref().unwrap_or("Unknown error");
        let msg = SharedString::from(error_msg.to_string());

        div()
            .flex()
            .flex_col()
            .items_center()
            .py_8()
            .gap(px(4.0))
            .child(div().text_sm().text_color(colors.error).child(msg))
    }

    /// T339: Render the namespace filter bar (only for namespace-scoped resources).
    fn render_namespace_filter(&self, colors: &ListColors) -> gpui::Div {
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_4()
            .py_1()
            .gap(px(4.0))
            .border_b_1()
            .border_color(colors.border);

        let label = SharedString::from("Namespaces:");
        bar = bar.child(div().text_xs().text_color(colors.text_muted).child(label));

        if self.state.selected_namespaces.is_empty() {
            let all_label = SharedString::from("All");
            bar = bar.child(div().text_xs().text_color(colors.text_primary).child(all_label));
        } else {
            for ns in &self.state.selected_namespaces {
                let ns_label = SharedString::from(ns.clone());
                bar = bar.child(
                    div()
                        .px_2()
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(colors.surface)
                        .border_1()
                        .border_color(colors.border)
                        .text_xs()
                        .text_color(colors.text_primary)
                        .child(ns_label),
                );
            }
        }

        bar
    }

    /// Render the empty state when no rows exist.
    fn render_empty_state(&self, colors: &ListColors) -> gpui::Div {
        let msg = SharedString::from(format!("No {}s found", self.state.kind));
        div()
            .flex()
            .flex_col()
            .items_center()
            .py_8()
            .child(div().text_sm().text_color(colors.text_muted).child(msg))
    }
}

/// Precomputed colors for rendering the resource list view.
struct ListColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
    button_text: Rgba,
}

impl Render for ResourceListView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ListColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            button_text: crate::theme::Color::rgb(255, 255, 255).to_gpui(),
        };

        let mut content = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(colors.background)
            .child(self.render_toolbar(&colors));

        // T339: Show namespace filter bar for namespace-scoped resources.
        if !self.state.is_cluster_scoped() {
            content = content.child(self.render_namespace_filter(&colors));
        }

        if self.state.loading {
            content = content.child(self.render_loading(&colors));
        } else if self.state.error.is_some() {
            content = content.child(self.render_error(&colors));
        } else if self.table_state.rows.is_empty() {
            content = content.child(self.render_empty_state(&colors));
        }

        // The table itself is rendered separately as a child view via
        // ResourceTableView, but we show the appropriate state feedback here.

        content
    }
}
