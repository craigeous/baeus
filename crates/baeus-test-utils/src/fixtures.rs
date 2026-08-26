use baeus_core::resource::{Condition, OwnerReference, Resource};
use baeus_core::{Event, EventType, Namespace, NamespacePhase};
use chrono::Utc;
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn sample_cluster_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}

pub fn sample_namespace(name: &str) -> Namespace {
    Namespace {
        name: name.to_string(),
        cluster_id: sample_cluster_id(),
        phase: NamespacePhase::Active,
        labels: BTreeMap::new(),
        annotations: BTreeMap::new(),
        resource_version: "1".to_string(),
    }
}

pub fn sample_pod(name: &str, namespace: &str) -> Resource {
    Resource {
        uid: format!("pod-uid-{name}"),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        kind: "Pod".to_string(),
        api_version: "v1".to_string(),
        labels: BTreeMap::from([("app".to_string(), name.to_string())]),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "100".to_string(),
        owner_references: vec![],
        spec: json!({
            "containers": [{
                "name": name,
                "image": "nginx:latest".to_string()
            }]
        }),
        status: Some(json!({"phase": "Running"})),
        conditions: vec![Condition {
            type_name: "Ready".to_string(),
            status: "True".to_string(),
            reason: None,
            message: None,
            last_transition: Utc::now(),
        }],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_deployment(name: &str, namespace: &str) -> Resource {
    Resource {
        uid: format!("deploy-uid-{name}"),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        kind: "Deployment".to_string(),
        api_version: "apps/v1".to_string(),
        labels: BTreeMap::from([("app".to_string(), name.to_string())]),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "200".to_string(),
        owner_references: vec![],
        spec: json!({
            "replicas": 3,
            "selector": {"matchLabels": {"app": name}}
        }),
        status: Some(json!({
            "readyReplicas": 3,
            "replicas": 3,
            "availableReplicas": 3
        })),
        conditions: vec![
            Condition {
                type_name: "Available".to_string(),
                status: "True".to_string(),
                reason: Some("MinimumReplicasAvailable".to_string()),
                message: None,
                last_transition: Utc::now(),
            },
            Condition {
                type_name: "Progressing".to_string(),
                status: "True".to_string(),
                reason: Some("NewReplicaSetAvailable".to_string()),
                message: None,
                last_transition: Utc::now(),
            },
        ],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_service(name: &str, namespace: &str) -> Resource {
    Resource {
        uid: format!("svc-uid-{name}"),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        kind: "Service".to_string(),
        api_version: "v1".to_string(),
        labels: BTreeMap::from([("app".to_string(), name.to_string())]),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "300".to_string(),
        owner_references: vec![],
        spec: json!({
            "type": "ClusterIP",
            "ports": [{"port": 80, "targetPort": 8080}],
            "selector": {"app": name}
        }),
        status: None,
        conditions: vec![],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_configmap(name: &str, namespace: &str) -> Resource {
    Resource {
        uid: format!("cm-uid-{name}"),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        kind: "ConfigMap".to_string(),
        api_version: "v1".to_string(),
        labels: BTreeMap::new(),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "400".to_string(),
        owner_references: vec![],
        spec: json!({"data": {"key": "value"}}),
        status: None,
        conditions: vec![],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_secret(name: &str, namespace: &str) -> Resource {
    Resource {
        uid: format!("secret-uid-{name}"),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        kind: "Secret".to_string(),
        api_version: "v1".to_string(),
        labels: BTreeMap::new(),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "500".to_string(),
        owner_references: vec![],
        spec: json!({"type": "Opaque"}),
        status: None,
        conditions: vec![],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_node(name: &str) -> Resource {
    Resource {
        uid: format!("node-uid-{name}"),
        name: name.to_string(),
        namespace: None,
        kind: "Node".to_string(),
        api_version: "v1".to_string(),
        labels: BTreeMap::from([
            ("kubernetes.io/os".to_string(), "linux".to_string()),
            ("node-role.kubernetes.io/control-plane".to_string(), String::new()),
        ]),
        annotations: BTreeMap::new(),
        creation_timestamp: Utc::now(),
        resource_version: "600".to_string(),
        owner_references: vec![],
        spec: json!({}),
        status: Some(json!({
            "conditions": [
                {"type": "Ready", "status": "True"}
            ],
            "capacity": {
                "cpu": "4",
                "memory": "16Gi"
            }
        })),
        conditions: vec![Condition {
            type_name: "Ready".to_string(),
            status: "True".to_string(),
            reason: Some("KubeletReady".to_string()),
            message: None,
            last_transition: Utc::now(),
        }],
        cluster_id: sample_cluster_id(),
    }
}

pub fn sample_pod_with_owner(
    name: &str,
    namespace: &str,
    owner_name: &str,
    owner_kind: &str,
) -> Resource {
    let mut pod = sample_pod(name, namespace);
    pod.owner_references.push(OwnerReference {
        uid: format!("{}-uid-{owner_name}", owner_kind.to_lowercase()),
        kind: owner_kind.to_string(),
        name: owner_name.to_string(),
        api_version: "apps/v1".to_string(),
        controller: true,
    });
    pod
}

pub fn sample_event(reason: &str, message: &str, event_type: EventType) -> Event {
    Event {
        uid: format!("evt-{reason}"),
        type_name: event_type,
        reason: reason.to_string(),
        message: message.to_string(),
        timestamp: Utc::now(),
        count: 1,
        involved_resource_uid: Some("pod-uid-test".to_string()),
        involved_resource_kind: Some("Pod".to_string()),
        involved_resource_name: Some("test-pod".to_string()),
        cluster_id: sample_cluster_id(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_pod_fixture() {
        let pod = sample_pod("nginx", "default");
        assert_eq!(pod.name, "nginx");
        assert_eq!(pod.namespace.as_deref(), Some("default"));
        assert_eq!(pod.kind, "Pod");
        assert!(pod.is_ready());
    }

    #[test]
    fn test_sample_deployment_fixture() {
        let deploy = sample_deployment("web-app", "production");
        assert_eq!(deploy.name, "web-app");
        assert_eq!(deploy.kind, "Deployment");
        assert_eq!(deploy.conditions.len(), 2);
    }

    #[test]
    fn test_sample_node_is_cluster_scoped() {
        let node = sample_node("worker-1");
        assert!(!node.is_namespaced());
        assert_eq!(node.kind, "Node");
    }

    #[test]
    fn test_sample_pod_with_owner() {
        let pod = sample_pod_with_owner("nginx-abc", "default", "nginx-rs", "ReplicaSet");
        assert!(pod.has_owner());
        let owner = pod.controller_owner().unwrap();
        assert_eq!(owner.kind, "ReplicaSet");
        assert_eq!(owner.name, "nginx-rs");
    }

    #[test]
    fn test_sample_event_fixture() {
        let event = sample_event("Scheduled", "Pod scheduled to node", EventType::Normal);
        assert!(event.is_normal());
        assert!(event.has_involved_resource());
    }
}
