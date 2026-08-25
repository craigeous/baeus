// T333: Per-resource column definitions tests (FR-069)

use baeus_ui::components::resource_table::*;

// ---------------------------------------------------------------------------
// Pod columns
// ---------------------------------------------------------------------------

#[test]
fn pod_columns_count() {
    let cols = columns_for_kind("Pod");
    assert_eq!(cols.len(), 12, "Pod should have 12 columns");
}

#[test]
fn pod_columns_labels() {
    let cols = columns_for_kind("Pod");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "Namespace",
            "Containers",
            "CPU",
            "Memory",
            "Restarts",
            "Controlled By",
            "Node",
            "IP",
            "QoS",
            "Age",
            "Status",
        ]
    );
}

// ---------------------------------------------------------------------------
// Deployment columns
// ---------------------------------------------------------------------------

#[test]
fn deployment_columns_count() {
    let cols = columns_for_kind("Deployment");
    assert_eq!(cols.len(), 8, "Deployment should have 8 columns");
}

#[test]
fn deployment_columns_labels() {
    let cols = columns_for_kind("Deployment");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Name", "Namespace", "Pods", "Ready", "Up-to-date", "Available", "Age", "Conditions",]
    );
}

// ---------------------------------------------------------------------------
// CronJob columns
// ---------------------------------------------------------------------------

#[test]
fn cronjob_columns_count() {
    let cols = columns_for_kind("CronJob");
    assert_eq!(cols.len(), 8, "CronJob should have 8 columns");
}

#[test]
fn cronjob_columns_labels() {
    let cols = columns_for_kind("CronJob");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "Namespace",
            "Schedule",
            "Timezone",
            "Suspend",
            "Active",
            "Last Schedule",
            "Age",
        ]
    );
}

// ---------------------------------------------------------------------------
// Node columns
// ---------------------------------------------------------------------------

#[test]
fn node_columns_count() {
    let cols = columns_for_kind("Node");
    assert_eq!(cols.len(), 11, "Node should have 11 columns");
}

#[test]
fn node_columns_labels() {
    let cols = columns_for_kind("Node");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "CPU",
            "Memory",
            "Disk",
            "Taints",
            "Roles",
            "Internal IP",
            "Schedulable",
            "Version",
            "Age",
            "Conditions",
        ]
    );
}

// ---------------------------------------------------------------------------
// Service columns
// ---------------------------------------------------------------------------

#[test]
fn service_columns_count() {
    let cols = columns_for_kind("Service");
    assert_eq!(cols.len(), 7, "Service should have 7 columns");
}

#[test]
fn service_columns_labels() {
    let cols = columns_for_kind("Service");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Name", "Namespace", "Type", "Cluster IP", "External IP", "Ports", "Age"]
    );
}

// ---------------------------------------------------------------------------
// Ingress columns
// ---------------------------------------------------------------------------

#[test]
fn ingress_columns_count() {
    let cols = columns_for_kind("Ingress");
    assert_eq!(cols.len(), 5, "Ingress should have 5 columns");
}

#[test]
fn ingress_columns_labels() {
    let cols = columns_for_kind("Ingress");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Load Balancers", "Rules", "Age"]);
}

// ---------------------------------------------------------------------------
// ConfigMap columns
// ---------------------------------------------------------------------------

#[test]
fn configmap_columns_count() {
    let cols = columns_for_kind("ConfigMap");
    assert_eq!(cols.len(), 4, "ConfigMap should have 4 columns");
}

#[test]
fn configmap_columns_labels() {
    let cols = columns_for_kind("ConfigMap");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Keys", "Age"]);
}

// ---------------------------------------------------------------------------
// Secret columns
// ---------------------------------------------------------------------------

#[test]
fn secret_columns_count() {
    let cols = columns_for_kind("Secret");
    assert_eq!(cols.len(), 5, "Secret should have 5 columns");
}

#[test]
fn secret_columns_labels() {
    let cols = columns_for_kind("Secret");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Type", "Keys", "Age"]);
}

// ---------------------------------------------------------------------------
// StatefulSet columns
// ---------------------------------------------------------------------------

#[test]
fn statefulset_columns_count() {
    let cols = columns_for_kind("StatefulSet");
    assert_eq!(cols.len(), 5, "StatefulSet should have 5 columns");
}

#[test]
fn statefulset_columns_labels() {
    let cols = columns_for_kind("StatefulSet");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Pods", "Replicas", "Age"]);
}

// ---------------------------------------------------------------------------
// DaemonSet columns
// ---------------------------------------------------------------------------

#[test]
fn daemonset_columns_count() {
    let cols = columns_for_kind("DaemonSet");
    assert_eq!(cols.len(), 9, "DaemonSet should have 9 columns");
}

#[test]
fn daemonset_columns_labels() {
    let cols = columns_for_kind("DaemonSet");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "Namespace",
            "Desired",
            "Current",
            "Ready",
            "Up-to-date",
            "Available",
            "Node Selector",
            "Age",
        ]
    );
}

// ---------------------------------------------------------------------------
// ReplicaSet columns
// ---------------------------------------------------------------------------

#[test]
fn replicaset_columns_count() {
    let cols = columns_for_kind("ReplicaSet");
    assert_eq!(cols.len(), 6, "ReplicaSet should have 6 columns");
}

#[test]
fn replicaset_columns_labels() {
    let cols = columns_for_kind("ReplicaSet");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Desired", "Current", "Ready", "Age"]);
}

// ---------------------------------------------------------------------------
// Job columns
// ---------------------------------------------------------------------------

#[test]
fn job_columns_count() {
    let cols = columns_for_kind("Job");
    assert_eq!(cols.len(), 8, "Job should have 8 columns");
}

#[test]
fn job_columns_labels() {
    let cols = columns_for_kind("Job");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "Namespace",
            "Completions",
            "Parallelism",
            "Duration",
            "Age",
            "Status",
            "Conditions",
        ]
    );
}

// ---------------------------------------------------------------------------
// Namespace columns
// ---------------------------------------------------------------------------

#[test]
fn namespace_columns_count() {
    let cols = columns_for_kind("Namespace");
    assert_eq!(cols.len(), 3, "Namespace should have 3 columns");
}

#[test]
fn namespace_columns_labels() {
    let cols = columns_for_kind("Namespace");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Status", "Age"]);
}

// ---------------------------------------------------------------------------
// PersistentVolume columns
// ---------------------------------------------------------------------------

#[test]
fn persistent_volume_columns_count() {
    let cols = columns_for_kind("PersistentVolume");
    assert_eq!(cols.len(), 8, "PersistentVolume should have 8 columns");
}

#[test]
fn persistent_volume_columns_labels() {
    let cols = columns_for_kind("PersistentVolume");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Name",
            "Capacity",
            "Access Modes",
            "Reclaim Policy",
            "Status",
            "Claim",
            "Storage Class",
            "Age",
        ]
    );
}

// ---------------------------------------------------------------------------
// PersistentVolumeClaim columns
// ---------------------------------------------------------------------------

#[test]
fn persistent_volume_claim_columns_count() {
    let cols = columns_for_kind("PersistentVolumeClaim");
    assert_eq!(cols.len(), 7, "PersistentVolumeClaim should have 7 columns");
}

#[test]
fn persistent_volume_claim_columns_labels() {
    let cols = columns_for_kind("PersistentVolumeClaim");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Name", "Namespace", "Status", "Volume", "Capacity", "Storage Class", "Age",]
    );
}

// ---------------------------------------------------------------------------
// StorageClass columns
// ---------------------------------------------------------------------------

#[test]
fn storage_class_columns_count() {
    let cols = columns_for_kind("StorageClass");
    assert_eq!(cols.len(), 5, "StorageClass should have 5 columns");
}

#[test]
fn storage_class_columns_labels() {
    let cols = columns_for_kind("StorageClass");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Provisioner", "Reclaim Policy", "Volume Binding", "Age"]);
}

// ---------------------------------------------------------------------------
// Unknown / default columns
// ---------------------------------------------------------------------------

#[test]
fn unknown_kind_returns_default_columns() {
    let cols = columns_for_kind("UnknownKind");
    assert_eq!(cols.len(), 4, "Unknown kind should have 4 default columns");
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Name", "Namespace", "Age", "Status"]);
}

#[test]
fn another_unknown_kind_returns_default_columns() {
    let cols = columns_for_kind("CustomResource");
    assert_eq!(cols.len(), 4);
}

// ---------------------------------------------------------------------------
// Cross-cutting: Name column is always sortable
// ---------------------------------------------------------------------------

#[test]
fn all_name_columns_are_sortable() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "CronJob",
        "Node",
        "Service",
        "Ingress",
        "ConfigMap",
        "Secret",
        "StatefulSet",
        "DaemonSet",
        "ReplicaSet",
        "Job",
        "Namespace",
        "PersistentVolume",
        "PersistentVolumeClaim",
        "StorageClass",
        "UnknownKind",
    ];

    for kind in kinds {
        let cols = columns_for_kind(kind);
        let name_col = cols.iter().find(|c| c.id == "name");
        assert!(name_col.is_some(), "Kind '{kind}' should have a Name column");
        assert!(name_col.unwrap().sortable, "Kind '{kind}': Name column should be sortable");
    }
}

// ---------------------------------------------------------------------------
// Column labels are human-readable (no snake_case, no empty strings)
// ---------------------------------------------------------------------------

#[test]
fn column_labels_are_human_readable() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "CronJob",
        "Node",
        "Service",
        "Ingress",
        "ConfigMap",
        "Secret",
        "StatefulSet",
        "DaemonSet",
        "ReplicaSet",
        "Job",
        "Namespace",
        "PersistentVolume",
        "PersistentVolumeClaim",
        "StorageClass",
        "UnknownKind",
    ];

    for kind in kinds {
        let cols = columns_for_kind(kind);
        for col in &cols {
            assert!(!col.label.is_empty(), "Kind '{kind}': column '{}' has empty label", col.id);
            // Human-readable labels start with an uppercase letter
            assert!(
                col.label.chars().next().unwrap().is_uppercase(),
                "Kind '{kind}': column '{}' label '{}' should start with uppercase",
                col.id,
                col.label
            );
            // Labels should not contain underscores (snake_case)
            assert!(
                !col.label.contains('_'),
                "Kind '{kind}': column '{}' label '{}' contains underscore (not human-readable)",
                col.id,
                col.label
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Width weights are positive
// ---------------------------------------------------------------------------

#[test]
fn all_column_width_weights_are_positive() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "CronJob",
        "Node",
        "Service",
        "Ingress",
        "ConfigMap",
        "Secret",
        "StatefulSet",
        "DaemonSet",
        "ReplicaSet",
        "Job",
        "Namespace",
        "PersistentVolume",
        "PersistentVolumeClaim",
        "StorageClass",
        "UnknownKind",
    ];

    for kind in kinds {
        let cols = columns_for_kind(kind);
        for col in &cols {
            assert!(
                col.width_weight > 0.0,
                "Kind '{kind}': column '{}' has non-positive width_weight {}",
                col.id,
                col.width_weight
            );
        }
    }
}
