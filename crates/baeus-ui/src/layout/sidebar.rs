use crate::icons::{ResourceCategory, ResourceIcon};
use crate::layout::NavigationTarget;
use gpui::{Context, ElementId, FontWeight, SharedString, Window, div, prelude::*, px, rgb, rgba};
use std::collections::HashSet;
use uuid::Uuid;

/// Represents a node in the navigator tree.
#[derive(Debug, Clone)]
pub enum NavigatorNode {
    /// A leaf node that navigates directly to a resource list.
    Leaf { label: &'static str, target_kind: &'static str },
    /// A branch node that can be expanded to show children.
    Branch { label: &'static str, category: ResourceCategory },
}

#[derive(Debug, Clone)]
pub struct SidebarSection {
    pub category: ResourceCategory,
    pub expanded: bool,
    pub items: Vec<SidebarItem>,
}

#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub icon: ResourceIcon,
    pub label: String,
    pub kind: String,
    pub badge_count: Option<u32>,
}

impl SidebarItem {
    pub fn navigation_target(
        &self,
        category: ResourceCategory,
        cluster_context: &str,
    ) -> NavigationTarget {
        NavigationTarget::ResourceList {
            cluster_context: cluster_context.to_string(),
            category,
            kind: self.kind.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Cluster-first sidebar types (FR-047 through FR-051)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

/// How the cluster was added (kubeconfig vs native EKS integration).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ClusterSource {
    /// Discovered from a kubeconfig file.
    #[default]
    Kubeconfig,
    /// Connected via native AWS EKS integration.
    AwsEks { region: String, account_id: Option<String> },
}

#[derive(Debug, Clone)]
pub struct ClusterEntry {
    pub id: Uuid,
    pub context_name: String,
    pub display_name: String,
    pub initials: String,
    pub color: u32,
    pub status: ClusterStatus,
    pub expanded: bool,
    pub sections: Vec<SidebarSection>,
    /// Per-category expand/collapse state for the Navigator tree (Phase 15 / T303).
    /// Categories in this set are expanded; absent categories are collapsed.
    pub expanded_categories: HashSet<ResourceCategory>,
    /// Optional path to a custom icon image for this cluster.
    pub custom_icon_path: Option<String>,
    /// How this cluster was discovered/connected.
    pub source: ClusterSource,
}

/// Generate 2-letter uppercase initials from a cluster name.
/// Takes the first letter of the first two words. Single-word names repeat the first letter.
/// E.g. "kind-dev" → "KD", "production" → "PR", "my-cool-cluster" → "MC"
pub fn generate_initials(name: &str) -> String {
    let parts: Vec<&str> = name.split(['-', '_', '.', ' ']).filter(|s| !s.is_empty()).collect();

    match parts.len() {
        0 => "??".to_string(),
        1 => {
            let chars: Vec<char> = parts[0].chars().collect();
            if chars.len() >= 2 {
                format!("{}{}", chars[0], chars[1]).to_uppercase()
            } else {
                format!("{}{}", chars[0], chars[0]).to_uppercase()
            }
        }
        _ => {
            let a = parts[0].chars().next().unwrap_or('?');
            let b = parts[1].chars().next().unwrap_or('?');
            format!("{a}{b}").to_uppercase()
        }
    }
}

/// Generate a deterministic color from a preset palette based on the cluster name.
/// Uses a simple hash to pick from a curated set of colors.
pub fn generate_cluster_color(name: &str) -> u32 {
    const PALETTE: [u32; 12] = [
        0x3B82F6, // blue
        0x10B981, // emerald
        0xF59E0B, // amber
        0xEF4444, // red
        0x8B5CF6, // violet
        0xEC4899, // pink
        0x06B6D4, // cyan
        0xF97316, // orange
        0x6366F1, // indigo
        0x14B8A6, // teal
        0xA855F7, // purple
        0x84CC16, // lime
    ];

    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// Build a tree connector prefix string using Unicode box-drawing characters.
/// - depth: nesting level (0 = top-level, 1 = under cluster, 2 = under category)
/// - is_last: whether this is the last sibling at its depth
/// - parent_has_more: for each ancestor level, whether there are more siblings below
///
/// Examples:
///   depth=1, is_last=false -> "├─ "
///   depth=1, is_last=true  -> "└─ "
///   depth=2, is_last=false, parent_has_more=[true] -> "│  ├─ "
///   depth=2, is_last=true,  parent_has_more=[true] -> "│  └─ "
///   depth=2, is_last=false, parent_has_more=[false] -> "   ├─ "
pub fn tree_connector_prefix(depth: usize, is_last: bool, parent_has_more: &[bool]) -> String {
    let mut prefix = String::new();
    // Add continuation lines for each parent level
    for i in 0..depth.saturating_sub(1) {
        if parent_has_more.get(i).copied().unwrap_or(false) {
            prefix.push_str("│  ");
        } else {
            prefix.push_str("   ");
        }
    }
    // Add the connector for this level
    if depth > 0 {
        if is_last {
            prefix.push_str("└─ ");
        } else {
            prefix.push_str("├─ ");
        }
    }
    prefix
}

#[derive(Debug)]
pub struct SidebarState {
    pub sections: Vec<SidebarSection>,
    pub active_kind: Option<String>,
    pub clusters: Vec<ClusterEntry>,
    pub selected_cluster_id: Option<Uuid>,
    /// When set, the sidebar shows only this cluster's tree (drill-into mode).
    pub drill_into_cluster: Option<Uuid>,
    /// Width of the sidebar in logical pixels (min 200.0).
    pub sidebar_width: f32,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            sections: default_sections(),
            active_kind: None,
            clusters: Vec::new(),
            selected_cluster_id: None,
            drill_into_cluster: None,
            sidebar_width: 260.0,
        }
    }
}

impl SidebarState {
    /// Returns the fixed navigator tree structure for a cluster.
    /// Top-level items are leaves (Overview, Nodes, Namespaces, Events) or
    /// branches (Workloads, Config, Network, Storage, Helm, Access Control, Custom Resources).
    pub fn navigator_tree() -> Vec<NavigatorNode> {
        vec![
            NavigatorNode::Leaf { label: "Overview", target_kind: "__Dashboard__" },
            NavigatorNode::Leaf { label: "Topology", target_kind: "__ClusterTopology__" },
            NavigatorNode::Leaf { label: "Nodes", target_kind: "Node" },
            NavigatorNode::Branch { label: "Workloads", category: ResourceCategory::Workloads },
            NavigatorNode::Branch { label: "Config", category: ResourceCategory::Configuration },
            NavigatorNode::Branch { label: "Network", category: ResourceCategory::Network },
            NavigatorNode::Branch { label: "Storage", category: ResourceCategory::Storage },
            NavigatorNode::Leaf { label: "Namespaces", target_kind: "Namespace" },
            NavigatorNode::Leaf { label: "Events", target_kind: "Event" },
            NavigatorNode::Branch { label: "Helm", category: ResourceCategory::Helm },
            NavigatorNode::Branch { label: "Access Control", category: ResourceCategory::Rbac },
            NavigatorNode::Branch { label: "ArgoCD", category: ResourceCategory::ArgoCD },
            NavigatorNode::Branch { label: "Custom Resources", category: ResourceCategory::Custom },
        ]
    }

    pub fn toggle_section(&mut self, category: ResourceCategory) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.category == category) {
            section.expanded = !section.expanded;
        }
    }

    pub fn expand_section(&mut self, category: ResourceCategory) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.category == category) {
            section.expanded = true;
        }
    }

    pub fn collapse_all(&mut self) {
        for section in &mut self.sections {
            section.expanded = false;
        }
    }

    pub fn update_badge(&mut self, kind: &str, count: Option<u32>) {
        for section in &mut self.sections {
            for item in &mut section.items {
                if item.kind == kind {
                    item.badge_count = count;
                    return;
                }
            }
        }
    }

    /// Set the active kind and auto-expand its parent section.
    pub fn set_active_kind(&mut self, kind: &str) {
        self.active_kind = Some(kind.to_string());
        // Auto-expand the section containing this kind
        for section in &mut self.sections {
            if section.items.iter().any(|item| item.kind == kind) {
                section.expanded = true;
            }
        }
    }

    /// Clear the active kind selection.
    pub fn clear_active_kind(&mut self) {
        self.active_kind = None;
    }

    /// Navigate to a resource kind: sets active, expands section, returns the NavigationTarget.
    /// Requires a cluster_context to scope the navigation target.
    pub fn navigate_to_kind(
        &mut self,
        kind: &str,
        cluster_context: &str,
    ) -> Option<NavigationTarget> {
        self.set_active_kind(kind);
        self.find_kind_category(kind).map(|category| NavigationTarget::ResourceList {
            cluster_context: cluster_context.to_string(),
            category,
            kind: kind.to_string(),
        })
    }

    /// Find which category a kind belongs to.
    pub fn find_kind_category(&self, kind: &str) -> Option<ResourceCategory> {
        self.sections.iter().find(|s| s.items.iter().any(|i| i.kind == kind)).map(|s| s.category)
    }

    /// Returns true if the given kind is the currently active one.
    pub fn is_active(&self, kind: &str) -> bool {
        self.active_kind.as_deref() == Some(kind)
    }

    /// Navigate to the namespace resource map view.
    /// Returns a `NavigationTarget::NamespaceMap` and sets the active kind
    /// to a sentinel value so it can be detected.
    pub fn navigate_to_map(&mut self, cluster_context: &str) -> NavigationTarget {
        self.active_kind = Some("__NamespaceMap__".to_string());
        NavigationTarget::NamespaceMap { cluster_context: cluster_context.to_string() }
    }

    /// Returns true if the namespace map is currently active.
    pub fn is_map_active(&self) -> bool {
        self.active_kind.as_deref() == Some("__NamespaceMap__")
    }

    // --- Cluster-first sidebar methods ---

    /// Add a cluster to the sidebar. Returns the generated UUID.
    pub fn add_cluster(&mut self, context_name: &str, display_name: &str) -> Uuid {
        let id = Uuid::new_v4();
        let entry = ClusterEntry {
            id,
            context_name: context_name.to_string(),
            display_name: display_name.to_string(),
            initials: generate_initials(display_name),
            color: generate_cluster_color(context_name),
            status: ClusterStatus::Disconnected,
            expanded: false,
            sections: default_sections(),
            expanded_categories: HashSet::new(),
            custom_icon_path: None,
            source: ClusterSource::default(),
        };
        self.clusters.push(entry);

        // Auto-select first cluster
        if self.selected_cluster_id.is_none() {
            self.selected_cluster_id = Some(id);
        }
        id
    }

    /// Remove a cluster from the sidebar by ID. Returns true if found and removed.
    pub fn remove_cluster(&mut self, id: Uuid) -> bool {
        let len_before = self.clusters.len();
        self.clusters.retain(|c| c.id != id);
        if self.selected_cluster_id == Some(id) {
            self.selected_cluster_id = self.clusters.first().map(|c| c.id);
        }
        if self.drill_into_cluster == Some(id) {
            self.drill_into_cluster = None;
        }
        self.clusters.len() < len_before
    }

    /// Select a cluster by ID.
    pub fn select_cluster(&mut self, id: Uuid) {
        if self.clusters.iter().any(|c| c.id == id) {
            self.selected_cluster_id = Some(id);
        }
    }

    /// Toggle a cluster's expanded/collapsed state.
    pub fn toggle_cluster(&mut self, id: Uuid) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == id) {
            cluster.expanded = !cluster.expanded;
        }
    }

    /// Get a reference to the currently selected cluster.
    pub fn selected_cluster(&self) -> Option<&ClusterEntry> {
        self.selected_cluster_id.and_then(|id| self.clusters.iter().find(|c| c.id == id))
    }

    /// Returns true if clusters are populated (use cluster-first layout).
    pub fn has_clusters(&self) -> bool {
        !self.clusters.is_empty()
    }

    // --- Phase 15 / T303: Navigator cluster category expand/collapse ---

    /// Toggle the expand/collapse state of a resource category within a specific cluster.
    /// If the category is currently expanded, it will be collapsed, and vice versa.
    pub fn toggle_category_expand(&mut self, cluster_id: Uuid, category: ResourceCategory) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == cluster_id) {
            if !cluster.expanded_categories.remove(&category) {
                cluster.expanded_categories.insert(category);
            }
        }
    }

    /// Returns true if the given resource category is expanded for the specified cluster.
    pub fn is_category_expanded(&self, cluster_id: Uuid, category: ResourceCategory) -> bool {
        self.clusters
            .iter()
            .find(|c| c.id == cluster_id)
            .map(|c| c.expanded_categories.contains(&category))
            .unwrap_or(false)
    }

    /// Ensure a category is expanded for a given cluster (expand-only, never collapses).
    /// Used by T310 contextual tracking to auto-expand the category matching the active tab.
    pub fn ensure_category_expanded(&mut self, cluster_id: Uuid, category: ResourceCategory) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == cluster_id) {
            cluster.expanded_categories.insert(category);
        }
    }

    /// Ensure a cluster node is expanded (expand-only, never collapses).
    /// Used by T310 contextual tracking to auto-expand the cluster matching the active tab.
    pub fn ensure_cluster_expanded(&mut self, cluster_id: Uuid) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == cluster_id) {
            cluster.expanded = true;
        }
    }

    /// Find a cluster by its context name and return its ID.
    /// Used by T310 to map from an active tab's cluster_context to a cluster ID.
    pub fn find_cluster_id_by_context(&self, context_name: &str) -> Option<Uuid> {
        self.clusters.iter().find(|c| c.context_name == context_name).map(|c| c.id)
    }

    /// Resolve a context name to its display name (e.g. "ClusterName(Context)").
    /// Falls back to the context name itself if not found.
    pub fn display_name_for_context(&self, context_name: &str) -> String {
        self.clusters
            .iter()
            .find(|c| c.context_name == context_name)
            .map(|c| c.display_name.clone())
            .unwrap_or_else(|| context_name.to_string())
    }

    /// Update a badge count for a specific resource kind within a specific cluster.
    pub fn update_cluster_badge(&mut self, cluster_id: Uuid, kind: &str, count: Option<u32>) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == cluster_id) {
            for section in &mut cluster.sections {
                for item in &mut section.items {
                    if item.kind == kind {
                        item.badge_count = count;
                        return;
                    }
                }
            }
        }
    }

    /// Returns the full FR-071 resource category list that each cluster should have.
    /// The Navigator tree categories per FR-071:
    /// Workloads, Configuration, Network, Storage, Rbac (Access Control),
    /// Monitoring (Events), Helm, Custom, Plugins, Cluster (Namespaces/Overview)
    pub fn fr071_categories() -> &'static [ResourceCategory] {
        ResourceCategory::all()
    }

    /// Get the list of resource kinds expected under a given category per FR-071.
    pub fn fr071_category_kinds(category: ResourceCategory) -> Vec<&'static str> {
        match category {
            ResourceCategory::Workloads => vec![
                "Pod",
                "Deployment",
                "DaemonSet",
                "StatefulSet",
                "ReplicaSet",
                "ReplicationController",
                "Job",
                "CronJob",
            ],
            ResourceCategory::Configuration => vec![
                "ConfigMap",
                "Secret",
                "ResourceQuota",
                "LimitRange",
                "HorizontalPodAutoscaler",
                "VerticalPodAutoscaler",
                "PodDisruptionBudget",
                "PriorityClass",
                "RuntimeClass",
                "Lease",
                "MutatingWebhookConfiguration",
                "ValidatingWebhookConfiguration",
            ],
            ResourceCategory::Network => vec![
                "Service",
                "Endpoints",
                "Ingress",
                "IngressClass",
                "NetworkPolicy",
                "PortForwarding",
            ],
            ResourceCategory::Storage => {
                vec!["PersistentVolumeClaim", "PersistentVolume", "StorageClass"]
            }
            ResourceCategory::Cluster => vec!["Namespace", "Node"],
            ResourceCategory::Monitoring => vec!["Event"],
            ResourceCategory::Helm => vec!["HelmChart", "HelmRelease"],
            ResourceCategory::Rbac => vec![
                "ServiceAccount",
                "ClusterRole",
                "Role",
                "ClusterRoleBinding",
                "RoleBinding",
                "PodSecurityPolicy",
            ],
            ResourceCategory::Custom => vec!["CustomResourceDefinition"],
            ResourceCategory::ArgoCD => vec!["Application", "ApplicationSet", "AppProject"],
            ResourceCategory::Plugins => vec!["Plugin"],
        }
    }

    // --- Phase 15 / T305: Navigator tree model refactor ---

    /// Toggle a cluster's `expanded` field by ID (Navigator tree expand/collapse).
    /// This controls whether the cluster node itself is expanded in the tree,
    /// distinct from `toggle_cluster` which already exists for the legacy sidebar.
    pub fn toggle_cluster_expand(&mut self, id: Uuid) {
        if let Some(cluster) = self.clusters.iter_mut().find(|c| c.id == id) {
            cluster.expanded = !cluster.expanded;
        }
    }

    /// Get the resource types (sidebar items) for a given category within a specific cluster.
    /// If the cluster already has a section for that category, returns references to its items.
    /// Otherwise, builds items from the FR-071 category kinds specification.
    pub fn get_resource_types(
        &self,
        cluster_id: Uuid,
        category: ResourceCategory,
    ) -> Vec<&SidebarItem> {
        if let Some(cluster) = self.clusters.iter().find(|c| c.id == cluster_id) {
            // Check if the cluster already has a section for this category
            if let Some(section) = cluster.sections.iter().find(|s| s.category == category) {
                if !section.items.is_empty() {
                    return section.items.iter().collect();
                }
            }
        }
        // If no cluster found or no section with items, return empty.
        // Callers can use fr071_category_kinds() to build items on-the-fly.
        Vec::new()
    }

    // --- Phase 15 / T306: Drill-into and sidebar width ---

    /// Enter drill-into mode for the given cluster. The sidebar will show
    /// only this cluster's resource tree.
    pub fn enter_drill_into(&mut self, id: Uuid) {
        self.drill_into_cluster = Some(id);
    }

    /// Exit drill-into mode, returning to the multi-cluster overview.
    pub fn exit_drill_into(&mut self) {
        self.drill_into_cluster = None;
    }

    /// Set the sidebar width in logical pixels, clamped to a minimum of 200.0.
    pub fn set_width(&mut self, w: f32) {
        self.sidebar_width = if w < 200.0 { 200.0 } else { w };
    }

    /// Returns true if the sidebar is currently in drill-into mode
    /// (showing a single cluster's tree).
    pub fn is_drill_into(&self) -> bool {
        self.drill_into_cluster.is_some()
    }

    /// Flatten a cluster's navigator tree into a list of entries for `uniform_list` rendering.
    /// Walks `navigator_tree()`, emitting entries at depth 1 for top-level items and
    /// depth 2 for children of expanded categories.
    pub fn flatten_navigator_tree(&self, cluster: &ClusterEntry) -> Vec<NavigatorFlatEntry> {
        let nav_tree = Self::navigator_tree();
        let tree_len = nav_tree.len();
        let mut entries = Vec::new();

        for (node_idx, node) in nav_tree.iter().enumerate() {
            let is_last_top = node_idx == tree_len - 1;

            match node {
                NavigatorNode::Leaf { label, target_kind } => {
                    entries.push(NavigatorFlatEntry::Leaf {
                        depth: 1,
                        label: label.to_string(),
                        target_kind: target_kind.to_string(),
                        cluster_id: cluster.id,
                        context_name: cluster.context_name.clone(),
                        is_last_sibling: is_last_top,
                    });
                }
                NavigatorNode::Branch { label, category } => {
                    let cat_expanded = cluster.expanded_categories.contains(category);

                    entries.push(NavigatorFlatEntry::CategoryHeader {
                        depth: 1,
                        label: label.to_string(),
                        category: *category,
                        cluster_id: cluster.id,
                        expanded: cat_expanded,
                        is_last_sibling: is_last_top,
                    });

                    if cat_expanded {
                        let kinds = self.get_category_kinds(cluster, *category);
                        let kinds_len = kinds.len();
                        for (idx, (kind_label, kind, badge)) in kinds.iter().enumerate() {
                            let is_last = idx == kinds_len - 1;
                            entries.push(NavigatorFlatEntry::ResourceKind {
                                depth: 2,
                                label: kind_label.clone(),
                                kind: kind.clone(),
                                category: *category,
                                cluster_id: cluster.id,
                                context_name: cluster.context_name.clone(),
                                badge_count: *badge,
                                is_last_sibling: is_last,
                            });
                        }
                    }
                }
            }
        }

        entries
    }

    /// Get the list of (label, kind, badge_count) for a category within a cluster.
    /// Tries the cluster's sections first, falls back to `fr071_category_kinds()`.
    fn get_category_kinds(
        &self,
        cluster: &ClusterEntry,
        category: ResourceCategory,
    ) -> Vec<(String, String, Option<u32>)> {
        if let Some(section) = cluster.sections.iter().find(|s| s.category == category) {
            if !section.items.is_empty() {
                return section
                    .items
                    .iter()
                    .map(|i| (i.label.clone(), i.kind.clone(), i.badge_count))
                    .collect();
            }
        }
        // Fall back to fr071 defaults (label = kind name, no badge)
        Self::fr071_category_kinds(category)
            .into_iter()
            .map(|k| (k.to_string(), k.to_string(), None))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Flat entry types for uniform_list navigator rendering
// ---------------------------------------------------------------------------

/// A flattened entry in the navigator tree, used with `uniform_list` for
/// efficient rendering and indent guide computation.
#[derive(Debug, Clone)]
pub enum NavigatorFlatEntry {
    /// A leaf node (Overview, Nodes, Namespaces, Events).
    Leaf {
        depth: usize,
        label: String,
        target_kind: String,
        cluster_id: Uuid,
        context_name: String,
        is_last_sibling: bool,
    },
    /// A category header (Workloads, Config, Network, etc.).
    CategoryHeader {
        depth: usize,
        label: String,
        category: ResourceCategory,
        cluster_id: Uuid,
        expanded: bool,
        is_last_sibling: bool,
    },
    /// A resource kind under an expanded category.
    ResourceKind {
        depth: usize,
        label: String,
        kind: String,
        category: ResourceCategory,
        cluster_id: Uuid,
        context_name: String,
        badge_count: Option<u32>,
        is_last_sibling: bool,
    },
}

impl NavigatorFlatEntry {
    /// Returns the tree depth of this entry.
    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf { depth, .. }
            | Self::CategoryHeader { depth, .. }
            | Self::ResourceKind { depth, .. } => *depth,
        }
    }

    /// Returns whether this entry is the last sibling at its depth level.
    pub fn is_last_sibling(&self) -> bool {
        match self {
            Self::Leaf { is_last_sibling, .. }
            | Self::CategoryHeader { is_last_sibling, .. }
            | Self::ResourceKind { is_last_sibling, .. } => *is_last_sibling,
        }
    }
}

fn default_sections() -> Vec<SidebarSection> {
    vec![
        SidebarSection {
            category: ResourceCategory::Workloads,
            expanded: true,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::Pod,
                    label: "Pods".to_string(),
                    kind: "Pod".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Deployment,
                    label: "Deployments".to_string(),
                    kind: "Deployment".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::StatefulSet,
                    label: "StatefulSets".to_string(),
                    kind: "StatefulSet".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::DaemonSet,
                    label: "DaemonSets".to_string(),
                    kind: "DaemonSet".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ReplicaSet,
                    label: "ReplicaSets".to_string(),
                    kind: "ReplicaSet".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Job,
                    label: "Jobs".to_string(),
                    kind: "Job".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::CronJob,
                    label: "CronJobs".to_string(),
                    kind: "CronJob".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::HorizontalPodAutoscaler,
                    label: "HPAs".to_string(),
                    kind: "HorizontalPodAutoscaler".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Network,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::Service,
                    label: "Services".to_string(),
                    kind: "Service".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Ingress,
                    label: "Ingresses".to_string(),
                    kind: "Ingress".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::NetworkPolicy,
                    label: "Network Policies".to_string(),
                    kind: "NetworkPolicy".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Endpoints,
                    label: "Endpoints".to_string(),
                    kind: "Endpoints".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::EndpointSlice,
                    label: "Endpoint Slices".to_string(),
                    kind: "EndpointSlice".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::IngressClass,
                    label: "Ingress Classes".to_string(),
                    kind: "IngressClass".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Configuration,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::ConfigMap,
                    label: "ConfigMaps".to_string(),
                    kind: "ConfigMap".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Secret,
                    label: "Secrets".to_string(),
                    kind: "Secret".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ResourceQuota,
                    label: "Resource Quotas".to_string(),
                    kind: "ResourceQuota".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::LimitRange,
                    label: "Limit Ranges".to_string(),
                    kind: "LimitRange".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Storage,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::PersistentVolume,
                    label: "Persistent Volumes".to_string(),
                    kind: "PersistentVolume".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::PersistentVolumeClaim,
                    label: "PVCs".to_string(),
                    kind: "PersistentVolumeClaim".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::StorageClass,
                    label: "Storage Classes".to_string(),
                    kind: "StorageClass".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Rbac,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::ServiceAccount,
                    label: "Service Accounts".to_string(),
                    kind: "ServiceAccount".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Role,
                    label: "Roles".to_string(),
                    kind: "Role".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ClusterRole,
                    label: "Cluster Roles".to_string(),
                    kind: "ClusterRole".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::RoleBinding,
                    label: "Role Bindings".to_string(),
                    kind: "RoleBinding".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ClusterRoleBinding,
                    label: "Cluster Role Bindings".to_string(),
                    kind: "ClusterRoleBinding".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Cluster,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::Node,
                    label: "Nodes".to_string(),
                    kind: "Node".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Namespace,
                    label: "Namespaces".to_string(),
                    kind: "Namespace".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::PriorityClass,
                    label: "Priority Classes".to_string(),
                    kind: "PriorityClass".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::Lease,
                    label: "Leases".to_string(),
                    kind: "Lease".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::PodDisruptionBudget,
                    label: "Pod Disruption Budgets".to_string(),
                    kind: "PodDisruptionBudget".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ValidatingWebhookConfiguration,
                    label: "Validating Webhooks".to_string(),
                    kind: "ValidatingWebhookConfiguration".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::MutatingWebhookConfiguration,
                    label: "Mutating Webhooks".to_string(),
                    kind: "MutatingWebhookConfiguration".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Monitoring,
            expanded: false,
            items: vec![SidebarItem {
                icon: ResourceIcon::Event,
                label: "Events".to_string(),
                kind: "Event".to_string(),
                badge_count: None,
            }],
        },
        SidebarSection {
            category: ResourceCategory::Helm,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::HelmRelease,
                    label: "Releases".to_string(),
                    kind: "HelmRelease".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::HelmChart,
                    label: "Install Chart".to_string(),
                    kind: "HelmChart".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::ArgoCD,
            expanded: false,
            items: vec![
                SidebarItem {
                    icon: ResourceIcon::Application,
                    label: "Applications".to_string(),
                    kind: "Application".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::ApplicationSet,
                    label: "ApplicationSets".to_string(),
                    kind: "ApplicationSet".to_string(),
                    badge_count: None,
                },
                SidebarItem {
                    icon: ResourceIcon::AppProject,
                    label: "AppProjects".to_string(),
                    kind: "AppProject".to_string(),
                    badge_count: None,
                },
            ],
        },
        SidebarSection {
            category: ResourceCategory::Plugins,
            expanded: false,
            items: vec![SidebarItem {
                icon: ResourceIcon::Plugin,
                label: "Plugins".to_string(),
                kind: "Plugin".to_string(),
                badge_count: None,
            }],
        },
        SidebarSection {
            category: ResourceCategory::Custom,
            expanded: false,
            items: vec![SidebarItem {
                icon: ResourceIcon::CustomResource,
                label: "Custom Resources".to_string(),
                kind: "CustomResourceDefinition".to_string(),
                badge_count: None,
            }],
        },
    ]
}

// ---------------------------------------------------------------------------
// GPUI View
// ---------------------------------------------------------------------------

pub struct SidebarView {
    state: SidebarState,
}

impl Default for SidebarView {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarView {
    pub fn new() -> Self {
        Self { state: SidebarState::default() }
    }

    pub fn state(&self) -> &SidebarState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut SidebarState {
        &mut self.state
    }
}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Clone the sections data so we can iterate without borrowing self mutably.
        let sections: Vec<SidebarSection> = self.state.sections.clone();
        let active_kind = self.state.active_kind.clone();

        let mut sidebar = div()
            .id("sidebar")
            .flex()
            .flex_col()
            .h_full()
            .w_48()
            .bg(rgb(0x111827))
            .border_r_1()
            .border_color(rgb(0x374151))
            .overflow_y_scroll()
            .py_2();

        for (section_idx, section) in sections.iter().enumerate() {
            let category = section.category;
            let expanded = section.expanded;
            let header_text = SharedString::from(category.label().to_string());
            let arrow = if expanded { "v " } else { "> " };
            let arrow_text = SharedString::from(arrow.to_string());

            let mut section_div = div().flex().flex_col().w_full();

            // Section header -- clickable to toggle expand/collapse
            let header_id =
                ElementId::Name(SharedString::from(format!("sidebar-section-{section_idx}")));
            let header = div()
                .id(header_id)
                .flex()
                .items_center()
                .px_3()
                .py_1()
                .cursor_pointer()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x9CA3AF))
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    this.state.toggle_section(category);
                }))
                .child(arrow_text)
                .child(header_text);

            section_div = section_div.child(header);

            // Items (only when expanded)
            if expanded {
                for (item_idx, item) in section.items.iter().enumerate() {
                    let is_active = active_kind.as_deref() == Some(&item.kind);
                    let item_label = SharedString::from(item.label.clone());
                    let item_kind = item.kind.clone();

                    let item_id = ElementId::Name(SharedString::from(format!(
                        "sidebar-item-{section_idx}-{item_idx}"
                    )));

                    let mut item_row = div()
                        .id(item_id)
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .cursor_pointer()
                        .text_sm()
                        .text_color(rgb(0xD1D5DB));

                    // Active item highlight
                    if is_active {
                        item_row = item_row.bg(rgba(0x60A5FA20)).text_color(rgb(0x60A5FA));
                    }

                    // Click handler to navigate
                    item_row = item_row.on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.state.navigate_to_kind(&item_kind, "default");
                    }));

                    // Icon placeholder: small colored dot
                    let dot = div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .flex_shrink_0()
                        .bg(if is_active { rgb(0x60A5FA) } else { rgb(0x6B7280) });

                    item_row = item_row.child(dot).child(item_label);

                    // Badge count
                    if let Some(count) = item.badge_count {
                        let badge_text = SharedString::from(count.to_string());
                        let badge = div()
                            .ml_auto()
                            .px_2()
                            .py(px(1.0))
                            .rounded(px(8.0))
                            .bg(rgb(0x374151))
                            .text_xs()
                            .text_color(rgb(0x9CA3AF))
                            .child(badge_text);
                        item_row = item_row.child(badge);
                    }

                    section_div = section_div.child(item_row);
                }
            }

            sidebar = sidebar.child(section_div);
        }

        sidebar
    }
}

// ===================================================================
// T303: NavigatorCluster inline tests (Phase 15)
// ===================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // FR-071: Each cluster has the correct expandable categories
    // -----------------------------------------------------------------

    #[test]
    fn test_navigator_cluster_has_all_fr071_categories() {
        // FR-071 mandates these categories in the Navigator tree:
        // Workloads, Config, Network, Storage, Namespaces (Cluster),
        // Events (Monitoring), Helm, Access Control (Rbac), Custom Resources, Plugins
        let categories = SidebarState::fr071_categories();
        assert!(categories.contains(&ResourceCategory::Workloads));
        assert!(categories.contains(&ResourceCategory::Configuration));
        assert!(categories.contains(&ResourceCategory::Network));
        assert!(categories.contains(&ResourceCategory::Storage));
        assert!(categories.contains(&ResourceCategory::Cluster));
        assert!(categories.contains(&ResourceCategory::Monitoring));
        assert!(categories.contains(&ResourceCategory::Helm));
        assert!(categories.contains(&ResourceCategory::Rbac));
        assert!(categories.contains(&ResourceCategory::Custom));
        assert!(categories.contains(&ResourceCategory::Plugins));
    }

    // -----------------------------------------------------------------
    // FR-071: Workloads category has correct resource types
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_workloads_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Workloads);
        assert!(kinds.contains(&"Pod"));
        assert!(kinds.contains(&"Deployment"));
        assert!(kinds.contains(&"DaemonSet"));
        assert!(kinds.contains(&"StatefulSet"));
        assert!(kinds.contains(&"ReplicaSet"));
        assert!(kinds.contains(&"ReplicationController"));
        assert!(kinds.contains(&"Job"));
        assert!(kinds.contains(&"CronJob"));
        assert_eq!(kinds.len(), 8);
    }

    // -----------------------------------------------------------------
    // FR-071: Config category has correct resource types
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_config_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Configuration);
        assert!(kinds.contains(&"ConfigMap"));
        assert!(kinds.contains(&"Secret"));
        assert!(kinds.contains(&"ResourceQuota"));
        assert!(kinds.contains(&"LimitRange"));
        assert!(kinds.contains(&"HorizontalPodAutoscaler"));
        assert!(kinds.contains(&"VerticalPodAutoscaler"));
        assert!(kinds.contains(&"PodDisruptionBudget"));
        assert!(kinds.contains(&"PriorityClass"));
        assert!(kinds.contains(&"RuntimeClass"));
        assert!(kinds.contains(&"Lease"));
        assert!(kinds.contains(&"MutatingWebhookConfiguration"));
        assert!(kinds.contains(&"ValidatingWebhookConfiguration"));
        assert_eq!(kinds.len(), 12);
    }

    // -----------------------------------------------------------------
    // FR-071: Network category has correct resource types
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_network_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Network);
        assert!(kinds.contains(&"Service"));
        assert!(kinds.contains(&"Endpoints"));
        assert!(kinds.contains(&"Ingress"));
        assert!(kinds.contains(&"IngressClass"));
        assert!(kinds.contains(&"NetworkPolicy"));
        assert!(kinds.contains(&"PortForwarding"));
        assert_eq!(kinds.len(), 6);
    }

    // -----------------------------------------------------------------
    // FR-071: Storage category has correct resource types
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_storage_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Storage);
        assert!(kinds.contains(&"PersistentVolumeClaim"));
        assert!(kinds.contains(&"PersistentVolume"));
        assert!(kinds.contains(&"StorageClass"));
        assert_eq!(kinds.len(), 3);
    }

    // -----------------------------------------------------------------
    // FR-071: Cluster/Namespaces category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_cluster_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Cluster);
        assert!(kinds.contains(&"Namespace"));
        assert!(kinds.contains(&"Node"));
        assert_eq!(kinds.len(), 2);
    }

    // -----------------------------------------------------------------
    // FR-071: Events/Monitoring category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_monitoring_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Monitoring);
        assert!(kinds.contains(&"Event"));
        assert_eq!(kinds.len(), 1);
    }

    // -----------------------------------------------------------------
    // FR-071: Helm category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_helm_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Helm);
        assert!(kinds.contains(&"HelmChart"));
        assert!(kinds.contains(&"HelmRelease"));
        assert_eq!(kinds.len(), 2);
    }

    // -----------------------------------------------------------------
    // FR-071: Access Control (Rbac) category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_rbac_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Rbac);
        assert!(kinds.contains(&"ServiceAccount"));
        assert!(kinds.contains(&"ClusterRole"));
        assert!(kinds.contains(&"Role"));
        assert!(kinds.contains(&"ClusterRoleBinding"));
        assert!(kinds.contains(&"RoleBinding"));
        assert!(kinds.contains(&"PodSecurityPolicy"));
        assert_eq!(kinds.len(), 6);
    }

    // -----------------------------------------------------------------
    // FR-071: Custom Resources category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_custom_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Custom);
        assert!(kinds.contains(&"CustomResourceDefinition"));
        assert_eq!(kinds.len(), 1);
    }

    // -----------------------------------------------------------------
    // FR-071: Plugins category
    // -----------------------------------------------------------------

    #[test]
    fn test_fr071_plugins_kinds() {
        let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Plugins);
        assert!(kinds.contains(&"Plugin"));
        assert_eq!(kinds.len(), 1);
    }

    // -----------------------------------------------------------------
    // Category expand/collapse per cluster
    // -----------------------------------------------------------------

    #[test]
    fn test_new_cluster_has_no_expanded_categories() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");
        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
        assert!(cluster.expanded_categories.is_empty());
    }

    #[test]
    fn test_toggle_category_expand_expands_category() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
        state.toggle_category_expand(id, ResourceCategory::Workloads);
        assert!(state.is_category_expanded(id, ResourceCategory::Workloads));
    }

    #[test]
    fn test_toggle_category_expand_collapses_expanded_category() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.toggle_category_expand(id, ResourceCategory::Workloads);
        assert!(state.is_category_expanded(id, ResourceCategory::Workloads));

        state.toggle_category_expand(id, ResourceCategory::Workloads);
        assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
    }

    #[test]
    fn test_toggle_category_expand_independent_categories() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        // Expand Workloads
        state.toggle_category_expand(id, ResourceCategory::Workloads);
        // Expand Network
        state.toggle_category_expand(id, ResourceCategory::Network);

        assert!(state.is_category_expanded(id, ResourceCategory::Workloads));
        assert!(state.is_category_expanded(id, ResourceCategory::Network));
        assert!(!state.is_category_expanded(id, ResourceCategory::Storage));

        // Collapse Workloads only
        state.toggle_category_expand(id, ResourceCategory::Workloads);
        assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
        assert!(state.is_category_expanded(id, ResourceCategory::Network));
    }

    // -----------------------------------------------------------------
    // Category expand/collapse independent per cluster
    // -----------------------------------------------------------------

    #[test]
    fn test_toggle_category_expand_independent_per_cluster() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        // Expand Workloads on cluster 1 only
        state.toggle_category_expand(id1, ResourceCategory::Workloads);

        assert!(state.is_category_expanded(id1, ResourceCategory::Workloads));
        assert!(!state.is_category_expanded(id2, ResourceCategory::Workloads));
    }

    #[test]
    fn test_toggle_category_expand_different_categories_different_clusters() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        // Expand Workloads on cluster 1
        state.toggle_category_expand(id1, ResourceCategory::Workloads);
        // Expand Storage on cluster 2
        state.toggle_category_expand(id2, ResourceCategory::Storage);

        assert!(state.is_category_expanded(id1, ResourceCategory::Workloads));
        assert!(!state.is_category_expanded(id1, ResourceCategory::Storage));
        assert!(!state.is_category_expanded(id2, ResourceCategory::Workloads));
        assert!(state.is_category_expanded(id2, ResourceCategory::Storage));
    }

    #[test]
    fn test_toggle_category_on_nonexistent_cluster_is_noop() {
        let mut state = SidebarState::default();
        let fake_id = Uuid::new_v4();

        // Should not panic, just a no-op
        state.toggle_category_expand(fake_id, ResourceCategory::Workloads);
        assert!(!state.is_category_expanded(fake_id, ResourceCategory::Workloads));
    }

    // -----------------------------------------------------------------
    // All FR-071 categories are independently togglable per cluster
    // -----------------------------------------------------------------

    #[test]
    fn test_all_fr071_categories_independently_expandable() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        // Expand every category
        for &cat in SidebarState::fr071_categories() {
            state.toggle_category_expand(id, cat);
        }

        // All should be expanded
        for &cat in SidebarState::fr071_categories() {
            assert!(state.is_category_expanded(id, cat), "Category {:?} should be expanded", cat);
        }

        // Collapse every category
        for &cat in SidebarState::fr071_categories() {
            state.toggle_category_expand(id, cat);
        }

        // All should be collapsed again
        for &cat in SidebarState::fr071_categories() {
            assert!(!state.is_category_expanded(id, cat), "Category {:?} should be collapsed", cat);
        }
    }

    // -----------------------------------------------------------------
    // Resource count badges update on connected clusters
    // -----------------------------------------------------------------

    #[test]
    fn test_update_cluster_badge_sets_count() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.update_cluster_badge(id, "Pod", Some(42));

        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
        let workloads =
            cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
        let pod_item = workloads.items.iter().find(|i| i.kind == "Pod").unwrap();
        assert_eq!(pod_item.badge_count, Some(42));
    }

    #[test]
    fn test_update_cluster_badge_clears_count() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.update_cluster_badge(id, "Pod", Some(42));
        state.update_cluster_badge(id, "Pod", None);

        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
        let workloads =
            cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
        let pod_item = workloads.items.iter().find(|i| i.kind == "Pod").unwrap();
        assert_eq!(pod_item.badge_count, None);
    }

    #[test]
    fn test_update_cluster_badge_isolated_per_cluster() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        state.update_cluster_badge(id1, "Pod", Some(10));
        state.update_cluster_badge(id2, "Pod", Some(200));

        let c1 = state.clusters.iter().find(|c| c.id == id1).unwrap();
        let c1_pods = c1
            .sections
            .iter()
            .find(|s| s.category == ResourceCategory::Workloads)
            .unwrap()
            .items
            .iter()
            .find(|i| i.kind == "Pod")
            .unwrap();
        assert_eq!(c1_pods.badge_count, Some(10));

        let c2 = state.clusters.iter().find(|c| c.id == id2).unwrap();
        let c2_pods = c2
            .sections
            .iter()
            .find(|s| s.category == ResourceCategory::Workloads)
            .unwrap()
            .items
            .iter()
            .find(|i| i.kind == "Pod")
            .unwrap();
        assert_eq!(c2_pods.badge_count, Some(200));
    }

    #[test]
    fn test_update_cluster_badge_on_nonexistent_cluster_is_noop() {
        let mut state = SidebarState::default();
        let _id = state.add_cluster("kind-dev", "kind-dev");

        // Should not panic
        state.update_cluster_badge(Uuid::new_v4(), "Pod", Some(99));

        // Original cluster unaffected
        let cluster = &state.clusters[0];
        let pods = cluster
            .sections
            .iter()
            .find(|s| s.category == ResourceCategory::Workloads)
            .unwrap()
            .items
            .iter()
            .find(|i| i.kind == "Pod")
            .unwrap();
        assert_eq!(pods.badge_count, None);
    }

    #[test]
    fn test_update_cluster_badge_multiple_kinds() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.update_cluster_badge(id, "Pod", Some(5));
        state.update_cluster_badge(id, "Deployment", Some(3));
        state.update_cluster_badge(id, "Service", Some(12));

        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();

        let workloads =
            cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
        assert_eq!(workloads.items.iter().find(|i| i.kind == "Pod").unwrap().badge_count, Some(5));
        assert_eq!(
            workloads.items.iter().find(|i| i.kind == "Deployment").unwrap().badge_count,
            Some(3)
        );

        let network =
            cluster.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
        assert_eq!(
            network.items.iter().find(|i| i.kind == "Service").unwrap().badge_count,
            Some(12)
        );
    }

    #[test]
    fn test_update_cluster_badge_on_connected_cluster() {
        // Verify that badge updates work when cluster status is Connected
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        // Simulate connection
        state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

        state.update_cluster_badge(id, "Pod", Some(42));

        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
        assert_eq!(cluster.status, ClusterStatus::Connected);
        let pods = cluster
            .sections
            .iter()
            .find(|s| s.category == ResourceCategory::Workloads)
            .unwrap()
            .items
            .iter()
            .find(|i| i.kind == "Pod")
            .unwrap();
        assert_eq!(pods.badge_count, Some(42));
    }

    // -----------------------------------------------------------------
    // Cluster sections match FR-071 categories
    // -----------------------------------------------------------------

    #[test]
    fn test_cluster_entry_has_sections_for_fr071_categories() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");
        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();

        // The default_sections() provides sections for these FR-071 categories
        let section_categories: Vec<ResourceCategory> =
            cluster.sections.iter().map(|s| s.category).collect();

        assert!(section_categories.contains(&ResourceCategory::Workloads));
        assert!(section_categories.contains(&ResourceCategory::Network));
        assert!(section_categories.contains(&ResourceCategory::Configuration));
        assert!(section_categories.contains(&ResourceCategory::Storage));
        assert!(section_categories.contains(&ResourceCategory::Rbac));
        assert!(section_categories.contains(&ResourceCategory::Monitoring));
        assert!(section_categories.contains(&ResourceCategory::Helm));
        assert!(section_categories.contains(&ResourceCategory::Custom));
        assert!(section_categories.contains(&ResourceCategory::Plugins));
    }

    #[test]
    fn test_expanded_categories_field_present_on_cluster_entry() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");
        let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();

        // The expanded_categories field should be a HashSet, initially empty
        assert!(cluster.expanded_categories.is_empty());

        // Verify it's a proper HashSet by checking membership
        assert!(!cluster.expanded_categories.contains(&ResourceCategory::Workloads));
    }

    // =================================================================
    // T305: toggle_cluster_expand tests
    // =================================================================

    #[test]
    fn test_toggle_cluster_expand_expands_collapsed_cluster() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        // Clusters start collapsed by default
        assert!(!state.clusters.iter().find(|c| c.id == id).unwrap().expanded);

        // Toggle to expanded
        state.toggle_cluster_expand(id);
        assert!(state.clusters.iter().find(|c| c.id == id).unwrap().expanded);
    }

    #[test]
    fn test_toggle_cluster_expand_collapses_expanded_cluster() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        // Expand it first
        state.toggle_cluster_expand(id);
        assert!(state.clusters.iter().find(|c| c.id == id).unwrap().expanded);

        // Toggle back to collapsed
        state.toggle_cluster_expand(id);
        assert!(!state.clusters.iter().find(|c| c.id == id).unwrap().expanded);
    }

    #[test]
    fn test_toggle_cluster_expand_independent_per_cluster() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        // Both start collapsed
        assert!(!state.clusters.iter().find(|c| c.id == id1).unwrap().expanded);
        assert!(!state.clusters.iter().find(|c| c.id == id2).unwrap().expanded);

        // Expand only the first
        state.toggle_cluster_expand(id1);
        assert!(state.clusters.iter().find(|c| c.id == id1).unwrap().expanded);
        assert!(!state.clusters.iter().find(|c| c.id == id2).unwrap().expanded);
    }

    #[test]
    fn test_toggle_cluster_expand_on_nonexistent_cluster_is_noop() {
        let mut state = SidebarState::default();
        let _id = state.add_cluster("kind-dev", "kind-dev");

        // Should not panic, just a no-op
        state.toggle_cluster_expand(Uuid::new_v4());

        // Original cluster unaffected (still collapsed)
        assert!(!state.clusters[0].expanded);
    }

    // =================================================================
    // T305: get_resource_types tests
    // =================================================================

    #[test]
    fn test_get_resource_types_returns_items_for_existing_section() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        let items = state.get_resource_types(id, ResourceCategory::Workloads);
        assert!(!items.is_empty());

        // Should contain Pod, Deployment, etc. from default_sections()
        let kinds: Vec<&str> = items.iter().map(|i| i.kind.as_str()).collect();
        assert!(kinds.contains(&"Pod"));
        assert!(kinds.contains(&"Deployment"));
    }

    #[test]
    fn test_get_resource_types_returns_items_for_network_section() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        let items = state.get_resource_types(id, ResourceCategory::Network);
        assert!(!items.is_empty());

        let kinds: Vec<&str> = items.iter().map(|i| i.kind.as_str()).collect();
        assert!(kinds.contains(&"Service"));
        assert!(kinds.contains(&"Ingress"));
    }

    #[test]
    fn test_get_resource_types_returns_empty_for_nonexistent_cluster() {
        let state = SidebarState::default();

        let items = state.get_resource_types(Uuid::new_v4(), ResourceCategory::Workloads);
        assert!(items.is_empty());
    }

    #[test]
    fn test_get_resource_types_different_categories_same_cluster() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        let workload_items = state.get_resource_types(id, ResourceCategory::Workloads);
        let storage_items = state.get_resource_types(id, ResourceCategory::Storage);

        // Workloads and Storage should have different items
        let workload_kinds: Vec<&str> = workload_items.iter().map(|i| i.kind.as_str()).collect();
        let storage_kinds: Vec<&str> = storage_items.iter().map(|i| i.kind.as_str()).collect();

        assert!(workload_kinds.contains(&"Pod"));
        assert!(!workload_kinds.contains(&"PersistentVolume"));
        assert!(storage_kinds.contains(&"PersistentVolume"));
        assert!(!storage_kinds.contains(&"Pod"));
    }

    #[test]
    fn test_get_resource_types_independent_per_cluster() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        // Set a badge on cluster 1's Pod to differentiate
        state.update_cluster_badge(id1, "Pod", Some(42));

        let items1 = state.get_resource_types(id1, ResourceCategory::Workloads);
        let items2 = state.get_resource_types(id2, ResourceCategory::Workloads);

        // Both should have items
        assert!(!items1.is_empty());
        assert!(!items2.is_empty());

        // Cluster 1's Pod should have a badge, cluster 2's should not
        let pod1 = items1.iter().find(|i| i.kind == "Pod").unwrap();
        let pod2 = items2.iter().find(|i| i.kind == "Pod").unwrap();
        assert_eq!(pod1.badge_count, Some(42));
        assert_eq!(pod2.badge_count, None);
    }

    // =================================================================
    // T306: drill_into_cluster tests
    // =================================================================

    #[test]
    fn test_default_state_not_in_drill_into() {
        let state = SidebarState::default();
        assert!(!state.is_drill_into());
        assert!(state.drill_into_cluster.is_none());
    }

    #[test]
    fn test_enter_drill_into_sets_cluster() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.enter_drill_into(id);
        assert!(state.is_drill_into());
        assert_eq!(state.drill_into_cluster, Some(id));
    }

    #[test]
    fn test_exit_drill_into_clears_cluster() {
        let mut state = SidebarState::default();
        let id = state.add_cluster("kind-dev", "kind-dev");

        state.enter_drill_into(id);
        assert!(state.is_drill_into());

        state.exit_drill_into();
        assert!(!state.is_drill_into());
        assert!(state.drill_into_cluster.is_none());
    }

    #[test]
    fn test_enter_drill_into_overwrites_previous() {
        let mut state = SidebarState::default();
        let id1 = state.add_cluster("kind-dev", "kind-dev");
        let id2 = state.add_cluster("prod", "production");

        state.enter_drill_into(id1);
        assert_eq!(state.drill_into_cluster, Some(id1));

        state.enter_drill_into(id2);
        assert_eq!(state.drill_into_cluster, Some(id2));
    }

    #[test]
    fn test_exit_drill_into_when_not_in_drill_into_is_noop() {
        let mut state = SidebarState::default();
        // Should not panic
        state.exit_drill_into();
        assert!(!state.is_drill_into());
    }

    // =================================================================
    // T306: sidebar_width tests
    // =================================================================

    #[test]
    fn test_default_sidebar_width() {
        let state = SidebarState::default();
        assert_eq!(state.sidebar_width, 260.0);
    }

    #[test]
    fn test_set_width_normal_value() {
        let mut state = SidebarState::default();
        state.set_width(300.0);
        assert_eq!(state.sidebar_width, 300.0);
    }

    #[test]
    fn test_set_width_clamps_to_minimum() {
        let mut state = SidebarState::default();
        state.set_width(100.0);
        assert_eq!(state.sidebar_width, 200.0);
    }

    #[test]
    fn test_set_width_clamps_zero() {
        let mut state = SidebarState::default();
        state.set_width(0.0);
        assert_eq!(state.sidebar_width, 200.0);
    }

    #[test]
    fn test_set_width_clamps_negative() {
        let mut state = SidebarState::default();
        state.set_width(-50.0);
        assert_eq!(state.sidebar_width, 200.0);
    }

    #[test]
    fn test_set_width_exactly_at_minimum() {
        let mut state = SidebarState::default();
        state.set_width(200.0);
        assert_eq!(state.sidebar_width, 200.0);
    }

    #[test]
    fn test_set_width_large_value() {
        let mut state = SidebarState::default();
        state.set_width(1000.0);
        assert_eq!(state.sidebar_width, 1000.0);
    }
}
