//! Native AWS EKS integration — SSO device flow, access keys, role assumption,
//! cluster discovery, and EKS bearer-token generation.
//!
//! Eliminates the need for the AWS CLI by using `aws-sdk-rust` directly.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// How the user authenticates to AWS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwsAuthMethod {
    /// AWS IAM Identity Center (SSO) device-code flow.
    Sso,
    /// Static access key + secret (+ optional session token).
    AccessKey,
    /// Assume an IAM role from a source credential set.
    AssumeRole,
}

/// Configuration for SSO device-code flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoConfig {
    pub start_url: String,
    pub region: String,
}

/// Configuration for static access-key authentication.
///
/// The secret key is zeroized on drop to avoid lingering in memory.
#[derive(Clone)]
pub struct AccessKeyConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
}

impl std::fmt::Debug for AccessKeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessKeyConfig")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"[REDACTED]")
            .field(
                "session_token",
                if self.session_token.is_some() {
                    &"Some([REDACTED])" as &dyn std::fmt::Debug
                } else {
                    &"None" as &dyn std::fmt::Debug
                },
            )
            .field("region", &self.region)
            .finish()
    }
}

impl Drop for AccessKeyConfig {
    fn drop(&mut self) {
        self.secret_access_key.zeroize();
        if let Some(ref mut tok) = self.session_token {
            tok.zeroize();
        }
    }
}

/// Configuration for assuming an IAM role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssumeRoleConfig {
    pub role_arn: String,
    pub external_id: Option<String>,
    pub session_name: Option<String>,
    pub region: String,
}

/// Intermediate state during SSO device-code authorisation.
#[derive(Clone)]
pub struct SsoDeviceAuth {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub poll_interval: Duration,
}

impl std::fmt::Debug for SsoDeviceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoDeviceAuth")
            .field("device_code", &"[REDACTED]")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("verification_uri_complete", &self.verification_uri_complete)
            .field("expires_at", &self.expires_at)
            .field("poll_interval", &self.poll_interval)
            .finish()
    }
}

/// Result of polling the SSO OIDC token endpoint.
pub enum SsoTokenResult {
    Pending,
    Success { access_token: String, expires_at: DateTime<Utc> },
    Denied(String),
}

impl std::fmt::Debug for SsoTokenResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "SsoTokenResult::Pending"),
            Self::Success { expires_at, .. } => f
                .debug_struct("SsoTokenResult::Success")
                .field("access_token", &"[REDACTED]")
                .field("expires_at", expires_at)
                .finish(),
            Self::Denied(msg) => f.debug_tuple("SsoTokenResult::Denied").field(msg).finish(),
        }
    }
}

/// An account visible via SSO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoAccount {
    pub account_id: String,
    pub account_name: Option<String>,
    pub email_address: Option<String>,
}

/// A role accessible within an SSO account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoRole {
    pub role_name: String,
    pub account_id: String,
}

/// Authenticated session — holds temporary credentials and metadata.
#[derive(Clone)]
pub struct AwsSession {
    pub credentials: Credentials,
    pub account_id: String,
    pub identity_arn: String,
    pub region: String,
    pub expires_at: Option<DateTime<Utc>>,
    /// SSO access token (for listing accounts/roles and re-auth).
    pub sso_access_token: Option<String>,
}

impl std::fmt::Debug for AwsSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsSession")
            .field("credentials", &"[REDACTED]")
            .field("account_id", &self.account_id)
            .field("identity_arn", &self.identity_arn)
            .field("region", &self.region)
            .field("expires_at", &self.expires_at)
            .field(
                "sso_access_token",
                if self.sso_access_token.is_some() {
                    &"Some([REDACTED])" as &dyn std::fmt::Debug
                } else {
                    &"None" as &dyn std::fmt::Debug
                },
            )
            .finish()
    }
}

/// Discovered EKS cluster metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EksCluster {
    pub name: String,
    pub arn: String,
    pub endpoint: String,
    pub region: String,
    pub version: Option<String>,
    pub status: Option<String>,
    pub certificate_authority_data: Option<String>,
    pub tags: HashMap<String, String>,
}

/// UI-facing state machine for the EKS auth/discovery flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EksAuthState {
    Idle,
    WaitingForBrowser,
    PollingForToken,
    SelectingAccount,
    DiscoveringClusters,
    Ready,
    Error(String),
}

// ---------------------------------------------------------------------------
// Default EKS regions to scan
// ---------------------------------------------------------------------------

/// Commonly used EKS regions.
pub const DEFAULT_EKS_REGIONS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-1",
    "us-west-2",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-central-1",
    "eu-north-1",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-south-1",
    "ca-central-1",
    "sa-east-1",
];

// ---------------------------------------------------------------------------
// SSO device-code flow
// ---------------------------------------------------------------------------

/// Register this application as an OIDC client with IAM Identity Center.
/// Returns (client_id, client_secret).
pub async fn sso_register_client(region: &str) -> Result<(String, String)> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    sso_register_client_with_config(&config).await
}

/// Test-injection seam for `sso_register_client`. Public consumers must use
/// `sso_register_client`, which delegates here with a default `SdkConfig`.
/// The parameter exists so tests can inject a `StaticReplayClient` via
/// `.http_client(...)` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn sso_register_client_with_config(
    config: &aws_config::SdkConfig,
) -> Result<(String, String)> {
    let client = aws_sdk_ssooidc::Client::new(config);

    let resp = client
        .register_client()
        .client_name("baeus-k8s")
        .client_type("public")
        .send()
        .await
        .context("SSO OIDC: register_client failed")?;

    let client_id = resp.client_id().unwrap_or_default().to_string();
    let client_secret = resp.client_secret().unwrap_or_default().to_string();
    Ok((client_id, client_secret))
}

/// Start the device authorisation flow.
/// The user should open `verification_uri_complete` (or `verification_uri` + enter `user_code`).
pub async fn sso_start_device_auth(
    region: &str,
    client_id: &str,
    client_secret: &str,
    start_url: &str,
) -> Result<SsoDeviceAuth> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    sso_start_device_auth_with_config(&config, client_id, client_secret, start_url).await
}

/// Test-injection seam for `sso_start_device_auth`. Public consumers must use
/// `sso_start_device_auth`. The parameter exists so tests can inject a
/// `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn sso_start_device_auth_with_config(
    config: &aws_config::SdkConfig,
    client_id: &str,
    client_secret: &str,
    start_url: &str,
) -> Result<SsoDeviceAuth> {
    let client = aws_sdk_ssooidc::Client::new(config);

    let resp = client
        .start_device_authorization()
        .client_id(client_id)
        .client_secret(client_secret)
        .start_url(start_url)
        .send()
        .await
        .context("SSO OIDC: start_device_authorization failed")?;

    let expires_in = resp.expires_in() as u64;
    let poll_secs = resp.interval() as u64;

    Ok(SsoDeviceAuth {
        device_code: resp.device_code().unwrap_or_default().to_string(),
        user_code: resp.user_code().unwrap_or_default().to_string(),
        verification_uri: resp.verification_uri().unwrap_or_default().to_string(),
        verification_uri_complete: resp.verification_uri_complete().map(|s| s.to_string()),
        expires_at: Utc::now() + chrono::Duration::seconds(expires_in as i64),
        poll_interval: Duration::from_secs(poll_secs.max(1)),
    })
}

/// Poll the OIDC token endpoint. Returns `Pending` while the user hasn't authorised yet.
pub async fn sso_poll_for_token(
    region: &str,
    client_id: &str,
    client_secret: &str,
    device_code: &str,
) -> Result<SsoTokenResult> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    sso_poll_for_token_with_config(&config, client_id, client_secret, device_code).await
}

/// Test-injection seam for `sso_poll_for_token`. Public consumers must use
/// `sso_poll_for_token`. The parameter exists so tests can inject a
/// `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn sso_poll_for_token_with_config(
    config: &aws_config::SdkConfig,
    client_id: &str,
    client_secret: &str,
    device_code: &str,
) -> Result<SsoTokenResult> {
    let client = aws_sdk_ssooidc::Client::new(config);

    let result = client
        .create_token()
        .client_id(client_id)
        .client_secret(client_secret)
        .grant_type("urn:ietf:params:oauth:grant-type:device_code")
        .device_code(device_code)
        .send()
        .await;

    match result {
        Ok(resp) => {
            let access_token = resp.access_token().unwrap_or_default().to_string();
            let expires_in = resp.expires_in() as i64;
            Ok(SsoTokenResult::Success {
                access_token,
                expires_at: Utc::now() + chrono::Duration::seconds(expires_in),
            })
        }
        Err(sdk_err) => {
            // Use typed error matching for reliable detection
            let service_err = sdk_err.into_service_error();
            if service_err.is_authorization_pending_exception()
                || service_err.is_slow_down_exception()
            {
                Ok(SsoTokenResult::Pending)
            } else if service_err.is_expired_token_exception()
                || service_err.is_access_denied_exception()
            {
                Ok(SsoTokenResult::Denied(service_err.to_string()))
            } else {
                Err(anyhow::anyhow!("SSO token error: {service_err}"))
            }
        }
    }
}

/// List accounts the user can access via the SSO portal.
pub async fn sso_list_accounts(region: &str, access_token: &str) -> Result<Vec<SsoAccount>> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    sso_list_accounts_with_config(&config, access_token).await
}

/// Test-injection seam for `sso_list_accounts`. Public consumers must use
/// `sso_list_accounts`. The parameter exists so tests can inject a
/// `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn sso_list_accounts_with_config(
    config: &aws_config::SdkConfig,
    access_token: &str,
) -> Result<Vec<SsoAccount>> {
    let client = aws_sdk_sso::Client::new(config);

    let mut accounts = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut req = client.list_accounts().access_token(access_token);
        if let Some(tok) = &next_token {
            req = req.next_token(tok);
        }
        let resp = req.send().await.context("SSO: list_accounts failed")?;

        for acct in resp.account_list() {
            accounts.push(SsoAccount {
                account_id: acct.account_id().unwrap_or_default().to_string(),
                account_name: acct.account_name().map(|s| s.to_string()),
                email_address: acct.email_address().map(|s| s.to_string()),
            });
        }

        next_token = resp.next_token().map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }
    Ok(accounts)
}

/// List roles available for an account.
pub async fn sso_list_account_roles(
    region: &str,
    access_token: &str,
    account_id: &str,
) -> Result<Vec<SsoRole>> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    let client = aws_sdk_sso::Client::new(&config);

    let mut roles = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut req = client.list_account_roles().access_token(access_token).account_id(account_id);
        if let Some(tok) = &next_token {
            req = req.next_token(tok);
        }
        let resp = req.send().await.context("SSO: list_account_roles failed")?;

        for role in resp.role_list() {
            roles.push(SsoRole {
                role_name: role.role_name().unwrap_or_default().to_string(),
                account_id: role.account_id().unwrap_or_default().to_string(),
            });
        }

        next_token = resp.next_token().map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }
    Ok(roles)
}

/// Get temporary AWS credentials for a specific account + role via SSO.
pub async fn sso_get_role_credentials(
    region: &str,
    access_token: &str,
    account_id: &str,
    role_name: &str,
) -> Result<AwsSession> {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .no_credentials()
        .load()
        .await;
    sso_get_role_credentials_with_config(&config, access_token, account_id, role_name, region).await
}

/// Test-injection seam for `sso_get_role_credentials`. Public consumers must use
/// `sso_get_role_credentials`. The `region` parameter is retained explicitly because
/// it is embedded in `AwsSession.region` and may differ from what the `SdkConfig`
/// holds in tests. The `sdk_config` parameter exists so tests can inject a
/// `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn sso_get_role_credentials_with_config(
    sdk_config: &aws_config::SdkConfig,
    access_token: &str,
    account_id: &str,
    role_name: &str,
    region: &str,
) -> Result<AwsSession> {
    let client = aws_sdk_sso::Client::new(sdk_config);

    let resp = client
        .get_role_credentials()
        .access_token(access_token)
        .account_id(account_id)
        .role_name(role_name)
        .send()
        .await
        .context("SSO: get_role_credentials failed")?;

    let creds = resp.role_credentials().context("No role credentials returned")?;

    let expiration_ms = creds.expiration();
    let expires_at = if expiration_ms > 0 {
        Some(DateTime::from_timestamp_millis(expiration_ms).unwrap_or_else(Utc::now))
    } else {
        None
    };

    let credentials = Credentials::new(
        creds.access_key_id().unwrap_or_default(),
        creds.secret_access_key().unwrap_or_default(),
        creds.session_token().map(|s| s.to_string()),
        expires_at
            .map(|dt| SystemTime::UNIX_EPOCH + Duration::from_millis(dt.timestamp_millis() as u64)),
        "baeus-sso",
    );

    Ok(AwsSession {
        credentials,
        account_id: account_id.to_string(),
        identity_arn: format!("arn:aws:sso:::account/{account_id}/role/{role_name}"),
        region: region.to_string(),
        expires_at,
        sso_access_token: Some(access_token.to_string()),
    })
}

// ---------------------------------------------------------------------------
// Access key authentication
// ---------------------------------------------------------------------------

/// Validate static access keys by calling STS GetCallerIdentity.
pub async fn authenticate_with_access_key(config: &AccessKeyConfig) -> Result<AwsSession> {
    let credentials = Credentials::new(
        &config.access_key_id,
        &config.secret_access_key,
        config.session_token.clone(),
        None,
        "baeus-access-key",
    );

    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(config.region.clone()))
        .credentials_provider(credentials.clone())
        .load()
        .await;

    authenticate_with_access_key_with_config(&sdk_config, config).await
}

/// Test-injection seam for `authenticate_with_access_key`. Public consumers must use
/// `authenticate_with_access_key`. The `sdk_config` parameter exists so tests can
/// inject a `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn authenticate_with_access_key_with_config(
    sdk_config: &aws_config::SdkConfig,
    config: &AccessKeyConfig,
) -> Result<AwsSession> {
    let credentials = Credentials::new(
        &config.access_key_id,
        &config.secret_access_key,
        config.session_token.clone(),
        None,
        "baeus-access-key",
    );

    let sts = aws_sdk_sts::Client::new(sdk_config);
    let identity = sts
        .get_caller_identity()
        .send()
        .await
        .context("STS GetCallerIdentity failed — check your access keys")?;

    Ok(AwsSession {
        credentials,
        account_id: identity.account().unwrap_or_default().to_string(),
        identity_arn: identity.arn().unwrap_or_default().to_string(),
        region: config.region.clone(),
        expires_at: None, // static keys don't expire (unless session token)
        sso_access_token: None,
    })
}

// ---------------------------------------------------------------------------
// IAM role assumption
// ---------------------------------------------------------------------------

/// Assume an IAM role using source credentials.
pub async fn assume_role(
    config: &AssumeRoleConfig,
    source_credentials: &Credentials,
) -> Result<AwsSession> {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(config.region.clone()))
        .credentials_provider(source_credentials.clone())
        .load()
        .await;

    assume_role_with_config(&sdk_config, config).await
}

/// Test-injection seam for `assume_role`. Public consumers must use `assume_role`,
/// which builds a default `SdkConfig` with the source credentials and delegates here.
/// The `sdk_config` parameter exists so tests can inject a `StaticReplayClient`
/// (spec 06 0002-H4).
#[doc(hidden)]
pub async fn assume_role_with_config(
    sdk_config: &aws_config::SdkConfig,
    config: &AssumeRoleConfig,
) -> Result<AwsSession> {
    let sts = aws_sdk_sts::Client::new(sdk_config);
    let session_name = config.session_name.as_deref().unwrap_or("baeus-session");

    let mut req = sts.assume_role().role_arn(&config.role_arn).role_session_name(session_name);

    if let Some(ref ext_id) = config.external_id {
        req = req.external_id(ext_id);
    }

    let resp = req
        .send()
        .await
        .with_context(|| format!("STS AssumeRole failed for role '{}'", config.role_arn))?;

    let assumed = resp.credentials().context("No credentials returned")?;
    let expiration = assumed.expiration();

    let expiration_secs = expiration.secs();
    let expiration_nanos = expiration.subsec_nanos();

    let expires_at = DateTime::from_timestamp(expiration_secs, expiration_nanos);

    let system_expiry = SystemTime::UNIX_EPOCH
        + Duration::from_secs(expiration_secs as u64)
        + Duration::from_nanos(expiration_nanos as u64);

    let credentials = Credentials::new(
        assumed.access_key_id(),
        assumed.secret_access_key(),
        Some(assumed.session_token().to_string()),
        Some(system_expiry),
        "baeus-assume-role",
    );

    Ok(AwsSession {
        credentials,
        account_id: resp
            .assumed_role_user()
            .map(|u| u.arn().split(':').nth(4).unwrap_or(""))
            .unwrap_or("")
            .to_string(),
        identity_arn: resp.assumed_role_user().map(|u| u.arn().to_string()).unwrap_or_default(),
        region: config.region.clone(),
        expires_at,
        sso_access_token: None,
    })
}

// ---------------------------------------------------------------------------
// EKS cluster discovery
// ---------------------------------------------------------------------------

/// Discover EKS clusters across multiple regions in parallel.
/// Calls `progress_callback` with (completed_regions, total_regions).
pub async fn discover_eks_clusters<F>(
    credentials: &Credentials,
    regions: &[String],
    progress_callback: F,
) -> Result<Vec<EksCluster>>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    let progress = std::sync::Arc::new(progress_callback);
    let total = regions.len();
    let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    for region in regions {
        let creds = credentials.clone();
        let region = region.clone();
        let completed = completed.clone();
        let progress = progress.clone();

        handles.push(tokio::spawn(async move {
            let result = discover_clusters_in_region(&creds, &region).await;
            let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            progress(done, total);
            result
        }));
    }

    let mut all_clusters = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(clusters)) => all_clusters.extend(clusters),
            Ok(Err(e)) => {
                tracing::warn!("EKS discovery error in a region: {e:#}");
            }
            Err(e) => {
                tracing::warn!("EKS discovery task panicked: {e}");
            }
        }
    }

    all_clusters.sort_by(|a, b| a.region.cmp(&b.region).then(a.name.cmp(&b.name)));
    Ok(all_clusters)
}

/// Discover EKS clusters in a single region.
pub async fn discover_clusters_in_region(
    credentials: &Credentials,
    region: &str,
) -> Result<Vec<EksCluster>> {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_types::region::Region::new(region.to_string()))
        .credentials_provider(credentials.clone())
        .load()
        .await;

    discover_clusters_in_region_with_config(&sdk_config, region).await
}

/// Test-injection seam for `discover_clusters_in_region`. Public consumers must use
/// `discover_clusters_in_region`. The `region` parameter is retained explicitly for
/// `EksCluster.region` tagging. The `sdk_config` parameter exists so tests can inject
/// a `StaticReplayClient` (spec 06 0002-H4).
#[doc(hidden)]
pub async fn discover_clusters_in_region_with_config(
    sdk_config: &aws_config::SdkConfig,
    region: &str,
) -> Result<Vec<EksCluster>> {
    let eks = aws_sdk_eks::Client::new(sdk_config);

    // List cluster names
    let mut cluster_names = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut req = eks.list_clusters();
        if let Some(tok) = &next_token {
            req = req.next_token(tok);
        }
        let resp = req.send().await.context("EKS: list_clusters failed")?;
        cluster_names.extend(resp.clusters().iter().map(|s| s.to_string()));

        next_token = resp.next_token().map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    // Describe each cluster to get details
    let mut clusters = Vec::new();
    for name in &cluster_names {
        match eks.describe_cluster().name(name).send().await {
            Ok(resp) => {
                if let Some(cluster) = resp.cluster() {
                    clusters.push(EksCluster {
                        name: cluster.name().unwrap_or_default().to_string(),
                        arn: cluster.arn().unwrap_or_default().to_string(),
                        endpoint: cluster.endpoint().unwrap_or_default().to_string(),
                        region: region.to_string(),
                        version: cluster.version().map(|s| s.to_string()),
                        status: cluster.status().map(|s| s.as_str().to_string()),
                        certificate_authority_data: cluster
                            .certificate_authority()
                            .and_then(|ca| ca.data())
                            .map(|s| s.to_string()),
                        tags: cluster
                            .tags()
                            .map(|t| {
                                t.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
                            })
                            .unwrap_or_default(),
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Failed to describe cluster '{}' in {}: {e}", name, region);
            }
        }
    }

    Ok(clusters)
}

// ---------------------------------------------------------------------------
// EKS bearer token generation (pre-signed STS URL)
// ---------------------------------------------------------------------------

/// Generate an EKS bearer token using a pre-signed STS GetCallerIdentity URL.
///
/// This is the same mechanism used by `aws eks get-token` and `aws-iam-authenticator`.
/// The token is a base64-encoded pre-signed URL with a `x-k8s-aws-id` header set to
/// the cluster name.
pub async fn generate_eks_token(
    cluster_name: &str,
    credentials: &Credentials,
    region: &str,
) -> Result<String> {
    // Build the pre-signed STS URL directly.
    build_eks_presigned_token(cluster_name, credentials, region).await
}

/// Manually build the pre-signed STS URL that serves as an EKS bearer token.
async fn build_eks_presigned_token(
    cluster_name: &str,
    credentials: &Credentials,
    region: &str,
) -> Result<String> {
    use aws_credential_types::provider::ProvideCredentials;

    let creds = credentials.provide_credentials().await.context("Failed to resolve credentials")?;

    let access_key = creds.access_key_id();
    let secret_key = creds.secret_access_key();
    let session_token = creds.session_token();

    let host = format!("sts.{region}.amazonaws.com");
    let now = Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential_scope = format!("{date_stamp}/{region}/sts/aws4_request");

    // Canonical request components
    let method = "GET";
    let canonical_uri = "/";
    let signed_headers = "host;x-k8s-aws-id";

    // Query parameters (sorted)
    let credential = format!("{access_key}/{credential_scope}");

    // Build canonical query string — must be sorted by param name
    let mut qp: Vec<(String, String)> = vec![
        ("Action".to_string(), "GetCallerIdentity".to_string()),
        ("Version".to_string(), "2011-06-15".to_string()),
        ("X-Amz-Algorithm".to_string(), "AWS4-HMAC-SHA256".to_string()),
        ("X-Amz-Credential".to_string(), credential.clone()),
        ("X-Amz-Date".to_string(), amz_date.clone()),
        ("X-Amz-Expires".to_string(), "60".to_string()),
        ("X-Amz-SignedHeaders".to_string(), signed_headers.to_string()),
    ];
    if let Some(tok) = session_token {
        qp.push(("X-Amz-Security-Token".to_string(), tok.to_string()));
    }
    qp.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_querystring: String = qp
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let canonical_headers = format!("host:{host}\nx-k8s-aws-id:{cluster_name}\n");
    let payload_hash = hex_sha256(b"");

    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{canonical_querystring}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    // String to sign
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex_sha256(canonical_request.as_bytes())
    );

    // Signing key
    let k_date = hmac_sha256(format!("AWS4{secret_key}").as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"sts");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex::encode(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    let presigned_url =
        format!("https://{host}/?{canonical_querystring}&X-Amz-Signature={signature}");

    // EKS token format: "k8s-aws-v1." + base64url(presigned_url) with padding stripped
    let encoded = base64_url_encode(presigned_url.as_bytes());
    Ok(format!("k8s-aws-v1.{encoded}"))
}

// ---------------------------------------------------------------------------
// EKS token refresh — spec 06 0002-H2
// ---------------------------------------------------------------------------

/// Token material and its expiry timestamp.
pub struct TokenState {
    /// The bearer token (presigned STS URL as EKS k8s-aws-v1.… string).
    pub token: secrecy::SecretString,
    /// Wall-clock instant at which the token expires.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Errors produced by `EksTokenRefresher::refresh`.
#[derive(Debug, thiserror::Error)]
pub enum EksTokenRefreshError {
    /// `generate_eks_token` failed to re-sign the presigned URL.
    #[error("Failed to regenerate EKS presigned token: {0}")]
    PresignFailed(String),
    /// The write lock on the token state was poisoned by a panicking writer.
    #[error("Refresh state lock poisoned")]
    LockPoisoned,
}

/// Boxed async refresh future returned by a `RefreshFn`.
type RefreshFuture = Pin<Box<dyn Future<Output = Result<TokenState, EksTokenRefreshError>> + Send>>;
/// Boxed refresh closure: takes no args, returns a `RefreshFuture`.
type RefreshFn = Arc<dyn Fn() -> RefreshFuture + Send + Sync>;

/// Number of seconds before expiry at which `should_refresh()` returns `true`.
const REFRESH_LEEWAY_SECS: i64 = 10;

/// Holds a refreshable EKS bearer token.
///
/// - `current_token()` / `should_refresh()` / `expires_at()` are synchronous and
///   cheap (short-lived `std::sync::RwLock` read).
/// - `refresh()` is async and serialised by a `tokio::sync::Mutex` so two
///   concurrent callers never double-presign.
pub struct EksTokenRefresher {
    inner: Arc<RwLock<TokenState>>,
    refresh_fn: RefreshFn,
    in_flight: Arc<tokio::sync::Mutex<()>>,
}

impl EksTokenRefresher {
    /// Construct with an initial `TokenState` and a boxed async refresh closure.
    pub fn new(initial: TokenState, refresh_fn: RefreshFn) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            refresh_fn,
            in_flight: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Returns `true` when the token is at or past `expires_at - 10s`.
    ///
    /// The 10-second leeway (spec 06 0002-H2 acceptance criterion 2c) gives the
    /// request pipeline time to complete before the token actually expires.
    pub fn should_refresh(&self) -> bool {
        let state = self.inner.read().expect("EksTokenRefresher lock poisoned");
        chrono::Utc::now() + chrono::Duration::seconds(REFRESH_LEEWAY_SECS) >= state.expires_at
    }

    /// Clone the current token (`SecretString::clone` is `O(len)` but cheap for
    /// a short AWS presigned URL string).
    pub fn current_token(&self) -> secrecy::SecretString {
        let state = self.inner.read().expect("EksTokenRefresher lock poisoned");
        state.token.clone()
    }

    /// Read the current expiry timestamp.
    pub fn expires_at(&self) -> chrono::DateTime<chrono::Utc> {
        let state = self.inner.read().expect("EksTokenRefresher lock poisoned");
        state.expires_at
    }

    /// Perform a refresh if `should_refresh()` still returns `true` after
    /// acquiring the in-flight mutex.
    ///
    /// Concurrent callers wait on the mutex; the first caller refreshes and the
    /// rest re-check `should_refresh()` on entry — if the first caller already
    /// refreshed, the subsequent callers return immediately.
    pub async fn refresh(&self) -> Result<(), EksTokenRefreshError> {
        let _guard = self.in_flight.lock().await;
        // Re-check after acquiring the guard: a prior holder may have just refreshed.
        if !self.should_refresh() {
            return Ok(());
        }
        let new_state = (self.refresh_fn)().await?;
        let mut w = self.inner.write().map_err(|_| EksTokenRefreshError::LockPoisoned)?;
        *w = new_state;
        Ok(())
    }

    /// Shallow-clone this refresher: the returned handle shares the same `Arc`s
    /// (same token state, same refresh closure, same in-flight guard) so the
    /// middleware and the returned refresher stay in sync.
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            refresh_fn: self.refresh_fn.clone(),
            in_flight: self.in_flight.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// tower `AuthRefreshLayer` — injects a refreshed bearer token before each
// outbound request to the Kubernetes API server.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AuthRefreshLayer {
    refresher: Arc<EksTokenRefresher>,
}

impl<S> tower::Layer<S> for AuthRefreshLayer {
    type Service = AuthRefreshService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthRefreshService { inner, refresher: self.refresher.clone() }
    }
}

#[derive(Clone)]
struct AuthRefreshService<S> {
    inner: S,
    refresher: Arc<EksTokenRefresher>,
}

impl<S, B> tower::Service<http::Request<B>> for AuthRefreshService<S>
where
    S: tower::Service<http::Request<B>, Response = http::Response<hyper::body::Incoming>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut req: http::Request<B>) -> Self::Future {
        let refresher = self.refresher.clone();
        let mut inner = self.inner.clone();
        Box::pin(async move {
            if refresher.should_refresh() {
                refresher
                    .refresh()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            }
            let token = refresher.current_token();
            let header_value = format!(
                "Bearer {}",
                <secrecy::SecretString as secrecy::ExposeSecret<str>>::expose_secret(&token)
            );
            req.headers_mut().insert(
                http::header::AUTHORIZATION,
                header_value.parse().map_err(|e: http::header::InvalidHeaderValue| {
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                })?,
            );
            inner.call(req).await.map_err(Into::into)
        })
    }
}

// ---------------------------------------------------------------------------
// kube::Client creation
// ---------------------------------------------------------------------------

/// Create a `kube::Client` for an EKS cluster using AWS credentials.
///
/// Returns `(client, refresher)`. The `client` is backed by an `AuthRefreshLayer`
/// that calls `EksTokenRefresher::refresh()` before each outbound API request
/// whenever the current token is within 10 seconds of expiry (spec 06 0002-H2).
///
/// The returned `EksTokenRefresher` allows callers to inspect the token expiry
/// and populate `ClusterConnection::token_expiry` for display in the UI.
pub async fn create_eks_client(
    cluster: &EksCluster,
    credentials: &Credentials,
) -> Result<(kube::Client, EksTokenRefresher)> {
    create_eks_client_with_initial_ttl_secs(cluster, credentials, 60).await
}

/// Test-injection seam for `create_eks_client`. Public consumers must use
/// `create_eks_client`, which delegates here with a 60-second initial TTL.
/// The parameter exists only so async wizard tests can seed an initial expiry
/// outside/inside the 10-second refresh leeway deterministically (spec 06 0002-H4
/// acceptance 4a for `create_eks_client`).
#[doc(hidden)]
pub async fn create_eks_client_with_initial_ttl_secs(
    cluster: &EksCluster,
    credentials: &Credentials,
    initial_ttl_secs: i64,
) -> Result<(kube::Client, EksTokenRefresher)> {
    use kube::client::ConfigExt as _;
    use tower::ServiceBuilder;

    let ca_data = cluster
        .certificate_authority_data
        .as_deref()
        .context("Cluster is missing certificate authority data")?;

    // Build initial token and set up the refresh closure.
    let initial_token = generate_eks_token(&cluster.name, credentials, &cluster.region).await?;
    let initial_state = TokenState {
        token: secrecy::SecretString::new(initial_token.clone().into()),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(initial_ttl_secs),
    };

    // Capture cluster identity + credentials for the refresh closure.
    let creds_for_refresh = credentials.clone();
    let cluster_name_for_refresh = cluster.name.clone();
    let region_for_refresh = cluster.region.clone();
    let refresh_fn: RefreshFn = Arc::new(move || {
        let creds = creds_for_refresh.clone();
        let cluster_name = cluster_name_for_refresh.clone();
        let region = region_for_refresh.clone();
        Box::pin(async move {
            let token = generate_eks_token(&cluster_name, &creds, &region)
                .await
                .map_err(|e| EksTokenRefreshError::PresignFailed(e.to_string()))?;
            Ok(TokenState {
                token: secrecy::SecretString::new(token.into()),
                expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
            })
        })
    });
    let refresher = EksTokenRefresher::new(initial_state, refresh_fn);

    // Build the kubeconfig with the initial static token (inert under the custom
    // service assembly below — `auth_layer` is not applied so this token is never
    // consulted for outbound requests).
    let kubeconfig = kube::config::Kubeconfig {
        clusters: vec![kube::config::NamedCluster {
            name: cluster.name.clone(),
            cluster: Some(kube::config::Cluster {
                server: Some(cluster.endpoint.clone()),
                certificate_authority_data: Some(ca_data.to_string()),
                ..Default::default()
            }),
        }],
        auth_infos: vec![kube::config::NamedAuthInfo {
            name: format!("eks-{}", cluster.name),
            auth_info: Some(kube::config::AuthInfo {
                token: Some(secrecy::SecretString::new(initial_token.into())),
                ..Default::default()
            }),
        }],
        contexts: vec![kube::config::NamedContext {
            name: format!("eks:{}:{}", cluster.region, cluster.name),
            context: Some(kube::config::Context {
                cluster: cluster.name.clone(),
                user: Some(format!("eks-{}", cluster.name)),
                ..Default::default()
            }),
        }],
        current_context: Some(format!("eks:{}:{}", cluster.region, cluster.name)),
        ..Default::default()
    };

    let kube_config = kube::Config::from_custom_kubeconfig(kubeconfig, &Default::default())
        .await
        .context("Failed to build kube config from EKS cluster data")?;
    let default_ns = kube_config.default_namespace.clone();

    // Build the layered HTTP service: base_uri → auth_refresh → hyper_util::Client.
    let https = kube_config
        .rustls_https_connector()
        .context("Failed to build rustls connector for EKS cluster")?;
    let http_client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build(https);
    let service = ServiceBuilder::new()
        .layer(kube_config.base_uri_layer())
        .layer(AuthRefreshLayer { refresher: Arc::new(refresher.clone_handle()) })
        .service(http_client);
    let client = kube::Client::new(service, default_ns);

    Ok((client, refresher))
}

/// Generate the context name for an EKS cluster.
pub fn eks_context_name(cluster: &EksCluster) -> String {
    format!("eks:{}:{}", cluster.region, cluster.name)
}

/// Build an EKS context name from individual parts (for matching without an EksCluster).
pub fn eks_context_name_from_parts(cluster_name: &str, region: &str) -> String {
    format!("eks:{region}:{cluster_name}")
}

// ---------------------------------------------------------------------------
// Crypto helpers (SHA-256 / HMAC-SHA256 using ring)
// ---------------------------------------------------------------------------

fn hex_sha256(data: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    hex::encode(digest.as_ref())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let s_key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let tag = ring::hmac::sign(&s_key, data);
    tag.as_ref().to_vec()
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Hex encoding helper.
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{b:02x}")).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_auth_method_serialization() {
        assert_eq!(serde_json::to_string(&AwsAuthMethod::Sso).unwrap(), "\"Sso\"");
        assert_eq!(serde_json::to_string(&AwsAuthMethod::AccessKey).unwrap(), "\"AccessKey\"");
        assert_eq!(serde_json::to_string(&AwsAuthMethod::AssumeRole).unwrap(), "\"AssumeRole\"");
    }

    #[test]
    fn test_eks_auth_state_serialization() {
        let states = vec![
            EksAuthState::Idle,
            EksAuthState::WaitingForBrowser,
            EksAuthState::PollingForToken,
            EksAuthState::SelectingAccount,
            EksAuthState::DiscoveringClusters,
            EksAuthState::Ready,
            EksAuthState::Error("test error".to_string()),
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let deserialized: EksAuthState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn test_eks_cluster_serialization() {
        let cluster = EksCluster {
            name: "my-cluster".to_string(),
            arn: "arn:aws:eks:us-east-1:123456789:cluster/my-cluster".to_string(),
            endpoint: "https://ABCDEF.eks.us-east-1.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            version: Some("1.28".to_string()),
            status: Some("ACTIVE".to_string()),
            certificate_authority_data: Some("LS0tLS1...".to_string()),
            tags: HashMap::from([("env".to_string(), "prod".to_string())]),
        };
        let json = serde_json::to_string(&cluster).unwrap();
        let deserialized: EksCluster = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-cluster");
        assert_eq!(deserialized.region, "us-east-1");
        assert_eq!(deserialized.tags.get("env").unwrap(), "prod");
    }

    #[test]
    fn test_sso_config_serialization() {
        let config = SsoConfig {
            start_url: "https://my-org.awsapps.com/start".to_string(),
            region: "us-east-1".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SsoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.start_url, config.start_url);
        assert_eq!(deserialized.region, config.region);
    }

    #[test]
    fn test_access_key_config_zeroize_on_drop() {
        let config = AccessKeyConfig {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: Some("session-token-value".to_string()),
            region: "us-east-1".to_string(),
        };
        // Just verify it compiles and doesn't panic on drop
        drop(config);
    }

    #[test]
    fn test_assume_role_config_serialization() {
        let config = AssumeRoleConfig {
            role_arn: "arn:aws:iam::123456789:role/MyRole".to_string(),
            external_id: Some("ext-id-123".to_string()),
            session_name: Some("baeus-session".to_string()),
            region: "us-west-2".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AssumeRoleConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role_arn, config.role_arn);
        assert_eq!(deserialized.external_id, config.external_id);
    }

    #[test]
    fn test_eks_context_name() {
        let cluster = EksCluster {
            name: "production".to_string(),
            arn: String::new(),
            endpoint: String::new(),
            region: "us-west-2".to_string(),
            version: None,
            status: None,
            certificate_authority_data: None,
            tags: HashMap::new(),
        };
        assert_eq!(eks_context_name(&cluster), "eks:us-west-2:production");
    }

    #[test]
    fn test_default_eks_regions() {
        assert!(DEFAULT_EKS_REGIONS.contains(&"us-east-1"));
        assert!(DEFAULT_EKS_REGIONS.contains(&"eu-west-1"));
        assert!(DEFAULT_EKS_REGIONS.contains(&"ap-northeast-1"));
        assert!(DEFAULT_EKS_REGIONS.len() >= 10);
    }

    #[test]
    fn test_sso_account_serialization() {
        let account = SsoAccount {
            account_id: "123456789012".to_string(),
            account_name: Some("Production".to_string()),
            email_address: Some("admin@example.com".to_string()),
        };
        let json = serde_json::to_string(&account).unwrap();
        let deserialized: SsoAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, "123456789012");
        assert_eq!(deserialized.account_name.as_deref(), Some("Production"));
    }

    #[test]
    fn test_sso_role_serialization() {
        let role = SsoRole {
            role_name: "AdministratorAccess".to_string(),
            account_id: "123456789012".to_string(),
        };
        let json = serde_json::to_string(&role).unwrap();
        let deserialized: SsoRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role_name, "AdministratorAccess");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("a b"), "a%20b");
        assert_eq!(url_encode("a+b"), "a%2Bb");
        assert_eq!(url_encode("a/b"), "a%2Fb");
        assert_eq!(url_encode("key=value"), "key%3Dvalue");
    }

    #[test]
    fn test_sha256_empty() {
        let hash = hex_sha256(b"");
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_sha256_hello() {
        let hash = hex_sha256(b"hello");
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn test_hmac_sha256_known_vector() {
        let result = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
        let hex_result = hex::encode(&result);
        assert_eq!(hex_result, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
    }

    #[test]
    fn test_base64_url_encode() {
        let encoded = base64_url_encode(b"hello world");
        assert_eq!(encoded, "aGVsbG8gd29ybGQ");
    }

    // --- Slice B step 4: EksTokenRefresher unit tests ---

    fn make_fixed_refresher(
        initial_expires_at: chrono::DateTime<chrono::Utc>,
    ) -> (EksTokenRefresher, Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let refresh_fn: RefreshFn = Arc::new(move || {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
            let token_str = format!("token-{}", n);
            // Return expires_at well inside leeway so subsequent calls also refresh.
            let expires = chrono::Utc::now() + chrono::Duration::seconds(5);
            Box::pin(async move {
                Ok(TokenState {
                    token: secrecy::SecretString::new(token_str.into()),
                    expires_at: expires,
                })
            })
        });
        let initial = TokenState {
            token: secrecy::SecretString::new("token-0".to_string().into()),
            expires_at: initial_expires_at,
        };
        (EksTokenRefresher::new(initial, refresh_fn), counter)
    }

    #[test]
    fn should_refresh_returns_true_when_within_ten_seconds_of_expiry() {
        // within leeway: now + 9s → should refresh
        let (r, _) = make_fixed_refresher(chrono::Utc::now() + chrono::Duration::seconds(9));
        assert!(r.should_refresh(), "9s remaining must trigger refresh");

        // outside leeway: now + 11s → should NOT refresh
        let (r2, _) = make_fixed_refresher(chrono::Utc::now() + chrono::Duration::seconds(11));
        assert!(!r2.should_refresh(), "11s remaining must not trigger refresh");

        // already expired: now - 1s → should refresh
        let (r3, _) = make_fixed_refresher(chrono::Utc::now() - chrono::Duration::seconds(1));
        assert!(r3.should_refresh(), "expired token must trigger refresh");
    }

    #[tokio::test]
    async fn refresher_produces_fresh_token_on_call() {
        use secrecy::ExposeSecret as _;
        // seed with expires_at = now + 5s (within leeway) so both refreshes are seen through.
        let initial_expires = chrono::Utc::now() + chrono::Duration::seconds(5);
        let (r, counter) = make_fixed_refresher(initial_expires);

        // First call — should_refresh() is true, so refresh fires.
        r.refresh().await.expect("first refresh should succeed");
        let t1 = r.current_token();

        // Second call — expires_at is still within leeway (+5s), refresh fires again.
        r.refresh().await.expect("second refresh should succeed");
        let t2 = r.current_token();

        // The two tokens must differ (counter was incremented twice).
        assert_ne!(
            t1.expose_secret(),
            t2.expose_secret(),
            "successive refreshes must produce distinct tokens"
        );
        assert_eq!(
            counter.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "exactly two refreshes should have fired"
        );
    }

    #[tokio::test]
    async fn refresher_surfaces_typed_error_on_failure() {
        let refresh_fn: RefreshFn = Arc::new(|| {
            Box::pin(async {
                Err(EksTokenRefreshError::PresignFailed("injected failure".to_string()))
            })
        });
        // Seed with expires_at in the past so should_refresh() == true immediately.
        let initial = TokenState {
            token: secrecy::SecretString::new("stale".to_string().into()),
            expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        };
        let r = EksTokenRefresher::new(initial, refresh_fn);
        match r.refresh().await {
            Err(EksTokenRefreshError::PresignFailed(msg)) => {
                assert_eq!(msg, "injected failure");
            }
            other => panic!("expected PresignFailed, got {:?}", other),
        }
    }

    // --- Slice B step 6: wiremock acceptance test (K8S API server + counter refresh) ---
    //
    // Placed inline (rather than in tests/) because `AuthRefreshLayer` is private to
    // this module. The `#[cfg(test)] mod tests { use super::*; }` pattern gives full
    // access. wiremock is a dev-dependency; this test exercises the real middleware
    // code path as specified in spec 06 0002-H2 acceptance criterion 2a.

    #[tokio::test]
    async fn eks_refresh_layer_refreshes_token_and_updates_authorization_header() {
        use kube::client::ConfigExt as _;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tower::ServiceBuilder;
        use wiremock::matchers::any;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // --- 1. Start a wiremock K8S API server mock --------------------------------
        let mock_server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "kind": "NamespaceList",
                "apiVersion": "v1",
                "metadata": {},
                "items": []
            })))
            .expect(2) // exactly two requests total
            .mount(&mock_server)
            .await;

        // --- 2. Construct an EksTokenRefresher directly ----------------------------
        //
        // Initial expires_at = Utc::now() + 12s — outside the 10-second leeway, so
        // the first request takes the fast path (no refresh).
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let refresh_fn: RefreshFn = Arc::new(move || {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
            let token_str = format!("token-{}", n);
            let expires = chrono::Utc::now() + chrono::Duration::seconds(60);
            Box::pin(async move {
                Ok(TokenState {
                    token: secrecy::SecretString::new(token_str.into()),
                    expires_at: expires,
                })
            })
        });
        let initial = TokenState {
            token: secrecy::SecretString::new("token-0".to_string().into()),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(12),
        };
        let refresher = EksTokenRefresher::new(initial, refresh_fn);

        // --- 3. Build kube::Client pointing at the mock server ---------------------
        let server_uri: http::Uri = mock_server.uri().parse().expect("valid URI");
        let kube_config = kube::Config::new(server_uri);
        let default_ns = kube_config.default_namespace.clone();

        let mut http = hyper_util::client::legacy::connect::HttpConnector::new();
        http.enforce_http(false);
        let http_client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(http);
        let service = ServiceBuilder::new()
            .layer(kube_config.base_uri_layer())
            .layer(AuthRefreshLayer { refresher: Arc::new(refresher.clone_handle()) })
            .service(http_client);
        let client = kube::Client::new(service, default_ns);

        // --- 4. First request: fast path, no refresh --------------------------------
        let api: kube::Api<k8s_openapi::api::core::v1::Namespace> = kube::Api::all(client.clone());
        let _ = api.list(&Default::default()).await;

        {
            let requests = mock_server.received_requests().await.expect("requests");
            assert_eq!(requests.len(), 1, "exactly one request after first list()");
            let auth = requests[0]
                .headers
                .get("authorization")
                .map(|v| v.to_str().unwrap_or(""))
                .unwrap_or("");
            assert_eq!(auth, "Bearer token-0", "first request should carry initial token");
            assert_eq!(
                counter.load(Ordering::SeqCst),
                0,
                "counter must be zero — no refresh on the fast path"
            );
        }

        // --- 5. Sleep past the leeway: remaining = 12 - 3 = 9s < 10s ---------------
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // --- 6. Second request: refresh fires, new token in header ------------------
        let _ = api.list(&Default::default()).await;

        {
            let requests = mock_server.received_requests().await.expect("requests");
            assert_eq!(requests.len(), 2, "exactly two requests after second list()");
            let auth = requests[1]
                .headers
                .get("authorization")
                .map(|v| v.to_str().unwrap_or(""))
                .unwrap_or("");
            assert_eq!(auth, "Bearer token-1", "second request should carry refreshed token");
            assert_eq!(
                counter.load(Ordering::SeqCst),
                1,
                "exactly one presign issued between the two API calls"
            );
        }
    }
}
