//! Native AWS EKS integration — SSO device flow, access keys, role assumption,
//! cluster discovery, and EKS bearer-token generation.
//!
//! Eliminates the need for the AWS CLI by using `aws-sdk-rust` directly.

use std::collections::HashMap;
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
    let client = aws_sdk_ssooidc::Client::new(&config);

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
    let client = aws_sdk_ssooidc::Client::new(&config);

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
    let client = aws_sdk_ssooidc::Client::new(&config);

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
    let client = aws_sdk_sso::Client::new(&config);

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
    let client = aws_sdk_sso::Client::new(&config);

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

    let sts = aws_sdk_sts::Client::new(&sdk_config);
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

    let sts = aws_sdk_sts::Client::new(&sdk_config);
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

    let eks = aws_sdk_eks::Client::new(&sdk_config);

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
// kube::Client creation
// ---------------------------------------------------------------------------

/// Create a `kube::Client` for an EKS cluster using AWS credentials.
pub async fn create_eks_client(
    cluster: &EksCluster,
    credentials: &Credentials,
) -> Result<kube::Client> {
    let token = generate_eks_token(&cluster.name, credentials, &cluster.region).await?;

    let ca_data = cluster
        .certificate_authority_data
        .as_deref()
        .context("Cluster is missing certificate authority data")?;

    // The kube crate expects certificate_authority_data as the raw base64 string —
    // it decodes it internally. Pass it through as-is from the EKS API.
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
                token: Some(secrecy::SecretString::new(token.into())),
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

    kube::Client::try_from(kube_config).context("Failed to create kube client for EKS cluster")
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
}
