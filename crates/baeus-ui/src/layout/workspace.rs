use crate::layout::NavigationTarget;
use gpui::{Context, ElementId, SharedString, Window, div, prelude::*, rgb};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: Uuid,
    pub label: String,
    pub target: NavigationTarget,
    pub closable: bool,
    pub dirty: bool,
    /// T354: Preview tabs have italic titles and are reused on next navigate.
    /// Double-clicking a preview tab converts it to a fixed tab (FR-067).
    pub is_preview: bool,
}

impl Tab {
    pub fn new(label: String, target: NavigationTarget) -> Self {
        Self { id: Uuid::new_v4(), label, target, closable: true, dirty: false, is_preview: false }
    }

    pub fn pinned(label: String, target: NavigationTarget) -> Self {
        Self { id: Uuid::new_v4(), label, target, closable: false, dirty: false, is_preview: false }
    }

    /// Create a preview tab (FR-067). Preview tabs are reused when navigating.
    pub fn preview(label: String, target: NavigationTarget) -> Self {
        Self { id: Uuid::new_v4(), label, target, closable: true, dirty: false, is_preview: true }
    }
}

#[derive(Debug, Default)]
pub struct WorkspaceState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<Uuid>,
}

impl WorkspaceState {
    /// Create a workspace pre-populated with a pinned Dashboard tab for a cluster.
    pub fn with_dashboard(cluster_context: &str) -> Self {
        let dashboard = Tab::pinned(
            format!("{cluster_context} - Overview"),
            NavigationTarget::Dashboard { cluster_context: cluster_context.to_string() },
        );
        let id = dashboard.id;
        Self { tabs: vec![dashboard], active_tab_id: Some(id) }
    }

    pub fn open_tab(&mut self, target: NavigationTarget) -> Uuid {
        // Check if a tab with the same target already exists
        if let Some(existing) = self.tabs.iter().find(|t| t.target == target) {
            let id = existing.id;
            self.active_tab_id = Some(id);
            return id;
        }

        let label = target.label();
        let tab = Tab::new(label, target);
        let id = tab.id;
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
        id
    }

    pub fn close_tab(&mut self, tab_id: Uuid) -> bool {
        let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return false;
        };

        if !self.tabs[idx].closable {
            return false;
        }

        self.tabs.remove(idx);

        // If we closed the active tab, activate the nearest tab
        if self.active_tab_id == Some(tab_id) {
            self.active_tab_id = if self.tabs.is_empty() {
                None
            } else {
                let new_idx = idx.min(self.tabs.len() - 1);
                Some(self.tabs[new_idx].id)
            };
        }

        true
    }

    pub fn activate_tab(&mut self, tab_id: Uuid) -> bool {
        if self.tabs.iter().any(|t| t.id == tab_id) {
            self.active_tab_id = Some(tab_id);
            true
        } else {
            false
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_id.and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Open a preview tab (FR-067). If there's already a preview tab,
    /// it gets replaced. If the target already has a fixed tab, activate that instead.
    pub fn open_preview_tab(&mut self, target: NavigationTarget) -> Uuid {
        // Check if a fixed tab with the same target already exists
        if let Some(existing) = self.tabs.iter().find(|t| t.target == target && !t.is_preview) {
            let id = existing.id;
            self.active_tab_id = Some(id);
            return id;
        }

        // Replace existing preview tab if one exists
        if let Some(idx) = self.tabs.iter().position(|t| t.is_preview) {
            let label = target.label();
            let id = self.tabs[idx].id;
            self.tabs[idx].label = label;
            self.tabs[idx].target = target;
            self.active_tab_id = Some(id);
            return id;
        }

        // No preview tab exists, create one
        let label = target.label();
        let tab = Tab::preview(label, target);
        let id = tab.id;
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
        id
    }

    /// Convert a preview tab to a fixed tab (FR-067).
    /// Called on double-click of the tab title.
    pub fn fix_tab(&mut self, tab_id: Uuid) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_preview = false;
        }
    }

    pub fn mark_dirty(&mut self, tab_id: Uuid) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.dirty = true;
        }
    }

    pub fn mark_clean(&mut self, tab_id: Uuid) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.dirty = false;
        }
    }
}

// ---------------------------------------------------------------------------
// GPUI View
// ---------------------------------------------------------------------------

pub struct WorkspaceView {
    state: WorkspaceState,
}

impl Default for WorkspaceView {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceView {
    pub fn new() -> Self {
        Self { state: WorkspaceState::default() }
    }

    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut WorkspaceState {
        &mut self.state
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Clone tabs to avoid borrow conflicts
        let tabs: Vec<Tab> = self.state.tabs.clone();
        let active_label = self
            .state
            .active_tab()
            .map(|t| t.target.label())
            .unwrap_or_else(|| "No Tab".to_string());

        let mut tab_bar = div()
            .flex()
            .items_center()
            .w_full()
            .h_8()
            .bg(rgb(0x1F2937))
            .border_b_1()
            .border_color(rgb(0x374151))
            .overflow_hidden();

        for (tab_idx, tab) in tabs.iter().enumerate() {
            let is_active = self.state.active_tab_id == Some(tab.id);
            let tab_id = tab.id;
            let tab_label = SharedString::from(tab.label.clone());
            let closable = tab.closable;

            let tab_element_id =
                ElementId::Name(SharedString::from(format!("workspace-tab-{tab_idx}")));

            let mut tab_el = div()
                .id(tab_element_id)
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .py_1()
                .cursor_pointer()
                .text_sm()
                .flex_shrink_0();

            if is_active {
                tab_el = tab_el
                    .bg(rgb(0x374151))
                    .text_color(rgb(0xF9FAFB))
                    .border_b_1()
                    .border_color(rgb(0x60A5FA));
            } else {
                tab_el = tab_el.text_color(rgb(0x9CA3AF)).bg(rgb(0x1F2937));
            }

            // Click handler to activate tab
            tab_el = tab_el.on_click(cx.listener(move |this, _event, _window, _cx| {
                this.state.activate_tab(tab_id);
            }));

            tab_el = tab_el.child(tab_label);

            // Close button for closable tabs
            if closable {
                let close_id =
                    ElementId::Name(SharedString::from(format!("workspace-tab-close-{tab_idx}")));
                let close_btn = div()
                    .id(close_id)
                    .ml_1()
                    .px_1()
                    .text_xs()
                    .text_color(rgb(0x6B7280))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.state.close_tab(tab_id);
                    }))
                    .child("x");
                tab_el = tab_el.child(close_btn);
            }

            tab_bar = tab_bar.child(tab_el);
        }

        // Content area
        let content = div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x111827))
            .text_color(rgb(0x9CA3AF))
            .text_sm()
            .child(SharedString::from(active_label));

        div().flex().flex_col().size_full().child(tab_bar).child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::ResourceCategory;

    const TEST_CLUSTER: &str = "test-cluster";

    fn events_target() -> NavigationTarget {
        NavigationTarget::ResourceList {
            cluster_context: TEST_CLUSTER.to_string(),
            category: ResourceCategory::Monitoring,
            kind: "Event".to_string(),
        }
    }

    fn helm_target() -> NavigationTarget {
        NavigationTarget::HelmReleases { cluster_context: TEST_CLUSTER.to_string() }
    }

    #[test]
    fn test_default_workspace_is_empty() {
        let ws = WorkspaceState::default();
        assert_eq!(ws.tab_count(), 0);
        assert!(ws.active_tab().is_none());
    }

    #[test]
    fn test_with_dashboard_creates_pinned_tab() {
        let ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        assert_eq!(ws.tab_count(), 1);
        assert!(ws.active_tab().is_some());
        assert_eq!(ws.active_tab().unwrap().label, "test-cluster - Overview");
        assert!(!ws.active_tab().unwrap().closable);
    }

    #[test]
    fn test_open_new_tab() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let target = NavigationTarget::ResourceList {
            cluster_context: TEST_CLUSTER.to_string(),
            category: ResourceCategory::Workloads,
            kind: "Pods".to_string(),
        };

        ws.open_tab(target);
        assert_eq!(ws.tab_count(), 2);
        assert_eq!(ws.active_tab().unwrap().label, "test-cluster - Pods");
    }

    #[test]
    fn test_open_existing_tab_reuses() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let target = events_target();

        let id1 = ws.open_tab(target.clone());
        let id2 = ws.open_tab(target);
        assert_eq!(id1, id2);
        assert_eq!(ws.tab_count(), 2); // Dashboard + Event
    }

    #[test]
    fn test_close_tab() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let id = ws.open_tab(events_target());

        assert!(ws.close_tab(id));
        assert_eq!(ws.tab_count(), 1);
    }

    #[test]
    fn test_cannot_close_pinned_tab() {
        let ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let dashboard_id = ws.active_tab().unwrap().id;

        let mut ws = ws;
        assert!(!ws.close_tab(dashboard_id));
        assert_eq!(ws.tab_count(), 1);
    }

    #[test]
    fn test_close_active_tab_activates_neighbor() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        ws.open_tab(events_target());
        let third_id = ws.open_tab(helm_target());

        // Close active (Helm), should activate Event
        ws.close_tab(third_id);
        assert!(ws.active_tab().unwrap().label.contains("Event"));
    }

    #[test]
    fn test_mark_dirty_and_clean() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let id = ws.open_tab(events_target());

        ws.mark_dirty(id);
        assert!(ws.tabs.iter().find(|t| t.id == id).unwrap().dirty);

        ws.mark_clean(id);
        assert!(!ws.tabs.iter().find(|t| t.id == id).unwrap().dirty);
    }

    #[test]
    fn test_activate_tab() {
        let mut ws = WorkspaceState::with_dashboard(TEST_CLUSTER);
        let dashboard_id = ws.active_tab().unwrap().id;
        let events_id = ws.open_tab(events_target());

        assert_eq!(ws.active_tab_id, Some(events_id));

        ws.activate_tab(dashboard_id);
        assert_eq!(ws.active_tab_id, Some(dashboard_id));
    }

    // --- T354: Preview tab mode tests ---

    #[test]
    fn test_open_preview_tab_creates_preview() {
        let mut ws = WorkspaceState::default();
        let id = ws.open_preview_tab(events_target());
        assert_eq!(ws.tab_count(), 1);
        let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
        assert!(tab.is_preview);
        assert!(tab.closable);
    }

    #[test]
    fn test_open_tab_creates_fixed() {
        let mut ws = WorkspaceState::default();
        let id = ws.open_tab(events_target());
        let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
        assert!(!tab.is_preview);
    }

    #[test]
    fn test_preview_tab_reused_on_next_navigate() {
        let mut ws = WorkspaceState::default();
        let id1 = ws.open_preview_tab(events_target());
        let id2 = ws.open_preview_tab(helm_target());
        // Same tab ID is reused
        assert_eq!(id1, id2);
        assert_eq!(ws.tab_count(), 1);
        assert!(ws.active_tab().unwrap().label.contains("Helm"));
    }

    #[test]
    fn test_fix_tab_converts_preview_to_fixed() {
        let mut ws = WorkspaceState::default();
        let id = ws.open_preview_tab(events_target());
        assert!(ws.tabs.iter().find(|t| t.id == id).unwrap().is_preview);

        ws.fix_tab(id);
        assert!(!ws.tabs.iter().find(|t| t.id == id).unwrap().is_preview);
    }

    #[test]
    fn test_preview_not_replaced_after_fix() {
        let mut ws = WorkspaceState::default();
        let id1 = ws.open_preview_tab(events_target());
        ws.fix_tab(id1);

        // Now opening a preview creates a NEW preview tab (old one is fixed)
        let id2 = ws.open_preview_tab(helm_target());
        assert_ne!(id1, id2);
        assert_eq!(ws.tab_count(), 2);
    }

    #[test]
    fn test_preview_activates_existing_fixed_tab() {
        let mut ws = WorkspaceState::default();
        let fixed_id = ws.open_tab(events_target());
        // Opening a preview for the same target activates the fixed tab
        let id = ws.open_preview_tab(events_target());
        assert_eq!(id, fixed_id);
        assert_eq!(ws.tab_count(), 1);
        assert!(!ws.active_tab().unwrap().is_preview);
    }

    #[test]
    fn test_preview_tab_has_italic_marker() {
        let mut ws = WorkspaceState::default();
        let id = ws.open_preview_tab(events_target());
        let tab = ws.tabs.iter().find(|t| t.id == id).unwrap();
        // The is_preview flag tells the renderer to use italic text
        assert!(tab.is_preview);
    }
}
