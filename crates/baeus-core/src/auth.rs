use crate::cluster::AuthMethod;
use anyhow::{Context, Result};
use std::fmt;
use zeroize::Zeroize;

pub struct AuthConfig {
    pub method: AuthMethod,
    pub details: AuthDetails,
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("method", &self.method)
            .field("details", &self.details)
            .finish()
    }
}

pub enum AuthDetails {
    Certificate {
        client_cert_data: Option<String>,
        client_cert_path: Option<String>,
        client_key_data: Option<String>,
        client_key_path: Option<String>,
    },
    Token {
        token: Option<String>,
        token_file: Option<String>,
    },
    Oidc {
        issuer_url: String,
        client_id: Option<String>,
    },
    ExecPlugin {
        command: String,
        args: Vec<String>,
        api_version: Option<String>,
    },
}

/// Clear sensitive credential data from memory when AuthDetails is dropped.
impl Drop for AuthDetails {
    fn drop(&mut self) {
        match self {
            AuthDetails::Certificate { client_cert_data, client_key_data, .. } => {
                if let Some(d) = client_cert_data {
                    d.zeroize();
                }
                if let Some(d) = client_key_data {
                    d.zeroize();
                }
            }
            AuthDetails::Token { token: Some(t), .. } => {
                t.zeroize();
            }
            _ => {}
        }
    }
}

/// Custom Debug impl to prevent credential leakage in logs/debug output.
/// Sensitive fields (cert data, key data, tokens) are always redacted.
impl fmt::Debug for AuthDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthDetails::Certificate { client_cert_path, client_key_path, .. } => f
                .debug_struct("Certificate")
                .field("client_cert_data", &"<redacted>")
                .field("client_cert_path", client_cert_path)
                .field("client_key_data", &"<redacted>")
                .field("client_key_path", client_key_path)
                .finish(),
            AuthDetails::Token { token_file, .. } => f
                .debug_struct("Token")
                .field("token", &"<redacted>")
                .field("token_file", token_file)
                .finish(),
            AuthDetails::Oidc { issuer_url, client_id } => f
                .debug_struct("Oidc")
                .field("issuer_url", issuer_url)
                .field("client_id", client_id)
                .finish(),
            AuthDetails::ExecPlugin { command, args, api_version } => f
                .debug_struct("ExecPlugin")
                .field("command", command)
                .field("args", args)
                .field("api_version", api_version)
                .finish(),
        }
    }
}

impl AuthConfig {
    pub fn from_certificate(
        client_cert_data: Option<String>,
        client_cert_path: Option<String>,
        client_key_data: Option<String>,
        client_key_path: Option<String>,
    ) -> Self {
        Self {
            method: AuthMethod::Certificate,
            details: AuthDetails::Certificate {
                client_cert_data,
                client_cert_path,
                client_key_data,
                client_key_path,
            },
        }
    }

    pub fn from_token(token: Option<String>, token_file: Option<String>) -> Self {
        Self { method: AuthMethod::Token, details: AuthDetails::Token { token, token_file } }
    }

    pub fn from_oidc(issuer_url: String, client_id: Option<String>) -> Self {
        Self { method: AuthMethod::OIDC, details: AuthDetails::Oidc { issuer_url, client_id } }
    }

    pub fn from_exec(command: String, args: Vec<String>, api_version: Option<String>) -> Self {
        Self {
            method: AuthMethod::ExecPlugin,
            details: AuthDetails::ExecPlugin { command, args, api_version },
        }
    }
}

pub fn detect_auth_from_user_config(user_config: &serde_json::Value) -> Result<AuthConfig> {
    if let Some(exec) = user_config.get("exec") {
        let command = exec
            .get("command")
            .and_then(|v| v.as_str())
            .context("exec config missing command")?
            .to_string();
        let args = exec
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
        let api_version = exec.get("apiVersion").and_then(|v| v.as_str()).map(|s| s.to_string());

        return Ok(AuthConfig::from_exec(command, args, api_version));
    }

    if let Some(auth_provider) = user_config.get("auth-provider") {
        let issuer_url = auth_provider
            .get("config")
            .and_then(|c| c.get("idp-issuer-url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let client_id = auth_provider
            .get("config")
            .and_then(|c| c.get("client-id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        return Ok(AuthConfig::from_oidc(issuer_url, client_id));
    }

    if user_config.get("client-certificate-data").is_some()
        || user_config.get("client-certificate").is_some()
    {
        let cert_data = user_config
            .get("client-certificate-data")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cert_path =
            user_config.get("client-certificate").and_then(|v| v.as_str()).map(|s| s.to_string());
        let key_data =
            user_config.get("client-key-data").and_then(|v| v.as_str()).map(|s| s.to_string());
        let key_path =
            user_config.get("client-key").and_then(|v| v.as_str()).map(|s| s.to_string());

        return Ok(AuthConfig::from_certificate(cert_data, cert_path, key_data, key_path));
    }

    let token = user_config.get("token").and_then(|v| v.as_str()).map(|s| s.to_string());
    let token_file = user_config.get("tokenFile").and_then(|v| v.as_str()).map(|s| s.to_string());

    Ok(AuthConfig::from_token(token, token_file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_certificate_auth() {
        let user_config = json!({
            "client-certificate-data": "base64cert",
            "client-key-data": "base64key"
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::Certificate);

        if let AuthDetails::Certificate { client_cert_data, client_key_data, .. } = &auth.details {
            assert_eq!(client_cert_data.as_deref(), Some("base64cert"));
            assert_eq!(client_key_data.as_deref(), Some("base64key"));
        } else {
            panic!("Expected Certificate auth details");
        }
    }

    #[test]
    fn test_detect_certificate_from_file_path() {
        let user_config = json!({
            "client-certificate": "/path/to/cert.pem",
            "client-key": "/path/to/key.pem"
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::Certificate);

        if let AuthDetails::Certificate { client_cert_path, client_key_path, .. } = &auth.details {
            assert_eq!(client_cert_path.as_deref(), Some("/path/to/cert.pem"));
            assert_eq!(client_key_path.as_deref(), Some("/path/to/key.pem"));
        } else {
            panic!("Expected Certificate auth details");
        }
    }

    #[test]
    fn test_detect_token_auth() {
        let user_config = json!({
            "token": "my-bearer-token"
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::Token);

        if let AuthDetails::Token { token, .. } = &auth.details {
            assert_eq!(token.as_deref(), Some("my-bearer-token"));
        } else {
            panic!("Expected Token auth details");
        }
    }

    #[test]
    fn test_detect_token_file_auth() {
        let user_config = json!({
            "tokenFile": "/var/run/secrets/kubernetes.io/serviceaccount/token"
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::Token);

        if let AuthDetails::Token { token_file, .. } = &auth.details {
            assert!(token_file.is_some());
        } else {
            panic!("Expected Token auth details");
        }
    }

    #[test]
    fn test_detect_oidc_auth() {
        let user_config = json!({
            "auth-provider": {
                "name": "oidc",
                "config": {
                    "idp-issuer-url": "https://accounts.google.com",
                    "client-id": "my-client-id"
                }
            }
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::OIDC);

        if let AuthDetails::Oidc { issuer_url, client_id } = &auth.details {
            assert_eq!(issuer_url, "https://accounts.google.com");
            assert_eq!(client_id.as_deref(), Some("my-client-id"));
        } else {
            panic!("Expected OIDC auth details");
        }
    }

    #[test]
    fn test_detect_exec_plugin_auth() {
        let user_config = json!({
            "exec": {
                "apiVersion": "client.authentication.k8s.io/v1beta1",
                "command": "aws-iam-authenticator",
                "args": ["token", "-i", "my-cluster"]
            }
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::ExecPlugin);

        if let AuthDetails::ExecPlugin { command, args, api_version } = &auth.details {
            assert_eq!(command, "aws-iam-authenticator");
            assert_eq!(args, &["token", "-i", "my-cluster"]);
            assert_eq!(api_version.as_deref(), Some("client.authentication.k8s.io/v1beta1"));
        } else {
            panic!("Expected ExecPlugin auth details");
        }
    }

    #[test]
    fn test_exec_takes_priority_over_certificate() {
        let user_config = json!({
            "client-certificate-data": "cert",
            "exec": {
                "command": "gke-gcloud-auth-plugin",
                "args": []
            }
        });

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::ExecPlugin);
    }

    #[test]
    fn test_fallback_to_token_on_empty_config() {
        let user_config = json!({});

        let auth = detect_auth_from_user_config(&user_config).unwrap();
        assert_eq!(auth.method, AuthMethod::Token);
    }

    #[test]
    fn test_auth_config_constructors() {
        let cert = AuthConfig::from_certificate(
            Some("cert".to_string()),
            None,
            Some("key".to_string()),
            None,
        );
        assert_eq!(cert.method, AuthMethod::Certificate);

        let token = AuthConfig::from_token(Some("tok".to_string()), None);
        assert_eq!(token.method, AuthMethod::Token);

        let oidc = AuthConfig::from_oidc("https://issuer.example.com".to_string(), None);
        assert_eq!(oidc.method, AuthMethod::OIDC);

        let exec = AuthConfig::from_exec("cmd".to_string(), vec![], None);
        assert_eq!(exec.method, AuthMethod::ExecPlugin);
    }

    // --- T137: Security hardening - credential redaction tests ---

    #[test]
    fn test_auth_details_debug_redacts_certificate_data() {
        let details = AuthDetails::Certificate {
            client_cert_data: Some("SENSITIVE_CERT_DATA".to_string()),
            client_cert_path: Some("/path/to/cert.pem".to_string()),
            client_key_data: Some("SENSITIVE_KEY_DATA".to_string()),
            client_key_path: Some("/path/to/key.pem".to_string()),
        };
        let debug_output = format!("{:?}", details);
        assert!(!debug_output.contains("SENSITIVE_CERT_DATA"));
        assert!(!debug_output.contains("SENSITIVE_KEY_DATA"));
        assert!(debug_output.contains("<redacted>"));
        assert!(debug_output.contains("/path/to/cert.pem"));
        assert!(debug_output.contains("/path/to/key.pem"));
    }

    #[test]
    fn test_auth_details_debug_redacts_token() {
        let details = AuthDetails::Token {
            token: Some("super-secret-bearer-token".to_string()),
            token_file: Some("/var/run/secrets/token".to_string()),
        };
        let debug_output = format!("{:?}", details);
        assert!(!debug_output.contains("super-secret-bearer-token"));
        assert!(debug_output.contains("<redacted>"));
        assert!(debug_output.contains("/var/run/secrets/token"));
    }

    #[test]
    fn test_auth_details_debug_oidc_not_redacted() {
        let details = AuthDetails::Oidc {
            issuer_url: "https://accounts.google.com".to_string(),
            client_id: Some("my-client-id".to_string()),
        };
        let debug_output = format!("{:?}", details);
        assert!(debug_output.contains("accounts.google.com"));
        assert!(debug_output.contains("my-client-id"));
    }

    #[test]
    fn test_auth_details_debug_exec_not_redacted() {
        let details = AuthDetails::ExecPlugin {
            command: "aws-iam-authenticator".to_string(),
            args: vec!["token".to_string(), "-i".to_string()],
            api_version: Some("v1beta1".to_string()),
        };
        let debug_output = format!("{:?}", details);
        assert!(debug_output.contains("aws-iam-authenticator"));
        assert!(debug_output.contains("token"));
    }
}
