pub mod auth;
pub mod aws_eks;
pub mod aws_sso;
pub mod client;
pub mod cluster;
pub mod crd;
pub mod exec;
pub mod informer;
pub mod kubeconfig;
pub mod logs;
pub mod metrics;
pub mod rbac;
pub mod resource;
pub mod runtime;
pub mod watch;

/// Re-export `kube::Client` so downstream crates (e.g. baeus-ui) can use it
/// without adding a direct dependency on the `kube` crate.
pub use kube::Client as KubeClient;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamespacePhase {
    Active,
    Terminating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub name: String,
    pub cluster_id: Uuid,
    pub phase: NamespacePhase,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub resource_version: String,
}

impl Namespace {
    pub fn new(name: String, cluster_id: Uuid) -> Self {
        Self {
            name,
            cluster_id,
            phase: NamespacePhase::Active,
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            resource_version: String::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.phase == NamespacePhase::Active
    }

    pub fn is_terminating(&self) -> bool {
        self.phase == NamespacePhase::Terminating
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Normal,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub uid: String,
    pub type_name: EventType,
    pub reason: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub count: u32,
    pub involved_resource_uid: Option<String>,
    pub involved_resource_kind: Option<String>,
    pub involved_resource_name: Option<String>,
    pub cluster_id: Uuid,
}

impl Event {
    pub fn is_warning(&self) -> bool {
        self.type_name == EventType::Warning
    }

    pub fn is_normal(&self) -> bool {
        self.type_name == EventType::Normal
    }

    pub fn has_involved_resource(&self) -> bool {
        self.involved_resource_uid.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_new_defaults() {
        let cluster_id = Uuid::new_v4();
        let ns = Namespace::new("default".to_string(), cluster_id);

        assert_eq!(ns.name, "default");
        assert_eq!(ns.cluster_id, cluster_id);
        assert_eq!(ns.phase, NamespacePhase::Active);
        assert!(ns.is_active());
        assert!(!ns.is_terminating());
        assert!(ns.labels.is_empty());
        assert!(ns.annotations.is_empty());
    }

    #[test]
    fn test_namespace_terminating_phase() {
        let mut ns = Namespace::new("old-ns".to_string(), Uuid::new_v4());
        ns.phase = NamespacePhase::Terminating;

        assert!(ns.is_terminating());
        assert!(!ns.is_active());
    }

    #[test]
    fn test_namespace_with_labels() {
        let mut ns = Namespace::new("monitoring".to_string(), Uuid::new_v4());
        ns.labels.insert("team".to_string(), "platform".to_string());
        ns.labels.insert("env".to_string(), "production".to_string());

        assert_eq!(ns.labels.len(), 2);
        assert_eq!(ns.labels.get("team").unwrap(), "platform");
    }

    #[test]
    fn test_namespace_serialization() {
        let ns = Namespace::new("test-ns".to_string(), Uuid::new_v4());
        let json = serde_json::to_string(&ns).unwrap();
        let deserialized: Namespace = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, ns.name);
        assert_eq!(deserialized.cluster_id, ns.cluster_id);
        assert_eq!(deserialized.phase, ns.phase);
    }

    #[test]
    fn test_event_normal() {
        let event = Event {
            uid: "evt-1".to_string(),
            type_name: EventType::Normal,
            reason: "Scheduled".to_string(),
            message: "Successfully assigned pod to node".to_string(),
            timestamp: Utc::now(),
            count: 1,
            involved_resource_uid: Some("pod-uid-1".to_string()),
            involved_resource_kind: Some("Pod".to_string()),
            involved_resource_name: Some("my-pod".to_string()),
            cluster_id: Uuid::new_v4(),
        };

        assert!(event.is_normal());
        assert!(!event.is_warning());
        assert!(event.has_involved_resource());
    }

    #[test]
    fn test_event_warning() {
        let event = Event {
            uid: "evt-2".to_string(),
            type_name: EventType::Warning,
            reason: "FailedScheduling".to_string(),
            message: "Insufficient CPU".to_string(),
            timestamp: Utc::now(),
            count: 3,
            involved_resource_uid: None,
            involved_resource_kind: None,
            involved_resource_name: None,
            cluster_id: Uuid::new_v4(),
        };

        assert!(event.is_warning());
        assert!(!event.is_normal());
        assert!(!event.has_involved_resource());
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            uid: "evt-3".to_string(),
            type_name: EventType::Warning,
            reason: "BackOff".to_string(),
            message: "Back-off restarting container".to_string(),
            timestamp: Utc::now(),
            count: 5,
            involved_resource_uid: Some("pod-uid".to_string()),
            involved_resource_kind: Some("Pod".to_string()),
            involved_resource_name: Some("crashing-pod".to_string()),
            cluster_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.uid, event.uid);
        assert_eq!(deserialized.type_name, event.type_name);
        assert_eq!(deserialized.count, event.count);
    }

    #[test]
    fn test_event_type_serialization() {
        assert_eq!(serde_json::to_string(&EventType::Normal).unwrap(), "\"Normal\"");
        assert_eq!(serde_json::to_string(&EventType::Warning).unwrap(), "\"Warning\"");
    }
}
