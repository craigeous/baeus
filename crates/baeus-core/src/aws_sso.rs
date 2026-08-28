//! AWS SSO authentication helpers for EKS clusters.
//!
//! Provides functions to inject `AWS_PROFILE` into a kubeconfig's exec env,
//! detect expired SSO session errors, and query the current caller identity.
//! and to detect expired SSO session errors.

use std::collections::HashMap;

use anyhow::{Context, Result};

/// Typed errors returned by the kubeconfig injection functions.
///
/// Both `inject_aws_profile_into_kubeconfig` and
/// `inject_aws_credentials_into_kubeconfig` return `Result<(),
/// KubeconfigInjectionError>` so callers can distinguish a missing exec block
/// (misconfigured kubeconfig) from a missing context or auth-info entry.
#[derive(Debug, thiserror::Error)]
pub enum KubeconfigInjectionError {
    #[error("Kubeconfig context '{context}' not found")]
    ContextNotFound { context: String },
    #[error("AuthInfo '{user}' (referenced by context '{context}') not found")]
    AuthInfoNotFound { user: String, context: String },
    #[error(
        "Kubeconfig context '{context}' has no `exec` block; AWS credential \
         injection requires an exec plugin. Add an `exec` block referencing \
         `aws eks get-token` or select a different context."
    )]
    ExecBlockMissing { context: String },
}

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
) -> Result<(), KubeconfigInjectionError> {
    // Find the context entry to get its user name.
    let ctx = kubeconfig
        .contexts
        .iter()
        .find(|c| c.name == context_name)
        .ok_or_else(|| KubeconfigInjectionError::ContextNotFound {
            context: context_name.to_string(),
        })?;

    let user_name = ctx.context.as_ref().and_then(|c| c.user.clone()).unwrap_or_default();

    // Find the matching auth info entry and inject the env var.
    let auth_info =
        kubeconfig.auth_infos.iter_mut().find(|a| a.name == user_name).ok_or_else(|| {
            KubeconfigInjectionError::AuthInfoNotFound {
                user: user_name.clone(),
                context: context_name.to_string(),
            }
        })?;

    let ai = auth_info.auth_info.as_mut().ok_or_else(|| KubeconfigInjectionError::ExecBlockMissing {
        context: context_name.to_string(),
    })?;
    let exec_cfg = ai.exec.as_mut().ok_or_else(|| KubeconfigInjectionError::ExecBlockMissing {
        context: context_name.to_string(),
    })?;

    // Build the env entry as a HashMap with "name" and "value" keys
    // (this is how kube-rs models exec env vars).
    let mut env_var = HashMap::new();
    env_var.insert("name".to_string(), "AWS_PROFILE".to_string());
    env_var.insert("value".to_string(), aws_profile.to_string());

    match exec_cfg.env {
        Some(ref mut envs) => {
            // Replace existing AWS_PROFILE or append.
            if let Some(existing) =
                envs.iter_mut().find(|e| e.get("name").map(|n| n.as_str()) == Some("AWS_PROFILE"))
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
) -> Result<(), KubeconfigInjectionError> {
    let ctx = kubeconfig
        .contexts
        .iter()
        .find(|c| c.name == context_name)
        .ok_or_else(|| KubeconfigInjectionError::ContextNotFound {
            context: context_name.to_string(),
        })?;

    let user_name = ctx.context.as_ref().and_then(|c| c.user.clone()).unwrap_or_default();

    let auth_info =
        kubeconfig.auth_infos.iter_mut().find(|a| a.name == user_name).ok_or_else(|| {
            KubeconfigInjectionError::AuthInfoNotFound {
                user: user_name.clone(),
                context: context_name.to_string(),
            }
        })?;

    let ai = auth_info.auth_info.as_mut().ok_or_else(|| KubeconfigInjectionError::ExecBlockMissing {
        context: context_name.to_string(),
    })?;
    let exec_cfg = ai.exec.as_mut().ok_or_else(|| KubeconfigInjectionError::ExecBlockMissing {
        context: context_name.to_string(),
    })?;

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
                if let Some(existing) =
                    envs.iter_mut().find(|e| e.get("name").map(|n| n.as_str()) == Some(name))
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

    // ---------------------------------------------------------------------------
    // Helper: build a kubeconfig with a context that has a valid exec block.
    // ---------------------------------------------------------------------------

    fn make_kubeconfig_with_exec() -> kube::config::Kubeconfig {
        use kube::config::{
            AuthInfo, Context as KubeContext, ExecConfig, Kubeconfig, NamedAuthInfo, NamedContext,
        };
        Kubeconfig {
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
        }
    }

    // ---------------------------------------------------------------------------
    // Helper: build a kubeconfig with a context but NO exec block.
    // ---------------------------------------------------------------------------

    fn make_kubeconfig_no_exec() -> kube::config::Kubeconfig {
        use kube::config::{AuthInfo, Context as KubeContext, Kubeconfig, NamedAuthInfo, NamedContext};
        Kubeconfig {
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
                auth_info: Some(AuthInfo { exec: None, ..Default::default() }),
            }],
            ..Default::default()
        }
    }

    // ---------------------------------------------------------------------------
    // Slice C step 1: new error-path tests (red → green with refactored fns)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_inject_profile_returns_exec_block_missing_when_absent() {
        let mut kc = make_kubeconfig_no_exec();
        let result = inject_aws_profile_into_kubeconfig(&mut kc, "my-cluster", "secops");
        match result {
            Err(KubeconfigInjectionError::ExecBlockMissing { context }) => {
                assert_eq!(context, "my-cluster");
            }
            other => panic!("expected ExecBlockMissing, got {:?}", other),
        }
    }

    #[test]
    fn test_inject_credentials_returns_exec_block_missing_when_absent() {
        let mut kc = make_kubeconfig_no_exec();
        let result = inject_aws_credentials_into_kubeconfig(
            &mut kc,
            "my-cluster",
            "AKIAEXAMPLE",
            "secret-key",
            None,
        );
        match result {
            Err(KubeconfigInjectionError::ExecBlockMissing { context }) => {
                assert_eq!(context, "my-cluster");
            }
            other => panic!("expected ExecBlockMissing, got {:?}", other),
        }
    }

    #[test]
    fn test_inject_credentials_returns_context_not_found() {
        let mut kc = kube::config::Kubeconfig::default();
        let result = inject_aws_credentials_into_kubeconfig(
            &mut kc,
            "nonexistent",
            "AKIAEXAMPLE",
            "secret-key",
            None,
        );
        match result {
            Err(KubeconfigInjectionError::ContextNotFound { context }) => {
                assert_eq!(context, "nonexistent");
            }
            other => panic!("expected ContextNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_inject_credentials_returns_auth_info_not_found() {
        use kube::config::{Context as KubeContext, Kubeconfig, NamedContext};
        // Context exists but references a user with no matching AuthInfo.
        let mut kc = Kubeconfig {
            contexts: vec![NamedContext {
                name: "ctx".to_string(),
                context: Some(KubeContext {
                    cluster: "c".to_string(),
                    user: Some("missing-user".to_string()),
                    ..Default::default()
                }),
            }],
            auth_infos: vec![],
            ..Default::default()
        };
        let result =
            inject_aws_credentials_into_kubeconfig(&mut kc, "ctx", "AKIAEXAMPLE", "secret", None);
        match result {
            Err(KubeconfigInjectionError::AuthInfoNotFound { user, context }) => {
                assert_eq!(user, "missing-user");
                assert_eq!(context, "ctx");
            }
            other => panic!("expected AuthInfoNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_inject_credentials_sets_aws_env_vars_on_valid_context() {
        // Part 1: with session token.
        let mut kc1 = make_kubeconfig_with_exec();
        let result = inject_aws_credentials_into_kubeconfig(
            &mut kc1,
            "my-cluster",
            "AKIAEXAMPLE",
            "secret-key",
            Some("session-token-1"),
        );
        assert!(result.is_ok(), "injection into valid exec block must succeed");
        let exec =
            kc1.auth_infos[0].auth_info.as_ref().unwrap().exec.as_ref().unwrap();
        let envs = exec.env.as_ref().expect("env must be set after injection");
        let get_env = |name: &str| {
            envs.iter()
                .find(|e| e.get("name").map(|n| n.as_str()) == Some(name))
                .and_then(|e| e.get("value").map(|v| v.as_str()))
        };
        assert_eq!(get_env("AWS_ACCESS_KEY_ID"), Some("AKIAEXAMPLE"));
        assert_eq!(get_env("AWS_SECRET_ACCESS_KEY"), Some("secret-key"));
        assert_eq!(get_env("AWS_SESSION_TOKEN"), Some("session-token-1"));

        // Part 2: fresh kubeconfig, session_token = None → must NOT contain AWS_SESSION_TOKEN.
        let mut kc2 = make_kubeconfig_with_exec();
        inject_aws_credentials_into_kubeconfig(
            &mut kc2,
            "my-cluster",
            "AKIAEXAMPLE",
            "secret-key",
            None,
        )
        .unwrap();
        let exec2 =
            kc2.auth_infos[0].auth_info.as_ref().unwrap().exec.as_ref().unwrap();
        let envs2 = exec2.env.as_ref().expect("env must be set after injection");
        let get_env2 = |name: &str| {
            envs2.iter().any(|e| e.get("name").map(|n| n.as_str()) == Some(name))
        };
        assert!(get_env2("AWS_ACCESS_KEY_ID"), "must contain AWS_ACCESS_KEY_ID");
        assert!(get_env2("AWS_SECRET_ACCESS_KEY"), "must contain AWS_SECRET_ACCESS_KEY");
        assert!(!get_env2("AWS_SESSION_TOKEN"), "must NOT contain AWS_SESSION_TOKEN when None passed");
    }

    // ---------------------------------------------------------------------------
    // Existing tests (retained, one retitled for typed-variant assertion)
    // ---------------------------------------------------------------------------

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
        let mut kubeconfig = make_kubeconfig_with_exec();

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

    /// Retitled from `test_inject_missing_context_returns_error` — now asserts
    /// the typed `ContextNotFound` variant.
    #[test]
    fn test_inject_missing_context_returns_context_not_found() {
        let mut kubeconfig = kube::config::Kubeconfig::default();
        let result = inject_aws_profile_into_kubeconfig(&mut kubeconfig, "nonexistent", "profile");
        match result {
            Err(KubeconfigInjectionError::ContextNotFound { context }) => {
                assert_eq!(context, "nonexistent");
            }
            other => panic!("expected ContextNotFound, got {:?}", other),
        }
    }
}
