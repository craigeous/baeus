use baeus_ui::components::json_extract::*;
use baeus_ui::components::resource_table::columns_for_kind;
use serde_json::json;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn pod_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "nginx-abc123",
            "namespace": "default",
            "uid": "uid-pod-123",
            "creationTimestamp": "2024-01-15T10:30:00Z",
            "ownerReferences": [{"kind": "ReplicaSet", "name": "nginx-abc"}],
            "labels": {"app": "nginx", "version": "1.0"}
        },
        "spec": {
            "containers": [{"name": "nginx"}, {"name": "sidecar"}],
            "nodeName": "node-1",
            "restartPolicy": "Always",
            "serviceAccountName": "default"
        },
        "status": {
            "phase": "Running",
            "qosClass": "BestEffort",
            "podIP": "10.0.0.5",
            "containerStatuses": [
                {"name": "nginx", "ready": true, "restartCount": 3, "state": {"running": {}}},
                {"name": "sidecar", "ready": false, "restartCount": 1, "state": {"waiting": {"reason": "CrashLoopBackOff"}}}
            ]
        }
    })
}

fn deployment_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "nginx-deploy",
            "namespace": "production",
            "uid": "uid-deploy-456",
            "creationTimestamp": "2024-01-10T08:00:00Z"
        },
        "spec": { "replicas": 3 },
        "status": {
            "readyReplicas": 2,
            "updatedReplicas": 3,
            "availableReplicas": 2,
            "conditions": [
                {"type": "Available", "status": "True"},
                {"type": "Progressing", "status": "True"}
            ]
        }
    })
}

fn service_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "web-svc",
            "namespace": "default",
            "uid": "uid-svc-789",
            "creationTimestamp": "2024-02-01T12:00:00Z"
        },
        "spec": {
            "type": "ClusterIP",
            "clusterIP": "10.96.0.1",
            "ports": [
                {"port": 80, "protocol": "TCP"},
                {"port": 443, "protocol": "TCP"}
            ],
            "selector": {"app": "web"}
        }
    })
}

fn node_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "worker-node-1",
            "uid": "uid-node-111",
            "creationTimestamp": "2023-06-01T00:00:00Z",
            "labels": {
                "node-role.kubernetes.io/worker": "",
                "node-role.kubernetes.io/control-plane": ""
            }
        },
        "spec": {
            "taints": [
                {"key": "node-role.kubernetes.io/control-plane", "effect": "NoSchedule"}
            ]
        },
        "status": {
            "nodeInfo": {
                "kubeletVersion": "v1.28.0",
                "osImage": "Ubuntu 22.04",
                "kernelVersion": "5.15.0",
                "containerRuntimeVersion": "containerd://1.6.0",
                "architecture": "amd64"
            },
            "conditions": [
                {"type": "Ready", "status": "True"},
                {"type": "MemoryPressure", "status": "False"},
                {"type": "DiskPressure", "status": "False"}
            ]
        }
    })
}

fn statefulset_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "postgres-ss",
            "namespace": "db",
            "uid": "uid-ss-001",
            "creationTimestamp": "2024-03-01T06:00:00Z"
        },
        "spec": { "replicas": 3 },
        "status": { "readyReplicas": 3 }
    })
}

fn daemonset_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "fluentd-ds",
            "namespace": "logging",
            "uid": "uid-ds-001",
            "creationTimestamp": "2024-01-01T00:00:00Z"
        },
        "spec": {
            "template": {
                "spec": {
                    "nodeSelector": {"kubernetes.io/os": "linux"}
                }
            }
        },
        "status": {
            "desiredNumberScheduled": 5,
            "numberReady": 4
        }
    })
}

fn replicaset_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "nginx-rs-abc",
            "namespace": "default",
            "uid": "uid-rs-001",
            "creationTimestamp": "2024-04-01T00:00:00Z"
        },
        "spec": { "replicas": 3 },
        "status": { "replicas": 3, "readyReplicas": 2 }
    })
}

fn job_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "migrate-db",
            "namespace": "jobs",
            "uid": "uid-job-001",
            "creationTimestamp": "2024-05-01T10:00:00Z"
        },
        "spec": { "completions": 1 },
        "status": {
            "succeeded": 1,
            "startTime": "2024-05-01T10:00:00Z",
            "completionTime": "2024-05-01T10:05:00Z",
            "conditions": [{"type": "Complete", "status": "True"}]
        }
    })
}

fn cronjob_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "backup-cron",
            "namespace": "ops",
            "uid": "uid-cj-001",
            "creationTimestamp": "2024-01-01T00:00:00Z"
        },
        "spec": {
            "schedule": "0 2 * * *",
            "suspend": false
        },
        "status": {
            "active": [{"name": "backup-cron-123"}],
            "lastScheduleTime": "2024-06-01T02:00:00Z"
        }
    })
}

fn ingress_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "web-ingress",
            "namespace": "default",
            "uid": "uid-ing-001",
            "creationTimestamp": "2024-02-15T00:00:00Z"
        },
        "spec": {
            "rules": [
                {"host": "example.com"},
                {"host": "api.example.com"}
            ]
        },
        "status": {
            "loadBalancer": {
                "ingress": [{"ip": "203.0.113.1"}]
            }
        }
    })
}

fn configmap_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "app-config",
            "namespace": "default",
            "uid": "uid-cm-001",
            "creationTimestamp": "2024-03-01T00:00:00Z"
        },
        "data": {
            "key1": "value1",
            "key2": "value2",
            "key3": "value3"
        }
    })
}

fn namespace_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "production",
            "uid": "uid-ns-001",
            "creationTimestamp": "2023-01-01T00:00:00Z"
        },
        "status": { "phase": "Active" }
    })
}

fn pv_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "pv-data-01",
            "uid": "uid-pv-001",
            "creationTimestamp": "2024-01-01T00:00:00Z"
        },
        "spec": {
            "capacity": {"storage": "10Gi"},
            "accessModes": ["ReadWriteOnce"],
            "persistentVolumeReclaimPolicy": "Retain",
            "storageClassName": "standard",
            "claimRef": {"namespace": "default", "name": "data-claim"}
        },
        "status": { "phase": "Bound" }
    })
}

fn pvc_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "data-claim",
            "namespace": "default",
            "uid": "uid-pvc-001",
            "creationTimestamp": "2024-01-15T00:00:00Z"
        },
        "spec": {
            "volumeName": "pv-data-01",
            "storageClassName": "standard"
        },
        "status": {
            "phase": "Bound",
            "capacity": {"storage": "10Gi"}
        }
    })
}

fn storageclass_json() -> serde_json::Value {
    json!({
        "metadata": {
            "name": "fast-ssd",
            "uid": "uid-sc-001",
            "creationTimestamp": "2024-01-01T00:00:00Z"
        },
        "provisioner": "kubernetes.io/gce-pd",
        "reclaimPolicy": "Delete",
        "volumeBindingMode": "WaitForFirstConsumer"
    })
}

// ---------------------------------------------------------------------------
// Cell count matches column count for every kind
// ---------------------------------------------------------------------------

#[test]
fn test_pod_cell_count_matches_columns() {
    let row = json_to_table_row("Pod", &pod_json());
    assert_eq!(row.cells.len(), columns_for_kind("Pod").len());
}

#[test]
fn test_deployment_cell_count() {
    let row = json_to_table_row("Deployment", &deployment_json());
    assert_eq!(row.cells.len(), columns_for_kind("Deployment").len());
}

#[test]
fn test_service_cell_count() {
    let row = json_to_table_row("Service", &service_json());
    assert_eq!(row.cells.len(), columns_for_kind("Service").len());
}

#[test]
fn test_node_cell_count() {
    let row = json_to_table_row("Node", &node_json());
    assert_eq!(row.cells.len(), columns_for_kind("Node").len());
}

#[test]
fn test_statefulset_cell_count() {
    let row = json_to_table_row("StatefulSet", &statefulset_json());
    assert_eq!(row.cells.len(), columns_for_kind("StatefulSet").len());
}

#[test]
fn test_daemonset_cell_count() {
    let row = json_to_table_row("DaemonSet", &daemonset_json());
    assert_eq!(row.cells.len(), columns_for_kind("DaemonSet").len());
}

#[test]
fn test_replicaset_cell_count() {
    let row = json_to_table_row("ReplicaSet", &replicaset_json());
    assert_eq!(row.cells.len(), columns_for_kind("ReplicaSet").len());
}

#[test]
fn test_job_cell_count() {
    let row = json_to_table_row("Job", &job_json());
    assert_eq!(row.cells.len(), columns_for_kind("Job").len());
}

#[test]
fn test_cronjob_cell_count() {
    let row = json_to_table_row("CronJob", &cronjob_json());
    assert_eq!(row.cells.len(), columns_for_kind("CronJob").len());
}

#[test]
fn test_ingress_cell_count() {
    let row = json_to_table_row("Ingress", &ingress_json());
    assert_eq!(row.cells.len(), columns_for_kind("Ingress").len());
}

#[test]
fn test_configmap_cell_count() {
    let row = json_to_table_row("ConfigMap", &configmap_json());
    assert_eq!(row.cells.len(), columns_for_kind("ConfigMap").len());
}

#[test]
fn test_secret_cell_count() {
    let row = json_to_table_row("Secret", &configmap_json());
    assert_eq!(row.cells.len(), columns_for_kind("Secret").len());
}

#[test]
fn test_namespace_cell_count() {
    let row = json_to_table_row("Namespace", &namespace_json());
    assert_eq!(row.cells.len(), columns_for_kind("Namespace").len());
}

#[test]
fn test_pv_cell_count() {
    let row = json_to_table_row("PersistentVolume", &pv_json());
    assert_eq!(row.cells.len(), columns_for_kind("PersistentVolume").len());
}

#[test]
fn test_pvc_cell_count() {
    let row = json_to_table_row("PersistentVolumeClaim", &pvc_json());
    assert_eq!(row.cells.len(), columns_for_kind("PersistentVolumeClaim").len());
}

#[test]
fn test_storageclass_cell_count() {
    let row = json_to_table_row("StorageClass", &storageclass_json());
    assert_eq!(row.cells.len(), columns_for_kind("StorageClass").len());
}

#[test]
fn test_generic_cell_count() {
    let generic = json!({"metadata": {"name": "foo", "namespace": "bar", "uid": "u1", "creationTimestamp": "2024-01-01T00:00:00Z"}, "status": {"phase": "Active"}});
    let row = json_to_table_row("UnknownKind", &generic);
    assert_eq!(row.cells.len(), columns_for_kind("UnknownKind").len());
}

// ---------------------------------------------------------------------------
// Per-kind extraction tests
// ---------------------------------------------------------------------------

#[test]
fn test_pod_row_values() {
    let row = json_to_table_row("Pod", &pod_json());
    assert_eq!(row.name, "nginx-abc123");
    assert_eq!(row.namespace.as_deref(), Some("default"));
    assert_eq!(row.uid, "uid-pod-123");
    assert_eq!(row.cells[0], "nginx-abc123"); // Name
    assert_eq!(row.cells[1], "default"); // Namespace
    assert_eq!(row.cells[2], "1/2"); // Containers (1 ready / 2 total)
    assert_eq!(row.cells[5], "4"); // Restarts (3+1)
    assert_eq!(row.cells[6], "ReplicaSet/nginx-abc"); // Controlled By
    assert_eq!(row.cells[7], "node-1"); // Node
    assert_eq!(row.cells[8], "10.0.0.5"); // IP (podIP)
    assert_eq!(row.cells[9], "BestEffort"); // QoS
    // cells[10] = Age (dynamic)
    assert_eq!(row.cells[11], "CrashLoopBackOff"); // Status (waiting reason)
}

#[test]
fn test_deployment_row_values() {
    let row = json_to_table_row("Deployment", &deployment_json());
    assert_eq!(row.cells[0], "nginx-deploy");
    assert_eq!(row.cells[1], "production");
    assert_eq!(row.cells[2], "2/3"); // Pods (ready/desired)
    assert_eq!(row.cells[3], "2"); // Ready
    assert_eq!(row.cells[4], "3"); // Up-to-date
    assert_eq!(row.cells[5], "2"); // Available
    // cells[6] = Age
    assert!(row.cells[7].contains("Available=True"));
}

#[test]
fn test_service_row_values() {
    let row = json_to_table_row("Service", &service_json());
    assert_eq!(row.cells[0], "web-svc");
    assert_eq!(row.cells[2], "ClusterIP");
    assert_eq!(row.cells[3], "10.96.0.1");
    assert_eq!(row.cells[4], "<none>"); // External IP
    assert_eq!(row.cells[5], "80/TCP, 443/TCP");
}

#[test]
fn test_node_row_values() {
    let row = json_to_table_row("Node", &node_json());
    assert_eq!(row.cells[0], "worker-node-1");
    assert_eq!(row.cells[4], "1"); // Taints count
    assert!(row.cells[5].contains("worker"));
    assert!(row.cells[5].contains("control-plane"));
    // cells[6] = Internal IP
    // cells[7] = Schedulable
    assert_eq!(row.cells[8], "v1.28.0");
    assert!(row.cells[10].contains("Ready=True"));
}

#[test]
fn test_statefulset_row_values() {
    let row = json_to_table_row("StatefulSet", &statefulset_json());
    assert_eq!(row.cells[0], "postgres-ss");
    assert_eq!(row.cells[2], "3/3"); // Pods
    assert_eq!(row.cells[3], "3"); // Replicas
}

#[test]
fn test_daemonset_row_values() {
    let row = json_to_table_row("DaemonSet", &daemonset_json());
    assert_eq!(row.cells[0], "fluentd-ds");
    assert_eq!(row.cells[2], "5"); // Desired
    // cells[3] = Current, cells[4] = Ready, cells[5] = Updated, cells[6] = Available
    assert!(row.cells[7].contains("kubernetes.io/os=linux")); // Node Selector
}

#[test]
fn test_replicaset_row_values() {
    let row = json_to_table_row("ReplicaSet", &replicaset_json());
    assert_eq!(row.cells[0], "nginx-rs-abc");
    assert_eq!(row.cells[2], "3"); // Desired
    assert_eq!(row.cells[3], "3"); // Current
    assert_eq!(row.cells[4], "2"); // Ready
}

#[test]
fn test_job_row_values() {
    let row = json_to_table_row("Job", &job_json());
    assert_eq!(row.cells[0], "migrate-db");
    assert_eq!(row.cells[2], "1/1"); // Completions
    assert_eq!(row.cells[3], "1"); // Parallelism
    assert_eq!(row.cells[4], "5m"); // Duration (5 min)
    // cells[5] = Age
    assert_eq!(row.cells[6], "Complete"); // Status
    assert!(row.cells[7].contains("Complete=True"));
}

#[test]
fn test_cronjob_row_values() {
    let row = json_to_table_row("CronJob", &cronjob_json());
    assert_eq!(row.cells[0], "backup-cron");
    assert_eq!(row.cells[2], "0 2 * * *"); // Schedule
    assert_eq!(row.cells[3], "—"); // Timezone
    assert_eq!(row.cells[4], "False"); // Suspend
    assert_eq!(row.cells[5], "1"); // Active
}

#[test]
fn test_ingress_row_values() {
    let row = json_to_table_row("Ingress", &ingress_json());
    assert_eq!(row.cells[0], "web-ingress");
    assert_eq!(row.cells[2], "203.0.113.1"); // Load Balancers
    assert!(row.cells[3].contains("example.com"));
    assert!(row.cells[3].contains("api.example.com"));
}

#[test]
fn test_configmap_row_values() {
    let row = json_to_table_row("ConfigMap", &configmap_json());
    assert_eq!(row.cells[0], "app-config");
    assert_eq!(row.cells[2], "3"); // Keys count
}

#[test]
fn test_namespace_row_values() {
    let row = json_to_table_row("Namespace", &namespace_json());
    assert_eq!(row.cells[0], "production");
    assert_eq!(row.cells[1], "Active");
}

#[test]
fn test_pv_row_values() {
    let row = json_to_table_row("PersistentVolume", &pv_json());
    assert_eq!(row.cells[0], "pv-data-01");
    assert_eq!(row.cells[1], "10Gi");
    assert_eq!(row.cells[2], "ReadWriteOnce");
    assert_eq!(row.cells[3], "Retain");
    assert_eq!(row.cells[4], "Bound");
    assert_eq!(row.cells[5], "default/data-claim");
    assert_eq!(row.cells[6], "standard");
}

#[test]
fn test_pvc_row_values() {
    let row = json_to_table_row("PersistentVolumeClaim", &pvc_json());
    assert_eq!(row.cells[0], "data-claim");
    assert_eq!(row.cells[2], "Bound");
    assert_eq!(row.cells[3], "pv-data-01");
    assert_eq!(row.cells[4], "10Gi");
    assert_eq!(row.cells[5], "standard");
}

#[test]
fn test_storageclass_row_values() {
    let row = json_to_table_row("StorageClass", &storageclass_json());
    assert_eq!(row.cells[0], "fast-ssd");
    assert_eq!(row.cells[1], "kubernetes.io/gce-pd");
    assert_eq!(row.cells[2], "Delete");
    assert_eq!(row.cells[3], "WaitForFirstConsumer");
}

// ---------------------------------------------------------------------------
// Utility function tests
// ---------------------------------------------------------------------------

#[test]
fn test_json_str_existing_path() {
    let j = json!({"metadata": {"name": "foo"}});
    assert_eq!(json_str(&j, "/metadata/name"), "foo");
}

#[test]
fn test_json_str_missing_path() {
    let j = json!({"metadata": {}});
    assert_eq!(json_str(&j, "/metadata/name"), "\u{2014}"); // em-dash
}

#[test]
fn test_human_age_recent() {
    // 30 seconds ago
    let now = chrono::Utc::now();
    let ts = (now - chrono::Duration::seconds(30)).to_rfc3339();
    let age = human_age(&ts);
    assert_eq!(age, "30s");
}

#[test]
fn test_human_age_minutes() {
    let now = chrono::Utc::now();
    let ts = (now - chrono::Duration::minutes(45)).to_rfc3339();
    let age = human_age(&ts);
    assert_eq!(age, "45m");
}

#[test]
fn test_human_age_hours() {
    let now = chrono::Utc::now();
    let ts = (now - chrono::Duration::hours(2) - chrono::Duration::minutes(30)).to_rfc3339();
    let age = human_age(&ts);
    assert_eq!(age, "2h30m");
}

#[test]
fn test_human_age_days() {
    let now = chrono::Utc::now();
    let ts = (now - chrono::Duration::days(3) - chrono::Duration::hours(12)).to_rfc3339();
    let age = human_age(&ts);
    assert_eq!(age, "3d12h");
}

#[test]
fn test_human_age_invalid() {
    assert_eq!(human_age("not-a-timestamp"), "\u{2014}");
}

#[test]
fn test_container_status_summary_mixed() {
    let j = pod_json();
    assert_eq!(container_status_summary(&j), "1/2"); // 1 ready, 2 total
}

#[test]
fn test_container_status_summary_empty() {
    let j = json!({"spec": {}, "status": {}});
    assert_eq!(container_status_summary(&j), "0/0");
}

#[test]
fn test_total_restarts() {
    let j = pod_json();
    assert_eq!(total_restarts(&j), "4"); // 3 + 1
}

#[test]
fn test_total_restarts_none() {
    let j = json!({"status": {}});
    assert_eq!(total_restarts(&j), "0");
}

#[test]
fn test_controlled_by_present() {
    let j = pod_json();
    assert_eq!(controlled_by(&j), "ReplicaSet/nginx-abc");
}

#[test]
fn test_controlled_by_absent() {
    let j = json!({"metadata": {}});
    assert_eq!(controlled_by(&j), "\u{2014}");
}

#[test]
fn test_format_ports_multiple() {
    let j = service_json();
    assert_eq!(format_ports(&j), "80/TCP, 443/TCP");
}

#[test]
fn test_format_ports_empty() {
    let j = json!({"spec": {}});
    assert_eq!(format_ports(&j), "\u{2014}");
}

#[test]
fn test_node_roles_multiple() {
    let j = node_json();
    let roles = node_roles(&j);
    assert!(roles.contains("worker"));
    assert!(roles.contains("control-plane"));
}

#[test]
fn test_node_roles_none() {
    let j = json!({"metadata": {"labels": {"kubernetes.io/hostname": "node-1"}}});
    assert_eq!(node_roles(&j), "<none>");
}

// ---------------------------------------------------------------------------
// Detail extraction tests
// ---------------------------------------------------------------------------

#[test]
fn test_extract_detail_properties_pod() {
    let props = extract_detail_properties("Pod", &pod_json());
    let map: std::collections::HashMap<_, _> = props.into_iter().collect();
    assert_eq!(map.get("Name").unwrap(), "nginx-abc123");
    assert_eq!(map.get("Namespace").unwrap(), "default");
    assert_eq!(map.get("Node").unwrap(), "node-1");
    assert_eq!(map.get("Pod IP").unwrap(), "10.0.0.5");
    assert_eq!(map.get("QoS Class").unwrap(), "BestEffort");
    assert_eq!(map.get("Controlled By").unwrap(), "ReplicaSet/nginx-abc");
}

#[test]
fn test_extract_detail_properties_deployment() {
    let props = extract_detail_properties("Deployment", &deployment_json());
    let map: std::collections::HashMap<_, _> = props.into_iter().collect();
    assert_eq!(map.get("Ready Replicas").unwrap(), "2/3");
    assert_eq!(map.get("Updated Replicas").unwrap(), "3");
    assert_eq!(map.get("Available Replicas").unwrap(), "2");
}

#[test]
fn test_extract_detail_properties_service() {
    let props = extract_detail_properties("Service", &service_json());
    let map: std::collections::HashMap<_, _> = props.into_iter().collect();
    assert_eq!(map.get("Type").unwrap(), "ClusterIP");
    assert_eq!(map.get("Cluster IP").unwrap(), "10.96.0.1");
    assert_eq!(map.get("Ports").unwrap(), "80/TCP, 443/TCP");
    assert!(map.get("Selector").unwrap().contains("app=web"));
}

#[test]
fn test_extract_detail_properties_node() {
    let props = extract_detail_properties("Node", &node_json());
    let map: std::collections::HashMap<_, _> = props.into_iter().collect();
    assert_eq!(map.get("Version").unwrap(), "v1.28.0");
    assert!(map.get("Roles").unwrap().contains("control-plane"));
}

#[test]
fn test_extract_conditions() {
    let conditions = extract_conditions(&deployment_json());
    assert_eq!(conditions.len(), 2);
    assert_eq!(conditions[0].0, "Available");
    assert_eq!(conditions[0].1, "True");
}

#[test]
fn test_extract_conditions_empty() {
    let j = json!({"status": {}});
    let conditions = extract_conditions(&j);
    assert!(conditions.is_empty());
}

#[test]
fn test_extract_labels() {
    let labels = extract_labels(&pod_json());
    let map: std::collections::HashMap<_, _> = labels.into_iter().collect();
    assert_eq!(map.get("app").unwrap(), "nginx");
    assert_eq!(map.get("version").unwrap(), "1.0");
}

#[test]
fn test_extract_labels_empty() {
    let j = json!({"metadata": {}});
    let labels = extract_labels(&j);
    assert!(labels.is_empty());
}

#[test]
fn test_extract_annotations() {
    let j = json!({"metadata": {"annotations": {"note": "test"}}});
    let anns = extract_annotations(&j);
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0], ("note".to_string(), "test".to_string()));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_pod_all_running() {
    let j = json!({
        "metadata": {"name": "ok-pod", "namespace": "ns", "uid": "u", "creationTimestamp": "2024-01-01T00:00:00Z"},
        "spec": {"containers": [{"name": "a"}], "nodeName": "n"},
        "status": {
            "phase": "Running",
            "qosClass": "Guaranteed",
            "containerStatuses": [{"name": "a", "ready": true, "restartCount": 0, "state": {"running": {}}}]
        }
    });
    let row = json_to_table_row("Pod", &j);
    assert_eq!(row.cells[2], "1/1"); // All running
    assert_eq!(row.cells[5], "0"); // No restarts
    assert_eq!(row.cells[11], "Running"); // Status from phase
}

#[test]
fn test_empty_json_generic_fallback() {
    let j = json!({"metadata": {"name": "x", "uid": "u"}, "status": {}});
    let row = json_to_table_row("Unknown", &j);
    assert_eq!(row.cells.len(), 4); // Name, Namespace, Age, Status
    assert_eq!(row.cells[0], "x");
}
