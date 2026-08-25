use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Certificate,
    Token,
    OIDC,
    ExecPlugin,
    /// Native AWS EKS authentication (SSO, access keys, or assumed role).
    AwsEks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConnection {
    pub id: Uuid,
    pub name: String,
    pub context_name: String,
    pub api_server_url: String,
    pub status: ConnectionStatus,
    pub error_message: Option<String>,
    pub auth_method: AuthMethod,
    pub tls_verified: bool,
    pub last_connected: Option<DateTime<Utc>>,
    pub favorite: bool,
    pub reconnect_attempts: u32,
    pub max_reconnect_attempts: u32,
    pub token_expiry: Option<DateTime<Utc>>,
}

impl ClusterConnection {
    pub fn new(
        name: String,
        context_name: String,
        api_server_url: String,
        auth_method: AuthMethod,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            context_name,
            api_server_url,
            status: ConnectionStatus::Disconnected,
            error_message: None,
            auth_method,
            tls_verified: true,
            last_connected: None,
            favorite: false,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            token_expiry: None,
        }
    }

    pub fn set_connecting(&mut self) {
        self.status = ConnectionStatus::Connecting;
        self.error_message = None;
    }

    pub fn set_connected(&mut self) {
        self.status = ConnectionStatus::Connected;
        self.error_message = None;
        self.last_connected = Some(Utc::now());
    }

    pub fn set_error(&mut self, message: String) {
        self.status = ConnectionStatus::Error;
        self.error_message = Some(message);
    }

    pub fn set_disconnected(&mut self) {
        self.status = ConnectionStatus::Disconnected;
        self.error_message = None;
    }

    pub fn set_reconnecting(&mut self) {
        self.status = ConnectionStatus::Reconnecting;
        self.reconnect_attempts += 1;
    }

    pub fn is_connected(&self) -> bool {
        self.status == ConnectionStatus::Connected
    }

    /// Returns true if the connection can attempt another reconnection
    /// (hasn't exceeded max attempts).
    pub fn can_reconnect(&self) -> bool {
        self.reconnect_attempts < self.max_reconnect_attempts
    }

    /// Reset reconnect attempt counter (e.g., after a successful connection).
    pub fn reset_reconnect_attempts(&mut self) {
        self.reconnect_attempts = 0;
    }

    /// Check if the auth token has expired.
    pub fn is_token_expired(&self) -> bool {
        self.token_expiry.map(|expiry| Utc::now() >= expiry).unwrap_or(false)
    }

    /// Set the token expiry time.
    pub fn set_token_expiry(&mut self, expiry: DateTime<Utc>) {
        self.token_expiry = Some(expiry);
    }

    /// Returns re-authentication guidance based on the auth method.
    pub fn re_auth_guidance(&self) -> &'static str {
        match self.auth_method {
            AuthMethod::Certificate => {
                "Certificate may have expired. Renew the client certificate in your kubeconfig."
            }
            AuthMethod::Token => {
                "Bearer token has expired. Refresh the token in your kubeconfig or token file."
            }
            AuthMethod::OIDC => {
                "OIDC token has expired. Run your OIDC login flow to obtain a new token."
            }
            AuthMethod::ExecPlugin => {
                "Exec plugin credential has expired. Re-run the credential plugin (e.g., aws-iam-authenticator, gke-gcloud-auth-plugin)."
            }
            AuthMethod::AwsEks => {
                "AWS EKS credentials have expired. Re-authenticate via SSO or refresh your access keys in the EKS wizard."
            }
        }
    }
}

/// Manages multiple cluster connections and tracks the active cluster.
#[derive(Debug, Default)]
pub struct ClusterManager {
    connections: HashMap<Uuid, ClusterConnection>,
    active_cluster_id: Option<Uuid>,
}

impl ClusterManager {
    pub fn new() -> Self {
        Self { connections: HashMap::new(), active_cluster_id: None }
    }

    /// Add a connection to the manager. Returns its id.
    pub fn add_connection(&mut self, conn: ClusterConnection) -> Uuid {
        let id = conn.id;
        self.connections.insert(id, conn);
        id
    }

    /// Remove a connection by id. If the removed connection was the active
    /// cluster, the active cluster is cleared. Returns the removed connection
    /// if it existed.
    pub fn remove_connection(&mut self, id: &Uuid) -> Option<ClusterConnection> {
        let removed = self.connections.remove(id);
        if self.active_cluster_id.as_ref() == Some(id) {
            self.active_cluster_id = None;
        }
        removed
    }

    /// Get a reference to a connection by id.
    pub fn get_connection(&self, id: &Uuid) -> Option<&ClusterConnection> {
        self.connections.get(id)
    }

    /// Get a mutable reference to a connection by id.
    pub fn get_connection_mut(&mut self, id: &Uuid) -> Option<&mut ClusterConnection> {
        self.connections.get_mut(id)
    }

    /// List all connections.
    pub fn list_connections(&self) -> Vec<&ClusterConnection> {
        self.connections.values().collect()
    }

    /// Return only connections with `ConnectionStatus::Connected`.
    pub fn connected_clusters(&self) -> Vec<&ClusterConnection> {
        self.connections.values().filter(|c| c.is_connected()).collect()
    }

    /// Set the active cluster. Returns `true` if the id was found and set,
    /// `false` if no connection with that id exists.
    pub fn set_active(&mut self, id: &Uuid) -> bool {
        if self.connections.contains_key(id) {
            self.active_cluster_id = Some(*id);
            true
        } else {
            false
        }
    }

    /// Returns a reference to the currently active cluster connection, if any.
    pub fn active_cluster(&self) -> Option<&ClusterConnection> {
        self.active_cluster_id.as_ref().and_then(|id| self.connections.get(id))
    }

    /// Returns the number of managed connections.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Returns `true` if there are no managed connections.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Returns the id of the currently active cluster, if any.
    pub fn active_cluster_id(&self) -> Option<Uuid> {
        self.active_cluster_id
    }

    /// Disconnect all connected clusters by setting their status to Disconnected.
    pub fn disconnect_all(&mut self) {
        for conn in self.connections.values_mut() {
            if conn.status == ConnectionStatus::Connected
                || conn.status == ConnectionStatus::Connecting
                || conn.status == ConnectionStatus::Reconnecting
            {
                conn.set_disconnected();
            }
        }
    }

    /// Returns connections that have expired tokens and need re-authentication.
    pub fn connections_needing_reauth(&self) -> Vec<&ClusterConnection> {
        self.connections.values().filter(|c| c.is_token_expired()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cluster_connection_defaults() {
        let conn = ClusterConnection::new(
            "test-cluster".to_string(),
            "test-context".to_string(),
            "https://127.0.0.1:6443".to_string(),
            AuthMethod::Certificate,
        );

        assert_eq!(conn.name, "test-cluster");
        assert_eq!(conn.context_name, "test-context");
        assert_eq!(conn.api_server_url, "https://127.0.0.1:6443");
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        assert_eq!(conn.auth_method, AuthMethod::Certificate);
        assert!(conn.tls_verified);
        assert!(!conn.favorite);
        assert!(conn.error_message.is_none());
        assert!(conn.last_connected.is_none());
    }

    #[test]
    fn test_connection_lifecycle_disconnected_to_connected() {
        let mut conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::Token,
        );

        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        assert!(!conn.is_connected());

        conn.set_connecting();
        assert_eq!(conn.status, ConnectionStatus::Connecting);
        assert!(!conn.is_connected());

        conn.set_connected();
        assert_eq!(conn.status, ConnectionStatus::Connected);
        assert!(conn.is_connected());
        assert!(conn.last_connected.is_some());
    }

    #[test]
    fn test_connection_error_state() {
        let mut conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::OIDC,
        );

        conn.set_connecting();
        conn.set_error("connection refused".to_string());

        assert_eq!(conn.status, ConnectionStatus::Error);
        assert_eq!(conn.error_message.as_deref(), Some("connection refused"));
        assert!(!conn.is_connected());
    }

    #[test]
    fn test_error_to_disconnected() {
        let mut conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::ExecPlugin,
        );

        conn.set_error("timeout".to_string());
        assert!(conn.error_message.is_some());

        conn.set_disconnected();
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        assert!(conn.error_message.is_none());
    }

    #[test]
    fn test_connected_to_error() {
        let mut conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::Certificate,
        );

        conn.set_connected();
        assert!(conn.is_connected());
        let connected_time = conn.last_connected;

        conn.set_error("network unreachable".to_string());
        assert!(!conn.is_connected());
        assert_eq!(conn.last_connected, connected_time);
    }

    #[test]
    fn test_auth_method_variants() {
        assert_eq!(serde_json::to_string(&AuthMethod::Certificate).unwrap(), "\"Certificate\"");
        assert_eq!(serde_json::to_string(&AuthMethod::Token).unwrap(), "\"Token\"");
        assert_eq!(serde_json::to_string(&AuthMethod::OIDC).unwrap(), "\"OIDC\"");
        assert_eq!(serde_json::to_string(&AuthMethod::ExecPlugin).unwrap(), "\"ExecPlugin\"");
        assert_eq!(serde_json::to_string(&AuthMethod::AwsEks).unwrap(), "\"AwsEks\"");
    }

    #[test]
    fn test_connection_status_serialization() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::Token,
        );

        let json = serde_json::to_string(&conn).unwrap();
        let deserialized: ClusterConnection = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, conn.name);
        assert_eq!(deserialized.context_name, conn.context_name);
        assert_eq!(deserialized.status, conn.status);
        assert_eq!(deserialized.auth_method, conn.auth_method);
    }

    // ---------- ClusterManager tests ----------

    fn make_conn(name: &str) -> ClusterConnection {
        ClusterConnection::new(
            name.to_string(),
            format!("{name}-ctx"),
            format!("https://{name}.example.com:6443"),
            AuthMethod::Token,
        )
    }

    #[test]
    fn test_cluster_manager_new_is_empty() {
        let mgr = ClusterManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
        assert!(mgr.active_cluster().is_none());
        assert!(mgr.list_connections().is_empty());
        assert!(mgr.connected_clusters().is_empty());
    }

    #[test]
    fn test_cluster_manager_default_is_empty() {
        let mgr = ClusterManager::default();
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_add_and_get_connection() {
        let mut mgr = ClusterManager::new();
        let conn = make_conn("prod");
        let id = mgr.add_connection(conn);

        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());

        let fetched = mgr.get_connection(&id).unwrap();
        assert_eq!(fetched.name, "prod");
    }

    #[test]
    fn test_add_multiple_connections() {
        let mut mgr = ClusterManager::new();
        mgr.add_connection(make_conn("prod"));
        mgr.add_connection(make_conn("staging"));
        mgr.add_connection(make_conn("dev"));

        assert_eq!(mgr.len(), 3);
        assert_eq!(mgr.list_connections().len(), 3);
    }

    #[test]
    fn test_remove_connection() {
        let mut mgr = ClusterManager::new();
        let id = mgr.add_connection(make_conn("prod"));

        let removed = mgr.remove_connection(&id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "prod");
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_connection() {
        let mut mgr = ClusterManager::new();
        let fake_id = Uuid::new_v4();
        assert!(mgr.remove_connection(&fake_id).is_none());
    }

    #[test]
    fn test_remove_active_cluster_clears_active() {
        let mut mgr = ClusterManager::new();
        let id = mgr.add_connection(make_conn("prod"));
        mgr.set_active(&id);
        assert!(mgr.active_cluster().is_some());

        mgr.remove_connection(&id);
        assert!(mgr.active_cluster().is_none());
    }

    #[test]
    fn test_set_active_valid() {
        let mut mgr = ClusterManager::new();
        let id = mgr.add_connection(make_conn("prod"));

        assert!(mgr.set_active(&id));
        let active = mgr.active_cluster().unwrap();
        assert_eq!(active.name, "prod");
    }

    #[test]
    fn test_set_active_invalid() {
        let mut mgr = ClusterManager::new();
        let fake_id = Uuid::new_v4();
        assert!(!mgr.set_active(&fake_id));
        assert!(mgr.active_cluster().is_none());
    }

    #[test]
    fn test_switch_active_cluster() {
        let mut mgr = ClusterManager::new();
        let id1 = mgr.add_connection(make_conn("prod"));
        let id2 = mgr.add_connection(make_conn("staging"));

        mgr.set_active(&id1);
        assert_eq!(mgr.active_cluster().unwrap().name, "prod");

        mgr.set_active(&id2);
        assert_eq!(mgr.active_cluster().unwrap().name, "staging");
    }

    #[test]
    fn test_connected_clusters_filters_correctly() {
        let mut mgr = ClusterManager::new();

        let mut conn1 = make_conn("prod");
        conn1.set_connected();
        mgr.add_connection(conn1);

        let conn2 = make_conn("staging"); // disconnected
        mgr.add_connection(conn2);

        let mut conn3 = make_conn("dev");
        conn3.set_connected();
        mgr.add_connection(conn3);

        let connected = mgr.connected_clusters();
        assert_eq!(connected.len(), 2);
        assert!(connected.iter().all(|c| c.is_connected()));
    }

    #[test]
    fn test_get_connection_mut() {
        let mut mgr = ClusterManager::new();
        let id = mgr.add_connection(make_conn("prod"));

        {
            let conn = mgr.get_connection_mut(&id).unwrap();
            conn.set_connecting();
        }
        assert_eq!(mgr.get_connection(&id).unwrap().status, ConnectionStatus::Connecting,);

        {
            let conn = mgr.get_connection_mut(&id).unwrap();
            conn.set_connected();
        }
        assert!(mgr.get_connection(&id).unwrap().is_connected());
    }

    // --- T038: Reconnection and auth expiry tests ---

    #[test]
    fn test_reconnecting_state() {
        let mut conn = make_conn("prod");
        conn.set_connected();
        conn.set_reconnecting();
        assert_eq!(conn.status, ConnectionStatus::Reconnecting);
        assert_eq!(conn.reconnect_attempts, 1);
    }

    #[test]
    fn test_reconnect_attempt_counting() {
        let mut conn = make_conn("prod");
        conn.set_connected();

        // Simulate multiple reconnection attempts
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 1);
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 2);
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 3);
    }

    #[test]
    fn test_can_reconnect_within_limit() {
        let mut conn = make_conn("prod");
        assert!(conn.can_reconnect());

        for _ in 0..4 {
            conn.set_reconnecting();
        }
        assert_eq!(conn.reconnect_attempts, 4);
        assert!(conn.can_reconnect()); // 4 < 5 (default max)
    }

    #[test]
    fn test_cannot_reconnect_at_limit() {
        let mut conn = make_conn("prod");
        for _ in 0..5 {
            conn.set_reconnecting();
        }
        assert_eq!(conn.reconnect_attempts, 5);
        assert!(!conn.can_reconnect()); // 5 >= 5
    }

    #[test]
    fn test_reset_reconnect_attempts() {
        let mut conn = make_conn("prod");
        conn.set_reconnecting();
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 2);

        conn.reset_reconnect_attempts();
        assert_eq!(conn.reconnect_attempts, 0);
        assert!(conn.can_reconnect());
    }

    #[test]
    fn test_successful_reconnect_resets_attempts() {
        let mut conn = make_conn("prod");
        conn.set_connected();
        // Simulate losing connection and reconnecting
        conn.set_reconnecting();
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 2);

        // Successful reconnection
        conn.set_connected();
        conn.reset_reconnect_attempts();
        assert_eq!(conn.reconnect_attempts, 0);
        assert!(conn.is_connected());
    }

    #[test]
    fn test_token_expiry_not_set() {
        let conn = make_conn("prod");
        assert!(!conn.is_token_expired());
        assert!(conn.token_expiry.is_none());
    }

    #[test]
    fn test_token_expired() {
        let mut conn = make_conn("prod");
        // Set expiry to 1 hour ago
        conn.set_token_expiry(Utc::now() - chrono::Duration::hours(1));
        assert!(conn.is_token_expired());
    }

    #[test]
    fn test_token_not_expired() {
        let mut conn = make_conn("prod");
        // Set expiry to 1 hour from now
        conn.set_token_expiry(Utc::now() + chrono::Duration::hours(1));
        assert!(!conn.is_token_expired());
    }

    #[test]
    fn test_re_auth_guidance_certificate() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::Certificate,
        );
        let guidance = conn.re_auth_guidance();
        assert!(guidance.contains("Certificate"));
        assert!(guidance.contains("kubeconfig"));
    }

    #[test]
    fn test_re_auth_guidance_token() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::Token,
        );
        let guidance = conn.re_auth_guidance();
        assert!(guidance.contains("Bearer token"));
    }

    #[test]
    fn test_re_auth_guidance_oidc() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::OIDC,
        );
        let guidance = conn.re_auth_guidance();
        assert!(guidance.contains("OIDC"));
    }

    #[test]
    fn test_re_auth_guidance_exec_plugin() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::ExecPlugin,
        );
        let guidance = conn.re_auth_guidance();
        assert!(guidance.contains("Exec plugin"));
    }

    #[test]
    fn test_re_auth_guidance_aws_eks() {
        let conn = ClusterConnection::new(
            "test".to_string(),
            "ctx".to_string(),
            "https://localhost:6443".to_string(),
            AuthMethod::AwsEks,
        );
        let guidance = conn.re_auth_guidance();
        assert!(guidance.contains("AWS EKS"));
    }

    #[test]
    fn test_full_reconnection_lifecycle() {
        let mut conn = make_conn("prod");

        // Connect successfully
        conn.set_connecting();
        conn.set_connected();
        assert!(conn.is_connected());

        // Connection drops
        conn.set_error("network timeout".to_string());
        assert!(!conn.is_connected());

        // First reconnection attempt
        assert!(conn.can_reconnect());
        conn.set_reconnecting();
        assert_eq!(conn.reconnect_attempts, 1);

        // Reconnection succeeds
        conn.set_connected();
        conn.reset_reconnect_attempts();
        assert!(conn.is_connected());
        assert_eq!(conn.reconnect_attempts, 0);
    }

    #[test]
    fn test_reconnection_exhaustion() {
        let mut conn = make_conn("prod");
        conn.set_connected();
        conn.set_error("connection lost".to_string());

        // Exhaust all reconnection attempts
        for i in 0..5 {
            assert!(conn.can_reconnect(), "Should be able to reconnect at attempt {i}");
            conn.set_reconnecting();
        }

        // No more attempts
        assert!(!conn.can_reconnect());
        assert_eq!(conn.reconnect_attempts, 5);

        // Should transition to error
        conn.set_error("max reconnection attempts reached".to_string());
        assert_eq!(conn.status, ConnectionStatus::Error);
    }

    // --- T039: Additional multi-cluster management tests ---

    #[test]
    fn test_disconnect_all() {
        let mut mgr = ClusterManager::new();

        let mut conn1 = make_conn("prod");
        conn1.set_connected();
        mgr.add_connection(conn1);

        let mut conn2 = make_conn("staging");
        conn2.set_connecting();
        mgr.add_connection(conn2);

        let conn3 = make_conn("dev"); // disconnected
        mgr.add_connection(conn3);

        mgr.disconnect_all();

        for conn in mgr.list_connections() {
            assert_eq!(conn.status, ConnectionStatus::Disconnected);
        }
        assert!(mgr.connected_clusters().is_empty());
    }

    #[test]
    fn test_active_cluster_id() {
        let mut mgr = ClusterManager::new();
        assert!(mgr.active_cluster_id().is_none());

        let id = mgr.add_connection(make_conn("prod"));
        mgr.set_active(&id);
        assert_eq!(mgr.active_cluster_id(), Some(id));
    }

    #[test]
    fn test_connections_needing_reauth_none() {
        let mut mgr = ClusterManager::new();
        mgr.add_connection(make_conn("prod"));
        mgr.add_connection(make_conn("staging"));

        assert!(mgr.connections_needing_reauth().is_empty());
    }

    #[test]
    fn test_connections_needing_reauth_some_expired() {
        let mut mgr = ClusterManager::new();

        let mut conn1 = make_conn("prod");
        conn1.set_token_expiry(Utc::now() - chrono::Duration::hours(1)); // expired
        let id1 = mgr.add_connection(conn1);

        let mut conn2 = make_conn("staging");
        conn2.set_token_expiry(Utc::now() + chrono::Duration::hours(1)); // not expired
        mgr.add_connection(conn2);

        let expired = mgr.connections_needing_reauth();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, id1);
    }

    #[test]
    fn test_multi_cluster_concurrent_connections() {
        let mut mgr = ClusterManager::new();

        // Connect multiple clusters simultaneously
        let mut conn1 = make_conn("us-east");
        conn1.set_connected();
        let id1 = mgr.add_connection(conn1);

        let mut conn2 = make_conn("us-west");
        conn2.set_connected();
        let id2 = mgr.add_connection(conn2);

        let mut conn3 = make_conn("eu-central");
        conn3.set_connected();
        let id3 = mgr.add_connection(conn3);

        assert_eq!(mgr.connected_clusters().len(), 3);

        // Switch between active clusters
        mgr.set_active(&id1);
        assert_eq!(mgr.active_cluster().unwrap().name, "us-east");

        mgr.set_active(&id3);
        assert_eq!(mgr.active_cluster().unwrap().name, "eu-central");

        // Disconnect one cluster doesn't affect others
        mgr.get_connection_mut(&id2).unwrap().set_disconnected();
        assert_eq!(mgr.connected_clusters().len(), 2);
        assert_eq!(mgr.active_cluster().unwrap().name, "eu-central");
    }

    #[test]
    fn test_disconnect_active_while_others_connected() {
        let mut mgr = ClusterManager::new();

        let mut conn1 = make_conn("prod");
        conn1.set_connected();
        let id1 = mgr.add_connection(conn1);

        let mut conn2 = make_conn("staging");
        conn2.set_connected();
        mgr.add_connection(conn2);

        mgr.set_active(&id1);

        // Remove the active cluster
        mgr.remove_connection(&id1);
        assert!(mgr.active_cluster().is_none());
        assert_eq!(mgr.connected_clusters().len(), 1);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_full_lifecycle_connect_switch_disconnect() {
        let mut mgr = ClusterManager::new();

        // Add two clusters
        let prod_id = mgr.add_connection(make_conn("prod"));
        let dev_id = mgr.add_connection(make_conn("dev"));
        assert_eq!(mgr.len(), 2);

        // Connect prod
        mgr.get_connection_mut(&prod_id).unwrap().set_connecting();
        mgr.get_connection_mut(&prod_id).unwrap().set_connected();
        mgr.set_active(&prod_id);
        assert_eq!(mgr.active_cluster().unwrap().name, "prod");
        assert_eq!(mgr.connected_clusters().len(), 1);

        // Connect dev and switch
        mgr.get_connection_mut(&dev_id).unwrap().set_connecting();
        mgr.get_connection_mut(&dev_id).unwrap().set_connected();
        mgr.set_active(&dev_id);
        assert_eq!(mgr.active_cluster().unwrap().name, "dev");
        assert_eq!(mgr.connected_clusters().len(), 2);

        // Disconnect prod
        mgr.get_connection_mut(&prod_id).unwrap().set_disconnected();
        assert_eq!(mgr.connected_clusters().len(), 1);

        // Remove dev (active) => active cleared
        mgr.remove_connection(&dev_id);
        assert!(mgr.active_cluster().is_none());
        assert_eq!(mgr.len(), 1);
    }
}
