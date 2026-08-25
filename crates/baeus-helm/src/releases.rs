use crate::{HelmRelease, HelmReleaseStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::io::Read;
use uuid::Uuid;

const HELM_RELEASE_LABEL: &str = "owner=helm";
/// Maximum decompressed size for a Helm release (50 MB).
const MAX_HELM_DECOMPRESSED_SIZE: u64 = 50 * 1024 * 1024;

pub fn decode_helm_release(
    secret_data: &str,
    namespace: &str,
    cluster_id: Uuid,
) -> Result<HelmRelease> {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, secret_data)
        .context("Failed to base64 decode Helm release data")?;

    let decoder = flate2::read::GzDecoder::new(&decoded[..]);
    let mut limited = decoder.take(MAX_HELM_DECOMPRESSED_SIZE);
    let mut decompressed = String::new();
    limited
        .read_to_string(&mut decompressed)
        .context("Failed to gzip decompress Helm release data")?;

    let release_json: Value =
        serde_json::from_str(&decompressed).context("Failed to parse Helm release JSON")?;

    let name = release_json["name"].as_str().unwrap_or("unknown").to_string();
    let info = &release_json["info"];
    let chart = &release_json["chart"]["metadata"];

    let status_str = info["status"].as_str().unwrap_or("unknown");
    let last_deployed_str = info["last_deployed"].as_str().unwrap_or("");
    let last_deployed = DateTime::parse_from_rfc3339(last_deployed_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(HelmRelease {
        name,
        namespace: namespace.to_string(),
        chart_name: chart["name"].as_str().unwrap_or("unknown").to_string(),
        chart_version: chart["version"].as_str().unwrap_or("0.0.0").to_string(),
        app_version: chart["appVersion"].as_str().map(|s| s.to_string()),
        status: HelmReleaseStatus::from_str_status(status_str),
        revision: release_json["version"].as_u64().unwrap_or(1) as u32,
        last_deployed,
        values: release_json["config"].clone(),
        cluster_id,
    })
}

pub fn helm_secret_label_selector() -> &'static str {
    HELM_RELEASE_LABEL
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn make_helm_secret_data(name: &str, status: &str) -> String {
        let release_json = serde_json::json!({
            "name": name,
            "version": 3,
            "info": {
                "status": status,
                "last_deployed": "2026-02-24T12:00:00Z"
            },
            "chart": {
                "metadata": {
                    "name": "nginx",
                    "version": "1.2.3",
                    "appVersion": "1.25.0"
                }
            },
            "config": {
                "replicaCount": 2
            }
        });

        let json_bytes = serde_json::to_vec(&release_json).unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&json_bytes).unwrap();
        let compressed = encoder.finish().unwrap();

        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &compressed)
    }

    #[test]
    fn test_decode_helm_release() {
        let data = make_helm_secret_data("my-release", "deployed");
        let release = decode_helm_release(&data, "default", Uuid::new_v4()).unwrap();

        assert_eq!(release.name, "my-release");
        assert_eq!(release.chart_name, "nginx");
        assert_eq!(release.chart_version, "1.2.3");
        assert_eq!(release.app_version.as_deref(), Some("1.25.0"));
        assert_eq!(release.status, HelmReleaseStatus::Deployed);
        assert_eq!(release.revision, 3);
        assert_eq!(release.namespace, "default");
    }

    #[test]
    fn test_decode_helm_release_failed_status() {
        let data = make_helm_secret_data("failed-release", "failed");
        let release = decode_helm_release(&data, "test-ns", Uuid::new_v4()).unwrap();

        assert_eq!(release.status, HelmReleaseStatus::Failed);
    }

    #[test]
    fn test_decode_invalid_base64() {
        let result = decode_helm_release("not-valid-base64!!!", "ns", Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn test_helm_secret_label_selector() {
        assert_eq!(helm_secret_label_selector(), "owner=helm");
    }

    // --- T086: Edge-case tests for Helm release parsing ---

    fn make_helm_secret_data_full(json: serde_json::Value) -> String {
        let json_bytes = serde_json::to_vec(&json).unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&json_bytes).unwrap();
        let compressed = encoder.finish().unwrap();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &compressed)
    }

    #[test]
    fn test_decode_multi_revision_releases() {
        // Simulate decoding two secrets that represent different revisions of the same release
        let rev1_json = serde_json::json!({
            "name": "my-app",
            "version": 1,
            "info": { "status": "superseded", "last_deployed": "2026-01-01T10:00:00Z" },
            "chart": { "metadata": { "name": "my-chart", "version": "1.0.0" } },
            "config": {}
        });
        let rev5_json = serde_json::json!({
            "name": "my-app",
            "version": 5,
            "info": { "status": "deployed", "last_deployed": "2026-02-20T15:30:00Z" },
            "chart": { "metadata": { "name": "my-chart", "version": "2.1.0" } },
            "config": { "replicas": 3 }
        });

        let cluster = Uuid::new_v4();
        let r1 =
            decode_helm_release(&make_helm_secret_data_full(rev1_json), "prod", cluster).unwrap();
        let r5 =
            decode_helm_release(&make_helm_secret_data_full(rev5_json), "prod", cluster).unwrap();

        assert_eq!(r1.name, "my-app");
        assert_eq!(r1.revision, 1);
        assert_eq!(r1.status, HelmReleaseStatus::Superseded);
        assert_eq!(r1.chart_version, "1.0.0");

        assert_eq!(r5.name, "my-app");
        assert_eq!(r5.revision, 5);
        assert_eq!(r5.status, HelmReleaseStatus::Deployed);
        assert_eq!(r5.chart_version, "2.1.0");

        // Both share the same cluster and namespace
        assert_eq!(r1.cluster_id, r5.cluster_id);
        assert_eq!(r1.namespace, r5.namespace);
    }

    #[test]
    fn test_decode_missing_optional_fields() {
        // Release JSON with no appVersion, no config, no last_deployed, and no version field
        let json = serde_json::json!({
            "name": "bare-release",
            "info": { "status": "deployed" },
            "chart": {
                "metadata": {
                    "name": "minimal-chart",
                    "version": "0.1.0"
                }
            }
        });

        let release =
            decode_helm_release(&make_helm_secret_data_full(json), "kube-system", Uuid::new_v4())
                .unwrap();

        assert_eq!(release.name, "bare-release");
        assert_eq!(release.chart_name, "minimal-chart");
        assert_eq!(release.chart_version, "0.1.0");
        assert!(release.app_version.is_none()); // appVersion absent
        assert_eq!(release.revision, 1); // default when version missing
        assert_eq!(release.values, serde_json::Value::Null); // config missing -> Null
    }

    #[test]
    fn test_decode_missing_name_and_chart_metadata() {
        // Completely bare JSON - all fields should fall back to defaults
        let json = serde_json::json!({
            "info": {},
            "chart": {}
        });

        let release =
            decode_helm_release(&make_helm_secret_data_full(json), "default", Uuid::new_v4())
                .unwrap();

        assert_eq!(release.name, "unknown");
        assert_eq!(release.chart_name, "unknown");
        assert_eq!(release.chart_version, "0.0.0");
        assert!(release.app_version.is_none());
        assert_eq!(release.status, HelmReleaseStatus::Unknown);
    }

    #[test]
    fn test_decode_all_status_values() {
        let statuses = vec![
            ("deployed", HelmReleaseStatus::Deployed),
            ("failed", HelmReleaseStatus::Failed),
            ("uninstalling", HelmReleaseStatus::Uninstalling),
            ("pending-install", HelmReleaseStatus::PendingInstall),
            ("pending-upgrade", HelmReleaseStatus::PendingUpgrade),
            ("pending-rollback", HelmReleaseStatus::PendingRollback),
            ("superseded", HelmReleaseStatus::Superseded),
            ("something-else", HelmReleaseStatus::Unknown),
        ];

        for (status_str, expected) in statuses {
            let data = make_helm_secret_data("status-test", status_str);
            let release = decode_helm_release(&data, "ns", Uuid::new_v4()).unwrap();
            assert_eq!(
                release.status, expected,
                "Status '{}' should parse to {:?}",
                status_str, expected
            );
        }
    }

    #[test]
    fn test_decode_invalid_gzip_data() {
        // Valid base64 but not valid gzip content
        let data =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"this is not gzip");
        let result = decode_helm_release(&data, "ns", Uuid::new_v4());
        assert!(result.is_err());
    }
}
