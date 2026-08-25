// Integration tests for crates/baeus-ui/src/layout/dock.rs (T315)

use baeus_ui::layout::dock::*;
use uuid::Uuid;

// ===================================================================
// 1. Tab bar state — label generation
// ===================================================================

#[test]
fn test_terminal_tab_label_format() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::Terminal {
        pod: "nginx-abc".to_string(),
        container: "nginx".to_string(),
        cluster: "prod".to_string(),
    });
    let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "Terminal: prod/nginx-abc/nginx");
}

#[test]
fn test_log_viewer_tab_label_format() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::LogViewer {
        pod: "api-xyz".to_string(),
        container: "api".to_string(),
        cluster: "staging".to_string(),
    });
    let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "Logs: staging/api-xyz/api");
}

#[test]
fn test_port_forward_manager_tab_label() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::PortForwardManager);
    let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.label, "Port Forwards");
}

#[test]
fn test_multiple_tab_labels_correct() {
    let mut state = DockState::default();
    let t1 = state.add_tab(DockTabKind::Terminal {
        pod: "web-1".to_string(),
        container: "app".to_string(),
        cluster: "dev".to_string(),
    });
    let t2 = state.add_tab(DockTabKind::LogViewer {
        pod: "worker-2".to_string(),
        container: "sidecar".to_string(),
        cluster: "dev".to_string(),
    });
    let t3 = state.add_tab(DockTabKind::PortForwardManager);

    assert_eq!(state.tabs.iter().find(|t| t.id == t1).unwrap().label, "Terminal: dev/web-1/app");
    assert_eq!(state.tabs.iter().find(|t| t.id == t2).unwrap().label, "Logs: dev/worker-2/sidecar");
    assert_eq!(state.tabs.iter().find(|t| t.id == t3).unwrap().label, "Port Forwards");
}

// ===================================================================
// 2. Active tab tracking
// ===================================================================

#[test]
fn test_first_tab_becomes_active() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::PortForwardManager);
    assert_eq!(state.active_tab_id, Some(id));
}

#[test]
fn test_adding_more_tabs_does_not_change_active() {
    let mut state = DockState::default();
    let first = state.add_tab(DockTabKind::PortForwardManager);
    let _second = state.add_tab(DockTabKind::Terminal {
        pod: "p".to_string(),
        container: "c".to_string(),
        cluster: "k".to_string(),
    });
    let _third = state.add_tab(DockTabKind::LogViewer {
        pod: "p".to_string(),
        container: "c".to_string(),
        cluster: "k".to_string(),
    });
    assert_eq!(state.active_tab_id, Some(first));
}

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
fn test_select_nonexistent_tab_is_noop() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    state.select_tab(Uuid::new_v4());
    assert_eq!(state.active_tab_id, Some(a));
}

// ===================================================================
// 3. Tab close behavior
// ===================================================================

#[test]
fn test_close_active_tab_selects_next_neighbor() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);
    let c = state.add_tab(DockTabKind::PortForwardManager);

    // Select middle tab and remove it — next neighbor (c) should become active.
    state.select_tab(b);
    state.remove_tab(b);
    assert_eq!(state.active_tab_id, Some(c));
    assert_eq!(state.tabs.len(), 2);

    // Now tabs are [a, c]. Remove c (last) — previous neighbor (a) becomes active.
    state.select_tab(c);
    state.remove_tab(c);
    assert_eq!(state.active_tab_id, Some(a));
    assert_eq!(state.tabs.len(), 1);
}

#[test]
fn test_close_only_tab_clears_active() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::PortForwardManager);
    state.remove_tab(id);
    assert!(state.tabs.is_empty());
    assert_eq!(state.active_tab_id, None);
}

#[test]
fn test_close_non_active_tab_keeps_selection() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);
    // a is active (auto-selected as first tab).
    assert_eq!(state.active_tab_id, Some(a));

    state.remove_tab(b);
    assert_eq!(state.active_tab_id, Some(a));
    assert_eq!(state.tabs.len(), 1);
}

#[test]
fn test_close_nonexistent_tab_is_noop() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::PortForwardManager);
    state.remove_tab(Uuid::new_v4());
    assert_eq!(state.tabs.len(), 1);
    assert_eq!(state.active_tab_id, Some(id));
}

#[test]
fn test_close_first_active_tab_selects_new_first() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);
    let _c = state.add_tab(DockTabKind::PortForwardManager);

    // a is active. Remove a — b (at index 0 now) should become active.
    state.remove_tab(a);
    assert_eq!(state.active_tab_id, Some(b));
}

// ===================================================================
// 4. Collapse/expand toggle
// ===================================================================

#[test]
fn test_default_is_collapsed() {
    let state = DockState::default();
    assert!(state.collapsed);
}

#[test]
fn test_toggle_collapsed_flips_state() {
    let mut state = DockState::default();
    assert!(state.collapsed);

    state.toggle_collapsed();
    assert!(!state.collapsed);

    state.toggle_collapsed();
    assert!(state.collapsed);
}

#[test]
fn test_toggle_collapsed_multiple_times() {
    let mut state = DockState::default();
    for i in 0..10 {
        state.toggle_collapsed();
        if i % 2 == 0 {
            assert!(!state.collapsed);
        } else {
            assert!(state.collapsed);
        }
    }
}

// ===================================================================
// 5. Height resize constraints
// ===================================================================

#[test]
fn test_default_height() {
    let state = DockState::default();
    assert!((state.height - 250.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_normal_value() {
    let mut state = DockState::default();
    state.set_height(300.0, 800.0);
    assert!((state.height - 300.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_below_minimum_clamps_to_100() {
    let mut state = DockState::default();
    state.set_height(50.0, 800.0);
    assert!((state.height - 100.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_zero_clamps_to_minimum() {
    let mut state = DockState::default();
    state.set_height(0.0, 800.0);
    assert!((state.height - 100.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_above_max_clamps_to_60_percent() {
    let mut state = DockState::default();
    // 60% of 600 = 360
    state.set_height(500.0, 600.0);
    assert!((state.height - 360.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_exact_minimum_boundary() {
    let mut state = DockState::default();
    state.set_height(100.0, 800.0);
    assert!((state.height - 100.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_exact_maximum_boundary() {
    let mut state = DockState::default();
    // 60% of 1000 = 600
    state.set_height(600.0, 1000.0);
    assert!((state.height - 600.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_just_below_max() {
    let mut state = DockState::default();
    // 60% of 1000 = 600; set to 599.9 — should be accepted as-is
    state.set_height(599.9, 1000.0);
    assert!((state.height - 599.9).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_just_above_min() {
    let mut state = DockState::default();
    state.set_height(100.1, 800.0);
    assert!((state.height - 100.1).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_with_small_window() {
    let mut state = DockState::default();
    // 60% of 500 = 300. Value 200 is within [100, 300].
    state.set_height(200.0, 500.0);
    assert!((state.height - 200.0).abs() < f32::EPSILON);
}

#[test]
fn test_set_height_different_window_sizes() {
    let mut state = DockState::default();

    // Window 600 → max = 360
    state.set_height(400.0, 600.0);
    assert!((state.height - 360.0).abs() < f32::EPSILON);

    // Window 2000 → max = 1200
    state.set_height(1000.0, 2000.0);
    assert!((state.height - 1000.0).abs() < f32::EPSILON);
}

// ===================================================================
// 6. Tab ordering
// ===================================================================

#[test]
fn test_tabs_maintain_insertion_order() {
    let mut state = DockState::default();
    let id1 = state.add_tab(DockTabKind::Terminal {
        pod: "a".to_string(),
        container: "c1".to_string(),
        cluster: "k1".to_string(),
    });
    let id2 = state.add_tab(DockTabKind::LogViewer {
        pod: "b".to_string(),
        container: "c2".to_string(),
        cluster: "k2".to_string(),
    });
    let id3 = state.add_tab(DockTabKind::PortForwardManager);

    assert_eq!(state.tabs.len(), 3);
    assert_eq!(state.tabs[0].id, id1);
    assert_eq!(state.tabs[1].id, id2);
    assert_eq!(state.tabs[2].id, id3);
}

#[test]
fn test_tab_ordering_preserved_after_selection() {
    let mut state = DockState::default();
    let id1 = state.add_tab(DockTabKind::PortForwardManager);
    let id2 = state.add_tab(DockTabKind::PortForwardManager);
    let id3 = state.add_tab(DockTabKind::PortForwardManager);

    // Selecting different tabs should not change ordering.
    state.select_tab(id3);
    state.select_tab(id1);
    state.select_tab(id2);

    assert_eq!(state.tabs[0].id, id1);
    assert_eq!(state.tabs[1].id, id2);
    assert_eq!(state.tabs[2].id, id3);
}

// ===================================================================
// 7. Empty state
// ===================================================================

#[test]
fn test_empty_state_no_tabs() {
    let state = DockState::default();
    assert!(state.tabs.is_empty());
    assert_eq!(state.active_tab_id, None);
}

#[test]
fn test_empty_after_removing_all_tabs() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);

    state.remove_tab(a);
    state.remove_tab(b);

    assert!(state.tabs.is_empty());
    assert_eq!(state.active_tab_id, None);
}

// ===================================================================
// 8. Multiple operations — compound scenarios
// ===================================================================

#[test]
fn test_add_three_remove_middle_verify_order_and_selection() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::Terminal {
        pod: "pod-a".to_string(),
        container: "c-a".to_string(),
        cluster: "cl-a".to_string(),
    });
    let b = state.add_tab(DockTabKind::LogViewer {
        pod: "pod-b".to_string(),
        container: "c-b".to_string(),
        cluster: "cl-b".to_string(),
    });
    let c = state.add_tab(DockTabKind::PortForwardManager);

    // First tab (a) should be active.
    assert_eq!(state.active_tab_id, Some(a));

    // Remove middle tab (b). Since b is not active, active stays at a.
    state.remove_tab(b);
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(state.tabs[0].id, a);
    assert_eq!(state.tabs[1].id, c);
    assert_eq!(state.active_tab_id, Some(a));
}

#[test]
fn test_add_tab_collapse_add_another_verify_state() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    assert_eq!(state.active_tab_id, Some(a));
    assert!(state.collapsed); // default collapsed

    state.toggle_collapsed(); // expand
    assert!(!state.collapsed);

    let b = state.add_tab(DockTabKind::Terminal {
        pod: "pod-x".to_string(),
        container: "cont-x".to_string(),
        cluster: "cluster-x".to_string(),
    });

    // Active tab should still be the first one.
    assert_eq!(state.active_tab_id, Some(a));
    // Collapsed state should still be expanded (not reset by add_tab).
    assert!(!state.collapsed);
    // Two tabs present.
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(state.tabs[0].id, a);
    assert_eq!(state.tabs[1].id, b);
}

#[test]
fn test_select_remove_add_cycle() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);
    let c = state.add_tab(DockTabKind::PortForwardManager);

    // Select c.
    state.select_tab(c);
    assert_eq!(state.active_tab_id, Some(c));

    // Remove a (not active). Selection stays at c.
    state.remove_tab(a);
    assert_eq!(state.active_tab_id, Some(c));
    assert_eq!(state.tabs.len(), 2);

    // Add a new tab. Selection still at c (not first anymore, but still unchanged).
    let d = state.add_tab(DockTabKind::PortForwardManager);
    assert_eq!(state.active_tab_id, Some(c));
    assert_eq!(state.tabs.len(), 3);

    // Tabs are now [b, c, d].
    assert_eq!(state.tabs[0].id, b);
    assert_eq!(state.tabs[1].id, c);
    assert_eq!(state.tabs[2].id, d);
}

#[test]
fn test_remove_all_then_add_new() {
    let mut state = DockState::default();
    let a = state.add_tab(DockTabKind::PortForwardManager);
    let b = state.add_tab(DockTabKind::PortForwardManager);

    state.remove_tab(a);
    state.remove_tab(b);
    assert!(state.tabs.is_empty());
    assert_eq!(state.active_tab_id, None);

    // Adding a new tab after clearing should auto-select it.
    let c = state.add_tab(DockTabKind::PortForwardManager);
    assert_eq!(state.active_tab_id, Some(c));
    assert_eq!(state.tabs.len(), 1);
}

#[test]
fn test_height_and_collapse_independent() {
    let mut state = DockState::default();
    state.set_height(300.0, 800.0);
    assert!((state.height - 300.0).abs() < f32::EPSILON);
    assert!(state.collapsed);

    state.toggle_collapsed();
    assert!(!state.collapsed);
    // Height unchanged after toggle.
    assert!((state.height - 300.0).abs() < f32::EPSILON);

    state.set_height(400.0, 800.0);
    // Collapsed state unchanged after set_height.
    assert!(!state.collapsed);
    assert!((state.height - 400.0).abs() < f32::EPSILON);
}

#[test]
fn test_tab_kind_stored_correctly() {
    let mut state = DockState::default();
    let id = state.add_tab(DockTabKind::Terminal {
        pod: "my-pod".to_string(),
        container: "my-container".to_string(),
        cluster: "my-cluster".to_string(),
    });

    let tab = state.tabs.iter().find(|t| t.id == id).unwrap();
    match &tab.kind {
        DockTabKind::Terminal { pod, container, cluster } => {
            assert_eq!(pod, "my-pod");
            assert_eq!(container, "my-container");
            assert_eq!(cluster, "my-cluster");
        }
        _ => panic!("Expected Terminal tab kind"),
    }
}

#[test]
fn test_unique_ids_across_all_add_operations() {
    let mut state = DockState::default();
    let mut ids = Vec::new();
    for _ in 0..20 {
        ids.push(state.add_tab(DockTabKind::PortForwardManager));
    }
    // All IDs should be unique.
    let unique_count = {
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        deduped.len()
    };
    assert_eq!(unique_count, 20);
}

// ===================================================================
// 9. T319: Terminal-to-dock wiring (via DockState directly)
// ===================================================================

/// Simulates the core logic of `AppShell::open_terminal_in_dock`:
/// add a Terminal tab, select it, and auto-expand if collapsed.
fn open_terminal_in_dock(dock: &mut DockState, pod: &str, container: &str, cluster: &str) {
    let kind = DockTabKind::Terminal {
        pod: pod.to_string(),
        container: container.to_string(),
        cluster: cluster.to_string(),
    };
    let tab_id = dock.add_tab(kind);
    dock.select_tab(tab_id);
    if dock.collapsed {
        dock.toggle_collapsed();
    }
}

#[test]
fn test_open_terminal_creates_dock_tab() {
    let mut dock = DockState::default();
    open_terminal_in_dock(&mut dock, "nginx-pod", "nginx", "prod-cluster");
    assert_eq!(dock.tabs.len(), 1);
    assert!(matches!(&dock.tabs[0].kind, DockTabKind::Terminal { pod, container, cluster }
        if pod == "nginx-pod" && container == "nginx" && cluster == "prod-cluster"));
}

#[test]
fn test_open_terminal_selects_new_tab() {
    let mut dock = DockState::default();
    // Add a pre-existing tab so the terminal is the second one.
    let _first = dock.add_tab(DockTabKind::PortForwardManager);
    open_terminal_in_dock(&mut dock, "my-pod", "main", "dev");
    // The newly opened terminal tab should be selected.
    let terminal_tab = &dock.tabs[1];
    assert_eq!(dock.active_tab_id, Some(terminal_tab.id));
}

#[test]
fn test_open_terminal_auto_expands_collapsed_dock() {
    let mut dock = DockState::default();
    assert!(dock.collapsed);
    open_terminal_in_dock(&mut dock, "pod-a", "cont-a", "cluster-a");
    assert!(!dock.collapsed);
}

#[test]
fn test_open_terminal_stays_expanded_if_already_expanded() {
    let mut dock = DockState::default();
    dock.toggle_collapsed(); // now expanded
    assert!(!dock.collapsed);
    open_terminal_in_dock(&mut dock, "pod-b", "cont-b", "cluster-b");
    assert!(!dock.collapsed);
}

#[test]
fn test_open_terminal_label_format() {
    let mut dock = DockState::default();
    open_terminal_in_dock(&mut dock, "web-abc", "web", "staging");
    assert_eq!(dock.tabs[0].label, "Terminal: staging/web-abc/web");
}

// ===================================================================
// 10. T320: Logs-to-dock wiring (via DockState directly)
// ===================================================================

/// Simulates the core logic of `AppShell::open_logs_in_dock`:
/// add a LogViewer tab, select it, and auto-expand if collapsed.
fn open_logs_in_dock(dock: &mut DockState, pod: &str, container: &str, cluster: &str) {
    let kind = DockTabKind::LogViewer {
        pod: pod.to_string(),
        container: container.to_string(),
        cluster: cluster.to_string(),
    };
    let tab_id = dock.add_tab(kind);
    dock.select_tab(tab_id);
    if dock.collapsed {
        dock.toggle_collapsed();
    }
}

#[test]
fn test_open_logs_creates_dock_tab() {
    let mut dock = DockState::default();
    open_logs_in_dock(&mut dock, "api-pod", "api", "prod-cluster");
    assert_eq!(dock.tabs.len(), 1);
    assert!(matches!(&dock.tabs[0].kind, DockTabKind::LogViewer { pod, container, cluster }
        if pod == "api-pod" && container == "api" && cluster == "prod-cluster"));
}

#[test]
fn test_open_logs_selects_new_tab() {
    let mut dock = DockState::default();
    let _first = dock.add_tab(DockTabKind::PortForwardManager);
    open_logs_in_dock(&mut dock, "worker-pod", "worker", "dev");
    let log_tab = &dock.tabs[1];
    assert_eq!(dock.active_tab_id, Some(log_tab.id));
}

#[test]
fn test_open_logs_auto_expands_collapsed_dock() {
    let mut dock = DockState::default();
    assert!(dock.collapsed);
    open_logs_in_dock(&mut dock, "pod-a", "cont-a", "cluster-a");
    assert!(!dock.collapsed);
}

#[test]
fn test_open_logs_stays_expanded_if_already_expanded() {
    let mut dock = DockState::default();
    dock.toggle_collapsed();
    assert!(!dock.collapsed);
    open_logs_in_dock(&mut dock, "pod-b", "cont-b", "cluster-b");
    assert!(!dock.collapsed);
}

#[test]
fn test_open_logs_label_format() {
    let mut dock = DockState::default();
    open_logs_in_dock(&mut dock, "api-xyz", "api", "staging");
    assert_eq!(dock.tabs[0].label, "Logs: staging/api-xyz/api");
}

// ===================================================================
// 11. T319 + T320: Combined terminal and logs scenarios
// ===================================================================

#[test]
fn test_open_terminal_then_logs_both_present() {
    let mut dock = DockState::default();
    open_terminal_in_dock(&mut dock, "pod-1", "c1", "cluster");
    open_logs_in_dock(&mut dock, "pod-1", "c1", "cluster");
    assert_eq!(dock.tabs.len(), 2);
    assert!(matches!(&dock.tabs[0].kind, DockTabKind::Terminal { .. }));
    assert!(matches!(&dock.tabs[1].kind, DockTabKind::LogViewer { .. }));
    // The logs tab (last opened) should be active.
    assert_eq!(dock.active_tab_id, Some(dock.tabs[1].id));
}

#[test]
fn test_open_multiple_terminals_all_tracked() {
    let mut dock = DockState::default();
    open_terminal_in_dock(&mut dock, "pod-a", "c1", "cluster1");
    open_terminal_in_dock(&mut dock, "pod-b", "c2", "cluster2");
    open_terminal_in_dock(&mut dock, "pod-c", "c3", "cluster3");
    assert_eq!(dock.tabs.len(), 3);
    // Last opened tab should be selected.
    assert_eq!(dock.active_tab_id, Some(dock.tabs[2].id));
}

#[test]
fn test_open_logs_after_collapsing_dock_re_expands() {
    let mut dock = DockState::default();
    open_terminal_in_dock(&mut dock, "pod-1", "c1", "cluster");
    assert!(!dock.collapsed);
    // User collapses the dock.
    dock.toggle_collapsed();
    assert!(dock.collapsed);
    // Opening logs should auto-expand.
    open_logs_in_dock(&mut dock, "pod-2", "c2", "cluster");
    assert!(!dock.collapsed);
}
