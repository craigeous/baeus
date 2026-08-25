use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HelmOperation {
    Install {
        release_name: String,
        chart: String,
        namespace: String,
        values_file: Option<String>,
        version: Option<String>,
        create_namespace: bool,
    },
    Upgrade {
        release_name: String,
        chart: String,
        namespace: String,
        values_file: Option<String>,
        version: Option<String>,
        reuse_values: bool,
    },
    Rollback {
        release_name: String,
        namespace: String,
        revision: u32,
    },
    Uninstall {
        release_name: String,
        namespace: String,
    },
}

impl HelmOperation {
    pub fn to_args(&self) -> Vec<String> {
        match self {
            HelmOperation::Install {
                release_name,
                chart,
                namespace,
                values_file,
                version,
                create_namespace,
            } => {
                let mut args = vec![
                    "install".to_string(),
                    release_name.clone(),
                    chart.clone(),
                    "--namespace".to_string(),
                    namespace.clone(),
                ];
                if let Some(vf) = values_file {
                    args.push("--values".to_string());
                    args.push(vf.clone());
                }
                if let Some(v) = version {
                    args.push("--version".to_string());
                    args.push(v.clone());
                }
                if *create_namespace {
                    args.push("--create-namespace".to_string());
                }
                args
            }
            HelmOperation::Upgrade {
                release_name,
                chart,
                namespace,
                values_file,
                version,
                reuse_values,
            } => {
                let mut args = vec![
                    "upgrade".to_string(),
                    release_name.clone(),
                    chart.clone(),
                    "--namespace".to_string(),
                    namespace.clone(),
                ];
                if let Some(vf) = values_file {
                    args.push("--values".to_string());
                    args.push(vf.clone());
                }
                if let Some(v) = version {
                    args.push("--version".to_string());
                    args.push(v.clone());
                }
                if *reuse_values {
                    args.push("--reuse-values".to_string());
                }
                args
            }
            HelmOperation::Rollback { release_name, namespace, revision } => {
                vec![
                    "rollback".to_string(),
                    release_name.clone(),
                    revision.to_string(),
                    "--namespace".to_string(),
                    namespace.clone(),
                ]
            }
            HelmOperation::Uninstall { release_name, namespace } => {
                vec![
                    "uninstall".to_string(),
                    release_name.clone(),
                    "--namespace".to_string(),
                    namespace.clone(),
                ]
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            HelmOperation::Install { .. } => "Install",
            HelmOperation::Upgrade { .. } => "Upgrade",
            HelmOperation::Rollback { .. } => "Rollback",
            HelmOperation::Uninstall { .. } => "Uninstall",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelmCommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_args() {
        let op = HelmOperation::Install {
            release_name: "my-release".to_string(),
            chart: "bitnami/nginx".to_string(),
            namespace: "default".to_string(),
            values_file: Some("/tmp/values.yaml".to_string()),
            version: Some("15.4.0".to_string()),
            create_namespace: true,
        };

        let args = op.to_args();
        assert_eq!(args[0], "install");
        assert_eq!(args[1], "my-release");
        assert_eq!(args[2], "bitnami/nginx");
        assert!(args.contains(&"--namespace".to_string()));
        assert!(args.contains(&"--values".to_string()));
        assert!(args.contains(&"--version".to_string()));
        assert!(args.contains(&"--create-namespace".to_string()));
    }

    #[test]
    fn test_upgrade_args() {
        let op = HelmOperation::Upgrade {
            release_name: "my-release".to_string(),
            chart: "bitnami/nginx".to_string(),
            namespace: "default".to_string(),
            values_file: None,
            version: None,
            reuse_values: true,
        };

        let args = op.to_args();
        assert_eq!(args[0], "upgrade");
        assert!(args.contains(&"--reuse-values".to_string()));
        assert!(!args.contains(&"--values".to_string()));
    }

    #[test]
    fn test_rollback_args() {
        let op = HelmOperation::Rollback {
            release_name: "my-release".to_string(),
            namespace: "default".to_string(),
            revision: 3,
        };

        let args = op.to_args();
        assert_eq!(args[0], "rollback");
        assert_eq!(args[1], "my-release");
        assert_eq!(args[2], "3");
    }

    #[test]
    fn test_uninstall_args() {
        let op = HelmOperation::Uninstall {
            release_name: "my-release".to_string(),
            namespace: "default".to_string(),
        };

        let args = op.to_args();
        assert_eq!(args[0], "uninstall");
        assert_eq!(args[1], "my-release");
    }

    #[test]
    fn test_operation_labels() {
        assert_eq!(
            HelmOperation::Install {
                release_name: String::new(),
                chart: String::new(),
                namespace: String::new(),
                values_file: None,
                version: None,
                create_namespace: false,
            }
            .label(),
            "Install"
        );
    }

    // --- T087: Edge-case tests for Helm CLI wrapper ---

    #[test]
    fn test_install_without_optional_args() {
        let op = HelmOperation::Install {
            release_name: "simple-app".to_string(),
            chart: "stable/app".to_string(),
            namespace: "default".to_string(),
            values_file: None,
            version: None,
            create_namespace: false,
        };

        let args = op.to_args();
        assert_eq!(args, vec!["install", "simple-app", "stable/app", "--namespace", "default",]);
        // Ensure no optional flags leak through
        assert!(!args.contains(&"--values".to_string()));
        assert!(!args.contains(&"--version".to_string()));
        assert!(!args.contains(&"--create-namespace".to_string()));
    }

    #[test]
    fn test_upgrade_without_values_and_no_reuse() {
        let op = HelmOperation::Upgrade {
            release_name: "my-svc".to_string(),
            chart: "oci://registry/chart".to_string(),
            namespace: "staging".to_string(),
            values_file: None,
            version: None,
            reuse_values: false,
        };

        let args = op.to_args();
        assert_eq!(
            args,
            vec!["upgrade", "my-svc", "oci://registry/chart", "--namespace", "staging",]
        );
        assert!(!args.contains(&"--values".to_string()));
        assert!(!args.contains(&"--version".to_string()));
        assert!(!args.contains(&"--reuse-values".to_string()));
    }

    #[test]
    fn test_upgrade_with_version_only() {
        let op = HelmOperation::Upgrade {
            release_name: "pinned".to_string(),
            chart: "bitnami/redis".to_string(),
            namespace: "cache".to_string(),
            values_file: None,
            version: Some("18.5.0".to_string()),
            reuse_values: false,
        };

        let args = op.to_args();
        assert!(args.contains(&"--version".to_string()));
        assert!(args.contains(&"18.5.0".to_string()));
        assert!(!args.contains(&"--values".to_string()));
        assert!(!args.contains(&"--reuse-values".to_string()));
    }

    #[test]
    fn test_install_with_version_no_values_no_create_ns() {
        let op = HelmOperation::Install {
            release_name: "pinned-install".to_string(),
            chart: "bitnami/postgresql".to_string(),
            namespace: "db".to_string(),
            values_file: None,
            version: Some("13.2.0".to_string()),
            create_namespace: false,
        };

        let args = op.to_args();
        assert!(args.contains(&"--version".to_string()));
        assert!(args.contains(&"13.2.0".to_string()));
        assert!(!args.contains(&"--values".to_string()));
        assert!(!args.contains(&"--create-namespace".to_string()));
    }

    #[test]
    fn test_all_operation_labels() {
        assert_eq!(
            HelmOperation::Upgrade {
                release_name: String::new(),
                chart: String::new(),
                namespace: String::new(),
                values_file: None,
                version: None,
                reuse_values: false,
            }
            .label(),
            "Upgrade"
        );
        assert_eq!(
            HelmOperation::Rollback {
                release_name: String::new(),
                namespace: String::new(),
                revision: 0,
            }
            .label(),
            "Rollback"
        );
        assert_eq!(
            HelmOperation::Uninstall { release_name: String::new(), namespace: String::new() }
                .label(),
            "Uninstall"
        );
    }

    #[test]
    fn test_rollback_revision_zero() {
        // Revision 0 means "rollback to previous" in Helm
        let op = HelmOperation::Rollback {
            release_name: "oops".to_string(),
            namespace: "production".to_string(),
            revision: 0,
        };

        let args = op.to_args();
        assert_eq!(args[0], "rollback");
        assert_eq!(args[1], "oops");
        assert_eq!(args[2], "0");
        assert_eq!(args[3], "--namespace");
        assert_eq!(args[4], "production");
    }
}
