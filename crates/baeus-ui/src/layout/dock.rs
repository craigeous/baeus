use uuid::Uuid;

/// The kind of content displayed in a dock tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockTabKind {
    Terminal { pod: String, container: String, cluster: String },
    LogViewer { pod: String, container: String, cluster: String },
    PortForwardManager,
}

/// A single tab within the dock panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockTab {
    pub id: Uuid,
    pub kind: DockTabKind,
    pub label: String,
}

/// Minimum height the dock panel can be resized to.
const MIN_DOCK_HEIGHT: f32 = 100.0;

/// Maximum fraction of window height the dock may occupy.
const MAX_DOCK_HEIGHT_FRACTION: f32 = 0.6;

/// Default height for the dock panel.
const DEFAULT_DOCK_HEIGHT: f32 = 250.0;

/// State for the bottom dock panel (terminal, logs, port-forward).
#[derive(Debug, Clone)]
pub struct DockState {
    pub tabs: Vec<DockTab>,
    pub active_tab_id: Option<Uuid>,
    pub collapsed: bool,
    pub height: f32,
}

impl Default for DockState {
    fn default() -> Self {
        Self { tabs: Vec::new(), active_tab_id: None, collapsed: true, height: DEFAULT_DOCK_HEIGHT }
    }
}

impl DockState {
    /// Adds a new tab of the given kind, returning its `Uuid`.
    /// If this is the first tab, it is automatically selected.
    pub fn add_tab(&mut self, kind: DockTabKind) -> Uuid {
        let label = match &kind {
            DockTabKind::Terminal { pod, container, cluster } => {
                if pod.is_empty() && container.is_empty() {
                    "Terminal".to_string()
                } else {
                    format!("Terminal: {cluster}/{pod}/{container}")
                }
            }
            DockTabKind::LogViewer { pod, container, cluster } => {
                format!("Logs: {cluster}/{pod}/{container}")
            }
            DockTabKind::PortForwardManager => "Port Forwards".to_string(),
        };
        let id = Uuid::new_v4();
        self.tabs.push(DockTab { id, kind, label });
        // Auto-select the first tab added.
        if self.tabs.len() == 1 {
            self.active_tab_id = Some(id);
        }
        id
    }

    /// Removes the tab with the given `id`.
    /// If the removed tab was active, a neighboring tab is selected
    /// (prefer the tab that was at the same index position, falling back
    /// to the previous one, or `None` if the list is now empty).
    pub fn remove_tab(&mut self, id: Uuid) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(pos);
            if self.active_tab_id == Some(id) {
                self.active_tab_id = if self.tabs.is_empty() {
                    None
                } else {
                    // Clamp to valid range: prefer same index (now the next
                    // neighbor), or fall back to the last tab.
                    let new_idx = pos.min(self.tabs.len() - 1);
                    Some(self.tabs[new_idx].id)
                };
            }
        }
    }

    /// Selects the tab with the given `id`. If the id does not exist
    /// among the current tabs, this is a no-op.
    pub fn select_tab(&mut self, id: Uuid) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab_id = Some(id);
        }
    }

    /// Toggles the collapsed state of the dock panel.
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Sets the dock height, clamping to `[MIN_DOCK_HEIGHT, window_height * MAX_DOCK_HEIGHT_FRACTION]`.
    /// The `window_height` parameter is used to compute the upper bound.
    pub fn set_height(&mut self, h: f32, window_height: f32) {
        let max = window_height * MAX_DOCK_HEIGHT_FRACTION;
        self.height = h.clamp(MIN_DOCK_HEIGHT, max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Defaults ----

    #[test]
    fn test_default_state() {
        let state = DockState::default();
        assert!(state.tabs.is_empty());
        assert_eq!(state.active_tab_id, None);
        assert!(state.collapsed);
        assert!((state.height - 250.0).abs() < f32::EPSILON);
    }

    // ---- add_tab ----

    #[test]
    fn test_add_tab_returns_unique_ids() {
        let mut state = DockState::default();
        let id1 = state.add_tab(DockTabKind::PortForwardManager);
        let id2 = state.add_tab(DockTabKind::PortForwardManager);
        assert_ne!(id1, id2);
        assert_eq!(state.tabs.len(), 2);
    }

    #[test]
    fn test_add_first_tab_auto_selects() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::PortForwardManager);
        assert_eq!(state.active_tab_id, Some(id));
    }

    #[test]
    fn test_add_second_tab_does_not_change_selection() {
        let mut state = DockState::default();
        let first = state.add_tab(DockTabKind::PortForwardManager);
        let _second = state.add_tab(DockTabKind::PortForwardManager);
        assert_eq!(state.active_tab_id, Some(first));
    }

    // ---- remove_tab ----

    #[test]
    fn test_remove_only_tab_clears_selection() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::PortForwardManager);
        state.remove_tab(id);
        assert!(state.tabs.is_empty());
        assert_eq!(state.active_tab_id, None);
    }

    #[test]
    fn test_remove_active_tab_selects_neighbor() {
        let mut state = DockState::default();
        let a = state.add_tab(DockTabKind::PortForwardManager);
        let b = state.add_tab(DockTabKind::PortForwardManager);
        let c = state.add_tab(DockTabKind::PortForwardManager);
        // Select the middle tab.
        state.select_tab(b);
        assert_eq!(state.active_tab_id, Some(b));

        // Remove the middle tab — the next neighbor (c) should become active.
        state.remove_tab(b);
        assert_eq!(state.active_tab_id, Some(c));
        assert_eq!(state.tabs.len(), 2);

        // Remove the last tab — the previous neighbor (a) should become active.
        state.select_tab(c);
        state.remove_tab(c);
        assert_eq!(state.active_tab_id, Some(a));
    }

    #[test]
    fn test_remove_non_active_tab_keeps_selection() {
        let mut state = DockState::default();
        let a = state.add_tab(DockTabKind::PortForwardManager);
        let b = state.add_tab(DockTabKind::PortForwardManager);
        // a is active (auto-selected).
        state.remove_tab(b);
        assert_eq!(state.active_tab_id, Some(a));
    }

    #[test]
    fn test_remove_nonexistent_tab_is_noop() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::PortForwardManager);
        state.remove_tab(Uuid::new_v4());
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab_id, Some(id));
    }

    // ---- select_tab ----

    #[test]
    fn test_select_tab_changes_active() {
        let mut state = DockState::default();
        let a = state.add_tab(DockTabKind::PortForwardManager);
        let b = state.add_tab(DockTabKind::PortForwardManager);
        assert_eq!(state.active_tab_id, Some(a));

        state.select_tab(b);
        assert_eq!(state.active_tab_id, Some(b));
    }

    #[test]
    fn test_select_unknown_tab_is_noop() {
        let mut state = DockState::default();
        let a = state.add_tab(DockTabKind::PortForwardManager);
        state.select_tab(Uuid::new_v4());
        assert_eq!(state.active_tab_id, Some(a));
    }

    // ---- toggle_collapsed ----

    #[test]
    fn test_toggle_collapsed() {
        let mut state = DockState::default();
        assert!(state.collapsed);

        state.toggle_collapsed();
        assert!(!state.collapsed);

        state.toggle_collapsed();
        assert!(state.collapsed);
    }

    // ---- set_height ----

    #[test]
    fn test_set_height_default() {
        let state = DockState::default();
        assert!((state.height - 250.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_height_normal() {
        let mut state = DockState::default();
        state.set_height(300.0, 600.0);
        assert!((state.height - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_height_clamps_to_min() {
        let mut state = DockState::default();
        state.set_height(50.0, 600.0);
        assert!((state.height - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_height_clamps_to_max_60_percent() {
        let mut state = DockState::default();
        // 60% of 600 = 360
        state.set_height(500.0, 600.0);
        assert!((state.height - 360.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_height_exact_min_boundary() {
        let mut state = DockState::default();
        state.set_height(100.0, 600.0);
        assert!((state.height - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_height_exact_max_boundary() {
        let mut state = DockState::default();
        state.set_height(360.0, 600.0);
        assert!((state.height - 360.0).abs() < f32::EPSILON);
    }

    // ---- Tab types / labels ----

    #[test]
    fn test_terminal_tab_kind_and_label() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::Terminal {
            pod: "nginx-abc".to_string(),
            container: "nginx".to_string(),
            cluster: "prod".to_string(),
        });
        let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
        assert_eq!(tab.label, "Terminal: prod/nginx-abc/nginx");
        assert!(matches!(tab.kind, DockTabKind::Terminal { .. }));
    }

    #[test]
    fn test_log_viewer_tab_kind_and_label() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::LogViewer {
            pod: "api-xyz".to_string(),
            container: "api".to_string(),
            cluster: "staging".to_string(),
        });
        let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
        assert_eq!(tab.label, "Logs: staging/api-xyz/api");
        assert!(matches!(tab.kind, DockTabKind::LogViewer { .. }));
    }

    #[test]
    fn test_port_forward_manager_tab_kind_and_label() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::PortForwardManager);
        let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
        assert_eq!(tab.label, "Port Forwards");
        assert!(matches!(tab.kind, DockTabKind::PortForwardManager));
    }

    #[test]
    fn test_dock_tab_kind_equality() {
        let a = DockTabKind::Terminal {
            pod: "p".to_string(),
            container: "c".to_string(),
            cluster: "k".to_string(),
        };
        let b = DockTabKind::Terminal {
            pod: "p".to_string(),
            container: "c".to_string(),
            cluster: "k".to_string(),
        };
        assert_eq!(a, b);

        let c = DockTabKind::LogViewer {
            pod: "p".to_string(),
            container: "c".to_string(),
            cluster: "k".to_string(),
        };
        assert_ne!(a, c);
    }

    #[test]
    fn test_local_shell_terminal_label() {
        let mut state = DockState::default();
        let id = state.add_tab(DockTabKind::Terminal {
            pod: String::new(),
            container: String::new(),
            cluster: "prod".to_string(),
        });
        let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
        assert_eq!(tab.label, "Terminal");
    }
}
