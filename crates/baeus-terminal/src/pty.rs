// PTY session management.
// Bridges processes (local shell or Kubernetes exec) to the terminal emulator.
// Actual PTY I/O will be added with portable-pty integration.

use uuid::Uuid;

use crate::emulator::TerminalSize;

/// Represents the source of a terminal session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtySource {
    /// Local shell session.
    LocalShell { shell_path: String },
    /// Remote exec into a Kubernetes pod.
    KubeExec {
        cluster_id: Uuid,
        namespace: String,
        pod_name: String,
        container_name: Option<String>,
    },
}

/// State of a PTY session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtySessionState {
    Starting,
    Running,
    Stopped,
    Error(String),
}

/// A PTY session that bridges a process (local or remote) to a terminal emulator.
pub struct PtySession {
    pub id: Uuid,
    pub source: PtySource,
    pub state: PtySessionState,
    pub size: TerminalSize,
    output_buffer: Vec<u8>,
    input_buffer: Vec<u8>,
}

impl PtySession {
    /// Create a new PTY session with the given source and terminal size.
    pub fn new(source: PtySource, size: TerminalSize) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            state: PtySessionState::Starting,
            size,
            output_buffer: Vec::new(),
            input_buffer: Vec::new(),
        }
    }

    /// Enqueue input data to be sent to the process.
    pub fn enqueue_input(&mut self, data: &[u8]) {
        self.input_buffer.extend_from_slice(data);
    }

    /// Take and clear the input buffer, returning its contents.
    pub fn take_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.input_buffer)
    }

    /// Push output data received from the process.
    pub fn push_output(&mut self, data: &[u8]) {
        self.output_buffer.extend_from_slice(data);
    }

    /// Take and clear the output buffer, returning its contents.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output_buffer)
    }

    /// Resize the terminal associated with this session.
    pub fn resize(&mut self, new_size: TerminalSize) {
        self.size = new_size;
    }

    /// Stop the session.
    pub fn stop(&mut self) {
        self.state = PtySessionState::Stopped;
    }

    /// Set the session to an error state with a message.
    pub fn set_error(&mut self, error: &str) {
        self.state = PtySessionState::Error(error.to_string());
    }

    /// Check whether the session is active (Starting or Running).
    pub fn is_active(&self) -> bool {
        matches!(self.state, PtySessionState::Starting | PtySessionState::Running)
    }
}

/// Manages multiple PTY sessions.
pub struct PtyManager {
    sessions: Vec<PtySession>,
}

impl PtyManager {
    /// Create a new empty PTY manager.
    pub fn new() -> Self {
        Self { sessions: Vec::new() }
    }

    /// Create a new session and return its UUID.
    pub fn create_session(&mut self, source: PtySource, size: TerminalSize) -> Uuid {
        let session = PtySession::new(source, size);
        let id = session.id;
        self.sessions.push(session);
        id
    }

    /// Get a reference to a session by its ID.
    pub fn get_session(&self, id: Uuid) -> Option<&PtySession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a session by its ID.
    pub fn get_session_mut(&mut self, id: Uuid) -> Option<&mut PtySession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// Remove a session by its ID. Returns `true` if the session was found and removed.
    pub fn remove_session(&mut self, id: Uuid) -> bool {
        let len_before = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        self.sessions.len() < len_before
    }

    /// Return references to all active sessions (Starting or Running).
    pub fn active_sessions(&self) -> Vec<&PtySession> {
        self.sessions.iter().filter(|s| s.is_active()).collect()
    }

    /// Return the total number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Stop all sessions.
    pub fn stop_all(&mut self) {
        for session in &mut self.sessions {
            session.stop();
        }
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::TerminalSize;

    // -----------------------------------------------------------------------
    // PtySource tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_pty_source_local_shell() {
        let source = PtySource::LocalShell { shell_path: "/bin/bash".to_string() };
        if let PtySource::LocalShell { shell_path } = &source {
            assert_eq!(shell_path, "/bin/bash");
        } else {
            panic!("Expected LocalShell variant");
        }
    }

    #[test]
    fn test_pty_source_kube_exec() {
        let cluster_id = Uuid::new_v4();
        let source = PtySource::KubeExec {
            cluster_id,
            namespace: "default".to_string(),
            pod_name: "nginx-abc123".to_string(),
            container_name: Some("nginx".to_string()),
        };
        if let PtySource::KubeExec { cluster_id: cid, namespace, pod_name, container_name } =
            &source
        {
            assert_eq!(*cid, cluster_id);
            assert_eq!(namespace, "default");
            assert_eq!(pod_name, "nginx-abc123");
            assert_eq!(container_name, &Some("nginx".to_string()));
        } else {
            panic!("Expected KubeExec variant");
        }
    }

    #[test]
    fn test_pty_source_kube_exec_no_container() {
        let source = PtySource::KubeExec {
            cluster_id: Uuid::new_v4(),
            namespace: "kube-system".to_string(),
            pod_name: "pod-1".to_string(),
            container_name: None,
        };
        if let PtySource::KubeExec { container_name, .. } = &source {
            assert_eq!(container_name, &None);
        } else {
            panic!("Expected KubeExec variant");
        }
    }

    #[test]
    fn test_pty_source_equality() {
        let a = PtySource::LocalShell { shell_path: "/bin/zsh".to_string() };
        let b = PtySource::LocalShell { shell_path: "/bin/zsh".to_string() };
        let c = PtySource::LocalShell { shell_path: "/bin/bash".to_string() };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // -----------------------------------------------------------------------
    // PtySessionState tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_pty_session_state_variants() {
        assert_eq!(PtySessionState::Starting, PtySessionState::Starting);
        assert_eq!(PtySessionState::Running, PtySessionState::Running);
        assert_eq!(PtySessionState::Stopped, PtySessionState::Stopped);
        assert_eq!(
            PtySessionState::Error("test".to_string()),
            PtySessionState::Error("test".to_string())
        );
        assert_ne!(PtySessionState::Starting, PtySessionState::Running);
        assert_ne!(
            PtySessionState::Error("a".to_string()),
            PtySessionState::Error("b".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // PtySession construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_new_local_shell() {
        let source = PtySource::LocalShell { shell_path: "/bin/bash".to_string() };
        let session = PtySession::new(source.clone(), TerminalSize::default());
        assert_eq!(session.source, source);
        assert_eq!(session.state, PtySessionState::Starting);
        assert_eq!(session.size, TerminalSize::default());
    }

    #[test]
    fn test_session_new_kube_exec() {
        let source = PtySource::KubeExec {
            cluster_id: Uuid::new_v4(),
            namespace: "prod".to_string(),
            pod_name: "app-pod".to_string(),
            container_name: Some("app".to_string()),
        };
        let size = TerminalSize { rows: 50, cols: 120 };
        let session = PtySession::new(source.clone(), size);
        assert_eq!(session.source, source);
        assert_eq!(session.size, size);
        assert_eq!(session.state, PtySessionState::Starting);
    }

    #[test]
    fn test_session_has_unique_id() {
        let source = PtySource::LocalShell { shell_path: "/bin/sh".to_string() };
        let s1 = PtySession::new(source.clone(), TerminalSize::default());
        let s2 = PtySession::new(source, TerminalSize::default());
        assert_ne!(s1.id, s2.id);
    }

    // -----------------------------------------------------------------------
    // Input/output buffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_enqueue_and_take_input() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.enqueue_input(b"hello");
        session.enqueue_input(b" world");
        let input = session.take_input();
        assert_eq!(input, b"hello world");
    }

    #[test]
    fn test_take_input_clears_buffer() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.enqueue_input(b"data");
        let _ = session.take_input();
        let input = session.take_input();
        assert!(input.is_empty());
    }

    #[test]
    fn test_push_and_take_output() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.push_output(b"response");
        session.push_output(b" data");
        let output = session.take_output();
        assert_eq!(output, b"response data");
    }

    #[test]
    fn test_take_output_clears_buffer() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.push_output(b"data");
        let _ = session.take_output();
        let output = session.take_output();
        assert!(output.is_empty());
    }

    #[test]
    fn test_input_and_output_buffers_independent() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.enqueue_input(b"input");
        session.push_output(b"output");

        let input = session.take_input();
        let output = session.take_output();

        assert_eq!(input, b"input");
        assert_eq!(output, b"output");
    }

    // -----------------------------------------------------------------------
    // Session resize tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_resize() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        let new_size = TerminalSize { rows: 50, cols: 120 };
        session.resize(new_size);
        assert_eq!(session.size, new_size);
    }

    // -----------------------------------------------------------------------
    // Session state transition tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_stop() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        assert!(session.is_active());
        session.stop();
        assert_eq!(session.state, PtySessionState::Stopped);
        assert!(!session.is_active());
    }

    #[test]
    fn test_session_set_error() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        session.set_error("connection lost");
        assert_eq!(session.state, PtySessionState::Error("connection lost".to_string()));
        assert!(!session.is_active());
    }

    #[test]
    fn test_is_active_starting() {
        let session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        assert!(session.is_active());
    }

    #[test]
    fn test_is_active_running() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        session.state = PtySessionState::Running;
        assert!(session.is_active());
    }

    #[test]
    fn test_is_active_stopped() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        session.stop();
        assert!(!session.is_active());
    }

    #[test]
    fn test_is_active_error() {
        let mut session = PtySession::new(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        session.set_error("fail");
        assert!(!session.is_active());
    }

    // -----------------------------------------------------------------------
    // PtyManager construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_new_empty() {
        let manager = PtyManager::new();
        assert_eq!(manager.session_count(), 0);
        assert!(manager.active_sessions().is_empty());
    }

    #[test]
    fn test_manager_default() {
        let manager = PtyManager::default();
        assert_eq!(manager.session_count(), 0);
    }

    // -----------------------------------------------------------------------
    // PtyManager create_session tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_create_session() {
        let mut manager = PtyManager::new();
        let source = PtySource::LocalShell { shell_path: "/bin/bash".to_string() };
        let id = manager.create_session(source.clone(), TerminalSize::default());

        assert_eq!(manager.session_count(), 1);
        let session = manager.get_session(id).unwrap();
        assert_eq!(session.id, id);
        assert_eq!(session.source, source);
    }

    #[test]
    fn test_manager_create_multiple_sessions() {
        let mut manager = PtyManager::new();
        let id1 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        let id2 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/zsh".to_string() },
            TerminalSize::default(),
        );
        let id3 = manager.create_session(
            PtySource::KubeExec {
                cluster_id: Uuid::new_v4(),
                namespace: "default".to_string(),
                pod_name: "pod-1".to_string(),
                container_name: None,
            },
            TerminalSize { rows: 30, cols: 100 },
        );

        assert_eq!(manager.session_count(), 3);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    // -----------------------------------------------------------------------
    // PtyManager get_session tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_get_session_exists() {
        let mut manager = PtyManager::new();
        let id = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        assert!(manager.get_session(id).is_some());
    }

    #[test]
    fn test_manager_get_session_not_found() {
        let manager = PtyManager::new();
        assert!(manager.get_session(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_manager_get_session_mut_exists() {
        let mut manager = PtyManager::new();
        let id = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        let session = manager.get_session_mut(id).unwrap();
        session.state = PtySessionState::Running;
        assert_eq!(manager.get_session(id).unwrap().state, PtySessionState::Running);
    }

    #[test]
    fn test_manager_get_session_mut_not_found() {
        let mut manager = PtyManager::new();
        assert!(manager.get_session_mut(Uuid::new_v4()).is_none());
    }

    // -----------------------------------------------------------------------
    // PtyManager remove_session tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_remove_session_exists() {
        let mut manager = PtyManager::new();
        let id = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        assert!(manager.remove_session(id));
        assert_eq!(manager.session_count(), 0);
        assert!(manager.get_session(id).is_none());
    }

    #[test]
    fn test_manager_remove_session_not_found() {
        let mut manager = PtyManager::new();
        assert!(!manager.remove_session(Uuid::new_v4()));
    }

    #[test]
    fn test_manager_remove_session_preserves_others() {
        let mut manager = PtyManager::new();
        let id1 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        let id2 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/zsh".to_string() },
            TerminalSize::default(),
        );

        manager.remove_session(id1);
        assert_eq!(manager.session_count(), 1);
        assert!(manager.get_session(id1).is_none());
        assert!(manager.get_session(id2).is_some());
    }

    // -----------------------------------------------------------------------
    // PtyManager active_sessions tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_active_sessions_all_active() {
        let mut manager = PtyManager::new();
        manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/zsh".to_string() },
            TerminalSize::default(),
        );
        assert_eq!(manager.active_sessions().len(), 2);
    }

    #[test]
    fn test_manager_active_sessions_some_stopped() {
        let mut manager = PtyManager::new();
        let id1 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        let _id2 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/zsh".to_string() },
            TerminalSize::default(),
        );

        manager.get_session_mut(id1).unwrap().stop();
        let active = manager.active_sessions();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_manager_active_sessions_none_active() {
        let mut manager = PtyManager::new();
        let id = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        manager.get_session_mut(id).unwrap().stop();
        assert!(manager.active_sessions().is_empty());
    }

    // -----------------------------------------------------------------------
    // PtyManager stop_all tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manager_stop_all() {
        let mut manager = PtyManager::new();
        let id1 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );
        let id2 = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/zsh".to_string() },
            TerminalSize::default(),
        );

        manager.stop_all();

        assert_eq!(manager.get_session(id1).unwrap().state, PtySessionState::Stopped);
        assert_eq!(manager.get_session(id2).unwrap().state, PtySessionState::Stopped);
        assert!(manager.active_sessions().is_empty());
        // Sessions still exist, just stopped
        assert_eq!(manager.session_count(), 2);
    }

    #[test]
    fn test_manager_stop_all_empty() {
        let mut manager = PtyManager::new();
        manager.stop_all(); // Should not panic
        assert_eq!(manager.session_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Integration-style tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_lifecycle() {
        let mut manager = PtyManager::new();

        // Create session
        let source = PtySource::KubeExec {
            cluster_id: Uuid::new_v4(),
            namespace: "production".to_string(),
            pod_name: "web-server-abc".to_string(),
            container_name: Some("web".to_string()),
        };
        let id = manager.create_session(source, TerminalSize { rows: 40, cols: 100 });

        // Session starts as Starting
        let session = manager.get_session(id).unwrap();
        assert_eq!(session.state, PtySessionState::Starting);
        assert!(session.is_active());

        // Transition to Running
        let session = manager.get_session_mut(id).unwrap();
        session.state = PtySessionState::Running;
        assert!(session.is_active());

        // Send some input
        let session = manager.get_session_mut(id).unwrap();
        session.enqueue_input(b"ls -la\n");
        let input = session.take_input();
        assert_eq!(input, b"ls -la\n");

        // Receive some output
        let session = manager.get_session_mut(id).unwrap();
        session.push_output(b"total 42\n");
        let output = session.take_output();
        assert_eq!(output, b"total 42\n");

        // Resize
        let session = manager.get_session_mut(id).unwrap();
        session.resize(TerminalSize { rows: 50, cols: 120 });
        assert_eq!(session.size, TerminalSize { rows: 50, cols: 120 });

        // Stop
        let session = manager.get_session_mut(id).unwrap();
        session.stop();
        assert!(!session.is_active());

        // Remove
        assert!(manager.remove_session(id));
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_multiple_sessions_different_sources() {
        let mut manager = PtyManager::new();

        let local_id = manager.create_session(
            PtySource::LocalShell { shell_path: "/bin/bash".to_string() },
            TerminalSize::default(),
        );

        let kube_id = manager.create_session(
            PtySource::KubeExec {
                cluster_id: Uuid::new_v4(),
                namespace: "staging".to_string(),
                pod_name: "api-server".to_string(),
                container_name: Some("api".to_string()),
            },
            TerminalSize { rows: 30, cols: 100 },
        );

        assert_eq!(manager.session_count(), 2);
        assert_eq!(manager.active_sessions().len(), 2);

        // Interact with each independently
        let local = manager.get_session_mut(local_id).unwrap();
        local.enqueue_input(b"echo hello\n");

        let kube = manager.get_session_mut(kube_id).unwrap();
        kube.enqueue_input(b"kubectl get pods\n");

        // Verify buffers are independent
        let local_input = manager.get_session_mut(local_id).unwrap().take_input();
        let kube_input = manager.get_session_mut(kube_id).unwrap().take_input();
        assert_eq!(local_input, b"echo hello\n");
        assert_eq!(kube_input, b"kubectl get pods\n");
    }
}
