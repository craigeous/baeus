//! JSON extraction utilities for converting raw Kubernetes JSON into table rows.
//!
//! Bridges `serde_json::Value` from the K8s API to `TableRow` cells matching
//! the column order defined in `columns_for_kind()`.

use gpui::Rgba;

use crate::components::pod_detail::*;
use crate::components::resource_table::{TableRow, columns_for_kind};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Status coloring
// ---------------------------------------------------------------------------

/// Returns a semantic color for a status value, or None if default text color should be used.
pub fn status_color(value: &str, theme: &Theme) -> Option<Rgba> {
    match value {
        "Running" | "Active" | "Bound" | "Succeeded" | "Complete" | "Completed" | "Ready"
        | "Healthy" | "Available" | "True" => Some(theme.colors.success.to_gpui()),
        "Pending" | "Waiting" | "Suspended" | "Scheduling" | "Progressing" => {
            Some(theme.colors.warning.to_gpui())
        }
        "Failed"
        | "CrashLoopBackOff"
        | "Error"
        | "Evicted"
        | "ImagePullBackOff"
        | "ErrImagePull"
        | "OOMKilled"
        | "CreateContainerError"
        | "InvalidImageName"
        | "RunContainerError"
        | "False" => Some(theme.colors.error.to_gpui()),
        "Terminating" | "Terminated" | "Unknown" => Some(theme.colors.text_muted.to_gpui()),
        "ContainerCreating" => Some(theme.colors.info.to_gpui()),
        _ => None,
    }
}

/// Returns a semantic color for a "ready/total" pod count string.
pub fn pods_color(value: &str, theme: &Theme) -> Option<Rgba> {
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let ready: i64 = parts[0].parse().ok()?;
    let desired: i64 = parts[1].parse().ok()?;
    if desired == 0 {
        return None;
    }
    if ready == 0 {
        Some(theme.colors.error.to_gpui())
    } else if ready < desired {
        Some(theme.colors.warning.to_gpui())
    } else {
        Some(theme.colors.success.to_gpui())
    }
}

// ---------------------------------------------------------------------------
// Container brick statuses
// ---------------------------------------------------------------------------

/// Status of an individual container for colored brick rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerBrickStatus {
    Running,
    Waiting,
    Terminated,
    Failed,
    Creating,
    Restarted,
}

/// Parse `/status/containerStatuses[]` into per-container brick statuses.
pub fn extract_container_statuses(json: &serde_json::Value) -> Vec<ContainerBrickStatus> {
    let mut result = Vec::new();
    if let Some(statuses) = json.pointer("/status/containerStatuses").and_then(|v| v.as_array()) {
        for cs in statuses {
            let restart_count = cs.get("restartCount").and_then(|v| v.as_i64()).unwrap_or(0);
            if cs.pointer("/state/running").is_some() {
                if restart_count > 0 {
                    result.push(ContainerBrickStatus::Restarted);
                } else {
                    result.push(ContainerBrickStatus::Running);
                }
            } else if let Some(waiting) = cs.pointer("/state/waiting") {
                let reason = waiting.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                if reason == "ContainerCreating" {
                    result.push(ContainerBrickStatus::Creating);
                } else if reason == "CrashLoopBackOff" || reason == "Error" || reason == "OOMKilled"
                {
                    result.push(ContainerBrickStatus::Failed);
                } else {
                    result.push(ContainerBrickStatus::Waiting);
                }
            } else if let Some(terminated) = cs.pointer("/state/terminated") {
                let exit_code = terminated.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                if exit_code != 0 {
                    result.push(ContainerBrickStatus::Failed);
                } else {
                    result.push(ContainerBrickStatus::Terminated);
                }
            } else {
                result.push(ContainerBrickStatus::Waiting);
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Condition extraction
// ---------------------------------------------------------------------------

/// Extract conditions as (type, is_true) summary pairs from `/status/conditions[]`.
/// This is a lightweight version for table row badges (not the full detail view).
pub fn extract_condition_summary(json: &serde_json::Value) -> Vec<(String, bool)> {
    json.pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let ctype = c.get("type")?.as_str()?.to_string();
                    let status = c.get("status")?.as_str()?;
                    Some((ctype, status == "True"))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Core dispatch
// ---------------------------------------------------------------------------

/// Convert a raw K8s JSON object into a `TableRow` whose cells match the
/// column order returned by `columns_for_kind(kind)`.
pub fn json_to_table_row(kind: &str, json: &serde_json::Value) -> TableRow {
    let columns = columns_for_kind(kind);
    let uid = json_str(json, "/metadata/uid");
    let name = json_str(json, "/metadata/name");
    let namespace_val =
        json.pointer("/metadata/namespace").and_then(|v| v.as_str()).map(|s| s.to_string());

    let cells: Vec<String> = match kind {
        "Pod" => extract_pod_cells(json),
        "Deployment" => extract_deployment_cells(json),
        "Service" => extract_service_cells(json),
        "Node" => extract_node_cells(json),
        "StatefulSet" => extract_statefulset_cells(json),
        "DaemonSet" => extract_daemonset_cells(json),
        "ReplicaSet" => extract_replicaset_cells(json),
        "Job" => extract_job_cells(json),
        "CronJob" => extract_cronjob_cells(json),
        "Ingress" => extract_ingress_cells(json),
        "ConfigMap" => extract_configmap_cells(json),
        "Secret" => extract_secret_cells(json),
        "Namespace" => extract_namespace_cells(json),
        "PersistentVolume" => extract_pv_cells(json),
        "PersistentVolumeClaim" => extract_pvc_cells(json),
        "StorageClass" => extract_storageclass_cells(json),
        "Event" => extract_event_cells(json),
        "ServiceAccount" => extract_serviceaccount_cells(json),
        "Role" => extract_role_cells(json),
        "ClusterRole" => extract_clusterrole_cells(json),
        "RoleBinding" => extract_rolebinding_cells(json),
        "ClusterRoleBinding" => extract_clusterrolebinding_cells(json),
        "NetworkPolicy" => extract_networkpolicy_cells(json),
        "Endpoints" => extract_endpoints_cells(json),
        "ResourceQuota" => extract_resourcequota_cells(json),
        "LimitRange" => extract_limitrange_cells(json),
        "HorizontalPodAutoscaler" => extract_hpa_cells(json),
        "PodDisruptionBudget" => extract_pdb_cells(json),
        "PriorityClass" => extract_priorityclass_cells(json),
        "Lease" => extract_lease_cells(json),
        "ValidatingWebhookConfiguration" => extract_validatingwebhook_cells(json),
        "MutatingWebhookConfiguration" => extract_mutatingwebhook_cells(json),
        "EndpointSlice" => extract_endpointslice_cells(json),
        "IngressClass" => extract_ingressclass_cells(json),
        "Application" => extract_application_cells(json),
        "ApplicationSet" => extract_applicationset_cells(json),
        "AppProject" => extract_appproject_cells(json),
        _ => extract_generic_cells(json),
    };

    debug_assert_eq!(
        cells.len(),
        columns.len(),
        "Cell count ({}) != column count ({}) for kind {kind}",
        cells.len(),
        columns.len(),
    );

    // Extract rich data for special columns
    let container_statuses =
        if kind == "Pod" { extract_container_statuses(json) } else { Vec::new() };

    let conditions = match kind {
        "Deployment" | "Node" | "Job" | "StatefulSet" | "DaemonSet" | "ReplicaSet" => {
            extract_condition_summary(json)
        }
        _ => Vec::new(),
    };

    TableRow {
        uid,
        cells,
        selected: false,
        kind: kind.to_string(),
        name,
        namespace: namespace_val,
        container_statuses,
        conditions,
    }
}

// ---------------------------------------------------------------------------
// Per-kind extractors
// ---------------------------------------------------------------------------

/// Pod: Name, Namespace, Containers, CPU, Memory, Restarts, Controlled By, Node, IP, QoS, Age, Status
fn extract_pod_cells(json: &serde_json::Value) -> Vec<String> {
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        container_status_summary(json),
        "—".to_string(), // CPU (requires metrics API)
        "—".to_string(), // Memory (requires metrics API)
        total_restarts(json),
        controlled_by(json),
        json_str(json, "/spec/nodeName"),
        json_str(json, "/status/podIP"),
        qos_class(json),
        human_age_from_json(json),
        pod_status(json),
    ]
}

/// Deployment: Name, Namespace, Pods, Ready, Up-to-date, Available, Age, Conditions
fn extract_deployment_cells(json: &serde_json::Value) -> Vec<String> {
    let ready = json.pointer("/status/readyReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let desired = json.pointer("/spec/replicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let updated = json.pointer("/status/updatedReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let available = json.pointer("/status/availableReplicas").and_then(|v| v.as_i64()).unwrap_or(0);

    let conditions = json
        .pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let ctype = c.get("type")?.as_str()?;
                    let status = c.get("status")?.as_str()?;
                    Some(format!("{ctype}={status}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "—".to_string());

    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        format!("{ready}/{desired}"),
        ready.to_string(),
        updated.to_string(),
        available.to_string(),
        human_age_from_json(json),
        conditions,
    ]
}

/// Service: Name, Namespace, Type, Cluster IP, External IP, Ports, Age
fn extract_service_cells(json: &serde_json::Value) -> Vec<String> {
    let external_ip = json
        .pointer("/status/loadBalancer/ingress")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|ing| {
            ing.get("ip").or(ing.get("hostname")).and_then(|v| v.as_str()).map(|s| s.to_string())
        })
        .or_else(|| {
            json.pointer("/spec/externalIPs")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "<none>".to_string());

    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        json_str(json, "/spec/type"),
        json_str(json, "/spec/clusterIP"),
        external_ip,
        format_ports(json),
        human_age_from_json(json),
    ]
}

/// Node: Name, CPU, Memory, Disk, Taints, Roles, Internal IP, Schedulable, Version, Age, Conditions
fn extract_node_cells(json: &serde_json::Value) -> Vec<String> {
    let taints = json
        .pointer("/spec/taints")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());

    let conditions = json
        .pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let ctype = c.get("type")?.as_str()?;
                    let status = c.get("status")?.as_str()?;
                    if status == "True" && ctype != "Ready" {
                        Some(ctype.to_string())
                    } else if ctype == "Ready" {
                        Some(format!("Ready={status}"))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "—".to_string());

    let internal_ip = json
        .pointer("/status/addresses")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|addr| {
                let addr_type = addr.get("type")?.as_str()?;
                if addr_type == "InternalIP" {
                    addr.get("address")?.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "—".to_string());

    let unschedulable =
        json.pointer("/spec/unschedulable").and_then(|v| v.as_bool()).unwrap_or(false);
    let schedulable = if unschedulable { "False" } else { "True" }.to_string();

    vec![
        json_str(json, "/metadata/name"),
        "—".to_string(), // CPU (requires metrics)
        "—".to_string(), // Memory (requires metrics)
        "—".to_string(), // Disk (requires metrics)
        taints,
        node_roles(json),
        internal_ip,
        schedulable,
        json_str(json, "/status/nodeInfo/kubeletVersion"),
        human_age_from_json(json),
        conditions,
    ]
}

/// StatefulSet: Name, Namespace, Pods, Replicas, Age
fn extract_statefulset_cells(json: &serde_json::Value) -> Vec<String> {
    let ready = json.pointer("/status/readyReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let desired = json.pointer("/spec/replicas").and_then(|v| v.as_i64()).unwrap_or(0);
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        format!("{ready}/{desired}"),
        desired.to_string(),
        human_age_from_json(json),
    ]
}

/// DaemonSet: Name, Namespace, Desired, Current, Ready, Up-to-date, Available, Node Selector, Age
fn extract_daemonset_cells(json: &serde_json::Value) -> Vec<String> {
    let desired =
        json.pointer("/status/desiredNumberScheduled").and_then(|v| v.as_i64()).unwrap_or(0);
    let current =
        json.pointer("/status/currentNumberScheduled").and_then(|v| v.as_i64()).unwrap_or(0);
    let ready = json.pointer("/status/numberReady").and_then(|v| v.as_i64()).unwrap_or(0);
    let updated =
        json.pointer("/status/updatedNumberScheduled").and_then(|v| v.as_i64()).unwrap_or(0);
    let available = json.pointer("/status/numberAvailable").and_then(|v| v.as_i64()).unwrap_or(0);
    let node_selector = json
        .pointer("/spec/template/spec/nodeSelector")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        desired.to_string(),
        current.to_string(),
        ready.to_string(),
        updated.to_string(),
        available.to_string(),
        node_selector,
        human_age_from_json(json),
    ]
}

/// ReplicaSet: Name, Namespace, Desired, Current, Ready, Age
fn extract_replicaset_cells(json: &serde_json::Value) -> Vec<String> {
    let desired = json.pointer("/spec/replicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let current = json.pointer("/status/replicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let ready = json.pointer("/status/readyReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        desired.to_string(),
        current.to_string(),
        ready.to_string(),
        human_age_from_json(json),
    ]
}

/// Job: Name, Namespace, Completions, Parallelism, Duration, Age, Status, Conditions
fn extract_job_cells(json: &serde_json::Value) -> Vec<String> {
    let succeeded = json.pointer("/status/succeeded").and_then(|v| v.as_i64()).unwrap_or(0);
    let completions = json.pointer("/spec/completions").and_then(|v| v.as_i64()).unwrap_or(1);
    let parallelism = json.pointer("/spec/parallelism").and_then(|v| v.as_i64()).unwrap_or(1);

    let duration = match (
        json.pointer("/status/startTime").and_then(|v| v.as_str()),
        json.pointer("/status/completionTime").and_then(|v| v.as_str()),
    ) {
        (Some(start), Some(end)) => {
            let start_dt = chrono::DateTime::parse_from_rfc3339(start).ok();
            let end_dt = chrono::DateTime::parse_from_rfc3339(end).ok();
            match (start_dt, end_dt) {
                (Some(s), Some(e)) => {
                    let secs = (e - s).num_seconds();
                    human_duration_secs(secs)
                }
                _ => "—".to_string(),
            }
        }
        _ => "—".to_string(),
    };

    // Derive status from conditions
    let status = json
        .pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            for c in arr {
                let ctype = c.get("type")?.as_str()?;
                let cstatus = c.get("status")?.as_str()?;
                if ctype == "Complete" && cstatus == "True" {
                    return Some("Complete".to_string());
                }
                if ctype == "Failed" && cstatus == "True" {
                    return Some("Failed".to_string());
                }
            }
            None
        })
        .unwrap_or_else(|| {
            if json.pointer("/status/active").and_then(|v| v.as_i64()).unwrap_or(0) > 0 {
                "Running".to_string()
            } else {
                "Pending".to_string()
            }
        });

    let conditions = json
        .pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let ctype = c.get("type")?.as_str()?;
                    let cstatus = c.get("status")?.as_str()?;
                    Some(format!("{ctype}={cstatus}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "—".to_string());

    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        format!("{succeeded}/{completions}"),
        parallelism.to_string(),
        duration,
        human_age_from_json(json),
        status,
        conditions,
    ]
}

/// CronJob: Name, Namespace, Schedule, Timezone, Suspend, Active, Last Schedule, Age
fn extract_cronjob_cells(json: &serde_json::Value) -> Vec<String> {
    let suspend = json.pointer("/spec/suspend").and_then(|v| v.as_bool()).unwrap_or(false);
    let active =
        json.pointer("/status/active").and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);
    let last_schedule = json
        .pointer("/status/lastScheduleTime")
        .and_then(|v| v.as_str())
        .map(human_age)
        .unwrap_or_else(|| "—".to_string());
    let timezone =
        json.pointer("/spec/timeZone").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        json_str(json, "/spec/schedule"),
        timezone,
        if suspend { "True" } else { "False" }.to_string(),
        active.to_string(),
        last_schedule,
        human_age_from_json(json),
    ]
}

/// Ingress: Name, Namespace, Load Balancers, Rules, Age
fn extract_ingress_cells(json: &serde_json::Value) -> Vec<String> {
    let lbs = json
        .pointer("/status/loadBalancer/ingress")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|ing| {
                    ing.get("ip")
                        .or(ing.get("hostname"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());

    let rules = json
        .pointer("/spec/rules")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.get("host").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());

    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        lbs,
        rules,
        human_age_from_json(json),
    ]
}

/// ConfigMap: Name, Namespace, Keys, Age
fn extract_configmap_cells(json: &serde_json::Value) -> Vec<String> {
    let keys = json
        .get("data")
        .and_then(|v| v.as_object())
        .map(|m| m.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        keys,
        human_age_from_json(json),
    ]
}

/// Secret: Name, Namespace, Type, Keys, Age
fn extract_secret_cells(json: &serde_json::Value) -> Vec<String> {
    let keys = json
        .get("data")
        .and_then(|v| v.as_object())
        .map(|m| m.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    let secret_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("Opaque").to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        secret_type,
        keys,
        human_age_from_json(json),
    ]
}

/// Namespace: Name, Status, Age
fn extract_namespace_cells(json: &serde_json::Value) -> Vec<String> {
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/status/phase"),
        human_age_from_json(json),
    ]
}

/// PersistentVolume: Name, Capacity, Access Modes, Reclaim Policy, Status, Claim, Storage Class, Age
fn extract_pv_cells(json: &serde_json::Value) -> Vec<String> {
    let capacity =
        json.pointer("/spec/capacity/storage").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let access_modes = json
        .pointer("/spec/accessModes")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
        .unwrap_or_else(|| "—".to_string());
    let claim = json
        .pointer("/spec/claimRef")
        .and_then(|v| {
            let ns = v.get("namespace")?.as_str()?;
            let name = v.get("name")?.as_str()?;
            Some(format!("{ns}/{name}"))
        })
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        capacity,
        access_modes,
        json_str(json, "/spec/persistentVolumeReclaimPolicy"),
        json_str(json, "/status/phase"),
        claim,
        json_str(json, "/spec/storageClassName"),
        human_age_from_json(json),
    ]
}

/// PersistentVolumeClaim: Name, Namespace, Status, Volume, Capacity, Storage Class, Age
fn extract_pvc_cells(json: &serde_json::Value) -> Vec<String> {
    let capacity = json
        .pointer("/status/capacity/storage")
        .and_then(|v| v.as_str())
        .unwrap_or("—")
        .to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        json_str(json, "/status/phase"),
        json_str(json, "/spec/volumeName"),
        capacity,
        json_str(json, "/spec/storageClassName"),
        human_age_from_json(json),
    ]
}

/// StorageClass: Name, Provisioner, Reclaim Policy, Volume Binding, Age
fn extract_storageclass_cells(json: &serde_json::Value) -> Vec<String> {
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/provisioner"),
        json_str(json, "/reclaimPolicy"),
        json_str(json, "/volumeBindingMode"),
        human_age_from_json(json),
    ]
}

/// Event: Type, Message, Namespace, Involved Object, Source, Count, Age, Last Seen
fn extract_event_cells(json: &serde_json::Value) -> Vec<String> {
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("Normal").to_string();

    let message = json.get("message").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let namespace = json_str(json, "/metadata/namespace");

    let object = json
        .pointer("/involvedObject/kind")
        .and_then(|v| v.as_str())
        .map(|kind| {
            let name = json.pointer("/involvedObject/name").and_then(|v| v.as_str()).unwrap_or("");
            format!("{kind}/{name}")
        })
        .unwrap_or_else(|| "—".to_string());

    let source =
        json.pointer("/source/component").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let count = json
        .get("count")
        .and_then(|v| v.as_i64())
        .map(|c| c.to_string())
        .unwrap_or_else(|| "1".to_string());

    let last_seen = json
        .get("lastTimestamp")
        .or_else(|| json.get("eventTime"))
        .and_then(|v| v.as_str())
        .map(human_age)
        .unwrap_or_else(|| "—".to_string());

    vec![
        event_type,
        message,
        namespace,
        object,
        source,
        count,
        human_age_from_json(json),
        last_seen,
    ]
}

/// ServiceAccount: Name, Namespace, Secrets, Age
fn extract_serviceaccount_cells(json: &serde_json::Value) -> Vec<String> {
    let secrets = json
        .get("secrets")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        secrets,
        human_age_from_json(json),
    ]
}

/// Role: Name, Namespace, Rules, Age
fn extract_role_cells(json: &serde_json::Value) -> Vec<String> {
    let rules = json
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        rules,
        human_age_from_json(json),
    ]
}

/// ClusterRole: Name, Rules, Age
fn extract_clusterrole_cells(json: &serde_json::Value) -> Vec<String> {
    let rules = json
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![json_str(json, "/metadata/name"), rules, human_age_from_json(json)]
}

/// RoleBinding: Name, Namespace, Role, Subjects, Age
fn extract_rolebinding_cells(json: &serde_json::Value) -> Vec<String> {
    let role_ref = json
        .pointer("/roleRef")
        .and_then(|v| {
            let kind = v.get("kind")?.as_str()?;
            let name = v.get("name")?.as_str()?;
            Some(format!("{kind}/{name}"))
        })
        .unwrap_or_else(|| "—".to_string());
    let subjects = json
        .get("subjects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let kind = s.get("kind")?.as_str()?;
                    let name = s.get("name")?.as_str()?;
                    Some(format!("{kind}/{name}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        role_ref,
        subjects,
        human_age_from_json(json),
    ]
}

/// ClusterRoleBinding: Name, Role, Subjects, Age
fn extract_clusterrolebinding_cells(json: &serde_json::Value) -> Vec<String> {
    let role_ref = json
        .pointer("/roleRef")
        .and_then(|v| {
            let kind = v.get("kind")?.as_str()?;
            let name = v.get("name")?.as_str()?;
            Some(format!("{kind}/{name}"))
        })
        .unwrap_or_else(|| "—".to_string());
    let subjects = json
        .get("subjects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let kind = s.get("kind")?.as_str()?;
                    let name = s.get("name")?.as_str()?;
                    Some(format!("{kind}/{name}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    vec![json_str(json, "/metadata/name"), role_ref, subjects, human_age_from_json(json)]
}

/// NetworkPolicy: Name, Namespace, Pod Selector, Policy Types, Age
fn extract_networkpolicy_cells(json: &serde_json::Value) -> Vec<String> {
    let pod_selector = json
        .pointer("/spec/podSelector/matchLabels")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "<all pods>".to_string());
    let policy_types = json
        .pointer("/spec/policyTypes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        pod_selector,
        policy_types,
        human_age_from_json(json),
    ]
}

/// Endpoints: Name, Namespace, Endpoints, Age
fn extract_endpoints_cells(json: &serde_json::Value) -> Vec<String> {
    let endpoints = json
        .get("subsets")
        .and_then(|v| v.as_array())
        .map(|subsets| {
            let total_addrs: usize = subsets
                .iter()
                .filter_map(|s| s.get("addresses").and_then(|v| v.as_array()))
                .map(|arr| arr.len())
                .sum();
            let total_ports: usize = subsets
                .iter()
                .filter_map(|s| s.get("ports").and_then(|v| v.as_array()))
                .map(|arr| arr.len())
                .sum();
            format!("{total_addrs} addresses, {total_ports} ports")
        })
        .unwrap_or_else(|| "<none>".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        endpoints,
        human_age_from_json(json),
    ]
}

/// ResourceQuota: Name, Namespace, Hard, Used, Age
fn extract_resourcequota_cells(json: &serde_json::Value) -> Vec<String> {
    let hard = json
        .pointer("/status/hard")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    let used = json
        .pointer("/status/used")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        hard,
        used,
        human_age_from_json(json),
    ]
}

/// LimitRange: Name, Namespace, Type, Default, Age
fn extract_limitrange_cells(json: &serde_json::Value) -> Vec<String> {
    let limit_type = json
        .pointer("/spec/limits")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|l| l.get("type").and_then(|v| v.as_str()))
        .unwrap_or("—")
        .to_string();
    let defaults = json
        .pointer("/spec/limits")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|l| l.get("default").and_then(|v| v.as_object()))
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        limit_type,
        defaults,
        human_age_from_json(json),
    ]
}

/// HorizontalPodAutoscaler: Name, Namespace, Reference, Min/Max, Current, Age
fn extract_hpa_cells(json: &serde_json::Value) -> Vec<String> {
    let reference = json
        .pointer("/spec/scaleTargetRef")
        .and_then(|v| {
            let kind = v.get("kind")?.as_str()?;
            let name = v.get("name")?.as_str()?;
            Some(format!("{kind}/{name}"))
        })
        .unwrap_or_else(|| "—".to_string());
    let min = json.pointer("/spec/minReplicas").and_then(|v| v.as_i64()).unwrap_or(1);
    let max = json.pointer("/spec/maxReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
    let current = json
        .pointer("/status/currentReplicas")
        .and_then(|v| v.as_i64())
        .map(|c| c.to_string())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        reference,
        format!("{min}/{max}"),
        current,
        human_age_from_json(json),
    ]
}

/// PodDisruptionBudget: Name, Namespace, Min Available, Max Unavailable, Allowed Disruptions, Age
fn extract_pdb_cells(json: &serde_json::Value) -> Vec<String> {
    let min_available = json
        .pointer("/spec/minAvailable")
        .map(|v| {
            v.as_i64()
                .map(|n| n.to_string())
                .or_else(|| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "—".to_string())
        })
        .unwrap_or_else(|| "—".to_string());
    let max_unavailable = json
        .pointer("/spec/maxUnavailable")
        .map(|v| {
            v.as_i64()
                .map(|n| n.to_string())
                .or_else(|| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "—".to_string())
        })
        .unwrap_or_else(|| "—".to_string());
    let allowed = json
        .pointer("/status/disruptionsAllowed")
        .and_then(|v| v.as_i64())
        .map(|c| c.to_string())
        .unwrap_or_else(|| "—".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        min_available,
        max_unavailable,
        allowed,
        human_age_from_json(json),
    ]
}

/// PriorityClass: Name, Value, Global Default, Age
fn extract_priorityclass_cells(json: &serde_json::Value) -> Vec<String> {
    let value = json
        .get("value")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "0".to_string());
    let global_default = json
        .get("globalDefault")
        .and_then(|v| v.as_bool())
        .map(|b| if b { "True" } else { "False" })
        .unwrap_or("False")
        .to_string();
    vec![json_str(json, "/metadata/name"), value, global_default, human_age_from_json(json)]
}

/// Lease: Name, Namespace, Holder, Age
fn extract_lease_cells(json: &serde_json::Value) -> Vec<String> {
    let holder =
        json.pointer("/spec/holderIdentity").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        holder,
        human_age_from_json(json),
    ]
}

/// ValidatingWebhookConfiguration: Name, Webhooks, Age
fn extract_validatingwebhook_cells(json: &serde_json::Value) -> Vec<String> {
    let webhooks = json
        .get("webhooks")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![json_str(json, "/metadata/name"), webhooks, human_age_from_json(json)]
}

/// MutatingWebhookConfiguration: Name, Webhooks, Age
fn extract_mutatingwebhook_cells(json: &serde_json::Value) -> Vec<String> {
    let webhooks = json
        .get("webhooks")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![json_str(json, "/metadata/name"), webhooks, human_age_from_json(json)]
}

/// EndpointSlice: Name, Namespace, Address Type, Ports, Endpoints, Age
fn extract_endpointslice_cells(json: &serde_json::Value) -> Vec<String> {
    let address_type = json.get("addressType").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let ports = json
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let port = p.get("port").and_then(|v| v.as_i64())?;
                    let proto = p.get("protocol").and_then(|v| v.as_str()).unwrap_or("TCP");
                    Some(format!("{port}/{proto}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string());
    let endpoint_count = json
        .get("endpoints")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        address_type,
        ports,
        endpoint_count,
        human_age_from_json(json),
    ]
}

/// IngressClass: Name, Controller, Default, Age
fn extract_ingressclass_cells(json: &serde_json::Value) -> Vec<String> {
    let controller =
        json.pointer("/spec/controller").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let is_default = json
        .pointer("/metadata/annotations")
        .and_then(|v| v.as_object())
        .and_then(|m| m.get("ingressclass.kubernetes.io/is-default-class"))
        .and_then(|v| v.as_str())
        .map(|s| if s == "true" { "True" } else { "False" })
        .unwrap_or("False")
        .to_string();
    vec![json_str(json, "/metadata/name"), controller, is_default, human_age_from_json(json)]
}

/// ArgoCD Application: Name, Namespace, Project, Sync Status, Health, Repo, Path, Destination, Age
fn extract_application_cells(json: &serde_json::Value) -> Vec<String> {
    let project = json.pointer("/spec/project").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let sync_status = json
        .pointer("/status/sync/status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let health = json
        .pointer("/status/health/status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let repo =
        json.pointer("/spec/source/repoURL").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let path =
        json.pointer("/spec/source/path").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let dest_server =
        json.pointer("/spec/destination/server").and_then(|v| v.as_str()).unwrap_or("");
    let dest_ns =
        json.pointer("/spec/destination/namespace").and_then(|v| v.as_str()).unwrap_or("");
    let destination = if dest_server.is_empty() && dest_ns.is_empty() {
        "—".to_string()
    } else {
        format!("{dest_server}/{dest_ns}")
    };
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        project,
        sync_status,
        health,
        repo,
        path,
        destination,
        human_age_from_json(json),
    ]
}

/// ArgoCD ApplicationSet: Name, Namespace, Generators, Template App, Age
fn extract_applicationset_cells(json: &serde_json::Value) -> Vec<String> {
    let generators = json
        .pointer("/spec/generators")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    let template = json
        .pointer("/spec/template/metadata/name")
        .and_then(|v| v.as_str())
        .unwrap_or("—")
        .to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        generators,
        template,
        human_age_from_json(json),
    ]
}

/// ArgoCD AppProject: Name, Namespace, Destinations, Sources, Age
fn extract_appproject_cells(json: &serde_json::Value) -> Vec<String> {
    let destinations = json
        .pointer("/spec/destinations")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    let sources = json
        .pointer("/spec/sourceRepos")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len().to_string())
        .unwrap_or_else(|| "0".to_string());
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        destinations,
        sources,
        human_age_from_json(json),
    ]
}

/// Generic fallback: Name, Namespace, Age, Status
fn extract_generic_cells(json: &serde_json::Value) -> Vec<String> {
    let status = json
        .pointer("/status/phase")
        .or_else(|| json.pointer("/status/state"))
        .and_then(|v| v.as_str())
        .unwrap_or("—")
        .to_string();
    vec![
        json_str(json, "/metadata/name"),
        json_str(json, "/metadata/namespace"),
        human_age_from_json(json),
        status,
    ]
}

// ---------------------------------------------------------------------------
// Detail extraction (Phase 4)
// ---------------------------------------------------------------------------

/// Extract key-value properties for the detail view of a resource.
pub fn extract_detail_properties(kind: &str, json: &serde_json::Value) -> Vec<(String, String)> {
    let mut props = vec![
        ("Name".to_string(), json_str(json, "/metadata/name")),
        ("Namespace".to_string(), json_str(json, "/metadata/namespace")),
        ("UID".to_string(), json_str(json, "/metadata/uid")),
        (
            "Created".to_string(),
            json.pointer("/metadata/creationTimestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("—")
                .to_string(),
        ),
        ("Resource Version".to_string(), json_str(json, "/metadata/resourceVersion")),
    ];

    match kind {
        "Pod" => {
            props.push(("Status".to_string(), pod_status(json)));
            props.push(("Node".to_string(), json_str(json, "/spec/nodeName")));
            props.push(("Pod IP".to_string(), json_str(json, "/status/podIP")));
            props.push(("Host IP".to_string(), json_str(json, "/status/hostIP")));
            props.push(("Service Account".to_string(), json_str(json, "/spec/serviceAccountName")));
            props.push(("QoS Class".to_string(), qos_class(json)));
            props.push(("Controlled By".to_string(), controlled_by(json)));
            props.push(("Restart Policy".to_string(), json_str(json, "/spec/restartPolicy")));
            props.push(("DNS Policy".to_string(), json_str(json, "/spec/dnsPolicy")));
            props.push(("Priority Class".to_string(), json_str(json, "/spec/priorityClassName")));
            props.push(("Scheduler".to_string(), json_str(json, "/spec/schedulerName")));
            let grace = json
                .pointer("/spec/terminationGracePeriodSeconds")
                .and_then(|v| v.as_i64())
                .map(|s| format!("{s}s"))
                .unwrap_or_else(|| "—".to_string());
            props.push(("Termination Grace Period".to_string(), grace));
            props.push(("Containers".to_string(), container_status_summary(json)));
            props.push(("Restarts".to_string(), total_restarts(json)));
        }
        "Deployment" => {
            let ready = json.pointer("/status/readyReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
            let desired = json.pointer("/spec/replicas").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated =
                json.pointer("/status/updatedReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
            let available =
                json.pointer("/status/availableReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
            props.push(("Strategy".to_string(), json_str(json, "/spec/strategy/type")));
            props.push(("Ready Replicas".to_string(), format!("{ready}/{desired}")));
            props.push(("Updated Replicas".to_string(), updated.to_string()));
            props.push(("Available Replicas".to_string(), available.to_string()));
        }
        "Service" => {
            props.push(("Type".to_string(), json_str(json, "/spec/type")));
            props.push(("Cluster IP".to_string(), json_str(json, "/spec/clusterIP")));
            props.push(("Ports".to_string(), format_ports(json)));
            let selector = json
                .pointer("/spec/selector")
                .and_then(|v| v.as_object())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "—".to_string());
            props.push(("Selector".to_string(), selector));
        }
        "Node" => {
            props.push(("Roles".to_string(), node_roles(json)));
            props.push(("Version".to_string(), json_str(json, "/status/nodeInfo/kubeletVersion")));
            props.push(("OS Image".to_string(), json_str(json, "/status/nodeInfo/osImage")));
            props.push((
                "Kernel Version".to_string(),
                json_str(json, "/status/nodeInfo/kernelVersion"),
            ));
            props.push((
                "Container Runtime".to_string(),
                json_str(json, "/status/nodeInfo/containerRuntimeVersion"),
            ));
            props.push((
                "Architecture".to_string(),
                json_str(json, "/status/nodeInfo/architecture"),
            ));
        }
        "ServiceAccount" => {
            let secrets = json
                .get("secrets")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| {
                            s.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "<none>".to_string());
            props.push(("Secrets".to_string(), secrets));
            let automount = json
                .get("automountServiceAccountToken")
                .and_then(|v| v.as_bool())
                .map(|b| if b { "true" } else { "false" })
                .unwrap_or("—")
                .to_string();
            props.push(("Automount Token".to_string(), automount));
        }
        "Role" | "ClusterRole" => {
            let rules_count =
                json.get("rules").and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);
            props.push(("Rules".to_string(), rules_count.to_string()));
            if let Some(rules) = json.get("rules").and_then(|v| v.as_array()) {
                for (i, rule) in rules.iter().enumerate() {
                    let verbs = rule
                        .get("verbs")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
                        })
                        .unwrap_or_else(|| "—".to_string());
                    let resources = rule
                        .get("resources")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
                        })
                        .unwrap_or_else(|| "*".to_string());
                    let api_groups = rule
                        .get("apiGroups")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| if s.is_empty() { "core" } else { s })
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_else(|| "*".to_string());
                    props.push((
                        format!("Rule {}", i + 1),
                        format!("[{api_groups}] {resources}: {verbs}"),
                    ));
                }
            }
        }
        "RoleBinding" | "ClusterRoleBinding" => {
            let role_ref = json
                .pointer("/roleRef")
                .and_then(|v| {
                    let kind = v.get("kind")?.as_str()?;
                    let name = v.get("name")?.as_str()?;
                    Some(format!("{kind}/{name}"))
                })
                .unwrap_or_else(|| "—".to_string());
            props.push(("Role Ref".to_string(), role_ref));
            if let Some(subjects) = json.get("subjects").and_then(|v| v.as_array()) {
                for (i, subj) in subjects.iter().enumerate() {
                    let kind = subj.get("kind").and_then(|v| v.as_str()).unwrap_or("—");
                    let name = subj.get("name").and_then(|v| v.as_str()).unwrap_or("—");
                    let ns = subj.get("namespace").and_then(|v| v.as_str()).unwrap_or("");
                    let display = if ns.is_empty() {
                        format!("{kind}/{name}")
                    } else {
                        format!("{kind}/{name} (ns: {ns})")
                    };
                    props.push((format!("Subject {}", i + 1), display));
                }
            }
        }
        "NetworkPolicy" => {
            let pod_selector = json
                .pointer("/spec/podSelector/matchLabels")
                .and_then(|v| v.as_object())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "<all pods>".to_string());
            props.push(("Pod Selector".to_string(), pod_selector));
            let policy_types = json
                .pointer("/spec/policyTypes")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                .unwrap_or_else(|| "—".to_string());
            props.push(("Policy Types".to_string(), policy_types));
            let ingress_rules = json
                .pointer("/spec/ingress")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Ingress Rules".to_string(), ingress_rules.to_string()));
            let egress_rules = json
                .pointer("/spec/egress")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Egress Rules".to_string(), egress_rules.to_string()));
        }
        "Endpoints" => {
            if let Some(subsets) = json.get("subsets").and_then(|v| v.as_array()) {
                let total_addrs: usize = subsets
                    .iter()
                    .filter_map(|s| s.get("addresses").and_then(|v| v.as_array()))
                    .map(|arr| arr.len())
                    .sum();
                let total_not_ready: usize = subsets
                    .iter()
                    .filter_map(|s| s.get("notReadyAddresses").and_then(|v| v.as_array()))
                    .map(|arr| arr.len())
                    .sum();
                props.push(("Ready Addresses".to_string(), total_addrs.to_string()));
                props.push(("Not Ready Addresses".to_string(), total_not_ready.to_string()));
                props.push(("Subsets".to_string(), subsets.len().to_string()));
            }
        }
        "ResourceQuota" => {
            if let Some(hard) = json.pointer("/status/hard").and_then(|v| v.as_object()) {
                for (k, v) in hard {
                    let used_val = json
                        .pointer("/status/used")
                        .and_then(|u| u.get(k))
                        .and_then(|u| u.as_str())
                        .unwrap_or("0");
                    let hard_val = v.as_str().unwrap_or("0");
                    props.push((format!("Quota: {k}"), format!("{used_val} / {hard_val}")));
                }
            }
        }
        "LimitRange" => {
            if let Some(limits) = json.pointer("/spec/limits").and_then(|v| v.as_array()) {
                for (i, limit) in limits.iter().enumerate() {
                    let ltype = limit.get("type").and_then(|v| v.as_str()).unwrap_or("—");
                    props.push((format!("Limit {} Type", i + 1), ltype.to_string()));
                }
            }
        }
        "HorizontalPodAutoscaler" => {
            let reference = json
                .pointer("/spec/scaleTargetRef")
                .and_then(|v| {
                    let kind = v.get("kind")?.as_str()?;
                    let name = v.get("name")?.as_str()?;
                    Some(format!("{kind}/{name}"))
                })
                .unwrap_or_else(|| "—".to_string());
            props.push(("Scale Target".to_string(), reference));
            props.push((
                "Min Replicas".to_string(),
                json.pointer("/spec/minReplicas")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "1".to_string()),
            ));
            props.push((
                "Max Replicas".to_string(),
                json.pointer("/spec/maxReplicas")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
            props.push((
                "Current Replicas".to_string(),
                json.pointer("/status/currentReplicas")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
            props.push((
                "Desired Replicas".to_string(),
                json.pointer("/status/desiredReplicas")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
        }
        "PodDisruptionBudget" => {
            let min_available = json
                .pointer("/spec/minAvailable")
                .map(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "—".to_string())
                })
                .unwrap_or_else(|| "—".to_string());
            props.push(("Min Available".to_string(), min_available));
            let max_unavailable = json
                .pointer("/spec/maxUnavailable")
                .map(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "—".to_string())
                })
                .unwrap_or_else(|| "—".to_string());
            props.push(("Max Unavailable".to_string(), max_unavailable));
            props.push((
                "Current Healthy".to_string(),
                json.pointer("/status/currentHealthy")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
            props.push((
                "Desired Healthy".to_string(),
                json.pointer("/status/desiredHealthy")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
            props.push((
                "Disruptions Allowed".to_string(),
                json.pointer("/status/disruptionsAllowed")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ));
        }
        "PriorityClass" => {
            props.push((
                "Value".to_string(),
                json.get("value")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "0".to_string()),
            ));
            props.push((
                "Global Default".to_string(),
                json.get("globalDefault")
                    .and_then(|v| v.as_bool())
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "false".to_string()),
            ));
            props.push((
                "Description".to_string(),
                json.get("description").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
            ));
            props.push((
                "Preemption Policy".to_string(),
                json.get("preemptionPolicy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("PreemptLowerPriority")
                    .to_string(),
            ));
        }
        "Lease" => {
            props.push((
                "Holder Identity".to_string(),
                json.pointer("/spec/holderIdentity")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Lease Duration".to_string(),
                json.pointer("/spec/leaseDurationSeconds")
                    .and_then(|v| v.as_i64())
                    .map(|s| format!("{s}s"))
                    .unwrap_or_else(|| "—".to_string()),
            ));
            props.push((
                "Renew Time".to_string(),
                json.pointer("/spec/renewTime").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
            ));
            props.push((
                "Acquire Time".to_string(),
                json.pointer("/spec/acquireTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
        }
        "ValidatingWebhookConfiguration" | "MutatingWebhookConfiguration" => {
            let webhook_count =
                json.get("webhooks").and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);
            props.push(("Webhooks".to_string(), webhook_count.to_string()));
            if let Some(hooks) = json.get("webhooks").and_then(|v| v.as_array()) {
                for (i, hook) in hooks.iter().enumerate() {
                    let name = hook.get("name").and_then(|v| v.as_str()).unwrap_or("—");
                    let failure_policy =
                        hook.get("failurePolicy").and_then(|v| v.as_str()).unwrap_or("—");
                    props.push((format!("Hook {} Name", i + 1), name.to_string()));
                    props.push((
                        format!("Hook {} Failure Policy", i + 1),
                        failure_policy.to_string(),
                    ));
                }
            }
        }
        "EndpointSlice" => {
            props.push((
                "Address Type".to_string(),
                json.get("addressType").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
            ));
            let endpoint_count =
                json.get("endpoints").and_then(|v| v.as_array()).map(|arr| arr.len()).unwrap_or(0);
            props.push(("Endpoints".to_string(), endpoint_count.to_string()));
        }
        "IngressClass" => {
            props.push((
                "Controller".to_string(),
                json.pointer("/spec/controller")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            let is_default = json
                .pointer("/metadata/annotations")
                .and_then(|v| v.as_object())
                .and_then(|m| m.get("ingressclass.kubernetes.io/is-default-class"))
                .and_then(|v| v.as_str())
                .unwrap_or("false")
                .to_string();
            props.push(("Default".to_string(), is_default));
        }
        "Application" => {
            props.push((
                "Project".to_string(),
                json.pointer("/spec/project").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
            ));
            props.push((
                "Repo URL".to_string(),
                json.pointer("/spec/source/repoURL")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Path".to_string(),
                json.pointer("/spec/source/path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Target Revision".to_string(),
                json.pointer("/spec/source/targetRevision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Destination Server".to_string(),
                json.pointer("/spec/destination/server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Destination Namespace".to_string(),
                json.pointer("/spec/destination/namespace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            props.push((
                "Sync Status".to_string(),
                json.pointer("/status/sync/status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
            ));
            props.push((
                "Health Status".to_string(),
                json.pointer("/status/health/status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
            ));
            let sync_policy = if json.pointer("/spec/syncPolicy/automated").is_some() {
                "Automated"
            } else {
                "Manual"
            };
            props.push(("Sync Policy".to_string(), sync_policy.to_string()));
        }
        "ApplicationSet" => {
            let gen_count = json
                .pointer("/spec/generators")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Generators".to_string(), gen_count.to_string()));
            props.push((
                "Template".to_string(),
                json.pointer("/spec/template/metadata/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            let strategy =
                json.pointer("/spec/strategy/type").and_then(|v| v.as_str()).unwrap_or("AllAtOnce");
            props.push(("Strategy".to_string(), strategy.to_string()));
        }
        "AppProject" => {
            props.push((
                "Description".to_string(),
                json.pointer("/spec/description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string(),
            ));
            let dest_count = json
                .pointer("/spec/destinations")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Destinations".to_string(), dest_count.to_string()));
            let src_count = json
                .pointer("/spec/sourceRepos")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Source Repos".to_string(), src_count.to_string()));
            let whitelist = json
                .pointer("/spec/clusterResourceWhitelist")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            props.push(("Cluster Resource Whitelist".to_string(), whitelist.to_string()));
        }
        _ => {}
    }

    props
}

/// Extract conditions from a resource's status.
/// Returns (Type, Status, Reason, Message, Last Transition Time).
pub fn extract_conditions(
    json: &serde_json::Value,
) -> Vec<(String, String, String, String, String)> {
    json.pointer("/status/conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    (
                        c.get("type").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                        c.get("status").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                        c.get("reason").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                        c.get("message").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                        c.get("lastTransitionTime")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Extract labels as key-value pairs.
pub fn extract_labels(json: &serde_json::Value) -> Vec<(String, String)> {
    json.pointer("/metadata/labels")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default()
}

/// Extract annotations as key-value pairs.
pub fn extract_annotations(json: &serde_json::Value) -> Vec<(String, String)> {
    json.pointer("/metadata/annotations")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Node detail extraction
// ---------------------------------------------------------------------------

/// Extract Node addresses: (Type, Address) pairs from `/status/addresses`.
pub fn extract_node_addresses(json: &serde_json::Value) -> Vec<(String, String)> {
    json.pointer("/status/addresses")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    (
                        a.get("type").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                        a.get("address").and_then(|v| v.as_str()).unwrap_or("—").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Extract Node capacity: (Resource, Value) pairs from `/status/capacity`.
pub fn extract_node_capacity(json: &serde_json::Value) -> Vec<(String, String)> {
    extract_resource_map(json, "/status/capacity")
}

/// Extract Node allocatable: (Resource, Value) pairs from `/status/allocatable`.
pub fn extract_node_allocatable(json: &serde_json::Value) -> Vec<(String, String)> {
    extract_resource_map(json, "/status/allocatable")
}

/// Helper: extract a key-value resource map from a JSON pointer path.
fn extract_resource_map(json: &serde_json::Value, path: &str) -> Vec<(String, String)> {
    json.pointer(path)
        .and_then(|v| v.as_object())
        .map(|m| {
            let mut pairs: Vec<(String, String)> = m
                .iter()
                .map(|(k, v)| {
                    let val = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
                    (k.clone(), val)
                })
                .collect();
            // Sort with well-known keys first: cpu, memory, pods, then the rest alphabetically
            pairs.sort_by(|a, b| {
                fn rank(key: &str) -> u8 {
                    match key {
                        "cpu" => 0,
                        "memory" => 1,
                        "pods" => 2,
                        "ephemeral-storage" => 3,
                        _ => 4,
                    }
                }
                rank(&a.0).cmp(&rank(&b.0)).then_with(|| a.0.cmp(&b.0))
            });
            pairs
        })
        .unwrap_or_default()
}

/// Node image info for display.
#[derive(Clone)]
pub struct NodeImage {
    pub names: Vec<String>,
    pub size_bytes: u64,
}

/// Extract Node container images from `/status/images`.
pub fn extract_node_images(json: &serde_json::Value) -> Vec<NodeImage> {
    json.pointer("/status/images")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|img| {
                    let names = img
                        .get("names")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                        })
                        .unwrap_or_default();
                    let size_bytes = img.get("sizeBytes").and_then(|v| v.as_u64()).unwrap_or(0);
                    NodeImage { names, size_bytes }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Format bytes into a human-readable size string (e.g., "123.4 MB").
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Extract a string from a JSON pointer path, returning "—" if missing.
pub fn json_str(json: &serde_json::Value, path: &str) -> String {
    json.pointer(path).and_then(|v| v.as_str()).unwrap_or("—").to_string()
}

/// Convert a Kubernetes timestamp string to a human-friendly age like "3d12h", "45m", "2h30m".
pub fn human_age(timestamp: &str) -> String {
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return "—".to_string();
    };
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(ts);
    let secs = duration.num_seconds();
    if secs < 0 {
        return "0s".to_string();
    }
    human_duration_secs(secs)
}

/// Format seconds into a human-friendly duration string.
fn human_duration_secs(secs: i64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let minutes = secs / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let rem_min = minutes % 60;
    if hours < 24 {
        if rem_min > 0 {
            return format!("{hours}h{rem_min}m");
        }
        return format!("{hours}h");
    }
    let days = hours / 24;
    let rem_hours = hours % 24;
    if days < 365 {
        if rem_hours > 0 {
            return format!("{days}d{rem_hours}h");
        }
        return format!("{days}d");
    }
    let years = days / 365;
    let rem_days = days % 365;
    if rem_days > 0 { format!("{years}y{rem_days}d") } else { format!("{years}y") }
}

/// Extract human age from a JSON object's creationTimestamp.
fn human_age_from_json(json: &serde_json::Value) -> String {
    json.pointer("/metadata/creationTimestamp")
        .and_then(|v| v.as_str())
        .map(human_age)
        .unwrap_or_else(|| "—".to_string())
}

/// Summarize container status as "running/total" (e.g., "2/3").
pub fn container_status_summary(json: &serde_json::Value) -> String {
    let total = json
        .pointer("/spec/containers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);
    let running = json
        .pointer("/status/containerStatuses")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter(|c| c.get("ready").and_then(|v| v.as_bool()).unwrap_or(false)).count()
        })
        .unwrap_or(0);
    format!("{running}/{total}")
}

/// Sum total restarts across all containers.
pub fn total_restarts(json: &serde_json::Value) -> String {
    let sum: i64 = json
        .pointer("/status/containerStatuses")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|c| c.get("restartCount").and_then(|v| v.as_i64())).sum())
        .unwrap_or(0);
    sum.to_string()
}

/// Extract "Kind/Name" from ownerReferences[0].
pub fn controlled_by(json: &serde_json::Value) -> String {
    json.pointer("/metadata/ownerReferences")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|owner| {
            let kind = owner.get("kind")?.as_str()?;
            let name = owner.get("name")?.as_str()?;
            Some(format!("{kind}/{name}"))
        })
        .unwrap_or_else(|| "—".to_string())
}

/// Format service ports as "80/TCP, 443/TCP".
pub fn format_ports(json: &serde_json::Value) -> String {
    json.pointer("/spec/ports")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let port = p.get("port").and_then(|v| v.as_i64())?;
                    let proto = p.get("protocol").and_then(|v| v.as_str()).unwrap_or("TCP");
                    Some(format!("{port}/{proto}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".to_string())
}

/// Parse node role labels (node-role.kubernetes.io/<role>).
pub fn node_roles(json: &serde_json::Value) -> String {
    let roles: Vec<String> = json
        .pointer("/metadata/labels")
        .and_then(|v| v.as_object())
        .map(|labels| {
            labels
                .keys()
                .filter_map(|k| {
                    k.strip_prefix("node-role.kubernetes.io/").map(|role| {
                        if role.is_empty() { "worker".to_string() } else { role.to_string() }
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if roles.is_empty() { "<none>".to_string() } else { roles.join(", ") }
}

/// Extract QoS class from pod status.
fn qos_class(json: &serde_json::Value) -> String {
    json.pointer("/status/qosClass").and_then(|v| v.as_str()).unwrap_or("—").to_string()
}

/// Compute pod status with container waiting reason fallback.
fn pod_status(json: &serde_json::Value) -> String {
    // Check for container waiting reasons first (CrashLoopBackOff, etc.)
    if let Some(statuses) = json.pointer("/status/containerStatuses").and_then(|v| v.as_array()) {
        for cs in statuses {
            if let Some(waiting) = cs.pointer("/state/waiting") {
                if let Some(reason) = waiting.get("reason").and_then(|v| v.as_str()) {
                    if reason != "ContainerCreating" {
                        return reason.to_string();
                    }
                }
            }
        }
    }
    // Fall back to phase
    json.pointer("/status/phase").and_then(|v| v.as_str()).unwrap_or("—").to_string()
}

// ---------------------------------------------------------------------------
// Pod detail extraction (Enhanced detail view)
// ---------------------------------------------------------------------------

/// Extract full pod detail data from a raw Pod JSON object.
pub fn extract_pod_detail(json: &serde_json::Value) -> PodDetailData {
    let spec = json.pointer("/spec").cloned().unwrap_or(serde_json::Value::Null);
    let status = json.pointer("/status").cloned().unwrap_or(serde_json::Value::Null);

    let containers = extract_containers_list(&spec, &status, "containers", "containerStatuses");
    let init_containers =
        extract_containers_list(&spec, &status, "initContainers", "initContainerStatuses");
    let volumes = extract_volumes(&spec);
    let tolerations = extract_tolerations(&spec);

    let node_selector = spec
        .get("nodeSelector")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();

    let annotations = json
        .pointer("/metadata/annotations")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();

    let affinity_json = spec
        .get("affinity")
        .filter(|v| !v.is_null())
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()));

    let host_ip = status.get("hostIP").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let dns_policy = spec.get("dnsPolicy").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let priority_class =
        spec.get("priorityClassName").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let scheduler_name =
        spec.get("schedulerName").and_then(|v| v.as_str()).unwrap_or("—").to_string();

    let termination_grace_period = spec
        .get("terminationGracePeriodSeconds")
        .and_then(|v| v.as_i64())
        .map(|s| format!("{s}s"))
        .unwrap_or_else(|| "—".to_string());

    PodDetailData {
        containers,
        init_containers,
        volumes,
        tolerations,
        node_selector,
        annotations,
        affinity_json,
        host_ip,
        dns_policy,
        priority_class,
        scheduler_name,
        termination_grace_period,
    }
}

/// Extract a list of containers from spec and match with status by name.
fn extract_containers_list(
    spec: &serde_json::Value,
    status: &serde_json::Value,
    spec_key: &str,
    status_key: &str,
) -> Vec<ContainerDetail> {
    let spec_containers = spec.get(spec_key).and_then(|v| v.as_array());
    let status_containers = status.get(status_key).and_then(|v| v.as_array());

    let Some(specs) = spec_containers else { return Vec::new() };

    specs
        .iter()
        .map(|cs| {
            let name = cs.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

            // Find matching status entry by name
            let cs_status = status_containers.and_then(|arr| {
                arr.iter().find(|s| s.get("name").and_then(|v| v.as_str()) == Some(&name))
            });

            extract_container_detail(cs, cs_status)
        })
        .collect()
}

/// Extract full detail for a single container spec + optional status.
fn extract_container_detail(
    spec: &serde_json::Value,
    status: Option<&serde_json::Value>,
) -> ContainerDetail {
    let name = spec.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let image = spec.get("image").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let image_pull_policy =
        spec.get("imagePullPolicy").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let ports = spec
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(extract_port).collect())
        .unwrap_or_default();

    let env_vars = spec
        .get("env")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(extract_env_var).collect())
        .unwrap_or_default();

    let volume_mounts = spec
        .get("volumeMounts")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(extract_volume_mount).collect())
        .unwrap_or_default();

    let resources = extract_container_resources(spec);

    let liveness_probe = spec.get("livenessProbe").map(|p| extract_probe(p, "liveness"));
    let readiness_probe = spec.get("readinessProbe").map(|p| extract_probe(p, "readiness"));
    let startup_probe = spec.get("startupProbe").map(|p| extract_probe(p, "startup"));

    let command = spec
        .get("command")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let args = spec
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let working_dir = spec.get("workingDir").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let security_context =
        spec.get("securityContext").filter(|v| !v.is_null()).map(extract_security_context);

    let state = status
        .and_then(|s| s.get("state"))
        .map(extract_container_state)
        .unwrap_or(ContainerStateDetail::Unknown);

    let ready = status.and_then(|s| s.get("ready")).and_then(|v| v.as_bool()).unwrap_or(false);

    let restart_count =
        status.and_then(|s| s.get("restartCount")).and_then(|v| v.as_i64()).unwrap_or(0);

    ContainerDetail {
        name,
        image,
        image_pull_policy,
        ports,
        env_vars,
        volume_mounts,
        resources,
        liveness_probe,
        readiness_probe,
        startup_probe,
        command,
        args,
        working_dir,
        security_context,
        state,
        ready,
        restart_count,
    }
}

fn extract_port(json: &serde_json::Value) -> PortDetail {
    PortDetail {
        name: json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        container_port: json.get("containerPort").and_then(|v| v.as_i64()).unwrap_or(0),
        protocol: json.get("protocol").and_then(|v| v.as_str()).unwrap_or("TCP").to_string(),
        host_port: json.get("hostPort").and_then(|v| v.as_i64()),
    }
}

fn extract_env_var(json: &serde_json::Value) -> EnvVarDetail {
    let name = json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let value = json.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let value_from = json
        .get("valueFrom")
        .map(|vf| {
            if let Some(cm) = vf.get("configMapKeyRef") {
                let cm_name = cm.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let key = cm.get("key").and_then(|v| v.as_str()).unwrap_or("?");
                format!("configMapKeyRef: {cm_name}.{key}")
            } else if let Some(sec) = vf.get("secretKeyRef") {
                let sec_name = sec.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let key = sec.get("key").and_then(|v| v.as_str()).unwrap_or("?");
                format!("secretKeyRef: {sec_name}.{key}")
            } else if let Some(field) = vf.get("fieldRef") {
                let fp = field.get("fieldPath").and_then(|v| v.as_str()).unwrap_or("?");
                format!("fieldRef: {fp}")
            } else if let Some(res) = vf.get("resourceFieldRef") {
                let container = res.get("containerName").and_then(|v| v.as_str()).unwrap_or("?");
                let resource = res.get("resource").and_then(|v| v.as_str()).unwrap_or("?");
                format!("resourceFieldRef: {container}.{resource}")
            } else {
                "unknown ref".to_string()
            }
        })
        .unwrap_or_default();

    EnvVarDetail { name, value, value_from }
}

fn extract_volume_mount(json: &serde_json::Value) -> VolumeMountDetail {
    VolumeMountDetail {
        name: json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        mount_path: json.get("mountPath").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        sub_path: json.get("subPath").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        read_only: json.get("readOnly").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

fn extract_container_resources(spec: &serde_json::Value) -> ContainerResources {
    let resources = spec.get("resources").cloned().unwrap_or(serde_json::Value::Null);
    ContainerResources {
        requests_cpu: resources
            .pointer("/requests/cpu")
            .and_then(|v| v.as_str())
            .unwrap_or("—")
            .to_string(),
        requests_memory: resources
            .pointer("/requests/memory")
            .and_then(|v| v.as_str())
            .unwrap_or("—")
            .to_string(),
        limits_cpu: resources
            .pointer("/limits/cpu")
            .and_then(|v| v.as_str())
            .unwrap_or("—")
            .to_string(),
        limits_memory: resources
            .pointer("/limits/memory")
            .and_then(|v| v.as_str())
            .unwrap_or("—")
            .to_string(),
    }
}

fn extract_probe(json: &serde_json::Value, probe_type: &str) -> ProbeDetail {
    let detail = if let Some(http) = json.get("httpGet") {
        let port = http
            .get("port")
            .map(|v| {
                v.as_i64()
                    .map(|n| n.to_string())
                    .or_else(|| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let path = http.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        format!("HTTP GET :{port}{path}")
    } else if let Some(exec) = json.get("exec") {
        let cmd = exec
            .get("command")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        format!("exec: [{cmd}]")
    } else if let Some(tcp) = json.get("tcpSocket") {
        let port = tcp
            .get("port")
            .map(|v| {
                v.as_i64()
                    .map(|n| n.to_string())
                    .or_else(|| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        format!("TCP :{port}")
    } else if let Some(grpc) = json.get("grpc") {
        let port = grpc.get("port").and_then(|v| v.as_i64()).unwrap_or(0);
        let service = grpc.get("service").and_then(|v| v.as_str()).unwrap_or("");
        if service.is_empty() { format!("gRPC :{port}") } else { format!("gRPC :{port}/{service}") }
    } else {
        "unknown".to_string()
    };

    ProbeDetail {
        probe_type: probe_type.to_string(),
        detail,
        initial_delay: json.get("initialDelaySeconds").and_then(|v| v.as_i64()).unwrap_or(0),
        period: json.get("periodSeconds").and_then(|v| v.as_i64()).unwrap_or(10),
        timeout: json.get("timeoutSeconds").and_then(|v| v.as_i64()).unwrap_or(1),
        success_threshold: json.get("successThreshold").and_then(|v| v.as_i64()).unwrap_or(1),
        failure_threshold: json.get("failureThreshold").and_then(|v| v.as_i64()).unwrap_or(3),
    }
}

fn extract_security_context(json: &serde_json::Value) -> SecurityContextDetail {
    SecurityContextDetail {
        run_as_user: json.get("runAsUser").and_then(|v| v.as_i64()),
        run_as_group: json.get("runAsGroup").and_then(|v| v.as_i64()),
        run_as_non_root: json.get("runAsNonRoot").and_then(|v| v.as_bool()),
        read_only_root_fs: json.get("readOnlyRootFilesystem").and_then(|v| v.as_bool()),
        privileged: json.get("privileged").and_then(|v| v.as_bool()),
        caps_add: json
            .pointer("/capabilities/add")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
        caps_drop: json
            .pointer("/capabilities/drop")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
    }
}

fn extract_container_state(json: &serde_json::Value) -> ContainerStateDetail {
    if let Some(running) = json.get("running") {
        ContainerStateDetail::Running {
            started_at: running.get("startedAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }
    } else if let Some(waiting) = json.get("waiting") {
        ContainerStateDetail::Waiting {
            reason: waiting.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            message: waiting.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }
    } else if let Some(terminated) = json.get("terminated") {
        ContainerStateDetail::Terminated {
            reason: terminated.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            exit_code: terminated.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1),
            finished_at: terminated
                .get("finishedAt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    } else {
        ContainerStateDetail::Unknown
    }
}

fn extract_volumes(spec: &serde_json::Value) -> Vec<VolumeDetail> {
    spec.get("volumes")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(extract_single_volume).collect())
        .unwrap_or_default()
}

fn extract_single_volume(json: &serde_json::Value) -> VolumeDetail {
    let name = json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let (volume_type, type_detail) = if let Some(cm) = json.get("configMap") {
        ("configMap".to_string(), cm.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string())
    } else if let Some(sec) = json.get("secret") {
        (
            "secret".to_string(),
            sec.get("secretName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        )
    } else if let Some(ed) = json.get("emptyDir") {
        let medium = ed.get("medium").and_then(|v| v.as_str()).unwrap_or("default");
        ("emptyDir".to_string(), format!("medium: {medium}"))
    } else if let Some(pvc) = json.get("persistentVolumeClaim") {
        (
            "persistentVolumeClaim".to_string(),
            pvc.get("claimName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        )
    } else if let Some(hp) = json.get("hostPath") {
        let path = hp.get("path").and_then(|v| v.as_str()).unwrap_or("");
        ("hostPath".to_string(), path.to_string())
    } else if json.get("projected").is_some() {
        ("projected".to_string(), "composite sources".to_string())
    } else if json.get("downwardAPI").is_some() {
        ("downwardAPI".to_string(), "pod metadata".to_string())
    } else if let Some(nfs) = json.get("nfs") {
        let server = nfs.get("server").and_then(|v| v.as_str()).unwrap_or("?");
        let path = nfs.get("path").and_then(|v| v.as_str()).unwrap_or("?");
        ("nfs".to_string(), format!("{server}:{path}"))
    } else if let Some(csi) = json.get("csi") {
        let driver = csi.get("driver").and_then(|v| v.as_str()).unwrap_or("?");
        ("csi".to_string(), driver.to_string())
    } else {
        ("unknown".to_string(), String::new())
    };

    VolumeDetail { name, volume_type, type_detail }
}

fn extract_tolerations(spec: &serde_json::Value) -> Vec<TolerationDetail> {
    spec.get("tolerations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|t| TolerationDetail {
                    key: t.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    operator: t
                        .get("operator")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Equal")
                        .to_string(),
                    value: t.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    effect: t.get("effect").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    toleration_seconds: t.get("tolerationSeconds").and_then(|v| v.as_i64()),
                })
                .collect()
        })
        .unwrap_or_default()
}
