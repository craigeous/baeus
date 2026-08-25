// T074: Terminal view component state
// Renders terminal emulator grid in GPUI with keyboard input forwarding.

use uuid::Uuid;

/// Terminal session display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalDisplayMode {
    /// Normal inline display within a tab.
    Inline,
    /// Full-screen maximized mode.
    Fullscreen,
    /// Split view alongside other content.
    Split,
}

/// Terminal view settings.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalViewSettings {
    pub font_size: f32,
    pub font_family: String,
    pub cursor_blink: bool,
    pub scroll_sensitivity: f32,
    pub display_mode: TerminalDisplayMode,
}

impl Default for TerminalViewSettings {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            font_family: "Menlo".to_string(),
            cursor_blink: true,
            scroll_sensitivity: 1.0,
            display_mode: TerminalDisplayMode::Inline,
        }
    }
}

/// Connection info for what this terminal is connected to.
#[derive(Debug, Clone)]
pub enum TerminalTarget {
    /// Local shell.
    LocalShell,
    /// Exec into a pod container.
    PodExec {
        cluster_id: Uuid,
        namespace: String,
        pod_name: String,
        container_name: Option<String>,
    },
}

/// State of the terminal connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// State for the terminal view component.
#[derive(Debug)]
pub struct TerminalViewState {
    pub session_id: Option<Uuid>,
    pub target: TerminalTarget,
    pub connection_state: TerminalConnectionState,
    pub settings: TerminalViewSettings,
    pub title: String,
    pub rows: u16,
    pub cols: u16,
    pub scrollback_offset: u32,
    pub has_selection: bool,
    pub bell_ringing: bool,
}

impl TerminalViewState {
    /// Creates a new terminal view for a pod exec session.
    pub fn for_pod_exec(
        cluster_id: Uuid,
        namespace: &str,
        pod_name: &str,
        container_name: Option<&str>,
    ) -> Self {
        let title = match container_name {
            Some(c) => format!("{pod_name}/{c}"),
            None => pod_name.to_string(),
        };
        Self {
            session_id: None,
            target: TerminalTarget::PodExec {
                cluster_id,
                namespace: namespace.to_string(),
                pod_name: pod_name.to_string(),
                container_name: container_name.map(|s| s.to_string()),
            },
            connection_state: TerminalConnectionState::Disconnected,
            settings: TerminalViewSettings::default(),
            title,
            rows: 24,
            cols: 80,
            scrollback_offset: 0,
            has_selection: false,
            bell_ringing: false,
        }
    }

    /// Creates a new terminal view for a local shell.
    pub fn for_local_shell() -> Self {
        Self {
            session_id: None,
            target: TerminalTarget::LocalShell,
            connection_state: TerminalConnectionState::Disconnected,
            settings: TerminalViewSettings::default(),
            title: "Shell".to_string(),
            rows: 24,
            cols: 80,
            scrollback_offset: 0,
            has_selection: false,
            bell_ringing: false,
        }
    }

    /// Set the session ID once connected.
    pub fn set_session(&mut self, id: Uuid) {
        self.session_id = Some(id);
    }

    /// Set connection state.
    pub fn set_connection_state(&mut self, state: TerminalConnectionState) {
        self.connection_state = state;
    }

    /// Connect the terminal.
    pub fn connect(&mut self) {
        self.connection_state = TerminalConnectionState::Connecting;
    }

    /// Mark as connected.
    pub fn set_connected(&mut self) {
        self.connection_state = TerminalConnectionState::Connected;
    }

    /// Disconnect the terminal.
    pub fn disconnect(&mut self) {
        self.connection_state = TerminalConnectionState::Disconnected;
        self.session_id = None;
    }

    /// Set an error state.
    pub fn set_error(&mut self, error: &str) {
        self.connection_state = TerminalConnectionState::Error(error.to_string());
    }

    /// Returns true if terminal is connected.
    pub fn is_connected(&self) -> bool {
        self.connection_state == TerminalConnectionState::Connected
    }

    /// Returns true if terminal is connecting.
    pub fn is_connecting(&self) -> bool {
        self.connection_state == TerminalConnectionState::Connecting
    }

    /// Resize the terminal grid.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
    }

    /// Set the terminal title (from escape sequence).
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Scroll up in scrollback buffer.
    pub fn scroll_up(&mut self, lines: u32) {
        self.scrollback_offset = self.scrollback_offset.saturating_add(lines);
    }

    /// Scroll down (towards latest output).
    pub fn scroll_down(&mut self, lines: u32) {
        self.scrollback_offset = self.scrollback_offset.saturating_sub(lines);
    }

    /// Jump to bottom (latest output).
    pub fn scroll_to_bottom(&mut self) {
        self.scrollback_offset = 0;
    }

    /// Returns true if viewing scrollback (not at bottom).
    pub fn is_scrolled_up(&self) -> bool {
        self.scrollback_offset > 0
    }

    /// Toggle display mode.
    pub fn set_display_mode(&mut self, mode: TerminalDisplayMode) {
        self.settings.display_mode = mode;
    }

    /// Set font size.
    pub fn set_font_size(&mut self, size: f32) {
        self.settings.font_size = size.clamp(8.0, 32.0);
    }

    /// Trigger bell (visual bell).
    pub fn ring_bell(&mut self) {
        self.bell_ringing = true;
    }

    /// Clear bell state.
    pub fn clear_bell(&mut self) {
        self.bell_ringing = false;
    }

    /// Returns true if this is a pod exec terminal.
    pub fn is_pod_exec(&self) -> bool {
        matches!(self.target, TerminalTarget::PodExec { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_for_pod_exec() {
        let cluster_id = Uuid::new_v4();
        let state =
            TerminalViewState::for_pod_exec(cluster_id, "default", "nginx-abc", Some("app"));

        assert_eq!(state.title, "nginx-abc/app");
        assert!(state.session_id.is_none());
        assert_eq!(state.connection_state, TerminalConnectionState::Disconnected);
        assert_eq!(state.rows, 24);
        assert_eq!(state.cols, 80);
        assert!(state.is_pod_exec());
        assert!(!state.is_connected());

        if let TerminalTarget::PodExec { cluster_id: cid, namespace, pod_name, container_name } =
            &state.target
        {
            assert_eq!(*cid, cluster_id);
            assert_eq!(namespace, "default");
            assert_eq!(pod_name, "nginx-abc");
            assert_eq!(container_name.as_deref(), Some("app"));
        } else {
            panic!("Expected PodExec target");
        }
    }

    #[test]
    fn test_for_pod_exec_no_container() {
        let state = TerminalViewState::for_pod_exec(Uuid::new_v4(), "default", "nginx", None);
        assert_eq!(state.title, "nginx");
    }

    #[test]
    fn test_for_local_shell() {
        let state = TerminalViewState::for_local_shell();
        assert_eq!(state.title, "Shell");
        assert!(!state.is_pod_exec());
        assert!(matches!(state.target, TerminalTarget::LocalShell));
    }

    #[test]
    fn test_connection_lifecycle() {
        let mut state = TerminalViewState::for_pod_exec(Uuid::new_v4(), "default", "nginx", None);

        assert!(!state.is_connected());
        assert!(!state.is_connecting());

        state.connect();
        assert!(state.is_connecting());
        assert!(!state.is_connected());

        let session_id = Uuid::new_v4();
        state.set_session(session_id);
        state.set_connected();
        assert!(state.is_connected());
        assert_eq!(state.session_id, Some(session_id));

        state.disconnect();
        assert!(!state.is_connected());
        assert!(state.session_id.is_none());
    }

    #[test]
    fn test_error_state() {
        let mut state = TerminalViewState::for_pod_exec(Uuid::new_v4(), "default", "nginx", None);

        state.set_error("connection refused");
        assert_eq!(
            state.connection_state,
            TerminalConnectionState::Error("connection refused".to_string())
        );
        assert!(!state.is_connected());
    }

    #[test]
    fn test_resize() {
        let mut state = TerminalViewState::for_local_shell();
        assert_eq!(state.rows, 24);
        assert_eq!(state.cols, 80);

        state.resize(50, 120);
        assert_eq!(state.rows, 50);
        assert_eq!(state.cols, 120);
    }

    #[test]
    fn test_scrollback() {
        let mut state = TerminalViewState::for_local_shell();
        assert!(!state.is_scrolled_up());
        assert_eq!(state.scrollback_offset, 0);

        state.scroll_up(10);
        assert!(state.is_scrolled_up());
        assert_eq!(state.scrollback_offset, 10);

        state.scroll_up(5);
        assert_eq!(state.scrollback_offset, 15);

        state.scroll_down(3);
        assert_eq!(state.scrollback_offset, 12);

        state.scroll_to_bottom();
        assert!(!state.is_scrolled_up());
        assert_eq!(state.scrollback_offset, 0);
    }

    #[test]
    fn test_scroll_down_underflow() {
        let mut state = TerminalViewState::for_local_shell();
        state.scroll_up(5);
        state.scroll_down(100); // should clamp to 0
        assert_eq!(state.scrollback_offset, 0);
    }

    #[test]
    fn test_set_title() {
        let mut state = TerminalViewState::for_local_shell();
        state.set_title("vim /etc/config");
        assert_eq!(state.title, "vim /etc/config");
    }

    #[test]
    fn test_display_mode() {
        let mut state = TerminalViewState::for_local_shell();
        assert_eq!(state.settings.display_mode, TerminalDisplayMode::Inline);

        state.set_display_mode(TerminalDisplayMode::Fullscreen);
        assert_eq!(state.settings.display_mode, TerminalDisplayMode::Fullscreen);

        state.set_display_mode(TerminalDisplayMode::Split);
        assert_eq!(state.settings.display_mode, TerminalDisplayMode::Split);
    }

    #[test]
    fn test_font_size_clamping() {
        let mut state = TerminalViewState::for_local_shell();
        state.set_font_size(5.0);
        assert_eq!(state.settings.font_size, 8.0);

        state.set_font_size(50.0);
        assert_eq!(state.settings.font_size, 32.0);

        state.set_font_size(16.0);
        assert_eq!(state.settings.font_size, 16.0);
    }

    #[test]
    fn test_bell() {
        let mut state = TerminalViewState::for_local_shell();
        assert!(!state.bell_ringing);

        state.ring_bell();
        assert!(state.bell_ringing);

        state.clear_bell();
        assert!(!state.bell_ringing);
    }

    #[test]
    fn test_default_settings() {
        let settings = TerminalViewSettings::default();
        assert_eq!(settings.font_size, 13.0);
        assert_eq!(settings.font_family, "Menlo");
        assert!(settings.cursor_blink);
        assert_eq!(settings.scroll_sensitivity, 1.0);
        assert_eq!(settings.display_mode, TerminalDisplayMode::Inline);
    }

    #[test]
    fn test_full_workflow() {
        let cluster_id = Uuid::new_v4();
        let mut state = TerminalViewState::for_pod_exec(
            cluster_id,
            "production",
            "api-server-xyz",
            Some("app"),
        );

        // Connect
        state.connect();
        assert!(state.is_connecting());

        let session = Uuid::new_v4();
        state.set_session(session);
        state.set_connected();
        assert!(state.is_connected());

        // Resize
        state.resize(40, 120);
        assert_eq!(state.rows, 40);
        assert_eq!(state.cols, 120);

        // Use scrollback
        state.scroll_up(20);
        assert!(state.is_scrolled_up());
        state.scroll_to_bottom();
        assert!(!state.is_scrolled_up());

        // Fullscreen
        state.set_display_mode(TerminalDisplayMode::Fullscreen);

        // Title update from escape sequence
        state.set_title("root@api-server-xyz:/app");

        // Disconnect
        state.disconnect();
        assert!(!state.is_connected());
    }
}
