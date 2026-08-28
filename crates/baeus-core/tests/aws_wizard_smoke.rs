//! Async smoke tests for the AWS wizard SDK-touching functions (spec 06 0002-H4).
//!
//! Uses `aws_smithy_http_client::test_util::StaticReplayClient` as the primary
//! mock transport — the non-deprecated canonical path in the locked tree
//! (aws-smithy-http-client 1.1.12). Every test injects stubbed credentials and
//! requires no live AWS network access.

use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
use aws_smithy_types::body::SdkBody;
use aws_types::region::Region;
use baeus_core::aws_eks::{
    AccessKeyConfig, AssumeRoleConfig, SsoTokenResult, assume_role_with_config,
    authenticate_with_access_key_with_config, discover_clusters_in_region_with_config,
    sso_get_role_credentials_with_config, sso_list_accounts_with_config,
    sso_poll_for_token_with_config, sso_register_client_with_config,
    sso_start_device_auth_with_config,
};

/// Build a test `SdkConfig` backed by the given replay events.
///
/// Credentials are set to a stub static key so tests that need auth don't fail
/// with a missing-credentials error. Call `.no_credentials()` variants as
/// needed (they are ignored by `StaticReplayClient` anyway).
async fn make_sdk_config(replay: StaticReplayClient) -> aws_config::SdkConfig {
    aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new("AKIATEST", "secrettest", None, None, "smoke-test"))
        .http_client(replay)
        .load()
        .await
}

/// Build a stub HTTP request (body not validated by replay client).
fn stub_request(uri: &str) -> http::Request<SdkBody> {
    http::Request::builder().uri(uri).body(SdkBody::empty()).expect("valid request")
}

// ---------------------------------------------------------------------------
// sso_register_client
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sso_register_client_returns_client_id_and_secret() {
    let body = r#"{"clientId":"cid-1","clientSecret":"csec-1","clientIdIssuedAt":1700000000,"clientSecretExpiresAt":1700000000}"#;
    let replay = StaticReplayClient::new(vec![ReplayEvent::new(
        stub_request("https://oidc.us-east-1.amazonaws.com/client/register"),
        http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(SdkBody::from(body))
            .unwrap(),
    )]);

    let config = make_sdk_config(replay).await;
    let (client_id, client_secret) =
        sso_register_client_with_config(&config).await.expect("register_client must succeed");

    assert_eq!(client_id, "cid-1");
    assert_eq!(client_secret, "csec-1");
}

// ---------------------------------------------------------------------------
// sso_start_device_auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sso_start_device_auth_returns_device_and_user_codes() {
    let body = r#"{
        "deviceCode":"dev-code-1",
        "userCode":"USER-1234",
        "verificationUri":"https://device.sso.us-east-1.amazonaws.com/",
        "verificationUriComplete":"https://device.sso.us-east-1.amazonaws.com/?user_code=USER-1234",
        "expiresIn":600,
        "interval":5
    }"#;
    let replay = StaticReplayClient::new(vec![ReplayEvent::new(
        stub_request("https://oidc.us-east-1.amazonaws.com/device_authorization"),
        http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(SdkBody::from(body))
            .unwrap(),
    )]);

    let config = make_sdk_config(replay).await;
    let auth = sso_start_device_auth_with_config(&config, "cid-1", "csec-1", "https://start.url")
        .await
        .expect("start_device_auth must succeed");

    assert_eq!(auth.device_code, "dev-code-1");
    assert_eq!(auth.user_code, "USER-1234");
    assert_eq!(auth.poll_interval.as_secs(), 5);
    assert!(auth.verification_uri_complete.is_some());
    // expires_at should be roughly now + 600s
    let remaining = auth.expires_at - chrono::Utc::now();
    assert!(remaining.num_seconds() > 590 && remaining.num_seconds() <= 600);
}

// ---------------------------------------------------------------------------
// sso_poll_for_token  (pending → success in one test to prove retry path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sso_poll_for_token_returns_pending_then_success() {
    let pending_body =
        r#"{"__type":"AuthorizationPendingException","message":"Authorization pending"}"#;
    let success_body = r#"{"accessToken":"at-1","tokenType":"Bearer","expiresIn":3600}"#;
    let replay = StaticReplayClient::new(vec![
        ReplayEvent::new(
            stub_request("https://oidc.us-east-1.amazonaws.com/token"),
            http::Response::builder()
                .status(400)
                .header("content-type", "application/json")
                .body(SdkBody::from(pending_body))
                .unwrap(),
        ),
        ReplayEvent::new(
            stub_request("https://oidc.us-east-1.amazonaws.com/token"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(success_body))
                .unwrap(),
        ),
    ]);

    // Both calls share the same SdkConfig and replay client — StaticReplayClient
    // pops events in order, so first call gets 400, second gets 200.
    let config = make_sdk_config(replay).await;

    // First poll: authorization_pending → SsoTokenResult::Pending
    let first =
        sso_poll_for_token_with_config(&config, "cid-1", "csec-1", "dev-code-1").await.unwrap();
    assert!(
        matches!(first, SsoTokenResult::Pending),
        "first poll must return Pending, got {:?}",
        first
    );

    // Second poll: 200 success → SsoTokenResult::Success
    let second =
        sso_poll_for_token_with_config(&config, "cid-1", "csec-1", "dev-code-1").await.unwrap();
    match second {
        SsoTokenResult::Success { access_token, expires_at } => {
            assert_eq!(access_token, "at-1");
            let remaining = expires_at - chrono::Utc::now();
            assert!(remaining.num_seconds() > 3590 && remaining.num_seconds() <= 3600);
        }
        other => panic!("second poll must return Success, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// sso_list_accounts  (paginated — two pages)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sso_list_accounts_paginates_and_returns_all() {
    let page1 = r#"{
        "accountList":[
            {"accountId":"111111111111","accountName":"Account A","emailAddress":"a@example.com"},
            {"accountId":"222222222222","accountName":"Account B","emailAddress":"b@example.com"}
        ],
        "nextToken":"tok-page-2"
    }"#;
    let page2 = r#"{
        "accountList":[
            {"accountId":"333333333333","accountName":"Account C","emailAddress":"c@example.com"},
            {"accountId":"444444444444","accountName":"Account D","emailAddress":"d@example.com"}
        ]
    }"#;
    let replay = StaticReplayClient::new(vec![
        ReplayEvent::new(
            stub_request("https://portal.sso.us-east-1.amazonaws.com/assignment/accounts"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(page1))
                .unwrap(),
        ),
        ReplayEvent::new(
            stub_request("https://portal.sso.us-east-1.amazonaws.com/assignment/accounts"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(page2))
                .unwrap(),
        ),
    ]);

    let config = make_sdk_config(replay).await;
    let accounts =
        sso_list_accounts_with_config(&config, "access-token-test").await.expect("list_accounts");

    assert_eq!(accounts.len(), 4, "must return all 4 accounts across 2 pages");
    assert!(accounts.iter().any(|a| a.account_id == "111111111111"));
    assert!(accounts.iter().any(|a| a.account_id == "444444444444"));
}

// ---------------------------------------------------------------------------
// sso_get_role_credentials
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sso_get_role_credentials_returns_session() {
    // Expiration as milliseconds since epoch (year ~2030)
    let expiration_ms: i64 = 1_900_000_000_000;
    let body = format!(
        r#"{{"roleCredentials":{{"accessKeyId":"AKIAROLE","secretAccessKey":"rolesecret","sessionToken":"roketoken","expiration":{expiration_ms}}}}}"#
    );
    let replay = StaticReplayClient::new(vec![ReplayEvent::new(
        stub_request("https://portal.sso.us-east-1.amazonaws.com/federation/credentials"),
        http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(SdkBody::from(body.as_str()))
            .unwrap(),
    )]);

    let config = make_sdk_config(replay).await;
    let session = sso_get_role_credentials_with_config(
        &config,
        "access-token-test",
        "111111111111",
        "MyRole",
        "us-east-1",
    )
    .await
    .expect("get_role_credentials");

    use aws_credential_types::provider::ProvideCredentials as _;
    let creds = session.credentials.provide_credentials().await.unwrap();
    assert_eq!(creds.access_key_id(), "AKIAROLE");
    assert_eq!(creds.secret_access_key(), "rolesecret");
    assert_eq!(creds.session_token(), Some("roketoken"));
    assert_eq!(session.region, "us-east-1");
    assert_eq!(session.account_id, "111111111111");
    assert!(session.expires_at.is_some());
}

// ---------------------------------------------------------------------------
// authenticate_with_access_key (STS GetCallerIdentity — XML protocol)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authenticate_with_access_key_returns_session() {
    let body = r#"<GetCallerIdentityResponse xmlns="https://sts.amazonaws.com/doc/2011-06-15/">
  <GetCallerIdentityResult>
    <Arn>arn:aws:iam::123456789012:user/test-user</Arn>
    <UserId>AIDAEXAMPLE</UserId>
    <Account>123456789012</Account>
  </GetCallerIdentityResult>
  <ResponseMetadata><RequestId>req-1</RequestId></ResponseMetadata>
</GetCallerIdentityResponse>"#;
    let replay = StaticReplayClient::new(vec![ReplayEvent::new(
        stub_request("https://sts.amazonaws.com/"),
        http::Response::builder()
            .status(200)
            .header("content-type", "text/xml")
            .body(SdkBody::from(body))
            .unwrap(),
    )]);

    let config = make_sdk_config(replay).await;
    let access_key_config = AccessKeyConfig {
        access_key_id: "AKIAEXAMPLE".to_string(),
        secret_access_key: "secretkey".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };
    let session = authenticate_with_access_key_with_config(&config, &access_key_config)
        .await
        .expect("authenticate_with_access_key must succeed");

    assert_eq!(session.account_id, "123456789012");
    assert_eq!(session.identity_arn, "arn:aws:iam::123456789012:user/test-user");
    assert_eq!(session.region, "us-east-1");
}

// ---------------------------------------------------------------------------
// assume_role (STS AssumeRole — XML protocol)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn assume_role_returns_session_with_temporary_credentials() {
    let body = r#"<AssumeRoleResponse xmlns="https://sts.amazonaws.com/doc/2011-06-15/">
  <AssumeRoleResult>
    <AssumedRoleUser>
      <Arn>arn:aws:sts::123456789012:assumed-role/MyRole/session</Arn>
      <AssumedRoleId>AROAEXAMPLE:session</AssumedRoleId>
    </AssumedRoleUser>
    <Credentials>
      <AccessKeyId>ASIAEXAMPLE</AccessKeyId>
      <SecretAccessKey>temporarysecret</SecretAccessKey>
      <SessionToken>temporarytoken</SessionToken>
      <Expiration>2030-01-01T00:00:00Z</Expiration>
    </Credentials>
  </AssumeRoleResult>
  <ResponseMetadata><RequestId>req-2</RequestId></ResponseMetadata>
</AssumeRoleResponse>"#;
    let replay = StaticReplayClient::new(vec![ReplayEvent::new(
        stub_request("https://sts.amazonaws.com/"),
        http::Response::builder()
            .status(200)
            .header("content-type", "text/xml")
            .body(SdkBody::from(body))
            .unwrap(),
    )]);

    let config = make_sdk_config(replay).await;
    let assume_config = AssumeRoleConfig {
        role_arn: "arn:aws:iam::123456789012:role/MyRole".to_string(),
        external_id: None,
        session_name: Some("test-session".to_string()),
        region: "us-east-1".to_string(),
    };
    let session =
        assume_role_with_config(&config, &assume_config).await.expect("assume_role must succeed");

    assert_eq!(session.identity_arn, "arn:aws:sts::123456789012:assumed-role/MyRole/session");
    assert!(session.expires_at.is_some());
}

// ---------------------------------------------------------------------------
// discover_clusters_in_region (ListClusters + two DescribeCluster)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn discover_clusters_in_region_returns_described_clusters() {
    // Base64-encoded self-signed CA PEM (same fixture convention as aws_eks.rs:1150)
    let ca_data = "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0tCk1JSUJJRENDQVFXZ0F3SUJBZ0lVWWVQNitVa2tQdlZmemRtMTZsVWQ2M1lYWDFBd0NnWUlLb1pJemowRUF3SXcKRlRFVE1CRUdBMVVFQXd3S2JXa3RhWFFnVWtOQk1CNFhEVEkxTURnd01UQXdNREF3TUZvWERUSTFNRGt3TURBdwpNREF3TUZvd0ZURVRNQkVHQTFVRUF3d0tiV2t0YVhRZ1VrTkJNRmt3RXdZSEtvWkl6ajBDQVFZSUtvWkl6ajBECkFRY0RRZ0FFdHNldjIzdXZ6SkVsTHdnalpCeXAxdkRXaXVlVW9OYTN3UVhGZVgwMm4yR1ZsK2txclMraU9LcEsKaVR2MXRKR3Y2YWdPTEViOHlpSEZRMHFkdXZzcWFOME1Bb3dIUVlEVlIwT0JCWUVGTXJxWHpteGtoVlAxejhHCjFqRVJVZzMvRkh3TE1Bb0dBMVVkRXdFQi93UUlNQVlCQWY4Q0FRTXdDZ1lJS29aSXpqMEVBd0lEU0FBd1JRSWgKQU5Zd2VuNFh6ejhqNHg1bGJnM3Z1cXVLSW5CUER3Z3Rndzl1Q04reCtWRWtBaUFpS3VyUFVGM0N1bEdXbjBECjZjb3QvWndFNWkwZ0dGMHJjVTBsckQ5M3p3PT0KLS0tLS1FTkQgQ0VSVElGSUNBVEUtLS0tLQo=";
    let list_body =
        r#"{"clusters":["cluster-1","cluster-2"],"nextToken":null}"#.to_string();
    let describe1 = format!(
        r#"{{"cluster":{{"name":"cluster-1","arn":"arn:aws:eks:us-east-1:123456789012:cluster/cluster-1","endpoint":"https://ENDPOINT1.eks.amazonaws.com","version":"1.28","status":"ACTIVE","certificateAuthority":{{"data":"{ca_data}"}},"tags":{{}}}}}}"#
    );
    let describe2 = format!(
        r#"{{"cluster":{{"name":"cluster-2","arn":"arn:aws:eks:us-east-1:123456789012:cluster/cluster-2","endpoint":"https://ENDPOINT2.eks.amazonaws.com","version":"1.28","status":"ACTIVE","certificateAuthority":{{"data":"{ca_data}"}},"tags":{{}}}}}}"#
    );
    let replay = StaticReplayClient::new(vec![
        ReplayEvent::new(
            stub_request("https://eks.us-east-1.amazonaws.com/clusters"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(list_body.as_str()))
                .unwrap(),
        ),
        ReplayEvent::new(
            stub_request("https://eks.us-east-1.amazonaws.com/clusters/cluster-1"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(describe1.as_str()))
                .unwrap(),
        ),
        ReplayEvent::new(
            stub_request("https://eks.us-east-1.amazonaws.com/clusters/cluster-2"),
            http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(SdkBody::from(describe2.as_str()))
                .unwrap(),
        ),
    ]);

    let config = make_sdk_config(replay).await;
    let clusters = discover_clusters_in_region_with_config(&config, "us-east-1")
        .await
        .expect("discover_clusters_in_region must succeed");

    assert_eq!(clusters.len(), 2, "must return both clusters");
    assert!(clusters.iter().any(|c| c.name == "cluster-1" && c.region == "us-east-1"));
    assert!(clusters.iter().any(|c| c.name == "cluster-2" && c.region == "us-east-1"));
    // certificate_authority_data must be present
    assert!(clusters.iter().all(|c| c.certificate_authority_data.is_some()));
}
