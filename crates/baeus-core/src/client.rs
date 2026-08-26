//! Kubernetes client creation and data fetching.
//!
//! Provides functions to create a real `kube::Client` from a kubeconfig context
//! and to fetch cluster data for the dashboard and resource views.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures::{Stream, TryStreamExt};
use tokio_util::sync::CancellationToken;
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod};
use kube::{Api, Client, Config, api::ListParams};
use kube_runtime::WatchStreamExt;
use kube_runtime::watcher::{self, Event as WatcherEvent};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---------------------------------------------------------------------------
// T364: RBAC error handling for 403 Forbidden responses
// ---------------------------------------------------------------------------

/// Information about a 403 Forbidden RBAC denial from the Kubernetes API.
///
/// Carries the verb, resource kind, and optional namespace so the UI can
/// display a human-readable permission-denied message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacDenied {
    /// The Kubernetes verb that was denied (e.g. "list", "get", "delete").
    pub verb: String,
    /// The resource kind that was denied (e.g. "pods", "deployments").
    pub resource: String,
    /// The namespace in which the action was attempted, if namespaced.
    pub namespace: Option<String>,
    /// The formatted user-facing error message.
    pub message: String,
}

impl RbacDenied {
    /// Create a new `RbacDenied` with auto-formatted message.
    pub fn new(verb: &str, resource: &str, namespace: Option<&str>) -> Self {
        let message = format_rbac_error(verb, resource, namespace);
        Self {
            verb: verb.to_string(),
            resource: resource.to_string(),
            namespace: namespace.map(|s| s.to_string()),
            message,
        }
    }
}

impl std::fmt::Display for RbacDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Check whether an `anyhow::Error` wraps a kube 403 Forbidden response.
///
/// Walks the error chain looking for a `kube_client::Error::Api` variant
/// whose `ErrorResponse.code` is 403.
pub fn is_forbidden_error(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(kube::Error::Api(resp)) = cause.downcast_ref::<kube::Error>() {
            if resp.code == 403 {
                return true;
            }
        }
    }
    false
}

/// Check whether an error string looks like a 403 Forbidden response.
///
/// This is a fallback for cases where the original `anyhow::Error` has been
/// stringified (e.g. via `.map_err(|e| e.to_string())`). Checks for common
/// 403/Forbidden indicators in the message text.
pub fn is_forbidden_error_string(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("403") || lower.contains("forbidden") || lower.contains("permission denied")
}

/// Check whether an `anyhow::Error` wraps a kube 409 Conflict response.
///
/// Walks the error chain looking for a `kube_client::Error::Api` variant
/// whose `ErrorResponse.code` is 409.
pub fn is_conflict_error(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(kube::Error::Api(resp)) = cause.downcast_ref::<kube::Error>() {
            if resp.code == 409 {
                return true;
            }
        }
    }
    false
}

/// Check whether an error string looks like a 409 Conflict response.
///
/// Fallback for cases where the original `anyhow::Error` has been stringified.
pub fn is_conflict_error_string(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("409") || lower.contains("conflict")
}

/// Format a user-friendly RBAC permission denied message.
///
/// # Examples
///
/// ```
/// use baeus_core::client::format_rbac_error;
///
/// let msg = format_rbac_error("list", "Pods", Some("default"));
/// assert_eq!(msg, "Permission denied: you do not have permission to list Pods in namespace \"default\"");
///
/// let msg = format_rbac_error("get", "Nodes", None);
/// assert_eq!(msg, "Permission denied: you do not have permission to get Nodes (cluster-scoped)");
/// ```
pub fn format_rbac_error(verb: &str, resource: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(ns) => format!(
            "Permission denied: you do not have permission to {verb} {resource} in namespace \"{ns}\""
        ),
        None => format!(
            "Permission denied: you do not have permission to {verb} {resource} (cluster-scoped)"
        ),
    }
}

/// Attempt to extract RBAC denied info from an error string.
///
/// If the error looks like a 403, builds an `RbacDenied` with the given
/// verb/resource/namespace context. Returns `None` if the error is not
/// a forbidden response.
pub fn rbac_denied_from_error(
    err_msg: &str,
    verb: &str,
    resource: &str,
    namespace: Option<&str>,
) -> Option<RbacDenied> {
    if is_forbidden_error_string(err_msg) {
        Some(RbacDenied::new(verb, resource, namespace))
    } else {
        None
    }
}

/// Apply security-hardened timeouts to a kube Config.
fn apply_timeouts(mut config: Config) -> Config {
    config.connect_timeout = Some(Duration::from_secs(15));
    config.read_timeout = Some(Duration::from_secs(60));
    config.write_timeout = Some(Duration::from_secs(60));
    config
}

/// Create a `kube::Client` for the given kubeconfig context name, loading from
/// a specific kubeconfig file path instead of the default resolution.
///
/// This is necessary when the context lives in a kubeconfig file outside the
/// standard `$KUBECONFIG` / `~/.kube/config` paths (e.g. additional scan dirs).
pub async fn create_client_from_path(
    context_name: &str,
    kubeconfig_path: &str,
    aws_profile: Option<&str>,
) -> Result<Client> {
    let mut kubeconfig = kube::config::Kubeconfig::read_from(kubeconfig_path)
        .with_context(|| format!("Failed to read kubeconfig from '{kubeconfig_path}'"))?;

    if let Some(profile) = aws_profile {
        crate::aws_sso::inject_aws_profile_into_kubeconfig(&mut kubeconfig, context_name, profile)?;
    }

    let config = Config::from_custom_kubeconfig(
        kubeconfig,
        &kube::config::KubeConfigOptions {
            context: Some(context_name.to_string()),
            ..Default::default()
        },
    )
    .await
    .with_context(|| format!("Failed to load context '{context_name}' from '{kubeconfig_path}'"))?;

    let config = apply_timeouts(config);

    let client = Client::try_from(config)
        .with_context(|| format!("Failed to create kube client for context '{context_name}'"))?;

    Ok(client)
}

/// Create a `kube::Client` with in-memory AWS credentials injected into the exec env.
/// Used for initial EKS wizard connections where the exec plugin doesn't have
/// system-level AWS credentials yet. Credentials stay in-memory, never written to disk.
pub async fn create_client_from_path_with_aws_creds(
    context_name: &str,
    kubeconfig_path: &str,
    access_key_id: &str,
    secret_access_key: &str,
    session_token: Option<&str>,
) -> Result<Client> {
    let mut kubeconfig = kube::config::Kubeconfig::read_from(kubeconfig_path)
        .with_context(|| format!("Failed to read kubeconfig from '{kubeconfig_path}'"))?;

    crate::aws_sso::inject_aws_credentials_into_kubeconfig(
        &mut kubeconfig,
        context_name,
        access_key_id,
        secret_access_key,
        session_token,
    )?;

    let config = Config::from_custom_kubeconfig(
        kubeconfig,
        &kube::config::KubeConfigOptions {
            context: Some(context_name.to_string()),
            ..Default::default()
        },
    )
    .await
    .with_context(|| format!("Failed to load context '{context_name}' from '{kubeconfig_path}'"))?;

    let config = apply_timeouts(config);

    let client = Client::try_from(config)
        .with_context(|| format!("Failed to create kube client for context '{context_name}'"))?;

    Ok(client)
}

/// Create a `kube::Client` for the given kubeconfig context name.
///
/// Uses the default kubeconfig resolution (KUBECONFIG env var → ~/.kube/config)
/// and selects the specified context.
pub async fn create_client(context_name: &str) -> Result<Client> {
    let config = Config::from_kubeconfig(&kube::config::KubeConfigOptions {
        context: Some(context_name.to_string()),
        ..Default::default()
    })
    .await
    .with_context(|| format!("Failed to load kubeconfig for context '{context_name}'"))?;

    let config = apply_timeouts(config);

    let client = Client::try_from(config)
        .with_context(|| format!("Failed to create kube client for context '{context_name}'"))?;

    Ok(client)
}

/// Verify that a client can reach the API server.
pub async fn verify_connection(client: &Client) -> Result<String> {
    let version =
        client.apiserver_version().await.context("Failed to reach Kubernetes API server")?;
    Ok(format!("{}.{}", version.major, version.minor))
}

/// Pre-parsed dashboard data fetched from a cluster.
pub struct DashboardData {
    pub nodes: Vec<NodeInfo>,
    pub pod_counts: PodCounts,
    pub namespaces: Vec<String>,
    pub events: Vec<EventInfo>,
    pub k8s_version: String,
    /// Resource counts fetched directly from the API for the dashboard.
    pub resource_counts: DashboardResourceCounts,
}

pub struct NodeInfo {
    pub name: String,
    pub ready: bool,
    pub roles: Vec<String>,
    /// Allocatable CPU in millicores (e.g. 4000 = 4 cores).
    pub allocatable_cpu_millis: Option<u64>,
    /// Allocatable memory in bytes.
    pub allocatable_memory_bytes: Option<u64>,
}

/// Resource counts fetched during dashboard loading.
#[derive(Debug, Clone, Default)]
pub struct DashboardResourceCounts {
    pub pods: u32,
    pub deployments: u32,
    pub daemonsets: u32,
    pub statefulsets: u32,
    pub replicasets: u32,
    pub jobs: u32,
    pub cronjobs: u32,
}

pub struct PodCounts {
    pub running: u32,
    pub pending: u32,
    pub failed: u32,
    pub succeeded: u32,
}

pub struct EventInfo {
    pub reason: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub is_warning: bool,
    pub namespace: Option<String>,
    pub involved_object_kind: Option<String>,
    pub involved_object_name: Option<String>,
    pub source: Option<String>,
    pub count: u32,
    pub last_seen: Option<DateTime<Utc>>,
}

/// Fetch all dashboard data for a cluster in parallel.
pub async fn fetch_dashboard_data(client: &Client) -> Result<DashboardData> {
    let nodes_api: Api<Node> = Api::all(client.clone());
    let pods_api: Api<Pod> = Api::all(client.clone());
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let events_api: Api<Event> = Api::all(client.clone());

    let lp = ListParams::default();
    let events_lp = ListParams::default().limit(100);

    // Fetch core resources + resource counts in parallel.
    let (
        nodes_result,
        pods_result,
        ns_result,
        events_result,
        version_result,
        deploy_count,
        ds_count,
        ss_count,
        rs_count,
        job_count,
        cj_count,
    ) = tokio::join!(
        nodes_api.list(&lp),
        pods_api.list(&lp),
        ns_api.list(&lp),
        events_api.list(&events_lp),
        client.apiserver_version(),
        count_resources(client, "Deployment"),
        count_resources(client, "DaemonSet"),
        count_resources(client, "StatefulSet"),
        count_resources(client, "ReplicaSet"),
        count_resources(client, "Job"),
        count_resources(client, "CronJob"),
    );

    let node_list = nodes_result.context("Failed to list nodes")?;
    let pod_list = pods_result.context("Failed to list pods")?;
    let ns_list = ns_result.context("Failed to list namespaces")?;
    let event_list = events_result.context("Failed to list events")?;
    let version = version_result
        .map(|v| format!("{}.{}", v.major, v.minor))
        .unwrap_or_else(|_| "unknown".to_string());

    // Parse nodes with allocatable resources.
    let nodes: Vec<NodeInfo> = node_list
        .items
        .iter()
        .map(|node| {
            let name = node.metadata.name.clone().unwrap_or_else(|| "<unknown>".to_string());

            let ready = node
                .status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .map(|conds| conds.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
                .unwrap_or(false);

            let roles: Vec<String> = node
                .metadata
                .labels
                .as_ref()
                .map(|labels| {
                    labels
                        .keys()
                        .filter_map(|k| {
                            k.strip_prefix("node-role.kubernetes.io/").map(|r| r.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Parse allocatable CPU and memory from node status.
            let allocatable = node.status.as_ref().and_then(|s| s.allocatable.as_ref());

            let allocatable_cpu_millis =
                allocatable.and_then(|a| a.get("cpu")).and_then(|q| parse_cpu_quantity(&q.0));

            let allocatable_memory_bytes =
                allocatable.and_then(|a| a.get("memory")).and_then(|q| parse_memory_quantity(&q.0));

            NodeInfo { name, ready, roles, allocatable_cpu_millis, allocatable_memory_bytes }
        })
        .collect();

    // Parse pod counts.
    let mut running = 0u32;
    let mut pending = 0u32;
    let mut failed = 0u32;
    let mut succeeded = 0u32;

    for pod in &pod_list.items {
        match pod.status.as_ref().and_then(|s| s.phase.as_deref()).unwrap_or("Unknown") {
            "Running" => running += 1,
            "Pending" => pending += 1,
            "Failed" => failed += 1,
            "Succeeded" => succeeded += 1,
            _ => pending += 1,
        }
    }

    // Parse namespaces.
    let namespaces: Vec<String> =
        ns_list.items.iter().filter_map(|ns| ns.metadata.name.clone()).collect();

    // Parse events (most recent first).
    let mut events: Vec<EventInfo> = event_list
        .items
        .iter()
        .map(|evt| {
            let reason = evt.reason.clone().unwrap_or_default();
            let message = evt.message.clone().unwrap_or_default();
            let is_warning = evt.type_.as_deref() == Some("Warning");

            // Use last_timestamp, then event_time, then fallback to now.
            let timestamp = evt
                .last_timestamp
                .as_ref()
                .map(|t| t.0)
                .or_else(|| evt.event_time.as_ref().map(|t| t.0))
                .unwrap_or_else(Utc::now);

            let last_seen = evt
                .last_timestamp
                .as_ref()
                .map(|t| t.0)
                .or_else(|| evt.event_time.as_ref().map(|t| t.0));

            EventInfo {
                reason,
                message,
                timestamp,
                is_warning,
                namespace: evt.metadata.namespace.clone(),
                involved_object_kind: evt.involved_object.kind.clone(),
                involved_object_name: evt.involved_object.name.clone(),
                source: evt.source.as_ref().and_then(|s| s.component.clone()),
                count: evt.count.unwrap_or(1).max(0) as u32,
                last_seen,
            }
        })
        .collect();

    // Sort by timestamp descending.
    events.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
    events.truncate(50);

    let pod_count = pod_list.items.len() as u32;

    let resource_counts = DashboardResourceCounts {
        pods: pod_count,
        deployments: deploy_count,
        daemonsets: ds_count,
        statefulsets: ss_count,
        replicasets: rs_count,
        jobs: job_count,
        cronjobs: cj_count,
    };

    Ok(DashboardData {
        nodes,
        pod_counts: PodCounts { running, pending, failed, succeeded },
        namespaces,
        events,
        k8s_version: version,
        resource_counts,
    })
}

/// Count the number of items of a given resource kind. Returns 0 on error.
async fn count_resources(client: &Client, kind: &str) -> u32 {
    match list_resources(client, kind, None).await {
        Ok(items) => items.len() as u32,
        Err(e) => {
            tracing::warn!("Failed to count {kind}: {e}");
            0
        }
    }
}

/// Parse a Kubernetes CPU quantity string to millicores.
/// E.g. "4" → 4000, "500m" → 500, "1.5" → 1500, "100n" → 0, "100u" → 0
pub fn parse_cpu_quantity(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(millis) = s.strip_suffix('m') {
        millis.parse::<u64>().ok()
    } else if let Some(nanos) = s.strip_suffix('n') {
        nanos.parse::<u64>().ok().map(|n| n / 1_000_000)
    } else if let Some(micros) = s.strip_suffix('u') {
        micros.parse::<u64>().ok().map(|u| u / 1_000)
    } else if let Ok(cores) = s.parse::<f64>() {
        Some((cores * 1000.0) as u64)
    } else {
        None
    }
}

/// Parse a Kubernetes memory quantity string to bytes.
/// E.g. "16Gi" → 17179869184, "1024Mi" → 1073741824, "1000000Ki" → 1024000000000
pub fn parse_memory_quantity(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("Ki") {
        v.parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(v) = s.strip_suffix("Mi") {
        v.parse::<u64>().ok().map(|v| v * 1024 * 1024)
    } else if let Some(v) = s.strip_suffix("Gi") {
        v.parse::<u64>().ok().map(|v| v * 1024 * 1024 * 1024)
    } else if let Some(v) = s.strip_suffix("Ti") {
        v.parse::<u64>().ok().map(|v| v * 1024 * 1024 * 1024 * 1024)
    } else if let Some(v) = s.strip_suffix('K') {
        v.parse::<u64>().ok().map(|v| v * 1000)
    } else if let Some(v) = s.strip_suffix('M') {
        v.parse::<u64>().ok().map(|v| v * 1_000_000)
    } else if let Some(v) = s.strip_suffix('G') {
        v.parse::<u64>().ok().map(|v| v * 1_000_000_000)
    } else if let Some(v) = s.strip_suffix('T') {
        v.parse::<u64>().ok().map(|v| v * 1_000_000_000_000)
    } else {
        // Plain bytes
        s.parse::<u64>().ok()
    }
}

/// Fetch resources of a specific kind and return them as generic JSON values.
/// This is used for the resource list view.
pub async fn list_resources(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    // Validate inputs before interpolating into URL paths.
    validate_path_segment(kind, "resource kind")?;
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    // Resolve the API group/version/plural for this kind.
    let res = resolve_api_resource(kind);

    // Build a kube::api::ApiResource so we can use the typed DynamicObject API
    // which goes through the full kube-rs middleware (auth, base URI, etc.).
    let kube_api_resource = kube::api::ApiResource {
        group: res.group.clone(),
        version: res.version.clone(),
        kind: kind.to_string(),
        api_version: if res.group.is_empty() {
            res.version.clone()
        } else {
            format!("{}/{}", res.group, res.version)
        },
        plural: res.plural.clone(),
    };

    let api: Api<kube::api::DynamicObject> = if res.namespaced {
        if let Some(ns) = namespace {
            Api::namespaced_with(client.clone(), ns, &kube_api_resource)
        } else {
            Api::all_with(client.clone(), &kube_api_resource)
        }
    } else {
        Api::all_with(client.clone(), &kube_api_resource)
    };

    let list =
        api.list(&ListParams::default()).await.with_context(|| format!("Failed to list {kind}"))?;

    let items: Vec<serde_json::Value> =
        list.items.into_iter().filter_map(|obj| serde_json::to_value(&obj).ok()).collect();

    Ok(items)
}

/// Like [`list_resources`] but adds a label selector to filter server-side.
/// Used for Service → Pod selector matching in the topology view.
pub async fn list_resources_with_selector(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
    label_selector: &str,
) -> Result<Vec<serde_json::Value>> {
    validate_path_segment(kind, "resource kind")?;
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    let res = resolve_api_resource(kind);
    let kube_api_resource = kube::api::ApiResource {
        group: res.group.clone(),
        version: res.version.clone(),
        kind: kind.to_string(),
        api_version: if res.group.is_empty() {
            res.version.clone()
        } else {
            format!("{}/{}", res.group, res.version)
        },
        plural: res.plural.clone(),
    };

    let api: Api<kube::api::DynamicObject> = if res.namespaced {
        if let Some(ns) = namespace {
            Api::namespaced_with(client.clone(), ns, &kube_api_resource)
        } else {
            Api::all_with(client.clone(), &kube_api_resource)
        }
    } else {
        Api::all_with(client.clone(), &kube_api_resource)
    };

    let list = api
        .list(&ListParams::default().labels(label_selector))
        .await
        .with_context(|| format!("Failed to list {kind} with selector {label_selector}"))?;

    let items: Vec<serde_json::Value> =
        list.items.into_iter().filter_map(|obj| serde_json::to_value(&obj).ok()).collect();

    Ok(items)
}

struct ApiResource {
    group: String,
    version: String,
    plural: String,
    namespaced: bool,
    /// True when this resource was resolved via the fallback heuristic rather
    /// than a known mapping.  Mutating operations should reject fallback kinds.
    is_fallback: bool,
}

impl ApiResource {
    /// Create a known (non-fallback) API resource.
    fn known(group: &str, version: &str, plural: &str, namespaced: bool) -> Self {
        Self {
            group: group.to_string(),
            version: version.to_string(),
            plural: plural.to_string(),
            namespaced,
            is_fallback: false,
        }
    }
}

/// Map a resource kind to its API group, version, and plural name.
fn resolve_api_resource(kind: &str) -> ApiResource {
    match kind {
        "Pod" => ApiResource::known("", "v1", "pods", true),
        "Service" => ApiResource::known("", "v1", "services", true),
        "ConfigMap" => ApiResource::known("", "v1", "configmaps", true),
        "Secret" => ApiResource::known("", "v1", "secrets", true),
        "Namespace" => ApiResource::known("", "v1", "namespaces", false),
        "Node" => ApiResource::known("", "v1", "nodes", false),
        "Event" => ApiResource::known("", "v1", "events", true),
        "PersistentVolume" => ApiResource::known("", "v1", "persistentvolumes", false),
        "PersistentVolumeClaim" => ApiResource::known("", "v1", "persistentvolumeclaims", true),
        "ServiceAccount" => ApiResource::known("", "v1", "serviceaccounts", true),
        "Endpoints" => ApiResource::known("", "v1", "endpoints", true),
        "Deployment" => ApiResource::known("apps", "v1", "deployments", true),
        "StatefulSet" => ApiResource::known("apps", "v1", "statefulsets", true),
        "DaemonSet" => ApiResource::known("apps", "v1", "daemonsets", true),
        "ReplicaSet" => ApiResource::known("apps", "v1", "replicasets", true),
        "Job" => ApiResource::known("batch", "v1", "jobs", true),
        "CronJob" => ApiResource::known("batch", "v1", "cronjobs", true),
        "Ingress" => ApiResource::known("networking.k8s.io", "v1", "ingresses", true),
        "NetworkPolicy" => ApiResource::known("networking.k8s.io", "v1", "networkpolicies", true),
        "StorageClass" => ApiResource::known("storage.k8s.io", "v1", "storageclasses", false),
        "Role" => ApiResource::known("rbac.authorization.k8s.io", "v1", "roles", true),
        "ClusterRole" => {
            ApiResource::known("rbac.authorization.k8s.io", "v1", "clusterroles", false)
        }
        "RoleBinding" => {
            ApiResource::known("rbac.authorization.k8s.io", "v1", "rolebindings", true)
        }
        "ClusterRoleBinding" => {
            ApiResource::known("rbac.authorization.k8s.io", "v1", "clusterrolebindings", false)
        }
        "ReplicationController" => ApiResource::known("", "v1", "replicationcontrollers", true),
        "ResourceQuota" => ApiResource::known("", "v1", "resourcequotas", true),
        "LimitRange" => ApiResource::known("", "v1", "limitranges", true),
        "HorizontalPodAutoscaler" => {
            ApiResource::known("autoscaling", "v2", "horizontalpodautoscalers", true)
        }
        "VerticalPodAutoscaler" => {
            ApiResource::known("autoscaling.k8s.io", "v1", "verticalpodautoscalers", true)
        }
        "PodDisruptionBudget" => ApiResource::known("policy", "v1", "poddisruptionbudgets", true),
        "PriorityClass" => ApiResource::known("scheduling.k8s.io", "v1", "priorityclasses", false),
        "RuntimeClass" => ApiResource::known("node.k8s.io", "v1", "runtimeclasses", false),
        "Lease" => ApiResource::known("coordination.k8s.io", "v1", "leases", true),
        "MutatingWebhookConfiguration" => ApiResource::known(
            "admissionregistration.k8s.io",
            "v1",
            "mutatingwebhookconfigurations",
            false,
        ),
        "ValidatingWebhookConfiguration" => ApiResource::known(
            "admissionregistration.k8s.io",
            "v1",
            "validatingwebhookconfigurations",
            false,
        ),
        "IngressClass" => ApiResource::known("networking.k8s.io", "v1", "ingressclasses", false),
        "EndpointSlice" => ApiResource::known("discovery.k8s.io", "v1", "endpointslices", true),
        "PodSecurityPolicy" => {
            ApiResource::known("policy", "v1beta1", "podsecuritypolicies", false)
        }
        "CustomResourceDefinition" => {
            ApiResource::known("apiextensions.k8s.io", "v1", "customresourcedefinitions", false)
        }
        "Application" => ApiResource::known("argoproj.io", "v1alpha1", "applications", true),
        "ApplicationSet" => ApiResource::known("argoproj.io", "v1alpha1", "applicationsets", true),
        "AppProject" => ApiResource::known("argoproj.io", "v1alpha1", "appprojects", true),
        // Fallback: assume core API v1 namespaced.
        // Mutating operations must check is_fallback and reject unknown kinds.
        other => {
            tracing::warn!("Unknown resource kind '{other}', using fallback API path");
            ApiResource {
                group: String::new(),
                version: "v1".to_string(),
                plural: format!("{}s", other.to_lowercase()),
                namespaced: true,
                is_fallback: true,
            }
        }
    }
}

/// Validate that a string is safe to interpolate into a URL path segment.
/// Rejects empty strings, path-traversal sequences (`..`, `/`), and
/// percent-encoded characters (`%`) that could bypass the literal checks,
/// and query/fragment separators (`?`, `#`) that could inject query parameters.
fn validate_path_segment(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || value.contains("..")
        || value.contains('%')
        || value.contains('?')
        || value.contains('#')
    {
        anyhow::bail!("Invalid {label}: {value:?}");
    }
    Ok(())
}

/// Resolve the API resource for a kind and bail if it is a fallback (unknown kind).
/// Used by mutating operations to prevent sending writes to guessed API paths.
fn resolve_known_api_resource(kind: &str) -> Result<ApiResource> {
    let r = resolve_api_resource(kind);
    if r.is_fallback {
        anyhow::bail!("Refusing mutating operation on unknown resource kind '{kind}'");
    }
    Ok(r)
}

/// Build the REST URL for a specific resource instance.
///
/// Resolves the API group, version, and plural for the given `kind`, then
/// constructs the appropriate namespaced or cluster-scoped URL.
/// Returns an error if kind, name, or namespace contain path-traversal characters.
fn build_resource_url(kind: &str, name: &str, namespace: Option<&str>) -> Result<String> {
    let api_resource = resolve_api_resource(kind);
    build_resource_url_with(name, namespace, &api_resource)
}

/// Build a resource URL using a pre-resolved `ApiResource`.
/// Validates name and namespace against path-traversal before interpolation.
fn build_resource_url_with(
    name: &str,
    namespace: Option<&str>,
    api_resource: &ApiResource,
) -> Result<String> {
    validate_path_segment(name, "resource name")?;
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    Ok(match (namespace, api_resource) {
        (Some(ns), ApiResource { namespaced: true, group, version, plural, .. }) => {
            if group.is_empty() {
                format!("/api/{version}/namespaces/{ns}/{plural}/{name}")
            } else {
                format!("/apis/{group}/{version}/namespaces/{ns}/{plural}/{name}")
            }
        }
        (_, ApiResource { group, version, plural, .. }) => {
            if group.is_empty() {
                format!("/api/{version}/{plural}/{name}")
            } else {
                format!("/apis/{group}/{version}/{plural}/{name}")
            }
        }
    })
}

/// Fetch a single resource by kind, name, and optional namespace, returned as JSON.
/// Used by the resource detail view (T328).
pub async fn get_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<serde_json::Value> {
    validate_path_segment(kind, "resource kind")?;
    validate_path_segment(name, "resource name")?;
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    let res = resolve_api_resource(kind);
    let kube_api_resource = kube::api::ApiResource {
        group: res.group.clone(),
        version: res.version.clone(),
        kind: kind.to_string(),
        api_version: if res.group.is_empty() {
            res.version.clone()
        } else {
            format!("{}/{}", res.group, res.version)
        },
        plural: res.plural.clone(),
    };

    let api: Api<kube::api::DynamicObject> = if res.namespaced {
        if let Some(ns) = namespace {
            Api::namespaced_with(client.clone(), ns, &kube_api_resource)
        } else {
            Api::default_namespaced_with(client.clone(), &kube_api_resource)
        }
    } else {
        Api::all_with(client.clone(), &kube_api_resource)
    };

    let obj = api.get(name).await.with_context(|| format!("Failed to get {kind}/{name}"))?;

    serde_json::to_value(&obj).context("Failed to serialize resource")
}

/// Update (PUT) a Kubernetes resource with the given YAML body.
///
/// Parses the YAML into JSON, injects the provided `resource_version` for
/// optimistic concurrency, and sends an HTTP PUT. Returns the updated
/// resource JSON (which contains the new `resourceVersion`).
pub async fn update_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    yaml_body: &str,
    resource_version: &str,
) -> Result<serde_json::Value> {
    let api_resource = resolve_known_api_resource(kind)?;
    let request_url = build_resource_url_with(name, namespace, &api_resource)?;

    // Reject excessively large YAML to prevent parsing DoS.
    const MAX_YAML_SIZE: usize = 10 * 1024 * 1024; // 10 MB
    if yaml_body.len() > MAX_YAML_SIZE {
        anyhow::bail!(
            "YAML body exceeds maximum allowed size ({} bytes > {} bytes)",
            yaml_body.len(),
            MAX_YAML_SIZE,
        );
    }

    // Parse YAML → JSON
    let mut json: serde_json::Value =
        serde_yaml_ng::from_str(yaml_body).context("Failed to parse YAML body")?;

    // Validate that the body's identity matches the URL target to prevent
    // accidentally sending a PUT to the wrong resource endpoint.
    match json.pointer("/metadata/name").and_then(|v| v.as_str()) {
        Some(body_name) if body_name != name => {
            anyhow::bail!("Body metadata.name '{}' does not match target '{}'", body_name, name);
        }
        None => {
            anyhow::bail!("Body is missing metadata.name (expected '{}')", name);
        }
        _ => {} // matches
    }
    if let Some(expected_ns) = namespace {
        match json.pointer("/metadata/namespace").and_then(|v| v.as_str()) {
            Some(body_ns) if body_ns != expected_ns => {
                anyhow::bail!(
                    "Body metadata.namespace '{}' does not match target '{}'",
                    body_ns,
                    expected_ns
                );
            }
            None => {
                anyhow::bail!("Body is missing metadata.namespace (expected '{}')", expected_ns);
            }
            _ => {} // matches
        }
    }

    // Inject resourceVersion for optimistic concurrency control
    if let Some(metadata) = json.get_mut("metadata").and_then(|m| m.as_object_mut()) {
        metadata.insert(
            "resourceVersion".to_string(),
            serde_json::Value::String(resource_version.to_string()),
        );
    }

    let body_bytes = serde_json::to_vec(&json).context("Failed to serialize resource JSON")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::PUT)
                .uri(&request_url)
                .header("content-type", "application/json")
                .body(body_bytes)
                .context("Failed to build PUT request")?,
        )
        .await
        .with_context(|| format!("Failed to update {kind}/{name}"))?;

    Ok(response)
}

/// Delete a Kubernetes resource.
pub async fn delete_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<()> {
    let api_resource = resolve_known_api_resource(kind)?;
    let request_url = build_resource_url_with(name, namespace, &api_resource)?;

    let _response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::DELETE)
                .uri(&request_url)
                .body(Vec::new())
                .context("Failed to build DELETE request")?,
        )
        .await
        .with_context(|| format!("Failed to delete {kind}/{name}"))?;

    Ok(())
}

/// Scale a Kubernetes resource (Deployment, StatefulSet, ReplicaSet) to the
/// given number of replicas via the `/scale` subresource.
pub async fn scale_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    replicas: u32,
) -> Result<serde_json::Value> {
    let api_resource = resolve_known_api_resource(kind)?;
    let base_url = build_resource_url_with(name, namespace, &api_resource)?;
    let request_url = format!("{base_url}/scale");

    let patch_body = serde_json::json!({
        "spec": {
            "replicas": replicas
        }
    });
    let body_bytes = serde_json::to_vec(&patch_body).context("Failed to serialize scale patch")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::PATCH)
                .uri(&request_url)
                .header("content-type", "application/strategic-merge-patch+json")
                .body(body_bytes)
                .context("Failed to build PATCH request")?,
        )
        .await
        .with_context(|| format!("Failed to scale {kind}/{name}"))?;

    Ok(response)
}

/// Restart a workload (Deployment, StatefulSet, DaemonSet) by patching the
/// pod template annotation `kubectl.kubernetes.io/restartedAt`.
///
/// This mimics `kubectl rollout restart` behavior.
pub async fn restart_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<serde_json::Value> {
    let api_resource = resolve_known_api_resource(kind)?;
    let request_url = build_resource_url_with(name, namespace, &api_resource)?;
    let now = chrono::Utc::now().to_rfc3339();

    let patch_body = serde_json::json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": now
                    }
                }
            }
        }
    });
    let body_bytes =
        serde_json::to_vec(&patch_body).context("Failed to serialize restart patch")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::PATCH)
                .uri(&request_url)
                .header("content-type", "application/strategic-merge-patch+json")
                .body(body_bytes)
                .context("Failed to build PATCH request")?,
        )
        .await
        .with_context(|| format!("Failed to restart {kind}/{name}"))?;

    Ok(response)
}

/// Cordon a Node by setting `spec.unschedulable = true`.
pub async fn cordon_node(client: &Client, name: &str) -> Result<serde_json::Value> {
    let request_url = build_resource_url("Node", name, None)?;

    let patch_body = serde_json::json!({
        "spec": {
            "unschedulable": true
        }
    });
    let body_bytes = serde_json::to_vec(&patch_body).context("Failed to serialize cordon patch")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::PATCH)
                .uri(&request_url)
                .header("content-type", "application/strategic-merge-patch+json")
                .body(body_bytes)
                .context("Failed to build PATCH request")?,
        )
        .await
        .with_context(|| format!("Failed to cordon node {name}"))?;

    Ok(response)
}

/// Uncordon a Node by setting `spec.unschedulable = false`.
pub async fn uncordon_node(client: &Client, name: &str) -> Result<serde_json::Value> {
    let request_url = build_resource_url("Node", name, None)?;

    let patch_body = serde_json::json!({
        "spec": {
            "unschedulable": false
        }
    });
    let body_bytes =
        serde_json::to_vec(&patch_body).context("Failed to serialize uncordon patch")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::PATCH)
                .uri(&request_url)
                .header("content-type", "application/strategic-merge-patch+json")
                .body(body_bytes)
                .context("Failed to build PATCH request")?,
        )
        .await
        .with_context(|| format!("Failed to uncordon node {name}"))?;

    Ok(response)
}

/// Create a new Kubernetes resource from a JSON body.
pub async fn create_resource(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
    json_body: &serde_json::Value,
) -> Result<serde_json::Value> {
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    let api_resource = resolve_known_api_resource(kind)?;
    let request_url = match (namespace, &api_resource) {
        (Some(ns), ApiResource { namespaced: true, group, version, plural, .. }) => {
            if group.is_empty() {
                format!("/api/{version}/namespaces/{ns}/{plural}")
            } else {
                format!("/apis/{group}/{version}/namespaces/{ns}/{plural}")
            }
        }
        (_, ApiResource { group, version, plural, .. }) => {
            if group.is_empty() {
                format!("/api/{version}/{plural}")
            } else {
                format!("/apis/{group}/{version}/{plural}")
            }
        }
    };

    let body_bytes = serde_json::to_vec(json_body).context("Failed to serialize resource JSON")?;

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .method(http::Method::POST)
                .uri(&request_url)
                .header("content-type", "application/json")
                .body(body_bytes)
                .context("Failed to build POST request")?,
        )
        .await
        .with_context(|| format!("Failed to create {kind}"))?;

    Ok(response)
}

/// Inner loop for `watch_events`: drives a pre-constructed stream with cancellation.
///
/// Takes the stream by value (making the future `'static` for `tokio::spawn`).
/// Polls the cancellation token first (`biased;`) so a cancel signal stops the loop
/// within one poll cycle, satisfying spec 06 0002-H1 acceptance criterion 1a.
async fn watch_events_inner<S, F>(stream: S, token: CancellationToken, mut on_event: F) -> Result<()>
where
    S: Stream<Item = Result<WatcherEvent<Event>, watcher::Error>> + Send + 'static,
    F: FnMut(EventInfo) + Send,
{
    tokio::pin!(stream);
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => return Ok(()),
            next = stream.try_next() => {
                match next.context("Event watcher stream error")? {
                    Some(watch_event) => {
                        match watch_event {
                            WatcherEvent::Apply(evt) | WatcherEvent::InitApply(evt) => {
                                let reason = evt.reason.clone().unwrap_or_default();
                                let message = evt.message.clone().unwrap_or_default();
                                let is_warning = evt.type_.as_deref() == Some("Warning");
                                let timestamp = evt
                                    .last_timestamp
                                    .as_ref()
                                    .map(|t| t.0)
                                    .or_else(|| evt.event_time.as_ref().map(|t| t.0))
                                    .unwrap_or_else(Utc::now);
                                let last_seen = evt
                                    .last_timestamp
                                    .as_ref()
                                    .map(|t| t.0)
                                    .or_else(|| evt.event_time.as_ref().map(|t| t.0));
                                on_event(EventInfo {
                                    reason,
                                    message,
                                    timestamp,
                                    is_warning,
                                    namespace: evt.metadata.namespace.clone(),
                                    involved_object_kind: evt.involved_object.kind.clone(),
                                    involved_object_name: evt.involved_object.name.clone(),
                                    source: evt.source.as_ref().and_then(|s| s.component.clone()),
                                    count: evt.count.unwrap_or(1).max(0) as u32,
                                    last_seen,
                                });
                            }
                            WatcherEvent::Delete(_) | WatcherEvent::Init | WatcherEvent::InitDone => {
                                // Deletions and bookmark/init events are not surfaced to the UI event feed.
                            }
                        }
                    }
                    None => return Ok(()),
                }
            }
        }
    }
}

/// Watch Kubernetes Events in real time and invoke a callback for each new event.
///
/// This opens a kube-rs watcher stream on `core/v1/Event` resources (cluster-wide)
/// and calls `on_event` for each event received. The callback receives an `EventInfo`
/// describing the event.
///
/// Pass `Some(token)` to stop the loop deterministically by cancelling the token.
/// Pass `None` to get the previous behaviour: a fresh token is created internally
/// and the loop runs until the stream ends or an error occurs (backward-compatible).
///
/// Callers should run this on a background Tokio task and use the callback to
/// forward events to the UI thread.
///
/// T326: Used by AppShell::start_event_watcher.
pub async fn watch_events<F>(
    client: &Client,
    cancel: Option<CancellationToken>,
    on_event: F,
) -> Result<()>
where
    F: FnMut(EventInfo) + Send,
{
    let token = cancel.unwrap_or_default();
    let events_api: Api<Event> = Api::all(client.clone());
    let watch_config = watcher::Config::default();
    let stream = kube_runtime::watcher(events_api, watch_config).default_backoff();
    watch_events_inner(stream, token, on_event).await
}

/// Inner loop for `watch_resources`: drives a pre-constructed stream with cancellation.
///
/// Mirrors `watch_events_inner` for dynamic-object streams. Maintains a local
/// snapshot of items and calls `on_change` on every mutation event.
async fn watch_resources_inner<S, F>(stream: S, token: CancellationToken, mut on_change: F) -> Result<()>
where
    S: Stream<Item = Result<WatcherEvent<kube::api::DynamicObject>, watcher::Error>> + Send + 'static,
    F: FnMut(Vec<serde_json::Value>) + Send,
{
    tokio::pin!(stream);
    let mut items: Vec<serde_json::Value> = Vec::new();
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => return Ok(()),
            next = stream.try_next() => {
                match next.context("Resource watcher stream error")? {
                    Some(watch_event) => {
                        match watch_event {
                            WatcherEvent::Init => {
                                items.clear();
                            }
                            WatcherEvent::InitApply(obj) => {
                                if let Ok(val) = serde_json::to_value(&obj) {
                                    items.push(val);
                                }
                            }
                            WatcherEvent::InitDone => {
                                on_change(items.clone());
                            }
                            WatcherEvent::Apply(obj) => {
                                // Upsert: replace existing item with same UID, or add new.
                                let new_uid = obj.metadata.uid.as_deref().unwrap_or("");
                                if let Ok(val) = serde_json::to_value(&obj) {
                                    if let Some(pos) = items.iter().position(|item| {
                                        item.pointer("/metadata/uid")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            == new_uid
                                    }) {
                                        items[pos] = val;
                                    } else {
                                        items.push(val);
                                    }
                                    on_change(items.clone());
                                }
                            }
                            WatcherEvent::Delete(obj) => {
                                let del_uid = obj.metadata.uid.as_deref().unwrap_or("");
                                items.retain(|item| {
                                    item.pointer("/metadata/uid")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        != del_uid
                                });
                                on_change(items.clone());
                            }
                        }
                    }
                    None => return Ok(()),
                }
            }
        }
    }
}

/// Watch a specific resource kind in real time and invoke callbacks for changes.
///
/// Streams Applied, Deleted, Init, and InitDone events for the given resource kind
/// and optional namespace. `on_change` is called with the full updated list of items
/// whenever a change occurs (full snapshot replacement strategy).
///
/// Pass `Some(token)` to stop the loop deterministically by cancelling the token.
/// Pass `None` to get the previous behaviour: a fresh token is created internally
/// and the loop runs until the stream ends or an error occurs (backward-compatible).
///
/// T327b: Used by AppShell for informer-backed live updates on resource list views.
pub async fn watch_resources<F>(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
    cancel: Option<CancellationToken>,
    on_change: F,
) -> Result<()>
where
    F: FnMut(Vec<serde_json::Value>) + Send,
{
    let token = cancel.unwrap_or_default();
    let api_resource = resolve_api_resource(kind);

    // Use kube's dynamic API via raw JSON to build a watcher.
    // We use the DynamicObject approach with kube_runtime::watcher.
    let kube_api_resource = kube::api::ApiResource {
        group: api_resource.group.clone(),
        version: api_resource.version.clone(),
        kind: kind.to_string(),
        api_version: if api_resource.group.is_empty() {
            api_resource.version.clone()
        } else {
            format!("{}/{}", api_resource.group, api_resource.version)
        },
        plural: api_resource.plural.clone(),
    };

    let api: Api<kube::api::DynamicObject> = if api_resource.namespaced {
        if let Some(ns) = namespace {
            Api::namespaced_with(client.clone(), ns, &kube_api_resource)
        } else {
            Api::all_with(client.clone(), &kube_api_resource)
        }
    } else {
        Api::all_with(client.clone(), &kube_api_resource)
    };

    let watch_config = watcher::Config::default();
    let stream = kube_runtime::watcher(api, watch_config).default_backoff();
    watch_resources_inner(stream, token, on_change).await
}

// ---------------------------------------------------------------------------
// Metrics-server API: fetch real CPU/memory usage from metrics.k8s.io/v1beta1
// ---------------------------------------------------------------------------

use crate::metrics::{ContainerMetrics, NodeMetrics, PodMetrics};

/// Raw metrics response from the metrics-server API.
#[derive(Debug, Deserialize)]
struct MetricsNodeItem {
    metadata: MetricsMetadata,
    usage: MetricsUsage,
}

#[derive(Debug, Deserialize)]
struct MetricsMetadata {
    name: String,
    #[serde(default)]
    namespace: String,
}

#[derive(Debug, Deserialize)]
struct MetricsUsage {
    cpu: String,
    memory: String,
}

#[derive(Debug, Deserialize)]
struct MetricsPodItem {
    metadata: MetricsMetadata,
    containers: Vec<MetricsContainerItem>,
}

#[derive(Debug, Deserialize)]
struct MetricsContainerItem {
    name: String,
    usage: MetricsUsage,
}

/// Fetch node metrics from the metrics-server API and combine with node
/// allocatable capacity from the core API.
///
/// Returns a `NodeMetrics` for each node that has both metrics and capacity data.
pub async fn fetch_node_metrics(client: &Client) -> Result<Vec<NodeMetrics>> {
    let (raw_metrics, allocatable) =
        tokio::join!(fetch_raw_node_metrics(client), fetch_node_allocatable(client),);

    let raw_metrics = raw_metrics?;
    let allocatable = allocatable.unwrap_or_default();
    let cluster_id = uuid::Uuid::nil();

    let mut result = Vec::new();
    for item in raw_metrics {
        let cpu_usage = parse_cpu_quantity(&item.usage.cpu).unwrap_or(0);
        let mem_usage = parse_memory_quantity(&item.usage.memory).unwrap_or(0);

        let (cpu_cap, mem_cap) = allocatable.get(&item.metadata.name).cloned().unwrap_or((0, 0));

        result.push(NodeMetrics {
            node_name: item.metadata.name,
            cpu_usage_millicores: cpu_usage,
            cpu_capacity_millicores: cpu_cap,
            memory_usage_bytes: mem_usage,
            memory_capacity_bytes: mem_cap,
            timestamp: Utc::now(),
            cluster_id,
        });
    }

    Ok(result)
}

/// Fetch pod metrics from the metrics-server API.
///
/// If `namespace` is `Some`, only fetches metrics for that namespace.
/// Otherwise fetches for all namespaces.
pub async fn fetch_pod_metrics(
    client: &Client,
    namespace: Option<&str>,
) -> Result<Vec<PodMetrics>> {
    if let Some(ns) = namespace {
        validate_path_segment(ns, "namespace")?;
    }

    let url = match namespace {
        Some(ns) => format!("/apis/metrics.k8s.io/v1beta1/namespaces/{ns}/pods"),
        None => "/apis/metrics.k8s.io/v1beta1/pods".to_string(),
    };

    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .uri(&url)
                .body(Vec::new())
                .context("Failed to build metrics request")?,
        )
        .await
        .context("Failed to fetch pod metrics from metrics-server")?;

    let items: Vec<MetricsPodItem> = response
        .get("items")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let cluster_id = uuid::Uuid::nil();

    let result = items
        .into_iter()
        .map(|item| {
            let containers = item
                .containers
                .into_iter()
                .map(|c| ContainerMetrics {
                    container_name: c.name,
                    cpu_usage_millicores: parse_cpu_quantity(&c.usage.cpu).unwrap_or(0),
                    memory_usage_bytes: parse_memory_quantity(&c.usage.memory).unwrap_or(0),
                })
                .collect();

            PodMetrics {
                pod_name: item.metadata.name,
                namespace: item.metadata.namespace,
                containers,
                timestamp: Utc::now(),
                cluster_id,
            }
        })
        .collect();

    Ok(result)
}

/// Fetch raw node metrics from the metrics-server API.
async fn fetch_raw_node_metrics(client: &Client) -> Result<Vec<MetricsNodeItem>> {
    let response: serde_json::Value = client
        .request(
            http::Request::builder()
                .uri("/apis/metrics.k8s.io/v1beta1/nodes")
                .body(Vec::new())
                .context("Failed to build node metrics request")?,
        )
        .await
        .context("Failed to fetch node metrics from metrics-server")?;

    let items: Vec<MetricsNodeItem> = response
        .get("items")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    Ok(items)
}

/// Fetch allocatable CPU (millicores) and memory (bytes) per node.
/// Returns a map of node_name → (cpu_millis, memory_bytes).
async fn fetch_node_allocatable(
    client: &Client,
) -> Result<std::collections::HashMap<String, (u64, u64)>> {
    let nodes_api: Api<Node> = Api::all(client.clone());
    let node_list = nodes_api
        .list(&ListParams::default())
        .await
        .context("Failed to list nodes for allocatable resources")?;

    let mut map = std::collections::HashMap::new();
    for node in &node_list.items {
        let name = node.metadata.name.as_deref().unwrap_or("<unknown>").to_string();
        let allocatable = node.status.as_ref().and_then(|s| s.allocatable.as_ref());

        let cpu = allocatable
            .and_then(|a| a.get("cpu"))
            .and_then(|q| parse_cpu_quantity(&q.0))
            .unwrap_or(0);
        let mem = allocatable
            .and_then(|a| a.get("memory"))
            .and_then(|q| parse_memory_quantity(&q.0))
            .unwrap_or(0);

        map.insert(name, (cpu, mem));
    }

    Ok(map)
}

/// Format CPU millicores for display.
/// E.g. 1500 → "1500m", 0 → "0m"
pub fn format_cpu_millicores(millis: u64) -> String {
    if millis >= 1000 && millis % 1000 == 0 {
        format!("{}", millis / 1000)
    } else {
        format!("{millis}m")
    }
}

/// Format memory bytes for display.
/// E.g. 1073741824 → "1.0Gi", 536870912 → "512Mi"
pub fn format_memory_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        let gi = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        if gi >= 10.0 { format!("{:.0}Gi", gi) } else { format!("{:.1}Gi", gi) }
    } else if bytes >= 1024 * 1024 {
        let mi = bytes as f64 / (1024.0 * 1024.0);
        if mi >= 10.0 { format!("{:.0}Mi", mi) } else { format!("{:.1}Mi", mi) }
    } else if bytes >= 1024 {
        format!("{}Ki", bytes / 1024)
    } else {
        format!("{bytes}B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_api_resource_core_types() {
        let pod = resolve_api_resource("Pod");
        assert_eq!(pod.plural, "pods");
        assert!(pod.group.is_empty());
        assert!(pod.namespaced);

        let node = resolve_api_resource("Node");
        assert_eq!(node.plural, "nodes");
        assert!(!node.namespaced);

        let ns = resolve_api_resource("Namespace");
        assert_eq!(ns.plural, "namespaces");
        assert!(!ns.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_apps_types() {
        let deploy = resolve_api_resource("Deployment");
        assert_eq!(deploy.group, "apps");
        assert_eq!(deploy.plural, "deployments");
        assert!(deploy.namespaced);

        let ss = resolve_api_resource("StatefulSet");
        assert_eq!(ss.group, "apps");
        assert_eq!(ss.plural, "statefulsets");
    }

    #[test]
    fn test_resolve_api_resource_batch_types() {
        let job = resolve_api_resource("Job");
        assert_eq!(job.group, "batch");
        assert_eq!(job.plural, "jobs");

        let cj = resolve_api_resource("CronJob");
        assert_eq!(cj.group, "batch");
        assert_eq!(cj.plural, "cronjobs");
    }

    #[test]
    fn test_resolve_api_resource_rbac_types() {
        let role = resolve_api_resource("Role");
        assert_eq!(role.group, "rbac.authorization.k8s.io");
        assert!(role.namespaced);

        let cr = resolve_api_resource("ClusterRole");
        assert!(!cr.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_unknown_fallback() {
        let unknown = resolve_api_resource("FooBar");
        assert_eq!(unknown.plural, "foobars");
        assert!(unknown.group.is_empty());
        assert!(unknown.namespaced);
    }

    // T321: FR-071 new resource kinds
    #[test]
    fn test_resolve_api_resource_replication_controller() {
        let rc = resolve_api_resource("ReplicationController");
        assert!(rc.group.is_empty());
        assert_eq!(rc.plural, "replicationcontrollers");
        assert!(rc.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_config_types() {
        let rq = resolve_api_resource("ResourceQuota");
        assert!(rq.group.is_empty());
        assert_eq!(rq.plural, "resourcequotas");
        assert!(rq.namespaced);

        let lr = resolve_api_resource("LimitRange");
        assert!(lr.group.is_empty());
        assert_eq!(lr.plural, "limitranges");
        assert!(lr.namespaced);

        let lease = resolve_api_resource("Lease");
        assert_eq!(lease.group, "coordination.k8s.io");
        assert_eq!(lease.plural, "leases");
        assert!(lease.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_autoscaling_types() {
        let hpa = resolve_api_resource("HorizontalPodAutoscaler");
        assert_eq!(hpa.group, "autoscaling");
        assert_eq!(hpa.version, "v2");
        assert_eq!(hpa.plural, "horizontalpodautoscalers");
        assert!(hpa.namespaced);

        let vpa = resolve_api_resource("VerticalPodAutoscaler");
        assert_eq!(vpa.group, "autoscaling.k8s.io");
        assert_eq!(vpa.plural, "verticalpodautoscalers");
        assert!(vpa.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_policy_types() {
        let pdb = resolve_api_resource("PodDisruptionBudget");
        assert_eq!(pdb.group, "policy");
        assert_eq!(pdb.plural, "poddisruptionbudgets");
        assert!(pdb.namespaced);

        let psp = resolve_api_resource("PodSecurityPolicy");
        assert_eq!(psp.group, "policy");
        assert_eq!(psp.plural, "podsecuritypolicies");
        assert!(!psp.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_scheduling_and_node_types() {
        let pc = resolve_api_resource("PriorityClass");
        assert_eq!(pc.group, "scheduling.k8s.io");
        assert_eq!(pc.plural, "priorityclasses");
        assert!(!pc.namespaced);

        let rc = resolve_api_resource("RuntimeClass");
        assert_eq!(rc.group, "node.k8s.io");
        assert_eq!(rc.plural, "runtimeclasses");
        assert!(!rc.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_admission_types() {
        let mwc = resolve_api_resource("MutatingWebhookConfiguration");
        assert_eq!(mwc.group, "admissionregistration.k8s.io");
        assert_eq!(mwc.plural, "mutatingwebhookconfigurations");
        assert!(!mwc.namespaced);

        let vwc = resolve_api_resource("ValidatingWebhookConfiguration");
        assert_eq!(vwc.group, "admissionregistration.k8s.io");
        assert_eq!(vwc.plural, "validatingwebhookconfigurations");
        assert!(!vwc.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_ingress_class() {
        let ic = resolve_api_resource("IngressClass");
        assert_eq!(ic.group, "networking.k8s.io");
        assert_eq!(ic.plural, "ingressclasses");
        assert!(!ic.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_networking_types() {
        let ingress = resolve_api_resource("Ingress");
        assert_eq!(ingress.group, "networking.k8s.io");
        assert_eq!(ingress.plural, "ingresses");
        assert!(ingress.namespaced);

        let np = resolve_api_resource("NetworkPolicy");
        assert_eq!(np.group, "networking.k8s.io");
        assert_eq!(np.plural, "networkpolicies");
        assert!(np.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_storage_types() {
        let sc = resolve_api_resource("StorageClass");
        assert_eq!(sc.group, "storage.k8s.io");
        assert_eq!(sc.plural, "storageclasses");
        assert!(!sc.namespaced);

        let pv = resolve_api_resource("PersistentVolume");
        assert!(pv.group.is_empty());
        assert!(!pv.namespaced);

        let pvc = resolve_api_resource("PersistentVolumeClaim");
        assert!(pvc.group.is_empty());
        assert!(pvc.namespaced);
    }

    #[test]
    fn test_resolve_api_resource_all_core_v1() {
        // Verify all core/v1 resources
        for kind in &[
            "Pod",
            "Service",
            "ConfigMap",
            "Secret",
            "Namespace",
            "Node",
            "Event",
            "PersistentVolume",
            "PersistentVolumeClaim",
            "ServiceAccount",
            "Endpoints",
            "ReplicationController",
            "ResourceQuota",
            "LimitRange",
        ] {
            let r = resolve_api_resource(kind);
            assert!(r.group.is_empty(), "{kind} should be in core API group");
            assert_eq!(r.version, "v1", "{kind} should be v1");
        }
    }

    // --- T364: RBAC error handling tests ---

    #[test]
    fn test_format_rbac_error_namespaced() {
        let msg = format_rbac_error("list", "Pods", Some("default"));
        assert_eq!(
            msg,
            "Permission denied: you do not have permission to list Pods in namespace \"default\""
        );
    }

    #[test]
    fn test_format_rbac_error_cluster_scoped() {
        let msg = format_rbac_error("get", "Nodes", None);
        assert_eq!(
            msg,
            "Permission denied: you do not have permission to get Nodes (cluster-scoped)"
        );
    }

    #[test]
    fn test_format_rbac_error_various_verbs() {
        let msg = format_rbac_error("delete", "Secrets", Some("kube-system"));
        assert!(msg.contains("delete"));
        assert!(msg.contains("Secrets"));
        assert!(msg.contains("kube-system"));
        assert!(msg.starts_with("Permission denied:"));
    }

    #[test]
    fn test_is_forbidden_error_string_with_403() {
        assert!(is_forbidden_error_string("API error: 403 Forbidden"));
        assert!(is_forbidden_error_string("Error: 403 - access denied"));
        assert!(is_forbidden_error_string("forbidden: user lacks permission"));
        assert!(is_forbidden_error_string("Permission denied for resource"));
    }

    #[test]
    fn test_is_forbidden_error_string_non_forbidden() {
        assert!(!is_forbidden_error_string("API error: 404 Not Found"));
        assert!(!is_forbidden_error_string("connection timeout"));
        assert!(!is_forbidden_error_string("internal server error 500"));
        assert!(!is_forbidden_error_string(""));
    }

    #[test]
    fn test_is_forbidden_error_string_case_insensitive() {
        assert!(is_forbidden_error_string("FORBIDDEN"));
        assert!(is_forbidden_error_string("Forbidden"));
        assert!(is_forbidden_error_string("PERMISSION DENIED"));
    }

    #[test]
    fn test_is_forbidden_error_with_kube_api_error() {
        // Construct a kube::Error::Api with a 403 response.
        let error_response = kube::error::ErrorResponse {
            status: "Failure".to_string(),
            message: "pods is forbidden".to_string(),
            reason: "Forbidden".to_string(),
            code: 403,
        };
        let kube_err = kube::Error::Api(error_response);
        let anyhow_err = anyhow::Error::new(kube_err);
        assert!(is_forbidden_error(&anyhow_err));
    }

    #[test]
    fn test_is_forbidden_error_with_non_403_kube_error() {
        let error_response = kube::error::ErrorResponse {
            status: "Failure".to_string(),
            message: "not found".to_string(),
            reason: "NotFound".to_string(),
            code: 404,
        };
        let kube_err = kube::Error::Api(error_response);
        let anyhow_err = anyhow::Error::new(kube_err);
        assert!(!is_forbidden_error(&anyhow_err));
    }

    #[test]
    fn test_is_forbidden_error_with_wrapped_anyhow() {
        // Wrap a kube 403 error inside anyhow context.
        let error_response = kube::error::ErrorResponse {
            status: "Failure".to_string(),
            message: "pods is forbidden".to_string(),
            reason: "Forbidden".to_string(),
            code: 403,
        };
        let kube_err = kube::Error::Api(error_response);
        let anyhow_err = anyhow::Error::new(kube_err).context("Failed to list pods");
        assert!(is_forbidden_error(&anyhow_err));
    }

    #[test]
    fn test_is_forbidden_error_with_non_kube_error() {
        let anyhow_err = anyhow::anyhow!("some random error");
        assert!(!is_forbidden_error(&anyhow_err));
    }

    #[test]
    fn test_rbac_denied_new() {
        let denied = RbacDenied::new("list", "Pods", Some("default"));
        assert_eq!(denied.verb, "list");
        assert_eq!(denied.resource, "Pods");
        assert_eq!(denied.namespace.as_deref(), Some("default"));
        assert!(denied.message.contains("Permission denied"));
        assert!(denied.message.contains("list"));
        assert!(denied.message.contains("Pods"));
        assert!(denied.message.contains("default"));
    }

    #[test]
    fn test_rbac_denied_cluster_scoped() {
        let denied = RbacDenied::new("get", "Nodes", None);
        assert_eq!(denied.verb, "get");
        assert_eq!(denied.resource, "Nodes");
        assert!(denied.namespace.is_none());
        assert!(denied.message.contains("cluster-scoped"));
    }

    #[test]
    fn test_rbac_denied_display() {
        let denied = RbacDenied::new("delete", "Secrets", Some("kube-system"));
        let displayed = format!("{}", denied);
        assert_eq!(displayed, denied.message);
    }

    #[test]
    fn test_rbac_denied_serialization() {
        let denied = RbacDenied::new("list", "Pods", Some("default"));
        let json = serde_json::to_string(&denied).unwrap();
        let deserialized: RbacDenied = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, denied);
    }

    #[test]
    fn test_rbac_denied_from_error_forbidden() {
        let denied =
            rbac_denied_from_error("API error: 403 Forbidden", "list", "Pods", Some("default"));
        assert!(denied.is_some());
        let denied = denied.unwrap();
        assert_eq!(denied.verb, "list");
        assert_eq!(denied.resource, "Pods");
    }

    #[test]
    fn test_rbac_denied_from_error_not_forbidden() {
        let denied =
            rbac_denied_from_error("API error: 404 Not Found", "get", "Pods", Some("default"));
        assert!(denied.is_none());
    }

    // --- Slice B step 1: watch inner helper cancellation tests ---

    #[tokio::test]
    async fn watch_events_inner_stops_on_cancellation() {
        use futures::stream;
        let pending = stream::pending::<Result<WatcherEvent<Event>, watcher::Error>>();
        let token = CancellationToken::new();
        let token_for_task = token.clone();
        let handle = tokio::spawn(async move {
            watch_events_inner(pending, token_for_task, |_| {}).await.unwrap();
        });
        token.cancel();
        tokio::time::timeout(std::time::Duration::from_millis(100), handle)
            .await
            .expect("watch_events_inner did not stop within 100 ms after cancellation")
            .expect("task join error");
    }

    #[tokio::test]
    async fn watch_resources_inner_stops_on_cancellation() {
        use futures::stream;
        let pending =
            stream::pending::<Result<WatcherEvent<kube::api::DynamicObject>, watcher::Error>>();
        let token = CancellationToken::new();
        let token_for_task = token.clone();
        let handle = tokio::spawn(async move {
            watch_resources_inner(pending, token_for_task, |_| {}).await.unwrap();
        });
        token.cancel();
        tokio::time::timeout(std::time::Duration::from_millis(100), handle)
            .await
            .expect("watch_resources_inner did not stop within 100 ms after cancellation")
            .expect("task join error");
    }
}
