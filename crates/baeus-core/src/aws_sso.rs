//! AWS SSO authentication helpers for EKS clusters.
//!
//! Provides functions to inject `AWS_PROFILE` into a kubeconfig's exec env,
//! detect expired SSO session errors, and query the current caller identity.
//! and to detect expired SSO session errors.

use std::collections::HashMap;

use anyhow::{Context, Result};

/// Parsed result from `aws sts get-caller-identity`.
#[derive(Debug, Clone)]
pub struct CallerIdentity {
    pub account: String,
    pub arn: String,
    pub user_id: String,
}

/// Run `aws sts get-caller-identity --output json` and parse the result.
///
/// Returns the Account, Arn, and UserId fields. Requires the AWS CLI to be
/// installed and configured. Returns an error if the command fails or the
/// output cannot be parsed.
pub async fn get_caller_identity() -> Result<CallerIdentity> {
    let output = tokio::process::Command::new("aws")
        .args(["sts", "get-caller-identity", "--output", "json"])
        .output()
        .await
        .context("Failed to run `aws sts get-caller-identity` — is the AWS CLI installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("aws sts get-caller-identity failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).context("Failed to parse caller identity JSON")?;

    Ok(CallerIdentity {
        account: json["Account"].as_str().unwrap_or("").to_string(),
        arn: json["Arn"].as_str().unwrap_or("").to_string(),
        user_id: json["UserId"].as_str().unwrap_or("").to_string(),
    })
}

/// Inject `AWS_PROFILE` into a kubeconfig's exec env for a specific context.
///
/// Finds the user entry referenced by the named context and adds (or overwrites)
/// an `AWS_PROFILE` environment variable in its `exec` block. This causes
/// `aws eks get-token` to use the specified profile without requiring the
/// environment variable to be set globally.
pub fn inject_aws_profile_into_kubeconfig(
    kubeconfig: &mut kube::config::Kubeconfig,
    context_name: &str,
    aws_profile: &str,
) -> Result<()> {
    // Find the context entry to get its user name.
    let ctx = kubeconfig
        .contexts
        .iter()
        .find(|c| c.name == context_name)
        .with_context(|| format!("Context '{context_name}' not found in kubeconfig"))?;

    let user_name = ctx.context.as_ref().and_then(|c| c.user.clone()).unwrap_or_default();

    // Find the matching auth info entry and inject the env var.
    let auth_info =
        kubeconfig.auth_infos.iter_mut().find(|a| a.name == user_name).with_context(|| {
            format!("AuthInfo '{user_name}' (referenced by context '{context_name}') not found")
        })?;

    if let Some(ref mut ai) = auth_info.auth_info {
        if let Some(ref mut exec_cfg) = ai.exec {
            // Build the env entry as a HashMap with "name" and "value" keys
            // (this is how kube-rs models exec env vars).
            let mut env_var = HashMap::new();
            env_var.insert("name".to_string(), "AWS_PROFILE".to_string());
            env_var.insert("value".to_string(), aws_profile.to_string());

            match exec_cfg.env {
                Some(ref mut envs) => {
                    // Replace existing AWS_PROFILE or append.
                    if let Some(existing) = envs
                        .iter_mut()
                        .find(|e| e.get("name").map(|n| n.as_str()) == Some("AWS_PROFILE"))
                    {
                        existing.insert("value".to_string(), aws_profile.to_string());
                    } else {
                        envs.push(env_var);
                    }
                }
                None => {
                    exec_cfg.env = Some(vec![env_var]);
                }
            }
        }
    }

    Ok(())
}

/// Inject AWS credentials into a kubeconfig's exec env section (in-memory only).
/// This allows the exec plugin (`aws eks get-token`) to use the wizard's
/// in-memory credentials without writing secrets to disk.
pub fn inject_aws_credentials_into_kubeconfig(
    kubeconfig: &mut kube::config::Kubeconfig,
    context_name: &str,
    access_key_id: &str,
    secret_access_key: &str,
    session_token: Option<&str>,
) -> Result<()> {
    let ctx = kubeconfig
        .contexts
        .iter()
        .find(|c| c.name == context_name)
        .with_context(|| format!("Context '{context_name}' not found in kubeconfig"))?;

    let user_name = ctx.context.as_ref().and_then(|c| c.user.clone()).unwrap_or_default();

    let auth_info = kubeconfig
        .auth_infos
        .iter_mut()
        .find(|a| a.name == user_name)
        .with_context(|| format!("AuthInfo '{user_name}' not found"))?;

    if let Some(ref mut ai) = auth_info.auth_info {
        if let Some(ref mut exec_cfg) = ai.exec {
            let env_vars = vec![
                ("AWS_ACCESS_KEY_ID", access_key_id),
                ("AWS_SECRET_ACCESS_KEY", secret_access_key),
            ];

            for (name, value) in env_vars {
                let mut env_var = HashMap::new();
                env_var.insert("name".to_string(), name.to_string());
                env_var.insert("value".to_string(), value.to_string());
                match exec_cfg.env {
                    Some(ref mut envs) => {
                        if let Some(existing) = envs
                            .iter_mut()
                            .find(|e| e.get("name").map(|n| n.as_str()) == Some(name))
                        {
                            existing.insert("value".to_string(), value.to_string());
                        } else {
                            envs.push(env_var);
                        }
                    }
                    None => {
                        exec_cfg.env = Some(vec![env_var]);
                    }
                }
            }

            if let Some(token) = session_token {
                let mut env_var = HashMap::new();
                env_var.insert("name".to_string(), "AWS_SESSION_TOKEN".to_string());
                env_var.insert("value".to_string(), token.to_string());
                if let Some(ref mut envs) = exec_cfg.env {
                    if let Some(existing) = envs
                        .iter_mut()
                        .find(|e| e.get("name").map(|n| n.as_str()) == Some("AWS_SESSION_TOKEN"))
                    {
                        existing.insert("value".to_string(), token.to_string());
                    } else {
                        envs.push(env_var);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check if an error message looks like an AWS SSO token expiry.
///
/// Matches common error strings from `aws eks get-token` and the AWS CLI
/// when the SSO session has expired or the cached token is invalid.
pub fn is_aws_sso_auth_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("sso token has expired")
        || lower.contains("the sso session associated with this profile has expired")
        || lower.contains("sso session expired")
        || lower.contains("token has expired and refresh failed")
        || lower.contains("error loading sso token")
        || lower.contains("to refresh this sso session run aws sso login")
        || (lower.contains("expiredtokenexception") && lower.contains("sso"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_aws_sso_auth_error_positive() {
        assert!(is_aws_sso_auth_error("Error: The SSO token has expired"));
        assert!(is_aws_sso_auth_error("The SSO session associated with this profile has expired"));
        assert!(is_aws_sso_auth_error("Error loading SSO token: token expired"));
        assert!(is_aws_sso_auth_error(
            "To refresh this SSO session run aws sso login with the corresponding profile"
        ));
        assert!(is_aws_sso_auth_error("SSO session expired for profile dev"));
        assert!(is_aws_sso_auth_error("Token has expired and refresh failed"));
    }

    #[test]
    fn test_is_aws_sso_auth_error_negative() {
        assert!(!is_aws_sso_auth_error("connection refused"));
        assert!(!is_aws_sso_auth_error("unable to connect to server"));
        assert!(!is_aws_sso_auth_error("certificate is not valid"));
        assert!(!is_aws_sso_auth_error("401 Unauthorized"));
        assert!(!is_aws_sso_auth_error(""));
    }

    #[test]
    fn test_is_aws_sso_auth_error_case_insensitive() {
        assert!(is_aws_sso_auth_error("SSO TOKEN HAS EXPIRED"));
        assert!(is_aws_sso_auth_error("sso token has expired"));
        assert!(is_aws_sso_auth_error("Sso Token Has Expired"));
    }

    #[test]
    fn test_inject_aws_profile_into_kubeconfig() {
        use kube::config::{
            AuthInfo, Context as KubeContext, ExecConfig, Kubeconfig, NamedAuthInfo, NamedContext,
        };

        let mut kubeconfig = Kubeconfig {
            contexts: vec![NamedContext {
                name: "my-cluster".to_string(),
                context: Some(KubeContext {
                    cluster: "my-cluster".to_string(),
                    user: Some("my-user".to_string()),
                    ..Default::default()
                }),
            }],
            auth_infos: vec![NamedAuthInfo {
                name: "my-user".to_string(),
                auth_info: Some(AuthInfo {
                    exec: Some(ExecConfig {
                        api_version: Some("client.authentication.k8s.io/v1beta1".to_string()),
                        command: Some("aws".to_string()),
                        args: Some(vec![
                            "eks".to_string(),
                            "get-token".to_string(),
                            "--cluster-name".to_string(),
                            "my-cluster".to_string(),
                        ]),
                        env: None,
                        drop_env: None,
                        interactive_mode: None,
                        provide_cluster_info: false,
                        cluster: None,
                    }),
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };

        let result = inject_aws_profile_into_kubeconfig(&mut kubeconfig, "my-cluster", "secops");
        assert!(result.is_ok());

        let exec = kubeconfig.auth_infos[0].auth_info.as_ref().unwrap().exec.as_ref().unwrap();
        let envs = exec.env.as_ref().unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].get("name").unwrap(), "AWS_PROFILE");
        assert_eq!(envs[0].get("value").unwrap(), "secops");
    }

    #[test]
    fn test_inject_aws_profile_overwrites_existing() {
        use kube::config::{
            AuthInfo, Context as KubeContext, ExecConfig, Kubeconfig, NamedAuthInfo, NamedContext,
        };

        let mut existing_env = HashMap::new();
        existing_env.insert("name".to_string(), "AWS_PROFILE".to_string());
        existing_env.insert("value".to_string(), "old-profile".to_string());

        let mut kubeconfig = Kubeconfig {
            contexts: vec![NamedContext {
                name: "ctx".to_string(),
                context: Some(KubeContext {
                    cluster: "c".to_string(),
                    user: Some("u".to_string()),
                    ..Default::default()
                }),
            }],
            auth_infos: vec![NamedAuthInfo {
                name: "u".to_string(),
                auth_info: Some(AuthInfo {
                    exec: Some(ExecConfig {
                        api_version: None,
                        command: Some("aws".to_string()),
                        args: None,
                        env: Some(vec![existing_env]),
                        drop_env: None,
                        interactive_mode: None,
                        provide_cluster_info: false,
                        cluster: None,
                    }),
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };

        inject_aws_profile_into_kubeconfig(&mut kubeconfig, "ctx", "new-profile").unwrap();

        let envs = kubeconfig.auth_infos[0]
            .auth_info
            .as_ref()
            .unwrap()
            .exec
            .as_ref()
            .unwrap()
            .env
            .as_ref()
            .unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].get("value").unwrap(), "new-profile");
    }

    #[test]
    fn test_inject_missing_context_returns_error() {
        let mut kubeconfig = kube::config::Kubeconfig::default();
        let result = inject_aws_profile_into_kubeconfig(&mut kubeconfig, "nonexistent", "profile");
        assert!(result.is_err());
    }
}
