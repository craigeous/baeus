pub mod charts;
pub mod operations;
pub mod releases;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelmReleaseStatus {
    Deployed,
    Failed,
    Uninstalling,
    PendingInstall,
    PendingUpgrade,
    PendingRollback,
    Superseded,
    Unknown,
}

impl HelmReleaseStatus {
    pub fn from_str_status(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "deployed" => Self::Deployed,
            "failed" => Self::Failed,
            "uninstalling" => Self::Uninstalling,
            "pending-install" => Self::PendingInstall,
            "pending-upgrade" => Self::PendingUpgrade,
            "pending-rollback" => Self::PendingRollback,
            "superseded" => Self::Superseded,
            _ => Self::Unknown,
        }
    }

    pub fn is_healthy(&self) -> bool {
        *self == Self::Deployed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelmRelease {
    pub name: String,
    pub namespace: String,
    pub chart_name: String,
    pub chart_version: String,
    pub app_version: Option<String>,
    pub status: HelmReleaseStatus,
    pub revision: u32,
    pub last_deployed: DateTime<Utc>,
    pub values: Value,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelmRepository {
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

impl HelmRepository {
    pub fn new(name: String, url: String) -> Self {
        Self { name, url, enabled: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_helm_release_status_from_string() {
        assert_eq!(HelmReleaseStatus::from_str_status("deployed"), HelmReleaseStatus::Deployed);
        assert_eq!(HelmReleaseStatus::from_str_status("failed"), HelmReleaseStatus::Failed);
        assert_eq!(
            HelmReleaseStatus::from_str_status("pending-install"),
            HelmReleaseStatus::PendingInstall
        );
        assert_eq!(HelmReleaseStatus::from_str_status("DEPLOYED"), HelmReleaseStatus::Deployed);
        assert_eq!(HelmReleaseStatus::from_str_status("unknown-state"), HelmReleaseStatus::Unknown);
    }

    #[test]
    fn test_helm_release_status_is_healthy() {
        assert!(HelmReleaseStatus::Deployed.is_healthy());
        assert!(!HelmReleaseStatus::Failed.is_healthy());
        assert!(!HelmReleaseStatus::PendingUpgrade.is_healthy());
    }

    #[test]
    fn test_helm_release_serialization() {
        let release = HelmRelease {
            name: "nginx-ingress".to_string(),
            namespace: "ingress-nginx".to_string(),
            chart_name: "ingress-nginx".to_string(),
            chart_version: "4.8.3".to_string(),
            app_version: Some("1.9.4".to_string()),
            status: HelmReleaseStatus::Deployed,
            revision: 5,
            last_deployed: Utc::now(),
            values: json!({"replicaCount": 2}),
            cluster_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&release).unwrap();
        let deserialized: HelmRelease = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "nginx-ingress");
        assert_eq!(deserialized.revision, 5);
        assert_eq!(deserialized.status, HelmReleaseStatus::Deployed);
    }

    #[test]
    fn test_helm_repository() {
        let repo = HelmRepository::new(
            "bitnami".to_string(),
            "https://charts.bitnami.com/bitnami".to_string(),
        );
        assert!(repo.enabled);
        assert_eq!(repo.name, "bitnami");
    }
}
