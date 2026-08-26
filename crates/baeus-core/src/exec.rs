use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecSessionState {
    Idle,
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub cluster_id: Uuid,
    pub namespace: String,
    pub pod_name: String,
    pub container_name: Option<String>,
    pub command: Vec<String>,
    pub tty: bool,
    pub stdin: bool,
}

impl ExecConfig {
    pub fn shell(cluster_id: Uuid, namespace: String, pod_name: String) -> Self {
        Self {
            cluster_id,
            namespace,
            pod_name,
            container_name: None,
            command: vec!["/bin/sh".to_string()],
            tty: true,
            stdin: true,
        }
    }

    pub fn with_container(mut self, name: String) -> Self {
        self.container_name = Some(name);
        self
    }

    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }
}

#[derive(Debug)]
pub struct ExecSession {
    pub id: Uuid,
    pub config: ExecConfig,
    pub state: ExecSessionState,
}

impl ExecSession {
    pub fn new(config: ExecConfig) -> Self {
        Self { id: Uuid::new_v4(), config, state: ExecSessionState::Idle }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, ExecSessionState::Connecting | ExecSessionState::Connected)
    }

    pub fn connect(&mut self) {
        self.state = ExecSessionState::Connecting;
    }

    pub fn set_connected(&mut self) {
        self.state = ExecSessionState::Connected;
    }

    pub fn disconnect(&mut self) {
        self.state = ExecSessionState::Disconnected;
    }

    pub fn set_error(&mut self, error: String) {
        self.state = ExecSessionState::Error(error);
    }
}

#[derive(Debug, Default)]
pub struct ExecManager {
    sessions: Vec<ExecSession>,
}

impl ExecManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, config: ExecConfig) -> Uuid {
        let session = ExecSession::new(config);
        let id = session.id;
        self.sessions.push(session);
        id
    }

    pub fn get_session(&self, id: Uuid) -> Option<&ExecSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn get_session_mut(&mut self, id: Uuid) -> Option<&mut ExecSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn remove_session(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn active_sessions(&self) -> Vec<&ExecSession> {
        self.sessions.iter().filter(|s| s.is_active()).collect()
    }

    pub fn sessions_for_pod(&self, pod_name: &str, namespace: &str) -> Vec<&ExecSession> {
        self.sessions
            .iter()
            .filter(|s| s.config.pod_name == pod_name && s.config.namespace == namespace)
            .collect()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn disconnect_all(&mut self) {
        for session in &mut self.sessions {
            session.disconnect();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortForwardState {
    Active,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForwardConfig {
    pub cluster_id: Uuid,
    pub namespace: String,
    pub pod_name: String,
    pub local_port: u16,
    pub remote_port: u16,
}

#[derive(Debug)]
pub struct PortForwardSession {
    pub id: Uuid,
    pub config: PortForwardConfig,
    pub state: PortForwardState,
}

impl PortForwardSession {
    pub fn new(config: PortForwardConfig) -> Self {
        Self { id: Uuid::new_v4(), config, state: PortForwardState::Active }
    }

    pub fn is_active(&self) -> bool {
        self.state == PortForwardState::Active
    }

    pub fn stop(&mut self) {
        self.state = PortForwardState::Stopped;
    }

    pub fn set_error(&mut self, error: &str) {
        self.state = PortForwardState::Error(error.to_string());
    }
}

#[derive(Debug, Default)]
pub struct PortForwardManager {
    sessions: Vec<PortForwardSession>,
}

impl PortForwardManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, config: PortForwardConfig) -> Uuid {
        let session = PortForwardSession::new(config);
        let id = session.id;
        self.sessions.push(session);
        id
    }

    pub fn remove(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn get(&self, id: Uuid) -> Option<&PortForwardSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn active_sessions(&self) -> Vec<&PortForwardSession> {
        self.sessions.iter().filter(|s| s.is_active()).collect()
    }

    pub fn sessions_for_pod(&self, pod_name: &str, namespace: &str) -> Vec<&PortForwardSession> {
        self.sessions
            .iter()
            .filter(|s| s.config.pod_name == pod_name && s.config.namespace == namespace)
            .collect()
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_port_in_use(&self, local_port: u16) -> bool {
        self.sessions.iter().any(|s| s.config.local_port == local_port && s.is_active())
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut PortForwardSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn stop_session(&mut self, id: Uuid) -> bool {
        if let Some(session) = self.get_mut(id) {
            session.stop();
            true
        } else {
            false
        }
    }

    pub fn set_session_error(&mut self, id: Uuid, error: &str) -> bool {
        if let Some(session) = self.get_mut(id) {
            session.set_error(error);
            true
        } else {
            false
        }
    }

    pub fn stop_all(&mut self) {
        for session in &mut self.sessions {
            session.stop();
        }
    }

    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.is_active()).count()
    }

    pub fn sessions_for_cluster(&self, cluster_id: &Uuid) -> Vec<&PortForwardSession> {
        self.sessions.iter().filter(|s| s.config.cluster_id == *cluster_id).collect()
    }

    pub fn reconnect_session(&mut self, id: Uuid) -> Option<Uuid> {
        let config = {
            let session = self.sessions.iter().find(|s| s.id == id)?;
            session.config.clone()
        };
        self.stop_session(id);
        let new_id = self.add(config);
        Some(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_config_shell() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        assert_eq!(config.command, vec!["/bin/sh"]);
        assert!(config.tty);
        assert!(config.stdin);
        assert!(config.container_name.is_none());
    }

    #[test]
    fn test_exec_config_with_container() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string())
            .with_container("sidecar".to_string());
        assert_eq!(config.container_name.as_deref(), Some("sidecar"));
    }

    #[test]
    fn test_exec_config_with_command() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string())
            .with_command(vec!["ls".to_string(), "-la".to_string()]);
        assert_eq!(config.command, vec!["ls", "-la"]);
    }

    #[test]
    fn test_exec_session_lifecycle() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        let mut session = ExecSession::new(config);

        assert_eq!(session.state, ExecSessionState::Idle);
        assert!(!session.is_active());

        session.state = ExecSessionState::Connecting;
        assert!(session.is_active());

        session.state = ExecSessionState::Connected;
        assert!(session.is_active());

        session.state = ExecSessionState::Disconnected;
        assert!(!session.is_active());
    }

    #[test]
    fn test_port_forward_manager() {
        let mut manager = PortForwardManager::new();
        assert_eq!(manager.count(), 0);

        let config = PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        };

        let id = manager.add(config);
        assert_eq!(manager.count(), 1);
        assert!(manager.get(id).is_some());
        assert!(manager.get(id).unwrap().is_active());
    }

    #[test]
    fn test_port_forward_remove() {
        let mut manager = PortForwardManager::new();
        let id = manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.remove(id));
        assert_eq!(manager.count(), 0);
        assert!(!manager.remove(id)); // already removed
    }

    #[test]
    fn test_port_forward_port_in_use() {
        let mut manager = PortForwardManager::new();
        manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.is_port_in_use(8080));
        assert!(!manager.is_port_in_use(9090));
    }

    #[test]
    fn test_port_forward_sessions_for_pod() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8443,
            remote_port: 443,
        });
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });

        assert_eq!(manager.sessions_for_pod("nginx", "default").len(), 2);
        assert_eq!(manager.sessions_for_pod("redis", "default").len(), 1);
        assert_eq!(manager.sessions_for_pod("missing", "default").len(), 0);
    }

    // ===== T067: ExecConfig builder pattern tests =====

    #[test]
    fn test_exec_config_shell_preserves_cluster_and_pod() {
        let cluster_id = Uuid::new_v4();
        let config =
            ExecConfig::shell(cluster_id, "kube-system".to_string(), "coredns-abc123".to_string());
        assert_eq!(config.cluster_id, cluster_id);
        assert_eq!(config.namespace, "kube-system");
        assert_eq!(config.pod_name, "coredns-abc123");
    }

    #[test]
    fn test_exec_config_chained_builder() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string())
            .with_container("istio-proxy".to_string())
            .with_command(vec!["curl".to_string(), "localhost:15000".to_string()]);

        assert_eq!(config.container_name.as_deref(), Some("istio-proxy"));
        assert_eq!(config.command, vec!["curl", "localhost:15000"]);
        assert!(config.tty);
        assert!(config.stdin);
    }

    #[test]
    fn test_exec_config_with_command_overrides_shell_default() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string())
            .with_command(vec!["/bin/bash".to_string()]);

        assert_eq!(config.command, vec!["/bin/bash"]);
    }

    #[test]
    fn test_exec_config_multiple_containers() {
        let cluster_id = Uuid::new_v4();
        let config_a =
            ExecConfig::shell(cluster_id, "default".to_string(), "multi-container-pod".to_string())
                .with_container("app".to_string());

        let config_b =
            ExecConfig::shell(cluster_id, "default".to_string(), "multi-container-pod".to_string())
                .with_container("sidecar".to_string());

        assert_eq!(config_a.container_name.as_deref(), Some("app"));
        assert_eq!(config_b.container_name.as_deref(), Some("sidecar"));
        assert_eq!(config_a.pod_name, config_b.pod_name);
    }

    // ===== T067: ExecSession state transition tests =====

    #[test]
    fn test_exec_session_state_transitions_with_methods() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        let mut session = ExecSession::new(config);

        assert_eq!(session.state, ExecSessionState::Idle);
        assert!(!session.is_active());

        session.connect();
        assert_eq!(session.state, ExecSessionState::Connecting);
        assert!(session.is_active());

        session.set_connected();
        assert_eq!(session.state, ExecSessionState::Connected);
        assert!(session.is_active());

        session.disconnect();
        assert_eq!(session.state, ExecSessionState::Disconnected);
        assert!(!session.is_active());
    }

    #[test]
    fn test_exec_session_error_state() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        let mut session = ExecSession::new(config);

        session.connect();
        session.set_error("connection refused".to_string());

        assert_eq!(session.state, ExecSessionState::Error("connection refused".to_string()));
        assert!(!session.is_active());
    }

    #[test]
    fn test_exec_session_error_from_connected() {
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        let mut session = ExecSession::new(config);

        session.connect();
        session.set_connected();
        session.set_error("stream closed unexpectedly".to_string());

        assert_eq!(
            session.state,
            ExecSessionState::Error("stream closed unexpectedly".to_string())
        );
        assert!(!session.is_active());
    }

    #[test]
    fn test_exec_session_unique_ids() {
        let config_a =
            ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());
        let config_b =
            ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());

        let session_a = ExecSession::new(config_a);
        let session_b = ExecSession::new(config_b);

        assert_ne!(session_a.id, session_b.id);
    }

    #[test]
    fn test_multiple_exec_sessions_simultaneously() {
        let cluster_id = Uuid::new_v4();
        let mut session_a = ExecSession::new(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-a".to_string(),
        ));
        let mut session_b = ExecSession::new(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-b".to_string(),
        ));
        let mut session_c = ExecSession::new(ExecConfig::shell(
            cluster_id,
            "kube-system".to_string(),
            "coredns".to_string(),
        ));

        session_a.connect();
        session_a.set_connected();
        session_b.connect();
        session_c.connect();
        session_c.set_error("timeout".to_string());

        assert!(session_a.is_active());
        assert!(session_b.is_active());
        assert!(!session_c.is_active());
        assert_eq!(session_a.state, ExecSessionState::Connected);
        assert_eq!(session_b.state, ExecSessionState::Connecting);
        assert_eq!(session_c.state, ExecSessionState::Error("timeout".to_string()));
    }

    // ===== T072: ExecManager tests =====

    #[test]
    fn test_exec_manager_new() {
        let manager = ExecManager::new();
        assert_eq!(manager.session_count(), 0);
        assert!(manager.active_sessions().is_empty());
    }

    #[test]
    fn test_exec_manager_create_session() {
        let mut manager = ExecManager::new();
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());

        let id = manager.create_session(config);
        assert_eq!(manager.session_count(), 1);
        assert!(manager.get_session(id).is_some());
        assert_eq!(manager.get_session(id).unwrap().state, ExecSessionState::Idle);
    }

    #[test]
    fn test_exec_manager_get_session_mut() {
        let mut manager = ExecManager::new();
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());

        let id = manager.create_session(config);
        manager.get_session_mut(id).unwrap().connect();
        assert_eq!(manager.get_session(id).unwrap().state, ExecSessionState::Connecting);
    }

    #[test]
    fn test_exec_manager_remove_session() {
        let mut manager = ExecManager::new();
        let config = ExecConfig::shell(Uuid::new_v4(), "default".to_string(), "nginx".to_string());

        let id = manager.create_session(config);
        assert!(manager.remove_session(id));
        assert_eq!(manager.session_count(), 0);
        assert!(manager.get_session(id).is_none());
    }

    #[test]
    fn test_exec_manager_remove_nonexistent() {
        let mut manager = ExecManager::new();
        assert!(!manager.remove_session(Uuid::new_v4()));
    }

    #[test]
    fn test_exec_manager_active_sessions() {
        let mut manager = ExecManager::new();
        let cluster_id = Uuid::new_v4();

        let id_a = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-a".to_string(),
        ));
        let id_b = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-b".to_string(),
        ));
        let _id_c = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-c".to_string(),
        ));

        manager.get_session_mut(id_a).unwrap().connect();
        manager.get_session_mut(id_a).unwrap().set_connected();
        manager.get_session_mut(id_b).unwrap().connect();
        // id_c stays Idle

        let active = manager.active_sessions();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_exec_manager_sessions_for_pod() {
        let mut manager = ExecManager::new();
        let cluster_id = Uuid::new_v4();

        manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "nginx".to_string(),
        ));
        manager.create_session(
            ExecConfig::shell(cluster_id, "default".to_string(), "nginx".to_string())
                .with_container("sidecar".to_string()),
        );
        manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "redis".to_string(),
        ));
        manager.create_session(ExecConfig::shell(
            cluster_id,
            "kube-system".to_string(),
            "nginx".to_string(),
        ));

        assert_eq!(manager.sessions_for_pod("nginx", "default").len(), 2);
        assert_eq!(manager.sessions_for_pod("redis", "default").len(), 1);
        assert_eq!(manager.sessions_for_pod("nginx", "kube-system").len(), 1);
        assert_eq!(manager.sessions_for_pod("missing", "default").len(), 0);
    }

    #[test]
    fn test_exec_manager_disconnect_all() {
        let mut manager = ExecManager::new();
        let cluster_id = Uuid::new_v4();

        let id_a = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-a".to_string(),
        ));
        let id_b = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-b".to_string(),
        ));
        let id_c = manager.create_session(ExecConfig::shell(
            cluster_id,
            "default".to_string(),
            "pod-c".to_string(),
        ));

        manager.get_session_mut(id_a).unwrap().connect();
        manager.get_session_mut(id_a).unwrap().set_connected();
        manager.get_session_mut(id_b).unwrap().connect();
        // id_c stays Idle

        manager.disconnect_all();

        assert_eq!(manager.get_session(id_a).unwrap().state, ExecSessionState::Disconnected);
        assert_eq!(manager.get_session(id_b).unwrap().state, ExecSessionState::Disconnected);
        assert_eq!(manager.get_session(id_c).unwrap().state, ExecSessionState::Disconnected);
        assert!(manager.active_sessions().is_empty());
    }

    #[test]
    fn test_exec_manager_get_nonexistent_session() {
        let manager = ExecManager::new();
        assert!(manager.get_session(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_exec_manager_get_mut_nonexistent_session() {
        let mut manager = ExecManager::new();
        assert!(manager.get_session_mut(Uuid::new_v4()).is_none());
    }

    // ===== T067a: PortForwardManager extended tests =====

    #[test]
    fn test_port_forward_manager_add_multiple_sessions() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        let id_a = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        let id_b = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });
        let id_c = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "monitoring".to_string(),
            pod_name: "grafana".to_string(),
            local_port: 3000,
            remote_port: 3000,
        });

        assert_eq!(manager.count(), 3);
        assert_ne!(id_a, id_b);
        assert_ne!(id_b, id_c);
        assert!(manager.get(id_a).is_some());
        assert!(manager.get(id_b).is_some());
        assert!(manager.get(id_c).is_some());
    }

    #[test]
    fn test_port_forward_manager_remove_and_get() {
        let mut manager = PortForwardManager::new();
        let id = manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.get(id).is_some());
        assert!(manager.remove(id));
        assert!(manager.get(id).is_none());
        assert!(!manager.remove(id));
    }

    #[test]
    fn test_port_forward_port_collision_detection() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.is_port_in_use(8080));
        assert!(!manager.is_port_in_use(8081));
        assert!(!manager.is_port_in_use(3000));
    }

    #[test]
    fn test_port_forward_port_not_in_use_when_stopped() {
        let mut manager = PortForwardManager::new();
        let id = manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.is_port_in_use(8080));
        manager.stop_session(id);
        assert!(!manager.is_port_in_use(8080));
    }

    #[test]
    fn test_port_forward_sessions_filter_by_namespace() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "staging".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8081,
            remote_port: 80,
        });

        assert_eq!(manager.sessions_for_pod("nginx", "default").len(), 1);
        assert_eq!(manager.sessions_for_pod("nginx", "staging").len(), 1);
        assert_eq!(manager.sessions_for_pod("nginx", "production").len(), 0);
    }

    // ===== T067a: PortForwardSession state transition tests =====

    #[test]
    fn test_port_forward_session_stop() {
        let config = PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        };
        let mut session = PortForwardSession::new(config);

        assert!(session.is_active());
        assert_eq!(session.state, PortForwardState::Active);

        session.stop();
        assert!(!session.is_active());
        assert_eq!(session.state, PortForwardState::Stopped);
    }

    #[test]
    fn test_port_forward_session_error() {
        let config = PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        };
        let mut session = PortForwardSession::new(config);

        session.set_error("connection reset by peer");
        assert!(!session.is_active());
        assert_eq!(session.state, PortForwardState::Error("connection reset by peer".to_string()));
    }

    // ===== T067a: Reconnect workflow test =====

    #[test]
    fn test_port_forward_reconnect_workflow() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();
        let original_id = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        // Simulate error then reconnect
        manager.set_session_error(original_id, "broken pipe");
        assert!(!manager.get(original_id).unwrap().is_active());

        let new_id = manager.reconnect_session(original_id).unwrap();
        assert_ne!(original_id, new_id);

        // Old session should be stopped
        let old = manager.get(original_id).unwrap();
        assert_eq!(old.state, PortForwardState::Stopped);

        // New session should be active with same config
        let new = manager.get(new_id).unwrap();
        assert!(new.is_active());
        assert_eq!(new.config.local_port, 8080);
        assert_eq!(new.config.remote_port, 80);
        assert_eq!(new.config.pod_name, "nginx");
        assert_eq!(new.config.namespace, "default");
    }

    // ===== T075a: PortForwardManager enhancement tests =====

    #[test]
    fn test_port_forward_manager_stop_session() {
        let mut manager = PortForwardManager::new();
        let id = manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.stop_session(id));
        assert_eq!(manager.get(id).unwrap().state, PortForwardState::Stopped);
        assert!(!manager.stop_session(Uuid::new_v4()));
    }

    #[test]
    fn test_port_forward_manager_set_session_error() {
        let mut manager = PortForwardManager::new();
        let id = manager.add(PortForwardConfig {
            cluster_id: Uuid::new_v4(),
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        assert!(manager.set_session_error(id, "port already bound"));
        assert_eq!(
            manager.get(id).unwrap().state,
            PortForwardState::Error("port already bound".to_string())
        );
        assert!(!manager.set_session_error(Uuid::new_v4(), "no such session"));
    }

    #[test]
    fn test_port_forward_manager_stop_all() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        let id_a = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        let id_b = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });

        assert_eq!(manager.active_count(), 2);

        manager.stop_all();

        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.get(id_a).unwrap().state, PortForwardState::Stopped);
        assert_eq!(manager.get(id_b).unwrap().state, PortForwardState::Stopped);
        assert!(manager.active_sessions().is_empty());
    }

    #[test]
    fn test_port_forward_manager_active_count() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        assert_eq!(manager.active_count(), 0);

        let id_a = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });
        manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "postgres".to_string(),
            local_port: 5432,
            remote_port: 5432,
        });

        assert_eq!(manager.active_count(), 3);

        manager.stop_session(id_a);
        assert_eq!(manager.active_count(), 2);
    }

    #[test]
    fn test_port_forward_manager_sessions_for_cluster() {
        let mut manager = PortForwardManager::new();
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();

        manager.add(PortForwardConfig {
            cluster_id: cluster_a,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        manager.add(PortForwardConfig {
            cluster_id: cluster_a,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });
        manager.add(PortForwardConfig {
            cluster_id: cluster_b,
            namespace: "default".to_string(),
            pod_name: "postgres".to_string(),
            local_port: 5432,
            remote_port: 5432,
        });

        assert_eq!(manager.sessions_for_cluster(&cluster_a).len(), 2);
        assert_eq!(manager.sessions_for_cluster(&cluster_b).len(), 1);
        assert_eq!(manager.sessions_for_cluster(&Uuid::new_v4()).len(), 0);
    }

    #[test]
    fn test_port_forward_manager_reconnect_session() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        let original_id = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });

        let new_id = manager.reconnect_session(original_id).unwrap();
        assert_ne!(original_id, new_id);

        // Old session is stopped
        assert_eq!(manager.get(original_id).unwrap().state, PortForwardState::Stopped);

        // New session is active with identical config
        let new_session = manager.get(new_id).unwrap();
        assert!(new_session.is_active());
        assert_eq!(new_session.config.cluster_id, cluster_id);
        assert_eq!(new_session.config.namespace, "default");
        assert_eq!(new_session.config.pod_name, "nginx");
        assert_eq!(new_session.config.local_port, 8080);
        assert_eq!(new_session.config.remote_port, 80);

        assert_eq!(manager.count(), 2); // both old and new exist
    }

    #[test]
    fn test_port_forward_manager_reconnect_nonexistent() {
        let mut manager = PortForwardManager::new();
        assert!(manager.reconnect_session(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_port_forward_manager_mixed_states() {
        let mut manager = PortForwardManager::new();
        let cluster_id = Uuid::new_v4();

        let id_active = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            local_port: 8080,
            remote_port: 80,
        });
        let id_stopped = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "redis".to_string(),
            local_port: 6379,
            remote_port: 6379,
        });
        let id_error = manager.add(PortForwardConfig {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "postgres".to_string(),
            local_port: 5432,
            remote_port: 5432,
        });

        manager.stop_session(id_stopped);
        manager.set_session_error(id_error, "address already in use");

        assert_eq!(manager.count(), 3);
        assert_eq!(manager.active_count(), 1);
        assert_eq!(manager.active_sessions().len(), 1);
        assert_eq!(manager.active_sessions()[0].id, id_active);
    }
}
