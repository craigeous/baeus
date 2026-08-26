use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName, Sizable};

use baeus_core::KubeClient;
use baeus_core::cluster::{AuthMethod, ClusterConnection, ClusterManager};
use baeus_core::informer::{InformerManager, InformerState};

use crate::components::json_extract;
use crate::components::log_viewer::{LogViewerState, LogViewerView};
use crate::components::resource_table::{ResourceTableState, TableRow, columns_for_kind};
use crate::components::terminal_view::TerminalViewState;
use crate::components::terminal_view_component::TerminalViewComponent;
use crate::icons::{ResourceIcon, SectionIcon};

use crate::layout::dock::{DockState, DockTabKind};
use crate::layout::header::{EnhancedNamespaceSelector, HeaderState};
use crate::layout::indent_guides::{INDENT_OFFSET, INDENT_STEP, NavigatorIndentGuideDecoration};
use crate::layout::sidebar::{ClusterStatus, NavigatorFlatEntry, SidebarState};
use crate::layout::workspace::WorkspaceState;
use crate::layout::{AppLayout, NavigationTarget};
use crate::theme::{Theme, ThemeMode};
use crate::views::dashboard::{DashboardEvent, DashboardState, ResourceCounts};
use crate::views::preferences::{PreferencesSection, PreferencesState};
use crate::views::resource_detail::{ConditionDisplay, ResourceDetailState};
use baeus_terminal::pty_process::PtyProcess;

/// Per-cluster appearance overrides (custom icon color, custom icon image).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ClusterAppearance {
    /// Override for the auto-generated palette color (RGB u32).
    pub custom_color: Option<u32>,
    /// Path to a custom icon image file.
    pub custom_icon_path: Option<String>,
}

/// Payload carrying metrics results from the Tokio polling loop back to GPUI.
struct MetricsPayload {
    nodes: Option<Vec<baeus_core::metrics::NodeMetrics>>,
    pods: Option<Vec<baeus_core::metrics::PodMetrics>>,
}

/// Pre-computed metrics data for a single table row.
struct RowMetrics {
    cpu_display: String,
    cpu_percent: Option<f32>,
    mem_display: String,
    mem_percent: Option<f32>,
}

/// Key identifying a resource list: (cluster_context, kind, namespace).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceListKey {
    pub cluster_context: String,
    pub kind: String,
    pub namespace: Option<String>,
}

/// Key identifying a single resource: (cluster_context, kind, name, namespace).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceDetailKey {
    pub cluster_context: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

/// Whether the resource detail view is showing the Overview, YAML, or Events tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTabMode {
    Overview,
    Yaml,
    Events,
    Topology,
}

/// Context for an active confirmation dialog (delete / scale).
pub struct ConfirmDialogContext {
    pub dialog: crate::components::confirm_dialog::ConfirmDialogState,
    pub action: PendingAction,
}

/// Action pending user confirmation via the confirm dialog.
#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteResource {
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    },
    ScaleResource {
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
        replicas: u32,
    },
    RestartResource {
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    },
    CordonNode {
        cluster_context: String,
        name: String,
    },
    UncordonNode {
        cluster_context: String,
        name: String,
    },
}

// ---------------------------------------------------------------------------
// GPUI Actions for menu bar dispatch
// ---------------------------------------------------------------------------

actions!(baeus, [OpenPreferencesAction]);

// ---------------------------------------------------------------------------
// T086: Keyboard Navigation types
// ---------------------------------------------------------------------------

/// An action that can be triggered by a keyboard shortcut.
/// This mirrors `baeus_app::settings::KeyAction` but lives in the UI layer
/// to avoid a circular dependency (baeus-app depends on baeus-ui).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    ToggleCommandPalette,
    NavigateToDashboard,
    NavigateToClusterList,
    NavigateToPods,
    NavigateToDeployments,
    NavigateToServices,
    NavigateToEvents,
    NavigateToHelmReleases,
    ToggleSidebar,
    NextTab,
    PrevTab,
    CloseTab,
    Refresh,
    FocusSearch,
    /// T366: Cmd+Shift+T — open terminal in dock
    OpenTerminal,
    /// T366: Cmd+, — open preferences / settings
    OpenPreferences,
    /// T366: Cmd+P — search / jump to cluster
    SearchClusters,
    /// T366: Ctrl+- — navigate back in history
    NavigateBack,
    /// T366: Ctrl+Shift+- — navigate forward in history
    NavigateForward,
    /// T366: Cmd+Option+Right — switch to next tab (alternative binding)
    NextTabAlt,
    /// T366: Cmd+Option+Left — switch to previous tab (alternative binding)
    PrevTabAlt,
}

/// Modifier keys that can be combined with a key press.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[allow(dead_code)]
impl KeyModifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn cmd() -> Self {
        Self { cmd: true, ..Self::default() }
    }

    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Self::default() }
    }

    pub fn cmd_shift() -> Self {
        Self { cmd: true, shift: true, ..Self::default() }
    }
}

/// A single key binding mapping a key + modifiers to an action.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindingEntry {
    pub key: String,
    pub modifiers: KeyModifiers,
    pub action: KeyAction,
}

/// Configuration holding all keybindings with lookup support.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct KeybindingConfig {
    pub bindings: Vec<KeyBindingEntry>,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self::default_bindings()
    }
}

#[allow(dead_code)]
impl KeybindingConfig {
    /// Look up the action for a given key and modifier combination.
    pub fn find_action(&self, key: &str, modifiers: &KeyModifiers) -> Option<KeyAction> {
        self.bindings.iter().find(|b| b.key == key && b.modifiers == *modifiers).map(|b| b.action)
    }

    /// Return the default set of keybindings for the application.
    pub fn default_bindings() -> Self {
        Self {
            bindings: vec![
                KeyBindingEntry {
                    key: "k".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::ToggleCommandPalette,
                },
                KeyBindingEntry {
                    key: "1".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToDashboard,
                },
                KeyBindingEntry {
                    key: "2".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToClusterList,
                },
                KeyBindingEntry {
                    key: "3".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToPods,
                },
                KeyBindingEntry {
                    key: "4".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToDeployments,
                },
                KeyBindingEntry {
                    key: "5".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToServices,
                },
                KeyBindingEntry {
                    key: "6".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToEvents,
                },
                KeyBindingEntry {
                    key: "7".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::NavigateToHelmReleases,
                },
                KeyBindingEntry {
                    key: "b".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::ToggleSidebar,
                },
                KeyBindingEntry {
                    key: "Tab".to_string(),
                    modifiers: KeyModifiers::ctrl(),
                    action: KeyAction::NextTab,
                },
                KeyBindingEntry {
                    key: "Tab".to_string(),
                    modifiers: KeyModifiers { ctrl: true, shift: true, ..KeyModifiers::default() },
                    action: KeyAction::PrevTab,
                },
                KeyBindingEntry {
                    key: "w".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::CloseTab,
                },
                KeyBindingEntry {
                    key: "r".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::Refresh,
                },
                KeyBindingEntry {
                    key: "f".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::FocusSearch,
                },
                // T366: New keyboard shortcuts
                KeyBindingEntry {
                    key: "t".to_string(),
                    modifiers: KeyModifiers::cmd_shift(),
                    action: KeyAction::OpenTerminal,
                },
                KeyBindingEntry {
                    key: ",".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::OpenPreferences,
                },
                KeyBindingEntry {
                    key: "p".to_string(),
                    modifiers: KeyModifiers::cmd(),
                    action: KeyAction::SearchClusters,
                },
                KeyBindingEntry {
                    key: "-".to_string(),
                    modifiers: KeyModifiers::ctrl(),
                    action: KeyAction::NavigateBack,
                },
                KeyBindingEntry {
                    key: "-".to_string(),
                    modifiers: KeyModifiers { ctrl: true, shift: true, ..KeyModifiers::default() },
                    action: KeyAction::NavigateForward,
                },
                KeyBindingEntry {
                    key: "Right".to_string(),
                    modifiers: KeyModifiers { cmd: true, alt: true, ..KeyModifiers::default() },
                    action: KeyAction::NextTabAlt,
                },
                KeyBindingEntry {
                    key: "Left".to_string(),
                    modifiers: KeyModifiers { cmd: true, alt: true, ..KeyModifiers::default() },
                    action: KeyAction::PrevTabAlt,
                },
            ],
        }
    }
}

/// Direction for table navigation movement.
#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Focus mode tracking for keyboard navigation.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FocusMode {
    #[default]
    Normal,
    TableNavigation {
        row: usize,
        col: usize,
    },
    CommandPalette,
    Search,
    Modal,
}

/// State object for keyboard navigation, combining keybinding config with focus tracking.
pub struct KeyboardNavigationState {
    pub config: KeybindingConfig,
    pub focus_mode: FocusMode,
    pub last_action: Option<KeyAction>,
}

impl KeyboardNavigationState {
    /// Create a new KeyboardNavigationState with default bindings.
    pub fn new() -> Self {
        Self {
            config: KeybindingConfig::default(),
            focus_mode: FocusMode::Normal,
            last_action: None,
        }
    }

    /// Look up a key + modifiers in the config and return the matching action.
    pub fn process_key(&self, key: &str, modifiers: &KeyModifiers) -> Option<KeyAction> {
        self.config.find_action(key, modifiers)
    }

    /// Check whether the given action has at least one binding in the config.
    pub fn is_shortcut_active(&self, action: &KeyAction) -> bool {
        self.config.bindings.iter().any(|b| b.action == *action)
    }
}

impl Default for KeyboardNavigationState {
    fn default() -> Self {
        Self::new()
    }
}

// Implement gpui::Global for our local wrapper so it can be stored as a GPUI global.
// We cannot impl Global directly for baeus_core::runtime::TokioHandle due to the
// orphan rule, so we use a local newtype.

/// GPUI-compatible wrapper around the Tokio runtime handle.
/// Stored via `cx.set_global(GpuiTokioHandle(...))` and retrieved in views.
#[derive(Clone)]
pub struct GpuiTokioHandle(pub tokio::runtime::Handle);

impl Global for GpuiTokioHandle {}

/// Tracks a pending AWS SSO login prompt triggered by an auth error.
#[derive(Debug, Clone)]
pub struct PendingSsoLogin {
    /// The AWS profile that needs re-authentication.
    pub profile: String,
    /// The cluster context that triggered the error.
    pub cluster_context: String,
}

/// Sanitize an error message for user display by stripping potential credentials.
/// Redacts base64-encoded tokens, bearer tokens, and common secret patterns.
fn sanitize_error_message(msg: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static BEARER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)bearer\s+[A-Za-z0-9_\-\.]+").unwrap());
    static BASE64_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/]{40,}={0,2}").unwrap());
    static SECRET_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(password|secret|token|key)=[^\s&]+").unwrap());

    let sanitized = BEARER_RE.replace_all(msg, "bearer <redacted>");
    let sanitized = BASE64_RE.replace_all(&sanitized, "<redacted>");
    let sanitized = SECRET_RE.replace_all(&sanitized, "$1=<redacted>");
    sanitized.into_owned()
}

pub struct AppShell {
    layout: AppLayout,
    pub(crate) sidebar: SidebarState,
    header: HeaderState,
    pub(crate) workspace: WorkspaceState,
    pub(crate) theme: Theme,
    /// Core cluster manager tracking connection state for all clusters.
    pub(crate) cluster_manager: ClusterManager,
    /// Informer manager for real-time resource watches.
    informer_manager: InformerManager,
    /// Dashboard state for the currently active cluster.
    dashboard_state: Option<DashboardState>,
    /// Context name of the cluster whose dashboard is currently loaded.
    active_dashboard_cluster: Option<String>,
    /// T086: Current focus mode for keyboard navigation.
    pub focus_mode: FocusMode,
    /// T323: Active kube clients keyed by context name, stored after successful connection
    /// so that views can reuse them for API calls.
    pub(crate) active_clients: HashMap<String, KubeClient>,
    /// T317/T318: Dock panel state (terminal, logs, port-forward tabs).
    dock: DockState,
    /// Port forward panel state for the dock's PortForwardManager tab.
    port_forward_panel: crate::components::port_forward::PortForwardPanelState,
    /// Helm releases state keyed by cluster context.
    helm_releases: HashMap<String, crate::views::helm_releases::HelmReleasesViewState>,
    /// CRD browser state keyed by cluster context.
    crd_browser: HashMap<String, crate::views::crd_browser::CrdBrowserState>,
    /// T313: Cluster ID for which the right-click context menu is currently visible.
    context_menu_cluster: Option<uuid::Uuid>,
    /// Y-coordinate (in window pixels) where the context menu was triggered.
    context_menu_position_y: f32,
    /// Frame-level guard: set to true when a context menu item is clicked (which clears
    /// `context_menu_cluster`). This prevents sidebar click handlers from also firing
    /// during the same event cycle. Cleared at the start of each render.
    context_menu_dismissed_this_frame: bool,
    /// T327: Cached resource list data keyed by (cluster_context, kind, namespace).
    /// Each entry holds the most recent JSON items from the API or informer stream.
    pub(crate) resource_list_data: HashMap<ResourceListKey, Vec<serde_json::Value>>,
    /// Cached ResourceTableState per resource list, built from resource_list_data.
    resource_table_states: HashMap<ResourceListKey, ResourceTableState>,
    /// T327b: Tracks which resource list keys currently have an active watcher running,
    /// so we don't start duplicate watchers.
    active_resource_watchers: HashSet<ResourceListKey>,
    /// T328: Cached resource detail data keyed by resource identity.
    resource_detail_data: HashMap<ResourceDetailKey, serde_json::Value>,
    /// T355: Unread notification count displayed in the header bell badge.
    notification_count: u32,
    /// T355: Navigation history for back/forward buttons in the title bar.
    pub(crate) navigation_history: Vec<NavigationTarget>,
    /// T355: Current position within `navigation_history` (points at the active entry).
    pub(crate) history_index: usize,
    /// T356: Kubernetes server version string for the active cluster (e.g. "v1.28.0").
    k8s_version: Option<String>,
    /// T362: Error state tracking for views. Key is a view identifier like
    /// "dashboard:{cluster}" or "resources:{cluster}:{kind}". Value is the error message.
    view_errors: HashMap<String, String>,
    /// T363: Connection error messages keyed by cluster context name.
    /// Populated by `on_cluster_connection_lost`, cleared by `on_cluster_reconnected`.
    pub(crate) connection_errors: HashMap<String, String>,
    /// Update notification: (latest_version, download_url) if newer than current.
    update_available: Option<(String, String)>,
    /// Scroll handles for navigator uniform_lists, keyed by cluster ID.
    navigator_scroll_handles: HashMap<uuid::Uuid, UniformListScrollHandle>,
    /// Tracks which detail sections are collapsed (by section ID string).
    pub(crate) detail_collapsed_sections: HashSet<String>,
    /// Secret keys that have been revealed (eye toggle). Key format: "context:namespace:name:key"
    revealed_secret_keys: HashSet<String>,
    /// Terminal view entities keyed by dock tab UUID.
    terminal_views: HashMap<uuid::Uuid, Entity<TerminalViewComponent>>,
    /// Log viewer entities keyed by dock tab UUID.
    log_viewer_views: HashMap<uuid::Uuid, Entity<LogViewerView>>,
    /// PTY processes keyed by dock tab UUID, for real shell sessions.
    pty_processes: HashMap<uuid::Uuid, Arc<Mutex<PtyProcess>>>,
    /// Shared output buffers filled by background reader threads, keyed by dock tab UUID.
    pty_output_buffers: HashMap<uuid::Uuid, Arc<Mutex<Vec<u8>>>>,
    /// Whether the user is currently dragging the dock resize handle.
    is_dragging_dock: bool,
    /// The Y coordinate where the dock drag started.
    dock_drag_start_y: f32,
    /// The dock height when the drag started.
    dock_drag_start_height: f32,
    /// Whether the user is currently dragging a column resize handle.
    is_dragging_column: bool,
    /// Index of the column being resized.
    column_drag_index: Option<usize>,
    /// X coordinate where the column drag started.
    column_drag_start_x: f32,
    /// Column width when the drag started.
    column_drag_start_width: f32,
    /// Resource list key for the table being resized.
    column_drag_table_key: Option<ResourceListKey>,
    /// Kubeconfig file path per context name, for spawning terminals with the right config.
    kubeconfig_paths: HashMap<String, String>,
    /// Mapping from disambiguated context name → original kubeconfig context name.
    /// Only present for contexts that were renamed to avoid duplicates.
    original_context_names: HashMap<String, String>,
    /// Per-cluster namespace selector for resource list filtering.
    namespace_selectors: HashMap<String, EnhancedNamespaceSelector>,
    /// Input entity for namespace dropdown search (created when dropdown opens, dropped when closed).
    ns_search_input: Option<Entity<InputState>>,
    /// Subscription for the namespace search input change events.
    _ns_search_subscription: Option<Subscription>,
    /// Input entity for topology namespace dropdown search.
    pub(crate) topo_ns_search_input: Option<Entity<InputState>>,
    /// Subscription for the topology namespace search input change events.
    pub(crate) _topo_ns_search_subscription: Option<Subscription>,
    /// Per-resource-list search/filter input entities.
    resource_filter_inputs: HashMap<ResourceListKey, Entity<InputState>>,
    /// Subscriptions for resource filter input change events.
    _resource_filter_subscriptions: HashMap<ResourceListKey, Subscription>,
    /// Maps cluster context name → dock terminal tab UUID (one terminal per cluster).
    cluster_terminals: HashMap<String, uuid::Uuid>,
    /// Preferences form state (persisted to ~/.config/baeus/preferences.json).
    pub(crate) preferences: PreferencesState,
    /// Currently active section in the preferences sidebar.
    pub(crate) active_prefs_section: PreferencesSection,
    /// Per-cluster metrics state from metrics-server polling.
    cluster_metrics: HashMap<String, baeus_core::metrics::MetricsState>,
    /// Overview vs YAML tab per resource detail.
    detail_active_tab: HashMap<ResourceDetailKey, DetailTabMode>,
    /// YAML editor states, lazily created when YAML tab opens.
    pub(crate) yaml_editors:
        HashMap<ResourceDetailKey, crate::components::editor_view::EditorViewState>,
    /// Focus handles for YAML editors, keyed by resource detail key.
    pub(crate) yaml_editor_focus_handles: HashMap<ResourceDetailKey, FocusHandle>,
    /// Confirm dialog for destructive actions (delete, scale).
    confirm_dialog: Option<ConfirmDialogContext>,
    /// Per-table context menu: which row index has its "..." menu open.
    context_menu_row: HashMap<ResourceListKey, usize>,
    /// Input entity for the cluster filter in the navigator sidebar.
    cluster_filter_input: Option<Entity<InputState>>,
    /// Subscription for the cluster filter input change events.
    _cluster_filter_subscription: Option<Subscription>,
    /// Current cluster filter text (case-insensitive substring match on display/context name).
    cluster_filter_text: String,
    /// Whether the user is currently dragging the sidebar resize handle.
    is_dragging_sidebar: bool,
    /// X coordinate where the sidebar drag started.
    sidebar_drag_start_x: f32,
    /// Sidebar width when the drag started.
    sidebar_drag_start_width: f32,
    /// Whether the cluster filter uses case-sensitive matching.
    cluster_filter_case_sensitive: bool,
    /// Whether the cluster filter uses regex matching.
    cluster_filter_regex: bool,
    /// Per-cluster appearance overrides (custom color, custom icon), keyed by context_name.
    cluster_appearances: HashMap<String, ClusterAppearance>,
    /// Context name of the cluster whose icon "..." popup is open in ClusterSettings view.
    cluster_settings_icon_menu: Option<String>,
    /// Context name of the cluster whose color picker is visible in ClusterSettings view.
    cluster_settings_color_picker: Option<String>,
    /// Per-resource topology visualization state, keyed by resource detail key.
    pub(crate) topology_data:
        HashMap<ResourceDetailKey, crate::components::topology_render::TopologyState>,
    /// Whether the user is currently dragging to pan the topology view.
    pub(crate) is_dragging_topology: bool,
    /// Last mouse position during topology drag (for computing delta).
    pub(crate) topology_drag_last: (f32, f32),
    /// Which topology view is being dragged.
    pub(crate) topology_drag_key: Option<ResourceDetailKey>,
    /// Per-cluster topology state for the cluster-level topology view.
    pub(crate) cluster_topology_states:
        HashMap<String, crate::components::topology_render::ClusterTopologyState>,
    /// Whether the user is currently dragging to pan the cluster topology view.
    pub(crate) is_dragging_cluster_topology: bool,
    /// Last mouse position during cluster topology drag.
    pub(crate) cluster_topology_drag_last: (f32, f32),
    /// Which cluster's topology is being dragged.
    pub(crate) cluster_topology_drag_context: Option<String>,
    /// Whether the user is dragging the cluster topology graph/table resize handle.
    pub(crate) is_dragging_cluster_topo_resize: bool,
    /// Starting Y position of the topology resize drag.
    pub(crate) cluster_topo_resize_start_y: f32,
    /// Starting graph height at the beginning of the resize drag.
    pub(crate) cluster_topo_resize_start_height: f32,
    /// Which cluster's topology resize is active.
    pub(crate) cluster_topo_resize_context: Option<String>,
    /// Default AWS profile for EKS authentication (from preferences).
    default_aws_profile: Option<String>,
    /// Per-cluster AWS profile overrides, keyed by context name.
    cluster_aws_profiles: HashMap<String, String>,
    /// If set, an AWS SSO auth error banner is shown with an Authenticate button.
    pub(crate) pending_sso_login: Option<PendingSsoLogin>,
    /// Cached AWS caller identity from `aws sts get-caller-identity`.
    aws_caller_identity: Option<baeus_core::aws_sso::CallerIdentity>,
    /// Whether an AWS caller identity fetch is in progress.
    aws_identity_loading: bool,
    /// EKS wizard state (None = wizard closed).
    pub(crate) eks_wizard: Option<crate::components::eks_wizard::EksWizardState>,
    /// Stored EKS cluster metadata + credentials + optional role ARN for clusters connected via the wizard.
    /// Keyed by context name (e.g. "eks:us-east-1:obelix-prod").
    /// Tuple: (cluster, credentials, optional role ARN for terminal kubeconfig)
    pub(crate) eks_cluster_data: HashMap<
        String,
        (baeus_core::aws_eks::EksCluster, aws_credential_types::Credentials, Option<String>),
    >,
    /// Input entity for the default AWS profile in Kubernetes preferences.
    aws_profile_input: Option<Entity<InputState>>,
    /// Subscription for the default AWS profile input change events.
    _aws_profile_subscription: Option<Subscription>,
    /// Per-cluster AWS profile input entities in Cluster Settings, keyed by context name.
    cluster_aws_profile_inputs: HashMap<String, Entity<InputState>>,
    /// Subscriptions for per-cluster AWS profile input change events.
    _cluster_aws_profile_subscriptions: HashMap<String, Subscription>,
}

impl AppShell {
    /// Create a new AppShell. Pass discovered cluster (context_name, display_name, kubeconfig_path)
    /// triples from kubeconfig discovery. If empty, falls back to static sidebar sections.
    pub fn new(
        clusters: Vec<(String, String, String, String)>,
        initial_prefs: PreferencesState,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut sidebar = SidebarState::default();
        let mut cluster_manager = ClusterManager::new();
        let mut kubeconfig_paths = HashMap::new();

        // Load cluster appearances from persisted user preferences.
        let cluster_appearances: HashMap<String, ClusterAppearance> =
            Self::load_cluster_appearances();

        // Map from effective (disambiguated) context name → original kubeconfig context name.
        let mut original_context_names: HashMap<String, String> = HashMap::new();

        for (context_name, display_name, config_path, original_name) in &clusters {
            sidebar.add_cluster(context_name, display_name);

            // Apply persisted custom color override if present.
            if let Some(appearance) = cluster_appearances.get(context_name) {
                if let Some(color) = appearance.custom_color {
                    if let Some(entry) =
                        sidebar.clusters.iter_mut().find(|c| c.context_name == *context_name)
                    {
                        entry.color = color;
                    }
                }
                if let Some(ref icon_path) = appearance.custom_icon_path {
                    if let Some(entry) =
                        sidebar.clusters.iter_mut().find(|c| c.context_name == *context_name)
                    {
                        entry.custom_icon_path = Some(icon_path.clone());
                    }
                }
            }

            kubeconfig_paths.insert(context_name.clone(), config_path.clone());
            if context_name != original_name {
                original_context_names.insert(context_name.clone(), original_name.clone());
            }

            // Register each discovered cluster in the core cluster manager.
            let conn = ClusterConnection::new(
                display_name.clone(),
                context_name.clone(),
                String::new(),     // API server URL not yet known
                AuthMethod::Token, // Default; will be resolved on connect
            );
            cluster_manager.add_connection(conn);
        }

        let mut shell = Self {
            layout: AppLayout {
                sidebar_collapsed: initial_prefs.sidebar_collapsed,
                ..AppLayout::default()
            },
            sidebar,
            header: HeaderState::default(),
            workspace: WorkspaceState::default(),
            theme: Theme::for_mode(initial_prefs.theme_mode),
            cluster_manager,
            informer_manager: InformerManager::new(),
            dashboard_state: None,
            active_dashboard_cluster: None,
            focus_mode: FocusMode::default(),
            active_clients: HashMap::new(),
            dock: DockState::default(),
            port_forward_panel: crate::components::port_forward::PortForwardPanelState::new(),
            helm_releases: HashMap::new(),
            crd_browser: HashMap::new(),
            context_menu_cluster: None,
            context_menu_position_y: 0.0,
            context_menu_dismissed_this_frame: false,
            resource_list_data: HashMap::new(),
            resource_table_states: HashMap::new(),
            active_resource_watchers: HashSet::new(),
            resource_detail_data: HashMap::new(),
            notification_count: 0,
            navigation_history: Vec::new(),
            history_index: 0,
            k8s_version: None,
            view_errors: HashMap::new(),
            connection_errors: HashMap::new(),
            update_available: None,
            navigator_scroll_handles: HashMap::new(),
            detail_collapsed_sections: HashSet::new(),
            revealed_secret_keys: HashSet::new(),
            terminal_views: HashMap::new(),
            log_viewer_views: HashMap::new(),
            pty_processes: HashMap::new(),
            pty_output_buffers: HashMap::new(),
            is_dragging_dock: false,
            dock_drag_start_y: 0.0,
            dock_drag_start_height: 0.0,
            is_dragging_column: false,
            column_drag_index: None,
            column_drag_start_x: 0.0,
            column_drag_start_width: 0.0,
            column_drag_table_key: None,
            kubeconfig_paths,
            original_context_names,
            namespace_selectors: HashMap::new(),
            ns_search_input: None,
            _ns_search_subscription: None,
            topo_ns_search_input: None,
            _topo_ns_search_subscription: None,
            resource_filter_inputs: HashMap::new(),
            _resource_filter_subscriptions: HashMap::new(),
            cluster_terminals: HashMap::new(),
            default_aws_profile: initial_prefs.default_aws_profile.clone(),
            cluster_aws_profiles: initial_prefs.cluster_aws_profiles.clone(),
            preferences: initial_prefs,
            active_prefs_section: PreferencesSection::default(),
            cluster_metrics: HashMap::new(),
            detail_active_tab: HashMap::new(),
            yaml_editors: HashMap::new(),
            yaml_editor_focus_handles: HashMap::new(),
            confirm_dialog: None,
            context_menu_row: HashMap::new(),
            cluster_filter_input: None,
            _cluster_filter_subscription: None,
            cluster_filter_text: String::new(),
            is_dragging_sidebar: false,
            sidebar_drag_start_x: 0.0,
            sidebar_drag_start_width: 0.0,
            cluster_filter_case_sensitive: false,
            cluster_filter_regex: false,
            cluster_appearances,
            cluster_settings_icon_menu: None,
            cluster_settings_color_picker: None,
            topology_data: HashMap::new(),
            is_dragging_topology: false,
            topology_drag_last: (0.0, 0.0),
            topology_drag_key: None,
            cluster_topology_states: HashMap::new(),
            is_dragging_cluster_topology: false,
            cluster_topology_drag_last: (0.0, 0.0),
            cluster_topology_drag_context: None,
            is_dragging_cluster_topo_resize: false,
            cluster_topo_resize_start_y: 0.0,
            cluster_topo_resize_start_height: 0.0,
            cluster_topo_resize_context: None,
            pending_sso_login: None,
            aws_caller_identity: None,
            aws_identity_loading: false,
            eks_wizard: None,
            eks_cluster_data: HashMap::new(),
            aws_profile_input: None,
            _aws_profile_subscription: None,
            cluster_aws_profile_inputs: HashMap::new(),
            _cluster_aws_profile_subscriptions: HashMap::new(),
        };

        // Set AWS_PROFILE env var from loaded preferences on startup.
        // This is safe at startup — single-threaded, before any cluster connections.
        if let Some(ref profile) = shell.default_aws_profile {
            tracing::info!("Default AWS profile: {profile}");
        }

        // Restore saved EKS connections from preferences.
        let restored_contexts = shell.restore_saved_eks_connections();

        // Auto-connect restored EKS clusters.
        for ctx in &restored_contexts {
            shell.handle_connect_cluster(ctx, cx);
        }

        // Check for updates in the background.
        shell.check_for_updates(cx);

        shell
    }

    /// Restore persisted EKS connections on app startup.
    /// Creates sidebar entries, writes kubeconfig files. Returns context names for auto-connect.
    fn restore_saved_eks_connections(&mut self) -> Vec<String> {
        use crate::layout::sidebar::{
            ClusterEntry, ClusterSource, generate_cluster_color, generate_initials,
        };
        use baeus_core::aws_eks::{self, EksCluster};

        let saved = self.preferences.saved_eks_connections.clone();
        if saved.is_empty() {
            return Vec::new();
        }
        tracing::info!("Restoring {} saved EKS connections", saved.len());
        let mut restored = Vec::new();

        for conn in &saved {
            let cluster = EksCluster {
                name: conn.cluster_name.clone(),
                arn: conn.cluster_arn.clone(),
                endpoint: conn.endpoint.clone(),
                region: conn.region.clone(),
                status: Some("ACTIVE".to_string()),
                certificate_authority_data: conn.certificate_authority_data.clone(),
                version: None,
                tags: std::collections::HashMap::new(),
            };
            let context_name = aws_eks::eks_context_name(&cluster);

            // Skip if this cluster is already in the sidebar (from kubeconfig discovery)
            if self.sidebar.clusters.iter().any(|c| c.context_name == context_name) {
                tracing::info!(
                    "EKS cluster '{}' already discovered, skipping restore",
                    context_name
                );
                continue;
            }

            let display_name = format!("{} ({})", cluster.name, cluster.region);
            let cluster_conn = ClusterConnection::new(
                cluster.name.clone(),
                context_name.clone(),
                cluster.endpoint.clone(),
                AuthMethod::AwsEks,
            );
            let cluster_id = self.cluster_manager.add_connection(cluster_conn);

            let entry = ClusterEntry {
                id: cluster_id,
                context_name: context_name.clone(),
                display_name,
                initials: generate_initials(&cluster.name),
                color: generate_cluster_color(&context_name),
                status: ClusterStatus::Disconnected,
                expanded: false,
                sections: Vec::new(),
                expanded_categories: std::collections::HashSet::new(),
                custom_icon_path: None,
                source: ClusterSource::AwsEks { region: cluster.region.clone(), account_id: None },
            };
            self.sidebar.clusters.push(entry);

            // Write kubeconfig file so aws exec plugin auth works on connect
            let role_arn = conn.role_arn.as_deref();
            match self.generate_eks_kubeconfig_file_with_role(&context_name, &cluster, role_arn) {
                Ok(path) => {
                    self.kubeconfig_paths.insert(context_name.clone(), path);
                    restored.push(context_name.clone());
                    tracing::info!("Restored EKS cluster '{}' with kubeconfig", context_name);
                }
                Err(e) => {
                    tracing::error!("Failed to write kubeconfig for '{}': {e}", context_name);
                }
            }
        }
        restored
    }

    /// Check GitHub releases for a newer version. Runs in the background.
    fn check_for_updates(&self, cx: &mut Context<Self>) {
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async {
                    let output = tokio::process::Command::new("curl")
                        .args([
                            "-sf",
                            "-H",
                            "Accept: application/vnd.github.v3+json",
                            "https://api.github.com/repos/Craigeous/baeus/releases/latest",
                        ])
                        .output()
                        .await
                        .map_err(|e| format!("curl failed: {e}"))?;

                    if !output.status.success() {
                        return Err("GitHub API request failed".to_string());
                    }

                    let body: serde_json::Value = serde_json::from_slice(&output.stdout)
                        .map_err(|e| format!("Failed to parse response: {e}"))?;

                    let tag = body
                        .get("tag_name")
                        .and_then(|v| v.as_str())
                        .ok_or("No tag_name in response")?
                        .to_string();
                    let url = body
                        .get("html_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("https://github.com/Craigeous/baeus/releases")
                        .to_string();

                    Ok::<(String, String), String>((tag, url))
                })
                .await;

            if let Ok(Ok((tag, url))) = result {
                let current = env!("CARGO_PKG_VERSION");
                let latest = tag.trim_start_matches('v');
                if latest != current {
                    tracing::info!("Update available: {current} → {latest}");
                    this.update(cx, |this, cx| {
                        this.update_available = Some((latest.to_string(), url));
                        cx.notify();
                    })
                    .ok();
                } else {
                    tracing::info!("App is up to date ({current})");
                }
            }
        })
        .detach();
    }

    /// T323: Retrieve a previously-stored kube client for the given context name.
    /// Returns `None` if the cluster is not connected or the client has not been stored yet.
    pub fn get_client(&self, context: &str) -> Option<&KubeClient> {
        self.active_clients.get(context)
    }

    /// Returns true if a modal overlay (context menu, confirm dialog, EKS wizard) is visible.
    /// Used to guard sidebar click handlers from firing through the overlay.
    pub(crate) fn has_modal_overlay(&self) -> bool {
        self.context_menu_cluster.is_some()
            || self.confirm_dialog.is_some()
            || self.eks_wizard.is_some()
            || self.context_menu_dismissed_this_frame
    }
}

// ---------------------------------------------------------------------------
// YAML Editor + Resource Write Operations
// ---------------------------------------------------------------------------

/// Redact Secret `.data` and `.stringData` values in a JSON object to prevent
/// credential exposure in the YAML editor tab.
fn redact_secret_data(json: &mut serde_json::Value) {
    if let Some(obj) = json.as_object_mut() {
        for key in &["data", "stringData"] {
            if let Some(section) = obj.get_mut(*key) {
                if let Some(map) = section.as_object_mut() {
                    for value in map.values_mut() {
                        *value = serde_json::Value::String("<REDACTED>".to_string());
                    }
                }
            }
        }
    }
}

impl AppShell {
    /// Apply the current YAML editor content to the cluster.
    /// Follows the `start_dashboard_loading` async pattern.
    pub(crate) fn handle_yaml_apply(&mut self, cx: &mut Context<Self>, key: ResourceDetailKey) {
        let Some(editor) = self.yaml_editors.get_mut(&key) else { return };
        if !editor.can_apply() {
            return;
        }
        editor.begin_apply();

        let yaml_text = editor.text();
        let resource_version = editor.resource_version.clone();
        let kind = key.kind.clone();
        let name = key.name.clone();
        let namespace = key.namespace.clone();

        let Some(client) = self.active_clients.get(&key.cluster_context).cloned() else {
            if let Some(editor) = self.yaml_editors.get_mut(&key) {
                editor.apply_failure("No active client for this cluster".to_string());
            }
            cx.notify();
            return;
        };

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = namespace.as_deref().map(|s| s.to_string());

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::update_resource(
                        &client,
                        &kind,
                        &name,
                        ns_ref.as_deref(),
                        &yaml_text,
                        &resource_version,
                    )
                    .await
                    .map_err(|e| (e.to_string(), baeus_core::client::is_conflict_error(&e)))
                })
                .await;

            let detail_key = key.clone();
            match result {
                Ok(Ok(updated_json)) => {
                    let new_rv = updated_json
                        .get("metadata")
                        .and_then(|m| m.get("resourceVersion"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let new_yaml = serde_yaml_ng::to_string(&updated_json).unwrap_or_default();
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&detail_key) {
                            editor.apply_success(&new_rv);
                            editor.original_yaml = new_yaml.clone();
                            editor.buffer = baeus_editor::buffer::TextBuffer::from_str(&new_yaml);
                            editor.is_dirty = false;
                        }
                        // Also update the cached detail data
                        this.resource_detail_data.insert(detail_key, updated_json);
                        this.evict_data_caches();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err((err_msg, is_conflict))) => {
                    this.update(cx, |this, cx| {
                        if is_conflict {
                            let dk = detail_key.clone();
                            this.handle_yaml_conflict(cx, dk, err_msg);
                        } else if let Some(editor) = this.yaml_editors.get_mut(&detail_key) {
                            editor.apply_failure(err_msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(join_err) => {
                    let msg = format!("Apply task panicked: {join_err}");
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&detail_key) {
                            editor.apply_failure(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Handle a 409 conflict during apply — re-fetch the server version.
    fn handle_yaml_conflict(
        &mut self,
        cx: &mut Context<Self>,
        key: ResourceDetailKey,
        _err_msg: String,
    ) {
        let Some(client) = self.active_clients.get(&key.cluster_context).cloned() else {
            return;
        };
        let kind = key.kind.clone();
        let name = key.name.clone();
        let ns = key.namespace.clone();
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = ns.as_deref().map(|s| s.to_string());

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::get_resource(&client, &kind, &name, ns_ref.as_deref()).await
                })
                .await;

            match result {
                Ok(Ok(server_json)) => {
                    let server_yaml = serde_yaml_ng::to_string(&server_json).unwrap_or_default();
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key) {
                            editor.apply_conflict(server_yaml);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                _ => {
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key) {
                            editor.apply_failure(
                                "Conflict detected but failed to fetch server version".to_string(),
                            );
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Accept the server version during conflict resolution.
    pub(crate) fn handle_yaml_accept_server(
        &mut self,
        cx: &mut Context<Self>,
        key: ResourceDetailKey,
    ) {
        let Some(client) = self.active_clients.get(&key.cluster_context).cloned() else {
            return;
        };
        let kind = key.kind.clone();
        let name = key.name.clone();
        let ns = key.namespace.clone();
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = ns.as_deref().map(|s| s.to_string());

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::get_resource(&client, &kind, &name, ns_ref.as_deref()).await
                })
                .await;

            match result {
                Ok(Ok(server_json)) => {
                    let new_rv = server_json
                        .get("metadata")
                        .and_then(|m| m.get("resourceVersion"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let server_yaml = serde_yaml_ng::to_string(&server_json).unwrap_or_default();
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key) {
                            editor.accept_server_version(&server_yaml, &new_rv);
                        }
                        this.resource_detail_data.insert(key, server_json);
                        this.evict_data_caches();
                        cx.notify();
                    })
                    .ok();
                }
                _ => {
                    this.update(cx, |this, cx| {
                        if let Some(editor) = this.yaml_editors.get_mut(&key) {
                            editor.apply_failure("Failed to fetch server version".to_string());
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Handle delete resource action after confirmation.
    fn handle_delete_resource(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    ) {
        let Some(client) = self.active_clients.get(&cluster_context).cloned() else {
            return;
        };
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = namespace.as_deref().map(|s| s.to_string());
        let ctx_clone = cluster_context.clone();
        let kind_clone = kind.clone();
        let name_clone = name.clone();
        let ns_clone = namespace.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::delete_resource(&client, &kind, &name, ns_ref.as_deref())
                        .await
                })
                .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => {
                        // Remove cached data
                        let detail_key = ResourceDetailKey {
                            cluster_context: ctx_clone.clone(),
                            kind: kind_clone.clone(),
                            name: name_clone.clone(),
                            namespace: ns_clone.clone(),
                        };
                        this.resource_detail_data.remove(&detail_key);
                        this.yaml_editors.remove(&detail_key);
                        this.yaml_editor_focus_handles.remove(&detail_key);
                        this.detail_active_tab.remove(&detail_key);

                        // Close the detail tab if open
                        let target = NavigationTarget::ResourceDetail {
                            cluster_context: ctx_clone,
                            kind: kind_clone,
                            name: name_clone,
                            namespace: ns_clone,
                        };
                        // Find tab by target and close it
                        if let Some(tab_id) =
                            this.workspace.tabs.iter().find(|t| t.target == target).map(|t| t.id)
                        {
                            this.workspace.close_tab(tab_id);
                        }
                    }
                    Ok(Err(err)) => {
                        tracing::error!("Delete failed: {err}");
                    }
                    Err(join_err) => {
                        tracing::error!("Delete task panicked: {join_err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Handle scale resource action after confirmation.
    fn handle_scale_resource(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
        replicas: u32,
    ) {
        let Some(client) = self.active_clients.get(&cluster_context).cloned() else {
            return;
        };
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = namespace.as_deref().map(|s| s.to_string());

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::scale_resource(
                        &client,
                        &kind,
                        &name,
                        ns_ref.as_deref(),
                        replicas,
                    )
                    .await
                })
                .await;

            this.update(cx, |_this, cx| {
                match result {
                    Ok(Ok(_)) => {
                        tracing::info!("Scale succeeded");
                    }
                    Ok(Err(err)) => {
                        tracing::error!("Scale failed: {err}");
                    }
                    Err(join_err) => {
                        tracing::error!("Scale task panicked: {join_err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Handle restart resource action after confirmation.
    fn handle_restart_resource(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    ) {
        let Some(client) = self.active_clients.get(&cluster_context).cloned() else {
            return;
        };
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let ns_ref = namespace.as_deref().map(|s| s.to_string());

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::restart_resource(&client, &kind, &name, ns_ref.as_deref())
                        .await
                })
                .await;

            this.update(cx, |_this, cx| {
                match result {
                    Ok(Ok(_)) => {
                        tracing::info!("Restart succeeded");
                    }
                    Ok(Err(err)) => {
                        tracing::error!("Restart failed: {err}");
                    }
                    Err(join_err) => {
                        tracing::error!("Restart task panicked: {join_err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Handle cordon/uncordon node action after confirmation.
    fn handle_cordon_node(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: String,
        name: String,
        cordon: bool,
    ) {
        let Some(client) = self.active_clients.get(&cluster_context).cloned() else {
            return;
        };
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    if cordon {
                        baeus_core::client::cordon_node(&client, &name).await
                    } else {
                        baeus_core::client::uncordon_node(&client, &name).await
                    }
                })
                .await;

            this.update(cx, |_this, cx| {
                let action_name = if cordon { "Cordon" } else { "Uncordon" };
                match result {
                    Ok(Ok(_)) => {
                        tracing::info!("{action_name} succeeded");
                    }
                    Ok(Err(err)) => {
                        tracing::error!("{action_name} failed: {err}");
                    }
                    Err(join_err) => {
                        tracing::error!("{action_name} task panicked: {join_err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Render the confirm dialog overlay (backdrop + dialog box).
    fn render_confirm_dialog_overlay(&self, cx: &mut Context<Self>) -> Div {
        let Some(ref ctx) = self.confirm_dialog else {
            return div();
        };

        let backdrop_color = crate::theme::Color::rgba(0, 0, 0, 128).to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let text_primary = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();

        let confirm_bg = match ctx.dialog.severity {
            crate::components::confirm_dialog::DialogSeverity::Destructive => {
                self.theme.colors.error.to_gpui()
            }
            crate::components::confirm_dialog::DialogSeverity::Warning => {
                self.theme.colors.warning.to_gpui()
            }
            crate::components::confirm_dialog::DialogSeverity::Info => {
                self.theme.colors.accent.to_gpui()
            }
        };

        let title = SharedString::from(ctx.dialog.title.clone());
        let message = SharedString::from(ctx.dialog.message.clone());
        let cancel_label = SharedString::from(ctx.dialog.cancel_label.clone());
        let confirm_label = SharedString::from(ctx.dialog.confirm_label.clone());

        let action = ctx.action.clone();

        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .justify_center()
            .items_center()
            // Backdrop — click to dismiss
            .child(
                div()
                    .id("dialog-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h_full()
                    .bg(backdrop_color)
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.confirm_dialog = None;
                        cx.notify();
                    })),
            )
            // Dialog box
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(400.0))
                    .bg(surface)
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    // Header
                    .child(
                        div().px_4().py_3().border_b_1().border_color(border).child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_primary)
                                .child(title),
                        ),
                    )
                    // Body
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .child(div().text_sm().text_color(text_secondary).child(message)),
                    )
                    // Footer buttons
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.0))
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(border)
                            // Cancel
                            .child(
                                div()
                                    .id("confirm-dialog-cancel")
                                    .px_4()
                                    .py_2()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(border)
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(text_primary)
                                    .child(cancel_label)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.confirm_dialog = None;
                                        cx.notify();
                                    })),
                            )
                            // Confirm
                            .child(
                                div()
                                    .id("confirm-dialog-confirm")
                                    .px_4()
                                    .py_2()
                                    .rounded(px(6.0))
                                    .bg(confirm_bg)
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(gpui::rgb(0xFFFFFF))
                                    .child(confirm_label)
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        let action = action.clone();
                                        this.confirm_dialog = None;
                                        match action {
                                            PendingAction::DeleteResource {
                                                cluster_context,
                                                kind,
                                                name,
                                                namespace,
                                            } => {
                                                this.handle_delete_resource(
                                                    cx,
                                                    cluster_context,
                                                    kind,
                                                    name,
                                                    namespace,
                                                );
                                            }
                                            PendingAction::ScaleResource {
                                                cluster_context,
                                                kind,
                                                name,
                                                namespace,
                                                replicas,
                                            } => {
                                                this.handle_scale_resource(
                                                    cx,
                                                    cluster_context,
                                                    kind,
                                                    name,
                                                    namespace,
                                                    replicas,
                                                );
                                            }
                                            PendingAction::RestartResource {
                                                cluster_context,
                                                kind,
                                                name,
                                                namespace,
                                            } => {
                                                this.handle_restart_resource(
                                                    cx,
                                                    cluster_context,
                                                    kind,
                                                    name,
                                                    namespace,
                                                );
                                            }
                                            PendingAction::CordonNode { cluster_context, name } => {
                                                this.handle_cordon_node(
                                                    cx,
                                                    cluster_context,
                                                    name,
                                                    true,
                                                );
                                            }
                                            PendingAction::UncordonNode {
                                                cluster_context,
                                                name,
                                            } => {
                                                this.handle_cordon_node(
                                                    cx,
                                                    cluster_context,
                                                    name,
                                                    false,
                                                );
                                            }
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }
}
// T027: Dashboard data loading
// T028: Real-time event updates
// T029: Cluster switching
// ---------------------------------------------------------------------------

impl AppShell {
    // --- T025: Wire cluster connection action ---

    /// Handle a connect action for a cluster identified by its context name.
    ///
    /// Steps:
    /// 1. Set the sidebar and cluster manager to "Connecting" state.
    /// 2. Retrieve the Tokio handle from the GPUI global context.
    /// 3. Spawn an async task that creates a real kube::Client via baeus_core::client.
    /// 4. On success, store the client and navigate to the dashboard.
    pub(crate) fn handle_connect_cluster(&mut self, context_name: &str, cx: &mut Context<Self>) {
        let context = context_name.to_string();
        tracing::info!("Connecting to cluster: {}", context);

        // If an EKS cluster has no stored credentials, no active client, and no kubeconfig,
        // we can't connect it — return silently. The async role assumption task
        // will call us when credentials are ready.
        if context.starts_with("eks:")
            && !self.eks_cluster_data.contains_key(&context)
            && !self.active_clients.contains_key(&context)
            && !self.kubeconfig_paths.contains_key(&context)
        {
            tracing::info!("EKS cluster '{}' has no credentials yet, skipping connection", context);
            return;
        }

        // If we already have an active client for this context (e.g., from EKS wizard),
        // just mark it connected and skip the kubeconfig-based connection.
        if self.active_clients.contains_key(&context) {
            Self::set_sidebar_cluster_status(&mut self.sidebar, &context, ClusterStatus::Connected);
            Self::set_manager_connecting(&mut self.cluster_manager, &context);
            for conn in self.cluster_manager.list_connections() {
                if conn.context_name == context {
                    let id = conn.id;
                    if let Some(c) = self.cluster_manager.get_connection_mut(&id) {
                        c.set_connected();
                    }
                    break;
                }
            }
            cx.notify();
            return;
        }

        // If this is an EKS wizard cluster, connect using temp kubeconfig with exec plugin.
        // This uses `aws eks get-token` which refreshes tokens automatically (no 60s expiry).
        if let Some((cluster, creds, role_arn)) = self.eks_cluster_data.get(&context).cloned() {
            tracing::info!(
                "EKS connect via kubeconfig exec: cluster '{}', role={:?}",
                context,
                role_arn,
            );

            // Generate temp kubeconfig with aws eks get-token exec plugin
            let kubeconfig_path = match self.generate_eks_kubeconfig_file_with_role(
                &context,
                &cluster,
                role_arn.as_deref(),
            ) {
                Ok(path) => {
                    self.kubeconfig_paths.insert(context.clone(), path.clone());
                    path
                }
                Err(e) => {
                    self.connection_errors
                        .insert(context.clone(), format!("Failed to write EKS kubeconfig: {e}"));
                    Self::set_sidebar_cluster_status(
                        &mut self.sidebar,
                        &context,
                        ClusterStatus::Error,
                    );
                    cx.notify();
                    return;
                }
            };

            Self::set_sidebar_cluster_status(
                &mut self.sidebar,
                &context,
                ClusterStatus::Connecting,
            );
            Self::set_manager_connecting(&mut self.cluster_manager, &context);

            // Extract credentials for in-memory injection (never written to disk)
            let ak = creds.access_key_id().to_string();
            let sk = creds.secret_access_key().to_string();
            let st = creds.session_token().map(|s| s.to_string());

            // Clear stored credentials
            self.eks_cluster_data.remove(&context);

            let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
            let ctx = context.clone();
            let kube_ctx = context.clone();
            let saved_connections = self.preferences.saved_eks_connections.clone();
            cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
                let result = tokio_handle
                    .spawn(async move {
                        // Inject wizard's in-memory credentials into the exec env
                        // so aws eks get-token can authenticate without system AWS CLI creds.
                        let client = baeus_core::client::create_client_from_path_with_aws_creds(
                            &kube_ctx,
                            &kubeconfig_path,
                            &ak,
                            &sk,
                            st.as_deref(),
                        )
                        .await
                        .map_err(|e| format!("{e:#}"))?;
                        let version = baeus_core::client::verify_connection(&client)
                            .await
                            .map_err(|e| format!("{e:#}"))?;
                        Ok::<(KubeClient, String), String>((client, version))
                    })
                    .await;
                match result {
                    Ok(Ok((client, version))) => {
                        tracing::info!("EKS cluster {ctx} connected, k8s {version}");
                        this.update(cx, |this, cx| {
                            this.active_clients.insert(ctx.clone(), client);
                            Self::set_sidebar_cluster_status(
                                &mut this.sidebar,
                                &ctx,
                                ClusterStatus::Connected,
                            );
                            if let Some(id) = Self::find_connection_id(&this.cluster_manager, &ctx)
                            {
                                if let Some(c) = this.cluster_manager.get_connection_mut(&id) {
                                    c.set_connected();
                                    c.reset_reconnect_attempts();
                                }
                            }
                            this.k8s_version = Some(version);
                            cx.notify();
                        })
                        .ok();
                    }
                    Ok(Err(msg)) => {
                        let enriched = Self::enrich_eks_error(&msg, &ctx, &saved_connections);
                        tracing::error!("EKS connection failed: {enriched}");
                        this.update(cx, |this, cx| {
                            Self::set_sidebar_cluster_status(
                                &mut this.sidebar,
                                &ctx,
                                ClusterStatus::Error,
                            );
                            this.connection_errors.insert(ctx.clone(), enriched);
                            cx.notify();
                        })
                        .ok();
                    }
                    Err(e) => {
                        let msg = format!("EKS connection task failed: {e}");
                        tracing::error!("{msg}");
                        this.update(cx, |this, cx| {
                            Self::set_sidebar_cluster_status(
                                &mut this.sidebar,
                                &ctx,
                                ClusterStatus::Error,
                            );
                            this.connection_errors.insert(ctx.clone(), msg);
                            cx.notify();
                        })
                        .ok();
                    }
                }
            })
            .detach();
            return;
        }

        // 1. Update sidebar cluster status to Connecting.
        Self::set_sidebar_cluster_status(&mut self.sidebar, &context, ClusterStatus::Connecting);

        // Standard kubeconfig connection path (non-EKS, or restored EKS with kubeconfig)
        self.connect_cluster_via_kubeconfig(context, cx);
    }

    /// Enrich an EKS connection error with re-auth guidance.
    fn enrich_eks_error(
        msg: &str,
        context_name: &str,
        saved_connections: &[crate::views::preferences::SavedEksConnectionInfo],
    ) -> String {
        let is_auth_error = msg.contains("Unauthorized")
            || msg.contains("401")
            || msg.contains("forbidden")
            || msg.contains("403")
            || msg.contains("ExpiredToken")
            || msg.contains("InvalidClientTokenId");

        if !is_auth_error {
            return msg.to_string();
        }

        // Find the saved connection for this context to get SSO info
        let sso_info = saved_connections.iter().find(|c| {
            let ctx = baeus_core::aws_eks::eks_context_name_from_parts(&c.cluster_name, &c.region);
            ctx == context_name
        });

        let mut enriched = format!("{msg}\n\nAWS credentials may have expired.");
        if let Some(conn) = sso_info {
            if conn.auth_method == "Sso" {
                enriched.push_str("\n\nTo re-authenticate, run:");
                enriched.push_str("\n  aws sso login");
                if let Some(ref url) = conn.sso_start_url {
                    enriched.push_str(&format!("\n\nSSO Start URL: {url}"));
                }
            }
        } else {
            enriched.push_str("\n\nRun 'aws sso login' or check your AWS credentials.");
        }
        enriched
    }

    fn connect_cluster_via_kubeconfig(&mut self, context: String, cx: &mut Context<Self>) {
        // 2. Update core cluster manager to Connecting.
        Self::set_manager_connecting(&mut self.cluster_manager, &context);

        // 3. Get the Tokio handle.
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();

        // Look up the kubeconfig file path for this context so we can load
        // configs from non-default scan directories.
        let kubeconfig_path = self.kubeconfig_paths.get(&context).cloned();

        // Use the original kubeconfig context name (before disambiguation) for kube-rs.
        let kube_context_name =
            self.original_context_names.get(&context).cloned().unwrap_or_else(|| context.clone());

        // Look up AWS profile: cluster-specific first, then default.
        // Passed to create_client_from_path which injects it into kubeconfig exec env.
        let aws_profile =
            self.cluster_aws_profiles.get(&context).or(self.default_aws_profile.as_ref()).cloned();

        // 4. Spawn an async task that creates a real kube::Client (T324).
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    // T324: Real kube-rs connection via baeus_core::client.
                    // Use path-specific loader when we know the kubeconfig file,
                    // otherwise fall back to default resolution.
                    let client = if let Some(path) = kubeconfig_path {
                        baeus_core::client::create_client_from_path(
                            &kube_context_name,
                            &path,
                            aws_profile.as_deref(),
                        )
                        .await
                        .map_err(|e| format!("{e:#}"))?
                    } else {
                        baeus_core::client::create_client(&kube_context_name)
                            .await
                            .map_err(|e| format!("{e:#}"))?
                    };
                    baeus_core::client::verify_connection(&client)
                        .await
                        .map_err(|e| format!("{e:#}"))?;
                    Ok::<KubeClient, String>(client)
                })
                .await;

            // Post the result back to the GPUI main thread.
            let context_for_update = context.clone();
            match result {
                Ok(Ok(client)) => {
                    this.update(cx, |this, cx| {
                        // T324: Store the client so views can reuse it.
                        this.active_clients.insert(context_for_update.clone(), client);
                        this.on_cluster_connected(&context_for_update, cx);
                    })
                    .ok();
                }
                Ok(Err(err)) => {
                    this.update(cx, |this, cx| {
                        this.on_cluster_connection_error(&context_for_update, &err, cx);
                    })
                    .ok();
                }
                Err(join_err) => {
                    let msg = format!("Connection task panicked: {join_err}");
                    this.update(cx, |this, cx| {
                        this.on_cluster_connection_error(&context_for_update, &msg, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Called on the main thread after a cluster connection succeeds.
    fn on_cluster_connected(&mut self, context_name: &str, cx: &mut Context<Self>) {
        tracing::info!("Cluster connected: {}", context_name);

        // Update sidebar status.
        Self::set_sidebar_cluster_status(&mut self.sidebar, context_name, ClusterStatus::Connected);

        // Update core cluster manager.
        Self::set_manager_connected(&mut self.cluster_manager, context_name);

        // Clear any previous connection error for this cluster.
        self.connection_errors.remove(context_name);

        // Navigate to the dashboard and start loading data.
        let dashboard_target =
            NavigationTarget::Dashboard { cluster_context: context_name.to_string() };
        self.layout.navigate(dashboard_target.clone());
        self.workspace.open_tab(dashboard_target);
        self.start_dashboard_loading(context_name, cx);
    }

    /// Handle a disconnect action for a cluster.
    fn handle_disconnect_cluster(&mut self, context_name: &str, _cx: &mut Context<Self>) {
        tracing::info!("Disconnecting cluster: {}", context_name);

        // Update sidebar status.
        Self::set_sidebar_cluster_status(
            &mut self.sidebar,
            context_name,
            ClusterStatus::Disconnected,
        );

        // Update core cluster manager.
        Self::set_manager_disconnected(&mut self.cluster_manager, context_name);

        // Stop any informers for this cluster (T028).
        self.stop_event_watcher(context_name);

        // T323: Remove the cached kube client for this cluster.
        self.active_clients.remove(context_name);

        // Clear dashboard if this was the active dashboard cluster.
        if self.active_dashboard_cluster.as_deref() == Some(context_name) {
            self.dashboard_state = None;
            self.active_dashboard_cluster = None;
        }
    }

    /// Handle a connection error for a cluster.
    fn on_cluster_connection_error(
        &mut self,
        context_name: &str,
        error_message: &str,
        cx: &mut Context<Self>,
    ) {
        // Sanitize error messages to prevent credential leakage in UI and logs.
        let sanitized = sanitize_error_message(error_message);
        tracing::warn!("Cluster connection error for {}: {}", context_name, sanitized);

        // Update sidebar status.
        Self::set_sidebar_cluster_status(&mut self.sidebar, context_name, ClusterStatus::Error);

        // Update core cluster manager.
        if let Some(id) = Self::find_connection_id(&self.cluster_manager, context_name) {
            if let Some(conn_mut) = self.cluster_manager.get_connection_mut(&id) {
                conn_mut.set_error(sanitized.clone());
            }
        }

        // Store the error so the dashboard can display it.
        self.connection_errors.insert(context_name.to_string(), sanitized);

        // Open a Dashboard tab so the user sees the error.
        let dashboard_target =
            NavigationTarget::Dashboard { cluster_context: context_name.to_string() };
        self.workspace.open_tab(dashboard_target);

        // Check if this is an AWS SSO token expiry and surface a login banner.
        if baeus_core::aws_sso::is_aws_sso_auth_error(error_message) {
            let profile = self
                .cluster_aws_profiles
                .get(context_name)
                .or(self.default_aws_profile.as_ref())
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            self.pending_sso_login =
                Some(PendingSsoLogin { profile, cluster_context: context_name.to_string() });
        }

        cx.notify();
    }

    // --- T027: Wire dashboard data loading ---

    /// Start loading dashboard data for a connected cluster.
    ///
    /// Sets the dashboard to loading state, then spawns a task to fetch:
    /// - Node list (count + health)
    /// - Pod summary (running, pending, failed, succeeded)
    /// - Namespace list
    /// - Recent events
    ///
    /// For now, the task simulates completion with placeholder data.
    fn start_dashboard_loading(&mut self, context_name: &str, cx: &mut Context<Self>) {
        let context = context_name.to_string();
        tracing::info!("Loading dashboard data for cluster: {}", context);

        // Set loading state.
        let loading_state = DashboardState::loading(context_name, uuid::Uuid::new_v4());
        self.dashboard_state = Some(loading_state);
        self.active_dashboard_cluster = Some(context.clone());

        // T325: Get the stored kube::Client for this cluster.
        let Some(client) = self.active_clients.get(&context).cloned() else {
            tracing::warn!("No active client for cluster {}; cannot load dashboard", context);
            if let Some(ref mut state) = self.dashboard_state {
                state.set_error("No active client — connect to cluster first".to_string());
            }
            return;
        };

        // Retrieve the Tokio handle.
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();

        // Spawn a GPUI task that runs real kube-rs data fetching on Tokio.
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    // T325: Real dashboard data via baeus_core::client.
                    baeus_core::client::fetch_dashboard_data(&client)
                        .await
                        .map_err(|e| e.to_string())
                })
                .await;

            let context_for_update = context.clone();
            match result {
                Ok(Ok(data)) => {
                    this.update(cx, |this, cx| {
                        this.on_dashboard_data_loaded(&context_for_update, data, cx);
                    })
                    .ok();
                }
                Ok(Err(err)) => {
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&err) {
                            this.on_cluster_connection_lost(&context_for_update, &err, cx);
                        }
                        // T364: Check for 403 Forbidden and format RBAC error.
                        let display_err = if baeus_core::client::is_forbidden_error_string(&err) {
                            baeus_core::client::format_rbac_error("access", "dashboard data", None)
                        } else {
                            err
                        };
                        // T362: Store error in view_errors map.
                        let error_key = format!("dashboard:{}", context_for_update);
                        this.view_errors.insert(error_key, display_err.clone());
                        if let Some(ref mut state) = this.dashboard_state {
                            state.set_error(display_err);
                        }
                    })
                    .ok();
                }
                Err(join_err) => {
                    let msg = format!("Dashboard load task panicked: {join_err}");
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&msg) {
                            this.on_cluster_connection_lost(&context_for_update, &msg, cx);
                        }
                        // T362: Store error in view_errors map.
                        let error_key = format!("dashboard:{}", context_for_update);
                        this.view_errors.insert(error_key, msg.clone());
                        if let Some(ref mut state) = this.dashboard_state {
                            state.set_error(msg);
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Called on the main thread when dashboard data has been fetched.
    fn on_dashboard_data_loaded(
        &mut self,
        context_name: &str,
        data: baeus_core::client::DashboardData,
        cx: &mut Context<Self>,
    ) {
        tracing::info!("Dashboard data loaded for cluster: {}", context_name);

        // T363: If there was a prior connection error, clear it on success.
        if self.connection_errors.contains_key(context_name) {
            self.on_cluster_reconnected(context_name, cx);
        }

        // T362: Clear any previous error for this dashboard view.
        let error_key = format!("dashboard:{}", context_name);
        self.view_errors.remove(&error_key);

        // Only update if we are still viewing this cluster's dashboard.
        if self.active_dashboard_cluster.as_deref() != Some(context_name) {
            return;
        }

        // Assign K8s version from the dashboard response so the status bar shows it.
        self.k8s_version = Some(data.k8s_version.clone());

        if let Some(ref mut state) = self.dashboard_state {
            // T325: Populate dashboard from real API response.
            state.node_count = data.nodes.len() as u32;
            state.nodes = data
                .nodes
                .iter()
                .map(|n| {
                    let mut node =
                        crate::views::dashboard::NodeHealth::new(n.name.clone(), n.ready);
                    for role in &n.roles {
                        node = node.with_role(role.clone());
                    }
                    node
                })
                .collect();
            state.pod_summary = crate::views::dashboard::PodSummary::new(
                data.pod_counts.running,
                data.pod_counts.pending,
                data.pod_counts.failed,
                data.pod_counts.succeeded,
            );
            state.namespaces = data.namespaces;

            // Populate per-cluster namespace selector for resource list filtering.
            let mut ns_sel = EnhancedNamespaceSelector::new();
            ns_sel.set_available_namespaces(state.namespaces.clone());
            self.namespace_selectors.insert(context_name.to_string(), ns_sel);

            state.recent_events = data
                .events
                .iter()
                .map(|e| {
                    DashboardEvent::with_details(
                        e.reason.clone(),
                        e.message.clone(),
                        e.timestamp,
                        e.is_warning,
                        e.namespace.clone(),
                        e.involved_object_kind.clone(),
                        e.involved_object_name.clone(),
                        e.source.clone(),
                        e.count,
                        e.last_seen,
                    )
                })
                .collect();

            // Populate resource counts from API.
            state.resource_counts = ResourceCounts {
                pods: data.resource_counts.pods,
                deployments: data.resource_counts.deployments,
                daemonsets: data.resource_counts.daemonsets,
                statefulsets: data.resource_counts.statefulsets,
                replicasets: data.resource_counts.replicasets,
                jobs: data.resource_counts.jobs,
                cronjobs: data.resource_counts.cronjobs,
            };

            // Compute aggregate CPU and memory capacity from node allocatable resources.
            let total_cpu_millis: u64 =
                data.nodes.iter().filter_map(|n| n.allocatable_cpu_millis).sum();
            let total_memory_bytes: u64 =
                data.nodes.iter().filter_map(|n| n.allocatable_memory_bytes).sum();

            if total_cpu_millis > 0 {
                let cpu_cores = total_cpu_millis as f64 / 1000.0;
                state.cpu_capacity = Some(cpu_cores);
                // Without metrics-server, show capacity but no usage percentage.
                state.cpu_used = None;
                state.cpu_usage_percent = None;
            }
            if total_memory_bytes > 0 {
                state.memory_capacity = Some(total_memory_bytes as f64);
                state.memory_used = None;
                state.memory_usage_percent = None;
            }

            state.set_loaded();
        }

        // Start the real-time event watcher (T028).
        self.start_event_watcher(context_name, cx);

        // Start metrics polling from the metrics-server.
        self.start_metrics_polling(context_name, cx);
    }

    // --- T326: Wire real event watcher ---

    /// Start a real-time event watcher for the given cluster.
    ///
    /// T326: Uses `baeus_core::client::watch_events` to open a kube-rs watcher
    /// stream for `core/v1/Event` resources. Each incoming event is pushed to
    /// `DashboardState.recent_events` via `WeakEntity::update` on the GPUI
    /// main thread.
    fn start_event_watcher(&mut self, context_name: &str, cx: &mut Context<Self>) {
        tracing::info!("T326: Starting real event watcher for cluster: {}", context_name);

        // Find the cluster ID for this context.
        let cluster_id = Self::find_connection_id(&self.cluster_manager, context_name);

        let Some(cluster_id) = cluster_id else {
            tracing::warn!(
                "Cannot start event watcher: cluster {} not found in manager",
                context_name
            );
            return;
        };

        // Get the stored kube::Client for this cluster.
        let Some(client) = self.active_clients.get(context_name).cloned() else {
            tracing::warn!(
                "No active client for cluster {}; cannot start event watcher",
                context_name
            );
            return;
        };

        // Register the standard set of informers (Namespace, Node, Pod, Event).
        let informer_ids = self.informer_manager.register_standard_watchers(cluster_id);

        // Mark all informers as Running.
        for id in &informer_ids {
            self.informer_manager.set_state(id, InformerState::Running);
        }

        // Retrieve the Tokio handle.
        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let context = context_name.to_string();

        // Use a channel to bridge the Tokio watch stream to GPUI entity updates.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DashboardEvent>();

        // T363: Separate channel to send watcher errors back to GPUI.
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn the actual kube-rs watcher on the Tokio runtime.
        tokio_handle.spawn(async move {
            let result = baeus_core::client::watch_events(&client, move |event_info| {
                let dashboard_event = DashboardEvent::with_details(
                    event_info.reason.clone(),
                    event_info.message.clone(),
                    event_info.timestamp,
                    event_info.is_warning,
                    event_info.namespace.clone(),
                    event_info.involved_object_kind.clone(),
                    event_info.involved_object_name.clone(),
                    event_info.source.clone(),
                    event_info.count,
                    event_info.last_seen,
                );
                // Send to the GPUI-side receiver; ignore errors if the receiver
                // has been dropped (cluster disconnected).
                let _ = tx.send(dashboard_event);
            })
            .await;

            if let Err(e) = result {
                tracing::warn!("Event watcher for {} ended with error: {}", context, e);
                // T363: Send the error back to the GPUI thread for connection loss detection.
                let _ = err_tx.send(e.to_string());
            }
        });

        // Spawn a GPUI task that reads from the channel and pushes events
        // into the AppShell's dashboard state on the main thread.
        let context_for_watcher_err = context_name.to_string();
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            // Process events and errors concurrently.
            loop {
                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                let ok = this.update(cx, |this, _cx| {
                                    this.push_dashboard_event(event);
                                });
                                if ok.is_err() {
                                    break;
                                }
                            }
                            None => break, // Channel closed
                        }
                    }
                    err = err_rx.recv() => {
                        if let Some(err_msg) = err {
                            if is_connection_error(&err_msg) {
                                let ctx = context_for_watcher_err.clone();
                                let _ = this.update(cx, |this, cx| {
                                    this.on_cluster_connection_lost(&ctx, &err_msg, cx);
                                });
                            }
                        }
                        break; // Watcher ended
                    }
                }
            }
        })
        .detach();
    }

    /// Stop the event watcher for a cluster by stopping its informers.
    fn stop_event_watcher(&mut self, context_name: &str) {
        tracing::info!("Stopping event watcher for cluster: {}", context_name);

        let cluster_id = Self::find_connection_id(&self.cluster_manager, context_name);

        if let Some(cluster_id) = cluster_id {
            self.informer_manager.stop_for_cluster(&cluster_id);
        }
    }

    // --- Metrics-server polling ---

    /// Start polling metrics-server for real CPU/memory usage data.
    ///
    /// Fetches node and pod metrics every 30 seconds, updating `self.cluster_metrics`
    /// and the dashboard donut chart values.
    fn start_metrics_polling(&mut self, context_name: &str, cx: &mut Context<Self>) {
        tracing::info!("Starting metrics polling for cluster: {}", context_name);

        let Some(client) = self.active_clients.get(context_name).cloned() else {
            tracing::warn!("No active client for {}; cannot start metrics polling", context_name);
            return;
        };

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let _context = context_name.to_string();

        // Channel to send metrics results back to the GPUI thread.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MetricsPayload>();

        // Spawn a polling loop on the Tokio runtime.
        tokio_handle.spawn(async move {
            loop {
                let (node_result, pod_result) = tokio::join!(
                    baeus_core::client::fetch_node_metrics(&client),
                    baeus_core::client::fetch_pod_metrics(&client, None),
                );

                let payload = MetricsPayload { nodes: node_result.ok(), pods: pod_result.ok() };

                if tx.send(payload).is_err() {
                    break; // Receiver dropped (view destroyed)
                }

                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });

        // GPUI task to process metrics results on the main thread.
        let ctx_for_update = context_name.to_string();
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            while let Some(payload) = rx.recv().await {
                let ctx = ctx_for_update.clone();
                let ok = this.update(cx, |this, cx| {
                    this.on_metrics_received(&ctx, payload, cx);
                });
                if ok.is_err() {
                    break; // Entity dropped
                }
            }
        })
        .detach();
    }

    /// Handle incoming metrics data from the polling loop.
    fn on_metrics_received(
        &mut self,
        context_name: &str,
        payload: MetricsPayload,
        cx: &mut Context<Self>,
    ) {
        let state = self.cluster_metrics.entry(context_name.to_string()).or_default();

        match (&payload.nodes, &payload.pods) {
            (Some(nodes), Some(pods)) => {
                state.set_available(nodes.clone(), pods.clone());

                // Also update DashboardState donut charts if this is the active cluster.
                if self.active_dashboard_cluster.as_deref() == Some(context_name) {
                    if let Some(ref mut dashboard) = self.dashboard_state {
                        let cpu_used_millis = state.total_node_cpu_millicores();
                        let cpu_cap_millis = state.total_node_cpu_capacity();
                        let mem_used = state.total_node_memory_bytes();
                        let mem_cap = state.total_node_memory_capacity();

                        if cpu_cap_millis > 0 {
                            let pct = (cpu_used_millis as f64 / cpu_cap_millis as f64) * 100.0;
                            dashboard.cpu_used = Some(cpu_used_millis as f64 / 1000.0);
                            dashboard.cpu_usage_percent = Some(pct as f32);
                        }
                        if mem_cap > 0 {
                            let pct = (mem_used as f64 / mem_cap as f64) * 100.0;
                            dashboard.memory_used = Some(mem_used as f64);
                            dashboard.memory_usage_percent = Some(pct as f32);
                        }
                    }
                }
            }
            (None, None) => {
                state.set_unavailable();
                tracing::debug!("Metrics unavailable for {context_name}");
            }
            _ => {
                // Partial data: set what we have.
                if let Some(nodes) = payload.nodes {
                    let pods = state.pod_metrics.clone();
                    state.set_available(nodes, pods);
                }
                if let Some(pods) = payload.pods {
                    let nodes = state.node_metrics.clone();
                    state.set_available(nodes, pods);
                }
            }
        }

        cx.notify();
    }

    /// Push a new event into the dashboard state (called from the watch loop).
    #[allow(dead_code)]
    fn push_dashboard_event(&mut self, event: DashboardEvent) {
        if let Some(ref mut state) = self.dashboard_state {
            state.recent_events.push(event);

            // Keep a bounded number of recent events.
            const MAX_EVENTS: usize = 100;
            if state.recent_events.len() > MAX_EVENTS {
                let drain_count = state.recent_events.len() - MAX_EVENTS;
                state.recent_events.drain(0..drain_count);
            }
        }
    }

    // --- T327: Wire real resource listing ---

    /// Start loading resources for a `NavigationTarget::ResourceList` tab.
    ///
    /// Fetches the initial list of resources from the K8s API via
    /// `baeus_core::client::list_resources`, then stores the result in
    /// `resource_list_data`. After the initial list is loaded, starts an
    /// informer-backed watcher for live updates (T327b).
    pub fn start_resource_loading(
        &mut self,
        cluster_context: &str,
        kind: &str,
        namespace: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };

        tracing::info!(
            "T327: Loading resources for {}/{} (ns={:?})",
            cluster_context,
            kind,
            namespace,
        );

        // Get the stored kube::Client for this cluster.
        let Some(client) = self.active_clients.get(cluster_context).cloned() else {
            tracing::warn!(
                "No active client for cluster {}; cannot load resources",
                cluster_context
            );
            return;
        };

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let kind_owned = kind.to_string();
        let ns_owned = namespace.map(|s| s.to_string());
        let key_for_update = key.clone();
        let ctx_for_watcher = cluster_context.to_string();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let kind_for_fetch = kind_owned.clone();
            let ns_for_fetch = ns_owned.clone();
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::list_resources(
                        &client,
                        &kind_for_fetch,
                        ns_for_fetch.as_deref(),
                    )
                    .await
                    .map_err(|e| e.to_string())
                })
                .await;

            match result {
                Ok(Ok(items)) => {
                    this.update(cx, |this, cx| {
                        tracing::info!(
                            "T327: Loaded {} items for {}/{}",
                            items.len(),
                            key_for_update.cluster_context,
                            key_for_update.kind,
                        );
                        // T363: If there was a prior connection error, clear it on success.
                        if this.connection_errors.contains_key(&key_for_update.cluster_context) {
                            let ctx = key_for_update.cluster_context.clone();
                            this.on_cluster_reconnected(&ctx, cx);
                        }
                        // T362: Clear any previous error for this resource list view.
                        let error_key = format!(
                            "resources:{}:{}",
                            key_for_update.cluster_context, key_for_update.kind,
                        );
                        this.view_errors.remove(&error_key);
                        // Build table rows from JSON via json_extract
                        let rows: Vec<TableRow> = items
                            .iter()
                            .map(|item| json_extract::json_to_table_row(&key_for_update.kind, item))
                            .collect();
                        let columns = columns_for_kind(&key_for_update.kind);
                        let mut table_state = ResourceTableState::new(columns, 50);
                        table_state.set_rows(rows);
                        this.resource_table_states.insert(key_for_update.clone(), table_state);
                        this.resource_list_data.insert(key_for_update.clone(), items);
                        this.evict_data_caches();
                        this.start_resource_watcher(
                            &ctx_for_watcher,
                            &kind_owned,
                            ns_owned.as_deref(),
                            cx,
                        );
                    })
                    .ok();
                }
                Ok(Err(err)) => {
                    tracing::warn!("T327: Failed to list resources: {}", err);
                    // T363: Detect connection-level errors.
                    let ctx_for_conn = ctx_for_watcher.clone();
                    // T362/T364: Store error in view_errors map.
                    // T364: Check for 403 Forbidden and format RBAC error.
                    let key_for_err = key_for_update.clone();
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&err) {
                            this.on_cluster_connection_lost(&ctx_for_conn, &err, cx);
                        }
                        let display_err = if baeus_core::client::is_forbidden_error_string(&err) {
                            baeus_core::client::format_rbac_error(
                                "list",
                                &key_for_err.kind,
                                key_for_err.namespace.as_deref(),
                            )
                        } else {
                            err
                        };
                        let error_key = format!(
                            "resources:{}:{}",
                            key_for_err.cluster_context, key_for_err.kind,
                        );
                        this.view_errors.insert(error_key, display_err);
                    })
                    .ok();
                }
                Err(join_err) => {
                    tracing::warn!("T327: Resource list task panicked: {}", join_err);
                    // T362: Store error in view_errors map.
                    let key_for_err = key_for_update.clone();
                    let msg = format!("Resource list task panicked: {join_err}");
                    let ctx_conn_r = ctx_for_watcher.clone();
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&msg) {
                            this.on_cluster_connection_lost(&ctx_conn_r, &msg, cx);
                        }
                        let error_key = format!(
                            "resources:{}:{}",
                            key_for_err.cluster_context, key_for_err.kind,
                        );
                        this.view_errors.insert(error_key, msg);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// T327: Public accessor for resource list data.
    /// Returns the cached list of JSON items for the given resource list key,
    /// or `None` if no data has been loaded yet.
    pub fn resource_list_items(
        &self,
        cluster_context: &str,
        kind: &str,
        namespace: Option<&str>,
    ) -> Option<&Vec<serde_json::Value>> {
        let key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };
        self.resource_list_data.get(&key)
    }

    // --- T327b: Upgrade to informer-backed live updates ---

    /// Start a watcher for the given resource kind on a cluster so that the
    /// resource list data stays in sync after the initial fetch.
    ///
    /// Uses `baeus_core::client::watch_resources` which opens a kube-rs watcher
    /// stream and sends full snapshot updates via a channel. Each update replaces
    /// the corresponding entry in `resource_list_data`.
    fn start_resource_watcher(
        &mut self,
        cluster_context: &str,
        kind: &str,
        namespace: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };

        // Don't start a duplicate watcher.
        if self.active_resource_watchers.contains(&key) {
            tracing::debug!("T327b: Watcher already running for {}/{}", cluster_context, kind,);
            return;
        }

        let Some(client) = self.active_clients.get(cluster_context).cloned() else {
            return;
        };

        tracing::info!(
            "T327b: Starting resource watcher for {}/{} (ns={:?})",
            cluster_context,
            kind,
            namespace,
        );

        self.active_resource_watchers.insert(key.clone());

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let kind_owned = kind.to_string();
        let ns_owned = namespace.map(|s| s.to_string());
        let key_for_channel = key.clone();

        // Channel to bridge snapshot updates from Tokio to GPUI.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<serde_json::Value>>();

        // T363: Separate channel to send watcher errors back to GPUI.
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn the watcher on Tokio.
        tokio_handle.spawn(async move {
            let result = baeus_core::client::watch_resources(
                &client,
                &kind_owned,
                ns_owned.as_deref(),
                move |items| {
                    let _ = tx.send(items);
                },
            )
            .await;

            if let Err(e) = result {
                tracing::warn!("T327b: Resource watcher ended with error: {}", e,);
                // T363: Send the error back to the GPUI thread.
                let _ = err_tx.send(e.to_string());
            }
        });

        // Receive updates on the GPUI thread.
        let key_for_cleanup = key.clone();
        let ctx_for_watcher_err = cluster_context.to_string();
        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            loop {
                tokio::select! {
                    item = rx.recv() => {
                        match item {
                            Some(items) => {
                                let ok = this.update(cx, |this, _cx| {
                                    // Rebuild table state from updated items
                                    let rows: Vec<TableRow> = items.iter()
                                        .map(|item| json_extract::json_to_table_row(&key_for_channel.kind, item))
                                        .collect();
                                    let columns = columns_for_kind(&key_for_channel.kind);
                                    let mut table_state = ResourceTableState::new(columns, 50);
                                    table_state.set_rows(rows);
                                    this.resource_table_states.insert(key_for_channel.clone(), table_state);
                                    this.resource_list_data.insert(key_for_channel.clone(), items);
                                    this.evict_data_caches();
                                });
                                if ok.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    err = err_rx.recv() => {
                        if let Some(err_msg) = err {
                            if is_connection_error(&err_msg) {
                                let ctx = ctx_for_watcher_err.clone();
                                let _ = this.update(cx, |this, cx| {
                                    this.on_cluster_connection_lost(&ctx, &err_msg, cx);
                                });
                            }
                        }
                        break; // Watcher ended
                    }
                }
            }
            // Clean up the tracker when the watcher ends.
            let _ = this.update(cx, |this, _cx| {
                this.active_resource_watchers.remove(&key_for_cleanup);
            });
        }).detach();
    }

    // --- T328: Wire real resource detail ---

    /// Fetch the detail for a single resource identified by kind, name, and
    /// optional namespace. Uses `baeus_core::client::get_resource` to hit the
    /// K8s API and stores the resulting JSON in `resource_detail_data`.
    ///
    /// If a `ResourceDetailState` is provided (by reference through the closure),
    /// the method populates its fields (spec, status, conditions, etc.) from the
    /// fetched JSON.
    pub fn start_resource_detail_loading(
        &mut self,
        cluster_context: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            "T328: Loading detail for {}/{}/{} (ns={:?})",
            cluster_context,
            kind,
            name,
            namespace,
        );

        let Some(client) = self.active_clients.get(cluster_context).cloned() else {
            tracing::warn!("No active client for cluster {}; cannot load detail", cluster_context);
            return;
        };

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        let kind_owned = kind.to_string();
        let name_owned = name.to_string();
        let ns_owned = namespace.map(|s| s.to_string());
        let detail_key = ResourceDetailKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let kind_for_fetch = kind_owned.clone();
            let name_for_fetch = name_owned.clone();
            let ns_for_fetch = ns_owned.clone();
            let result = tokio_handle
                .spawn(async move {
                    baeus_core::client::get_resource(
                        &client,
                        &kind_for_fetch,
                        &name_for_fetch,
                        ns_for_fetch.as_deref(),
                    )
                    .await
                    .map_err(|e| e.to_string())
                })
                .await;

            match result {
                Ok(Ok(resource_json)) => {
                    this.update(cx, |this, cx| {
                        tracing::info!(
                            "T328: Loaded detail for {}/{}",
                            detail_key.kind,
                            detail_key.name,
                        );
                        // T363: If there was a prior connection error, clear it on success.
                        if this.connection_errors.contains_key(&detail_key.cluster_context) {
                            let ctx = detail_key.cluster_context.clone();
                            this.on_cluster_reconnected(&ctx, cx);
                        }
                        // T362: Clear any previous error for this resource detail view.
                        let ns = detail_key.namespace.as_deref().unwrap_or("_");
                        let error_key = format!(
                            "detail:{}:{}:{}:{}",
                            detail_key.cluster_context, detail_key.kind, detail_key.name, ns,
                        );
                        this.view_errors.remove(&error_key);
                        this.resource_detail_data.insert(detail_key, resource_json);
                    })
                    .ok();
                }
                Ok(Err(err)) => {
                    tracing::warn!("T328: Failed to get resource detail: {}", err);
                    // T363: Detect connection-level errors.
                    let ctx_for_conn_detail = detail_key.cluster_context.clone();
                    // T362/T364: Store error in view_errors map.
                    // T364: Check for 403 Forbidden and format RBAC error.
                    let dk = detail_key.clone();
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&err) {
                            this.on_cluster_connection_lost(&ctx_for_conn_detail, &err, cx);
                        }
                        let display_err = if baeus_core::client::is_forbidden_error_string(&err) {
                            baeus_core::client::format_rbac_error(
                                "get",
                                &dk.kind,
                                dk.namespace.as_deref(),
                            )
                        } else {
                            err
                        };
                        let ns = dk.namespace.as_deref().unwrap_or("_");
                        let error_key = format!(
                            "detail:{}:{}:{}:{}",
                            dk.cluster_context, dk.kind, dk.name, ns,
                        );
                        this.view_errors.insert(error_key, display_err);
                    })
                    .ok();
                }
                Err(join_err) => {
                    tracing::warn!("T328: Resource detail task panicked: {}", join_err);
                    // T362: Store error in view_errors map.
                    let dk = detail_key.clone();
                    let msg = format!("Resource detail task panicked: {join_err}");
                    this.update(cx, |this, cx| {
                        // T363: Detect connection-level errors.
                        if is_connection_error(&msg) {
                            this.on_cluster_connection_lost(&dk.cluster_context, &msg, cx);
                        }
                        let ns = dk.namespace.as_deref().unwrap_or("_");
                        let error_key = format!(
                            "detail:{}:{}:{}:{}",
                            dk.cluster_context, dk.kind, dk.name, ns,
                        );
                        this.view_errors.insert(error_key, msg);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// T328: Public accessor for resource detail data.
    /// Returns the cached JSON value for the given resource, or `None` if not loaded yet.
    pub fn resource_detail_json(
        &self,
        cluster_context: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Option<&serde_json::Value> {
        let key = ResourceDetailKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };
        self.resource_detail_data.get(&key)
    }

    /// T328: Parse a resource JSON into a populated `ResourceDetailState`.
    ///
    /// Extracts spec, status, conditions, and events from the JSON and populates
    /// the provided detail state. This is a utility method that can be called
    /// after `resource_detail_json` returns data.
    pub fn populate_detail_state(
        detail_state: &mut ResourceDetailState,
        resource_json: &serde_json::Value,
    ) {
        // UID
        if let Some(uid) = resource_json.pointer("/metadata/uid").and_then(|v| v.as_str()) {
            detail_state.uid = Some(uid.to_string());
        }

        // resource_version
        if let Some(rv) =
            resource_json.pointer("/metadata/resourceVersion").and_then(|v| v.as_str())
        {
            detail_state.resource_version = Some(rv.to_string());
        }

        // Spec (pretty-printed JSON)
        if let Some(spec) = resource_json.get("spec") {
            if let Ok(pretty) = serde_json::to_string_pretty(spec) {
                detail_state.set_spec(pretty);
            }
        }

        // Status (pretty-printed JSON)
        if let Some(status) = resource_json.get("status") {
            if let Ok(pretty) = serde_json::to_string_pretty(status) {
                detail_state.set_status(pretty.clone());
            }

            // Conditions from status
            if let Some(conditions) = status.get("conditions").and_then(|c| c.as_array()) {
                let parsed: Vec<ConditionDisplay> = conditions
                    .iter()
                    .filter_map(|c| {
                        let type_name = c.get("type")?.as_str()?.to_string();
                        let status_val = c.get("status")?.as_str()?.to_string();
                        let reason =
                            c.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let message =
                            c.get("message").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let age = c
                            .get("lastTransitionTime")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some(ConditionDisplay {
                            type_name,
                            status: status_val,
                            reason,
                            message,
                            age,
                        })
                    })
                    .collect();
                detail_state.set_conditions(parsed);
            }
        }

        // Container names (for Pods)
        if detail_state.kind == "Pod" {
            if let Some(containers) =
                resource_json.pointer("/spec/containers").and_then(|c| c.as_array())
            {
                let names: Vec<String> = containers
                    .iter()
                    .filter_map(|c| c.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect();
                detail_state.set_container_names(names);
            }
        }

        // Full resource YAML for the YAML tab.
        // Redact Secret .data and .stringData values to prevent credential exposure.
        let yaml_json = if detail_state.kind == "Secret" {
            let mut redacted = resource_json.clone();
            redact_secret_data(&mut redacted);
            redacted
        } else {
            resource_json.clone()
        };
        if let Ok(yaml) = serde_yaml_ng::to_string(&yaml_json) {
            detail_state.resource_yaml = Some(yaml);
        }

        detail_state.set_loading(false);
    }

    // --- T029: Implement cluster switching ---

    /// Switch to a different cluster. This:
    /// 1. Disconnects the currently active cluster (stops watchers, clears dashboard).
    /// 2. Connects to the new cluster (reuses T025 pattern).
    /// 3. Starts loading dashboard data (reuses T027 pattern).
    fn handle_switch_cluster(&mut self, new_context_name: &str, cx: &mut Context<Self>) {
        tracing::info!("Switching to cluster: {}", new_context_name);

        // 1. Disconnect the current cluster if one is active.
        if let Some(current_context) = self.active_dashboard_cluster.clone() {
            if current_context == new_context_name {
                tracing::debug!("Already on cluster {}, no switch needed", new_context_name);
                // Still select it in the sidebar in case the UI state is out of sync.
                if let Some(cluster) =
                    self.sidebar.clusters.iter().find(|c| c.context_name == new_context_name)
                {
                    let id = cluster.id;
                    self.sidebar.select_cluster(id);
                }
                return;
            }
            self.handle_disconnect_cluster(&current_context, cx);
        }

        // 2. Select the new cluster in the sidebar.
        if let Some(cluster) =
            self.sidebar.clusters.iter().find(|c| c.context_name == new_context_name)
        {
            let id = cluster.id;
            self.sidebar.select_cluster(id);
        }

        // 3. Reset the header namespace selector for the new cluster.
        self.header.namespace_selector = crate::layout::header::NamespaceSelector::new();

        // 4. Connect to the new cluster (T025 pattern).
        self.handle_connect_cluster(new_context_name, cx);
    }

    // --- Helper methods ---

    /// Find a cluster connection's UUID by context name.
    fn find_connection_id(manager: &ClusterManager, context_name: &str) -> Option<uuid::Uuid> {
        manager.list_connections().iter().find(|c| c.context_name == context_name).map(|c| c.id)
    }

    /// Update sidebar cluster status by context name.
    pub(crate) fn set_sidebar_cluster_status(
        sidebar: &mut SidebarState,
        context_name: &str,
        status: ClusterStatus,
    ) {
        if let Some(cluster) = sidebar.clusters.iter_mut().find(|c| c.context_name == context_name)
        {
            cluster.status = status;
        }
    }

    /// Set a cluster to Connecting state in the cluster manager.
    fn set_manager_connecting(manager: &mut ClusterManager, context_name: &str) {
        if let Some(id) = Self::find_connection_id(manager, context_name) {
            if let Some(conn_mut) = manager.get_connection_mut(&id) {
                conn_mut.set_connecting();
            }
        }
    }

    /// Set a cluster to Connected state in the cluster manager.
    fn set_manager_connected(manager: &mut ClusterManager, context_name: &str) {
        if let Some(id) = Self::find_connection_id(manager, context_name) {
            if let Some(conn_mut) = manager.get_connection_mut(&id) {
                conn_mut.set_connected();
                conn_mut.reset_reconnect_attempts();
            }
            manager.set_active(&id);
        }
    }

    /// Set a cluster to Disconnected state in the cluster manager.
    fn set_manager_disconnected(manager: &mut ClusterManager, context_name: &str) {
        if let Some(id) = Self::find_connection_id(manager, context_name) {
            if let Some(conn_mut) = manager.get_connection_mut(&id) {
                conn_mut.set_disconnected();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// T363: Connection loss detection
// ---------------------------------------------------------------------------

/// Check whether an error message string indicates a connection-level failure
/// (timeout, refused, network unreachable, DNS resolution failure, etc.).
///
/// This is intentionally broad: kube-rs surfaces connection errors from hyper /
/// reqwest as stringified error chains, so we pattern-match on common substrings.
pub fn is_connection_error(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connection timed out")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("network unreachable")
        || lower.contains("network is unreachable")
        || lower.contains("no route to host")
        || lower.contains("dns error")
        || lower.contains("dns resolution")
        || lower.contains("name or service not known")
        || lower.contains("broken pipe")
        || lower.contains("connection closed")
        || lower.contains("eof")
        || lower.contains("hyper::error")
        || lower.contains("connect error")
        || lower.contains("tcp connect error")
}

impl AppShell {
    /// T363: Handle connection loss for a cluster.
    ///
    /// Called when an in-flight kube-rs API call (dashboard loading, resource
    /// listing, event watcher, etc.) returns a connection-level error after the
    /// cluster was previously connected.
    ///
    /// Actions:
    /// 1. Update sidebar cluster status to `ClusterStatus::Error`.
    /// 2. Store the error message in `connection_errors`.
    /// 3. Increment the notification count so the header badge alerts the user.
    /// 4. Mark all views for this cluster as stale by inserting into `view_errors`.
    /// 5. Trigger a re-render via `cx.notify()`.
    pub fn on_cluster_connection_lost(
        &mut self,
        cluster_context: &str,
        error_msg: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::warn!("T363: Connection lost for cluster {}: {}", cluster_context, error_msg,);

        // 1. Update sidebar status to Error.
        Self::set_sidebar_cluster_status(&mut self.sidebar, cluster_context, ClusterStatus::Error);

        // 2. Store the connection error.
        self.connection_errors.insert(cluster_context.to_string(), error_msg.to_string());

        // Update the core cluster manager error state.
        if let Some(id) = Self::find_connection_id(&self.cluster_manager, cluster_context) {
            if let Some(conn_mut) = self.cluster_manager.get_connection_mut(&id) {
                conn_mut.set_error(format!("Connection lost: {}", error_msg));
            }
        }

        // 3. Increment notification count.
        self.notification_count = self.notification_count.saturating_add(1);

        // 4. Mark views for this cluster as stale.
        let stale_msg = format!("Connection lost: {}", error_msg);
        let dashboard_key = format!("dashboard:{}", cluster_context);
        self.view_errors.insert(dashboard_key, stale_msg.clone());

        // Also mark any resource views for this cluster as stale.
        let resource_keys: Vec<String> = self
            .resource_list_data
            .keys()
            .filter(|k| k.cluster_context == cluster_context)
            .map(|k| format!("resources:{}:{}", k.cluster_context, k.kind,))
            .collect();
        for key in resource_keys {
            self.view_errors.insert(key, stale_msg.clone());
        }

        // Also insert a generic connection-level view error for the cluster.
        let conn_view_key = format!("connection:{}", cluster_context);
        self.view_errors.insert(conn_view_key, stale_msg);

        // 5. Trigger re-render.
        cx.notify();
    }

    /// T363: Handle successful reconnection of a cluster.
    ///
    /// Called when a retry or fresh API call succeeds after a prior connection
    /// loss. Clears all error state for the cluster.
    pub fn on_cluster_reconnected(&mut self, cluster_context: &str, cx: &mut Context<Self>) {
        tracing::info!("T363: Cluster reconnected: {}", cluster_context,);

        // Restore sidebar status to Connected.
        Self::set_sidebar_cluster_status(
            &mut self.sidebar,
            cluster_context,
            ClusterStatus::Connected,
        );

        // Clear the connection error.
        self.connection_errors.remove(cluster_context);

        // Clear core cluster manager error state.
        if let Some(id) = Self::find_connection_id(&self.cluster_manager, cluster_context) {
            if let Some(conn_mut) = self.cluster_manager.get_connection_mut(&id) {
                conn_mut.set_connected();
                conn_mut.reset_reconnect_attempts();
            }
        }

        // Clear view errors related to this cluster.
        let dashboard_key = format!("dashboard:{}", cluster_context);
        self.view_errors.remove(&dashboard_key);
        let conn_view_key = format!("connection:{}", cluster_context);
        self.view_errors.remove(&conn_view_key);

        // Remove all resource-scoped view errors for this cluster.
        let resource_prefix = format!("resources:{}:", cluster_context);
        self.view_errors.retain(|k, _| !k.starts_with(&resource_prefix));

        // Trigger re-render.
        cx.notify();
    }

    /// T363: Read-only accessor for the connection errors map.
    pub fn connection_errors(&self) -> &HashMap<String, String> {
        &self.connection_errors
    }

    /// T363: Check whether a specific cluster currently has a connection error.
    pub fn has_connection_error(&self, cluster_context: &str) -> bool {
        self.connection_errors.contains_key(cluster_context)
    }

    /// T363: Get the connection error message for a specific cluster, if any.
    pub fn connection_error_message(&self, cluster_context: &str) -> Option<&str> {
        self.connection_errors.get(cluster_context).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// T310: Contextual tracking — sync Navigator tree to active tab (FR-070)
// ---------------------------------------------------------------------------

impl AppShell {
    /// Synchronise the Navigator tree highlight to match the currently active tab.
    ///
    /// When the active tab changes:
    /// 1. Read the active tab's NavigationTarget.
    /// 2. If it carries a `cluster_context`, find the matching cluster and ensure it is
    ///    expanded (never collapses an already-open cluster).
    /// 3. If the target is a `ResourceList`, ensure the matching category is expanded
    ///    and set the sidebar's `active_kind` so the item row is highlighted.
    /// 4. For Dashboard / non-resource-list targets, clear `active_kind` so no item row
    ///    is highlighted.
    fn sync_navigator_to_active_tab(&mut self) {
        let Some(active_tab) = self.workspace.active_tab() else {
            self.sidebar.clear_active_kind();
            return;
        };

        let target = active_tab.target.clone();

        // Find the cluster matching this target.
        if let Some(ctx) = target.cluster_context() {
            if let Some(cluster_id) = self.sidebar.find_cluster_id_by_context(ctx) {
                // Ensure the cluster node is expanded in the tree.
                self.sidebar.ensure_cluster_expanded(cluster_id);

                // If the target is a ResourceList, expand the matching category and
                // set the active kind so the item row is highlighted.
                if let NavigationTarget::ResourceList { category, kind, .. } = &target {
                    self.sidebar.ensure_category_expanded(cluster_id, *category);
                    self.sidebar.set_active_kind(kind);
                } else {
                    // Dashboard, Events, etc. — no specific kind highlighted.
                    self.sidebar.clear_active_kind();
                }
            }
        } else {
            // ClusterList or similar — no cluster context, clear highlight.
            self.sidebar.clear_active_kind();
        }
    }

    /// T332: Trigger data loading when a tab becomes active and data isn't loaded yet.
    pub(crate) fn trigger_data_loading_for_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(active_tab) = self.workspace.active_tab() else {
            return;
        };
        let target = active_tab.target.clone();

        match &target {
            NavigationTarget::Dashboard { cluster_context } => {
                // Load dashboard if not already loaded for this cluster
                if self.active_dashboard_cluster.as_deref() != Some(cluster_context) {
                    self.start_dashboard_loading(cluster_context, cx);
                }
            }
            NavigationTarget::ResourceList { cluster_context, kind, .. } => {
                let key = ResourceListKey {
                    cluster_context: cluster_context.clone(),
                    kind: kind.clone(),
                    namespace: None,
                };
                if !self.resource_list_data.contains_key(&key) {
                    self.start_resource_loading(cluster_context, kind, None, cx);
                }
            }
            NavigationTarget::ResourceDetail { cluster_context, kind, name, namespace } => {
                let key = ResourceDetailKey {
                    cluster_context: cluster_context.clone(),
                    kind: kind.clone(),
                    name: name.clone(),
                    namespace: namespace.clone(),
                };
                if !self.resource_detail_data.contains_key(&key) {
                    self.start_resource_detail_loading(
                        cluster_context,
                        kind,
                        name,
                        namespace.as_deref(),
                        cx,
                    );
                }
            }
            NavigationTarget::ClusterTopology { cluster_context } => {
                let should_load = match self.cluster_topology_states.get(cluster_context) {
                    None => true,
                    Some(state) => state.error.is_some(), // Retry on previous error
                };
                if should_load {
                    self.start_cluster_topology_loading(cluster_context, cx);
                }
            }
            _ => {
                // Other view types don't need data loading yet
            }
        }

        // Auto-spawn per-cluster terminal when a cluster-scoped tab is activated.
        if let Some(ctx) = target.cluster_context() {
            let ctx = ctx.to_string();
            self.spawn_terminal_for_cluster(&ctx, cx);
            // Select this cluster's terminal as the active dock tab.
            if let Some(&dock_id) = self.cluster_terminals.get(&ctx) {
                self.dock.select_tab(dock_id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// T086: Keyboard navigation methods
// ---------------------------------------------------------------------------

/// Standalone state for tracking keyboard navigation without full AppShell.
/// Useful for unit tests and reusable outside of GPUI entity context.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppShellState {
    pub focus_mode: FocusMode,
}

impl AppShellState {
    /// Dispatch a key action to the appropriate handler.
    pub fn handle_key_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::ToggleCommandPalette => {
                if self.focus_mode == FocusMode::CommandPalette {
                    self.exit_focus_mode();
                } else {
                    self.focus_mode = FocusMode::CommandPalette;
                }
            }
            KeyAction::FocusSearch => {
                if self.focus_mode == FocusMode::Search {
                    self.exit_focus_mode();
                } else {
                    self.focus_mode = FocusMode::Search;
                }
            }
            KeyAction::ToggleSidebar
            | KeyAction::NavigateToDashboard
            | KeyAction::NavigateToClusterList
            | KeyAction::NavigateToPods
            | KeyAction::NavigateToDeployments
            | KeyAction::NavigateToServices
            | KeyAction::NavigateToEvents
            | KeyAction::NavigateToHelmReleases
            | KeyAction::NextTab
            | KeyAction::PrevTab
            | KeyAction::CloseTab
            | KeyAction::Refresh
            | KeyAction::OpenTerminal
            | KeyAction::OpenPreferences
            | KeyAction::SearchClusters
            | KeyAction::NavigateBack
            | KeyAction::NavigateForward
            | KeyAction::NextTabAlt
            | KeyAction::PrevTabAlt => {
                // These actions are handled at the AppShell/GPUI level;
                // they do not change focus mode.
            }
        }
    }

    /// Enter table navigation mode at (0, 0).
    pub fn enter_table_navigation(&mut self) {
        self.focus_mode = FocusMode::TableNavigation { row: 0, col: 0 };
    }

    /// Exit any focus mode and return to Normal.
    pub fn exit_focus_mode(&mut self) {
        self.focus_mode = FocusMode::Normal;
    }

    /// Move the table selection in the given direction.
    /// `max_rows` and `max_cols` define the upper bounds (exclusive).
    /// Movement is clamped to valid indices (saturating at 0 and max-1).
    pub fn move_table_selection(&mut self, direction: Direction, max_rows: usize, max_cols: usize) {
        if let FocusMode::TableNavigation { ref mut row, ref mut col } = self.focus_mode {
            match direction {
                Direction::Up => {
                    *row = row.saturating_sub(1);
                }
                Direction::Down => {
                    if max_rows > 0 {
                        *row = (*row + 1).min(max_rows - 1);
                    }
                }
                Direction::Left => {
                    *col = col.saturating_sub(1);
                }
                Direction::Right => {
                    if max_cols > 0 {
                        *col = (*col + 1).min(max_cols - 1);
                    }
                }
            }
        }
    }

    /// Check whether the current focus mode is a modal (Modal or CommandPalette).
    pub fn is_modal_open(&self) -> bool {
        matches!(self.focus_mode, FocusMode::Modal | FocusMode::CommandPalette)
    }
}

// ---------------------------------------------------------------------------
// T366: Keyboard shortcut handling on AppShell
// ---------------------------------------------------------------------------

impl AppShell {
    /// Convert a GPUI `KeyDownEvent` into a `KeyAction` and dispatch it.
    pub fn handle_keyboard_shortcut(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If a namespace dropdown is open, only handle Escape to close it.
        // Character input is handled by the gpui_component::Input widget.
        let any_ns_dropdown_open = self.namespace_selectors.values().any(|s| s.is_dropdown_open);
        if any_ns_dropdown_open {
            let key = &event.keystroke.key;
            if key == "escape" {
                if let Some(sel) =
                    self.namespace_selectors.values_mut().find(|s| s.is_dropdown_open)
                {
                    sel.is_dropdown_open = false;
                    sel.search_query.clear();
                }
                self.ns_search_input = None;
                self._ns_search_subscription = None;
            }
            cx.notify();
            return;
        }

        let key = &event.keystroke.key;
        let mods = &event.keystroke.modifiers;

        let our_mods = KeyModifiers {
            cmd: mods.platform,
            ctrl: mods.control,
            alt: mods.alt,
            shift: mods.shift,
        };

        let kbd_nav = KeyboardNavigationState::new();
        let Some(action) = kbd_nav.config.find_action(key, &our_mods) else {
            return;
        };

        match action {
            KeyAction::CloseTab => {
                if let Some(tab) = self.workspace.active_tab() {
                    let id = tab.id;
                    let ctx = tab.target.cluster_context().map(|s| s.to_string());
                    self.workspace.close_tab(id);
                    if let Some(ctx) = ctx {
                        self.cleanup_cluster_terminal_if_last(&ctx);
                    }
                }
            }
            KeyAction::NextTab | KeyAction::NextTabAlt => {
                self.switch_to_next_tab();
            }
            KeyAction::PrevTab | KeyAction::PrevTabAlt => {
                self.switch_to_prev_tab();
            }
            KeyAction::ToggleSidebar => {
                self.layout.sidebar_collapsed = !self.layout.sidebar_collapsed;
            }
            KeyAction::ToggleCommandPalette => {
                if self.focus_mode == FocusMode::CommandPalette {
                    self.focus_mode = FocusMode::Normal;
                } else {
                    self.focus_mode = FocusMode::CommandPalette;
                }
            }
            KeyAction::FocusSearch | KeyAction::SearchClusters => {
                if self.focus_mode == FocusMode::Search {
                    self.focus_mode = FocusMode::Normal;
                } else {
                    self.focus_mode = FocusMode::Search;
                }
            }
            KeyAction::Refresh => {
                self.trigger_data_loading_for_active_tab(cx);
            }
            KeyAction::OpenTerminal => {
                if let Some(tab) = self.workspace.active_tab() {
                    if let Some(ctx) = tab.target.cluster_context() {
                        let ctx = ctx.to_string();
                        if let Some(&dock_id) = self.cluster_terminals.get(&ctx) {
                            // Focus existing terminal
                            self.dock.select_tab(dock_id);
                            if self.dock.collapsed {
                                self.dock.toggle_collapsed();
                            }
                        } else {
                            self.spawn_terminal_for_cluster(&ctx, cx);
                        }
                    }
                }
            }
            KeyAction::OpenPreferences => {
                self.workspace.open_tab(NavigationTarget::Preferences);
            }
            KeyAction::NavigateBack => {
                self.navigate_back();
            }
            KeyAction::NavigateForward => {
                self.navigate_forward();
            }
            KeyAction::NavigateToDashboard
            | KeyAction::NavigateToClusterList
            | KeyAction::NavigateToPods
            | KeyAction::NavigateToDeployments
            | KeyAction::NavigateToServices
            | KeyAction::NavigateToEvents
            | KeyAction::NavigateToHelmReleases => {
                if let Some(target) = self.navigation_target_for_action(action) {
                    self.workspace.open_tab(target);
                    self.trigger_data_loading_for_active_tab(cx);
                }
            }
        }
        cx.notify();
    }

    fn switch_to_next_tab(&mut self) {
        let tabs = &self.workspace.tabs;
        if tabs.is_empty() {
            return;
        }
        if let Some(active_id) = self.workspace.active_tab_id {
            if let Some(idx) = tabs.iter().position(|t| t.id == active_id) {
                let next = (idx + 1) % tabs.len();
                self.workspace.active_tab_id = Some(tabs[next].id);
            }
        }
    }

    fn switch_to_prev_tab(&mut self) {
        let tabs = &self.workspace.tabs;
        if tabs.is_empty() {
            return;
        }
        if let Some(active_id) = self.workspace.active_tab_id {
            if let Some(idx) = tabs.iter().position(|t| t.id == active_id) {
                let prev = if idx == 0 { tabs.len() - 1 } else { idx - 1 };
                self.workspace.active_tab_id = Some(tabs[prev].id);
            }
        }
    }

    fn navigation_target_for_action(&self, action: KeyAction) -> Option<NavigationTarget> {
        let cluster = self
            .active_dashboard_cluster
            .clone()
            .or_else(|| self.sidebar.clusters.first().map(|c| c.context_name.clone()))?;

        Some(match action {
            KeyAction::NavigateToDashboard => {
                NavigationTarget::Dashboard { cluster_context: cluster }
            }
            KeyAction::NavigateToClusterList => NavigationTarget::ClusterList,
            KeyAction::NavigateToPods => NavigationTarget::ResourceList {
                cluster_context: cluster,
                category: crate::icons::ResourceCategory::Workloads,
                kind: "Pod".to_string(),
            },
            KeyAction::NavigateToDeployments => NavigationTarget::ResourceList {
                cluster_context: cluster,
                category: crate::icons::ResourceCategory::Workloads,
                kind: "Deployment".to_string(),
            },
            KeyAction::NavigateToServices => NavigationTarget::ResourceList {
                cluster_context: cluster,
                category: crate::icons::ResourceCategory::Network,
                kind: "Service".to_string(),
            },
            KeyAction::NavigateToEvents => NavigationTarget::ResourceList {
                cluster_context: cluster,
                category: crate::icons::ResourceCategory::Monitoring,
                kind: "Event".to_string(),
            },
            KeyAction::NavigateToHelmReleases => {
                NavigationTarget::HelmReleases { cluster_context: cluster }
            }
            _ => return None,
        })
    }
}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Clear frame-level guard from previous event cycle.
        self.context_menu_dismissed_this_frame = false;

        let bg = self.theme.colors.background.to_gpui();
        let text = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let sidebar_bg = self.theme.colors.sidebar_bg.to_gpui();

        div()
            .id(ElementId::Name(SharedString::from("app-shell-root")))
            .flex()
            .flex_col()
            .size_full()
            .bg(bg)
            .text_color(text)
            // Handle OpenPreferences menu action
            .on_action(_cx.listener(|this, _: &OpenPreferencesAction, _window, cx| {
                this.workspace.open_tab(NavigationTarget::Preferences);
                cx.notify();
            }))
            // T366: Wire keyboard shortcuts via on_key_down
            .on_key_down(_cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_keyboard_shortcut(event, window, cx);
            }))
            // Global mouse_move for dock drag resize, column drag resize, and sidebar drag resize
            .on_mouse_move(_cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_dragging_dock {
                    let y: f32 = event.position.y.into();
                    let delta = this.dock_drag_start_y - y;
                    let new_height = (this.dock_drag_start_height + delta).clamp(100.0, 1200.0);
                    this.dock.height = new_height;
                    cx.notify();
                }
                if this.is_dragging_column {
                    let x: f32 = event.position.x.into();
                    let delta = x - this.column_drag_start_x;
                    let new_width = (this.column_drag_start_width + delta).max(40.0);
                    if let (Some(key), Some(idx)) =
                        (&this.column_drag_table_key, this.column_drag_index)
                    {
                        if let Some(ts) = this.resource_table_states.get_mut(key) {
                            ts.set_column_width(idx, new_width);
                        }
                    }
                    cx.notify();
                }
                if this.is_dragging_sidebar {
                    let x: f32 = event.position.x.into();
                    let delta = x - this.sidebar_drag_start_x;
                    let new_width = (this.sidebar_drag_start_width + delta).clamp(200.0, 600.0);
                    this.sidebar.set_width(new_width);
                    cx.notify();
                }
                if this.is_dragging_topology {
                    let x: f32 = event.position.x.into();
                    let y: f32 = event.position.y.into();
                    let dx = x - this.topology_drag_last.0;
                    let dy = y - this.topology_drag_last.1;
                    this.topology_drag_last = (x, y);
                    if let Some(ref key) = this.topology_drag_key {
                        if let Some(state) = this.topology_data.get_mut(key) {
                            state.pan_offset.0 += dx as f64;
                            state.pan_offset.1 += dy as f64;
                        }
                    }
                    cx.notify();
                }
                if this.is_dragging_cluster_topology {
                    let x: f32 = event.position.x.into();
                    let y: f32 = event.position.y.into();
                    let dx = x - this.cluster_topology_drag_last.0;
                    let dy = y - this.cluster_topology_drag_last.1;
                    this.cluster_topology_drag_last = (x, y);
                    if let Some(ref ctx) = this.cluster_topology_drag_context {
                        if let Some(state) = this.cluster_topology_states.get_mut(ctx) {
                            state.pan_offset.0 += dx as f64;
                            state.pan_offset.1 += dy as f64;
                        }
                    }
                    cx.notify();
                }
                if this.is_dragging_cluster_topo_resize {
                    let y: f32 = event.position.y.into();
                    let delta = y - this.cluster_topo_resize_start_y;
                    let new_height =
                        (this.cluster_topo_resize_start_height + delta).clamp(150.0, 800.0);
                    if let Some(ref ctx) = this.cluster_topo_resize_context {
                        if let Some(state) = this.cluster_topology_states.get_mut(ctx) {
                            state.graph_height = new_height;
                        }
                    }
                    cx.notify();
                }
            }))
            // Global mouse_up to end dock drag resize, column drag resize, and sidebar drag resize
            .on_mouse_up(
                MouseButton::Left,
                _cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                    this.is_dragging_dock = false;
                    this.is_dragging_column = false;
                    this.is_dragging_sidebar = false;
                    this.is_dragging_topology = false;
                    this.topology_drag_key = None;
                    this.is_dragging_cluster_topology = false;
                    this.cluster_topology_drag_context = None;
                    this.is_dragging_cluster_topo_resize = false;
                    this.cluster_topo_resize_context = None;
                    this.column_drag_index = None;
                    this.column_drag_table_key = None;
                }),
            )
            // Title bar row (transparent macOS title bar with gear button)
            .child(self.render_titlebar_row(_cx, text_secondary, border))
            // Main content area: sidebar + right column
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar: Navigator tree (always show grouped view)
                    .when(!self.layout.sidebar_collapsed, |el| {
                        el.child(self.render_navigator(
                            _cx,
                            text,
                            text_secondary,
                            border,
                            accent,
                            sidebar_bg,
                        ))
                    })
                    // Right side: tab bar + content + dock + status bar
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            // Tab bar
                            .child(self.render_tab_bar(_cx, text, text_secondary, border, accent))
                            // AWS SSO login banner (shown when token expires)
                            .children(self.render_sso_login_banner(_cx))
                            // T330: Content area (min_h_0 + flex container allows child scroll)
                            .child(
                                div().min_h_0().flex_1().flex().flex_col().overflow_hidden().child(
                                    self.render_content_area(
                                        _window,
                                        _cx,
                                        text,
                                        text_secondary,
                                        border,
                                        accent,
                                    ),
                                ),
                            )
                            // T317/T318: Dock panel at bottom
                            .child(self.render_dock_panel(_cx))
                            // T356: Status bar at very bottom
                            .child(self.render_status_bar(text, text_secondary, border)),
                    ),
            )
            // Namespace dropdown overlay
            .children(self.render_namespace_dropdown_overlay(_cx))
            // Cluster context menu overlay (rendered at root to avoid scroll clipping)
            .when(self.context_menu_cluster.is_some(), |el| {
                el.child(self.render_cluster_context_menu_overlay(_cx))
            })
            // Confirm dialog overlay
            .when(self.confirm_dialog.is_some(), |el| {
                el.child(self.render_confirm_dialog_overlay(_cx))
            })
            // EKS wizard overlay — ensure inputs exist before rendering
            .map(|el| {
                self.ensure_eks_inputs(_window, _cx);
                el.children(self.render_eks_wizard(_cx))
            })
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers (extracted for readability)
// ---------------------------------------------------------------------------

impl AppShell {
    /// Render the macOS title bar row. With `appears_transparent: true`, our
    /// content extends into the title bar area. This renders a thin strip with
    /// a settings gear button on the right, leaving space on the left for the
    /// macOS traffic light buttons.
    fn render_titlebar_row(
        &self,
        cx: &mut Context<Self>,
        text_secondary: Rgba,
        border: Rgba,
    ) -> Div {
        let settings_id = ElementId::Name(SharedString::from("titlebar-settings"));
        let settings_btn = div()
            .id(settings_id)
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_color(text_secondary)
            .hover(|el| el.bg(gpui::rgb(0x374151)))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.workspace.open_tab(NavigationTarget::Preferences);
                cx.notify();
            }))
            .child(Icon::new(IconName::Settings).small());

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .h(px(38.0))
            .px_3()
            .border_b_1()
            .border_color(border)
            .flex_shrink_0()
            .child(settings_btn)
    }

    /// Render the cluster-first sidebar: icon strip (left) + resource tree (right).
    /// NOTE: Superseded by render_navigator (T307). Retained for reference.
    #[allow(dead_code)]
    fn render_cluster_sidebar(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
        _sidebar_bg: Rgba,
    ) -> Div {
        let icon_strip_bg = gpui::rgb(0x0F172A);

        div()
            .flex()
            .flex_row()
            .h_full()
            .flex_shrink_0()
            // Left icon strip (~48px)
            .child(self.render_icon_strip(cx, icon_strip_bg, accent, border))
            // Main sidebar tree (~224px)
            .child(self.render_sidebar_tree(cx, text, text_secondary, border, accent))
    }

    /// Render the narrow icon strip with one colored square per cluster.
    /// NOTE: Superseded by render_navigator (T307). Retained for reference.
    #[allow(dead_code)]
    fn render_icon_strip(
        &self,
        cx: &mut Context<Self>,
        bg: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> Div {
        let clusters = self.sidebar.clusters.clone();
        let selected_id = self.sidebar.selected_cluster_id;

        let mut strip = div()
            .flex()
            .flex_col()
            .items_center()
            .w(px(48.0))
            .h_full()
            .bg(bg)
            .border_r_1()
            .border_color(border)
            .pt_2()
            .gap_2()
            .overflow_hidden();

        for cluster in &clusters {
            let is_selected = selected_id == Some(cluster.id);
            let initials = SharedString::from(cluster.initials.clone());
            let color = cluster.color;
            let status = cluster.status.clone();
            let context_name = cluster.context_name.clone();

            let icon_id = ElementId::Name(SharedString::from(format!(
                "cluster-icon-{}",
                cluster.context_name
            )));

            // Status dot color
            let status_color = match status {
                ClusterStatus::Connected => gpui::rgb(0x22C55E), // green
                ClusterStatus::Connecting => gpui::rgb(0xFBBF24), // yellow
                ClusterStatus::Disconnected => gpui::rgb(0x6B7280), // gray
                ClusterStatus::Error => gpui::rgb(0xEF4444),     // red
            };

            let mut icon = div()
                .id(icon_id)
                .relative()
                .w(px(36.0))
                .h(px(36.0))
                .rounded_md()
                .bg(gpui::rgb(color))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(gpui::rgb(0xFFFFFF))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    // T029: Switch to this cluster when its icon is clicked.
                    this.handle_switch_cluster(&context_name, cx);
                }))
                .child(initials);

            // Selected accent border
            if is_selected {
                icon = icon.border_2().border_color(accent);
            }

            // Status dot overlay (bottom-right)
            let status_dot = div()
                .absolute()
                .bottom(px(-1.0))
                .right(px(-1.0))
                .w(px(10.0))
                .h(px(10.0))
                .rounded_full()
                .bg(status_color)
                .border_2()
                .border_color(gpui::rgb(0x0F172A));

            icon = icon.child(status_dot);

            strip = strip.child(icon);
        }

        strip
    }

    /// Render the main sidebar tree for the selected cluster.
    /// NOTE: Superseded by render_navigator (T307). Retained for reference.
    #[allow(dead_code)]
    fn render_sidebar_tree(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let sidebar_bg = gpui::rgb(0x111827);

        let mut tree = div()
            .flex()
            .flex_col()
            .w(px(224.0))
            .h_full()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border)
            .overflow_hidden()
            .py_2();

        // Selected cluster name at top
        if let Some(cluster) = self.sidebar.selected_cluster() {
            let display_name = cluster.display_name.clone();
            let context_name = cluster.context_name.clone();
            tree = tree.child(
                div()
                    .px_3()
                    .py_2()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text)
                    .text_sm()
                    .child(display_name),
            );

            // Render that cluster's sections
            let sections = cluster.sections.clone();
            tree = self.render_sections(tree, &sections, &context_name, cx, text_secondary, accent);
        }

        tree
    }

    // -----------------------------------------------------------------------
    // T307 / T308 / T309: Navigator tree
    // -----------------------------------------------------------------------

    /// Render the Navigator sidebar — a single scrollable tree with all clusters.
    /// Replaces the old icon-strip + single-cluster-tree layout (T307).
    ///
    /// T311: In drill-into mode, only the drilled-into cluster is shown with a
    ///       breadcrumb bar at the top.
    /// T312: A 4px drag handle is rendered on the right edge for future resize support.
    fn render_navigator(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
        sidebar_bg: Rgba,
    ) -> Div {
        let width = self.sidebar.sidebar_width;

        // Fixed header section (doesn't scroll)
        let mut header = div().flex().flex_col().flex_shrink_0().bg(sidebar_bg);

        // Navigator header bar with filter input
        let filter_input = self.cluster_filter_input.clone();
        let case_active = self.cluster_filter_case_sensitive;
        let regex_active = self.cluster_filter_regex;
        let filter_border = if filter_input.is_some() {
            gpui::rgba(0x3B82F680) // accent ~50% opacity when active
        } else {
            gpui::rgba(0x3B82F640) // accent ~25% opacity when inactive
        };
        let toggle_bg_on = gpui::rgba(0x3B82F650);
        let toggle_bg_off = gpui::rgba(0x00000000);

        if let Some(input_entity) = filter_input {
            // Active filter bar with input + toggle buttons
            let case_btn = div()
                .id("filter-case-toggle")
                .flex()
                .items_center()
                .justify_center()
                .w(px(24.0))
                .h(px(20.0))
                .rounded(px(3.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .cursor_pointer()
                .bg(if case_active { toggle_bg_on } else { toggle_bg_off })
                .text_color(if case_active { accent } else { text_secondary })
                .hover(|s| s.bg(gpui::rgba(0x3B82F630)))
                .on_click(cx.listener(|this, _evt, _window, cx| {
                    this.cluster_filter_case_sensitive = !this.cluster_filter_case_sensitive;
                    cx.notify();
                }))
                .child("Aa");
            let regex_btn = div()
                .id("filter-regex-toggle")
                .flex()
                .items_center()
                .justify_center()
                .w(px(24.0))
                .h(px(20.0))
                .rounded(px(3.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .cursor_pointer()
                .bg(if regex_active { toggle_bg_on } else { toggle_bg_off })
                .text_color(if regex_active { accent } else { text_secondary })
                .hover(|s| s.bg(gpui::rgba(0x3B82F630)))
                .on_click(cx.listener(|this, _evt, _window, cx| {
                    this.cluster_filter_regex = !this.cluster_filter_regex;
                    cx.notify();
                }))
                .child(".*");

            header = header.child(
                div()
                    .mx_2()
                    .my_1p5()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .py(px(2.0))
                    .rounded_md()
                    .border_1()
                    .border_color(filter_border)
                    .bg(Rgba { r: sidebar_bg.r, g: sidebar_bg.g, b: sidebar_bg.b, a: 0.5 })
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .pl_1()
                            .child(Icon::new(IconName::Search).small()),
                    )
                    .child(
                        div().flex_1().child(
                            Input::new(&input_entity)
                                .appearance(false)
                                .cleanable(true)
                                .text_sm()
                                .small(),
                        ),
                    )
                    .child(case_btn)
                    .child(regex_btn),
            );
        } else {
            // Placeholder bar — click to activate
            header = header.child(
                div()
                    .id("cluster-filter-placeholder")
                    .mx_2()
                    .my_1p5()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .py(px(2.0))
                    .rounded_md()
                    .border_1()
                    .border_color(filter_border)
                    .bg(Rgba { r: sidebar_bg.r, g: sidebar_bg.g, b: sidebar_bg.b, a: 0.5 })
                    .cursor_pointer()
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .pl_1()
                            .child(Icon::new(IconName::Search).small()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_1()
                            .text_sm()
                            .text_color(text_secondary)
                            .child("Filter clusters..."),
                    )
                    .on_click(cx.listener(move |this, _evt, window, cx| {
                        let input = cx.new(|cx| {
                            InputState::new(window, cx).placeholder("Filter clusters...")
                        });
                        let sub = cx.subscribe(
                            &input,
                            |this: &mut AppShell, entity, event: &InputEvent, cx| {
                                if matches!(event, InputEvent::Change) {
                                    this.cluster_filter_text = entity.read(cx).value().to_string();
                                    cx.notify();
                                }
                            },
                        );
                        let fh = input.read(cx).focus_handle(cx);
                        fh.focus(window);
                        this.cluster_filter_input = Some(input);
                        this._cluster_filter_subscription = Some(sub);
                    })),
            );
        }

        // (No standalone "Add EKS" button — integrated into section headers below)

        // T311: Drill-into breadcrumb bar
        if self.sidebar.is_drill_into() {
            header = header.child(self.render_drill_into_breadcrumb(cx, text_secondary));
        }

        // Scrollable cluster list
        let mut cluster_list =
            div().id("nav-cluster-list").relative().flex().flex_col().flex_1().overflow_y_scroll();

        // T311: Determine which clusters to render (drill-into + text filter)
        let clusters = self.sidebar.clusters.clone();
        let filter_text = &self.cluster_filter_text;
        let visible_clusters: Vec<_> = if let Some(drill_id) = self.sidebar.drill_into_cluster {
            clusters.into_iter().filter(|c| c.id == drill_id).collect()
        } else if !filter_text.is_empty() {
            if self.cluster_filter_regex {
                // Regex mode: compile pattern (case-insensitive unless case_sensitive is on)
                // Use RegexBuilder with size_limit to prevent ReDoS via NFA explosion.
                let pattern = if self.cluster_filter_case_sensitive {
                    filter_text.clone()
                } else {
                    format!("(?i){}", filter_text)
                };
                match regex::RegexBuilder::new(&pattern)
                    .size_limit(1 << 16) // 64KB compiled size limit
                    .build()
                {
                    Ok(re) => clusters
                        .into_iter()
                        .filter(|c| re.is_match(&c.display_name) || re.is_match(&c.context_name))
                        .collect(),
                    Err(_) => clusters, // Invalid regex — show all
                }
            } else if self.cluster_filter_case_sensitive {
                // Case-sensitive substring match
                clusters
                    .into_iter()
                    .filter(|c| {
                        c.display_name.contains(filter_text.as_str())
                            || c.context_name.contains(filter_text.as_str())
                    })
                    .collect()
            } else {
                // Default: case-insensitive substring match
                let filter = filter_text.to_lowercase();
                clusters
                    .into_iter()
                    .filter(|c| {
                        c.display_name.to_lowercase().contains(&filter)
                            || c.context_name.to_lowercase().contains(&filter)
                    })
                    .collect()
            }
        } else {
            clusters
        };

        // Group clusters by source type
        let eks_clusters: Vec<_> = visible_clusters
            .iter()
            .filter(|c| matches!(c.source, crate::layout::sidebar::ClusterSource::AwsEks { .. }))
            .collect();
        let local_clusters: Vec<_> = visible_clusters
            .iter()
            .filter(|c| matches!(c.source, crate::layout::sidebar::ClusterSource::Kubeconfig))
            .collect();

        // Top-level "KUBERNETES CLUSTERS" header
        cluster_list = cluster_list.child(
            div().flex().flex_row().items_center().px_3().py(px(6.0)).mt_1().child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("KUBERNETES CLUSTERS"),
            ),
        );

        // --- AWS EKS section ---
        cluster_list = cluster_list.child(
            div()
                .id("eks-section-header")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.04 }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_color(gpui::rgb(0xFF9900))
                                .child(Icon::new(IconName::Globe).small()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text)
                                .child("AWS EKS"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Rgba {
                                    r: text_secondary.r,
                                    g: text_secondary.g,
                                    b: text_secondary.b,
                                    a: 0.5,
                                })
                                .child(SharedString::from(format!("({})", eks_clusters.len()))),
                        ),
                )
                .child(
                    div()
                        .id("eks-section-add")
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Rgba {
                            r: text_secondary.r,
                            g: text_secondary.g,
                            b: text_secondary.b,
                            a: 0.3,
                        })
                        .hover(|s| s.text_color(gpui::rgb(0xFF9900)).bg(gpui::rgba(0xFF990020)))
                        .child("+")
                        .on_click(cx.listener(|this, _evt, window, cx| {
                            this.open_eks_wizard(window, cx);
                        })),
                ),
        );
        for cluster in &eks_clusters {
            cluster_list = self.render_cluster_with_tree(
                cluster_list,
                cx,
                cluster,
                text,
                text_secondary,
                accent,
            );
        }

        // --- Local Kubeconfigs section ---
        cluster_list = cluster_list.child(
            div()
                .id("local-section-header")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.04 }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_color(gpui::rgb(0x326CE5))
                                .child(Icon::new(IconName::Folder).small()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text)
                                .child("Local Kubeconfigs"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Rgba {
                                    r: text_secondary.r,
                                    g: text_secondary.g,
                                    b: text_secondary.b,
                                    a: 0.5,
                                })
                                .child(SharedString::from(format!("({})", local_clusters.len()))),
                        ),
                )
                .child(
                    div()
                        .id("local-section-add")
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Rgba {
                            r: text_secondary.r,
                            g: text_secondary.g,
                            b: text_secondary.b,
                            a: 0.3,
                        })
                        .hover(|s| s.text_color(gpui::rgb(0x326CE5)).bg(gpui::rgba(0x326CE520)))
                        .child("+")
                        .on_click(cx.listener(|this, _evt, _window, cx| {
                            this.workspace.open_tab(crate::layout::NavigationTarget::Preferences);
                            this.active_prefs_section =
                                crate::layout::app_shell::PreferencesSection::Kubernetes;
                            cx.notify();
                        })),
                ),
        );
        for cluster in &local_clusters {
            cluster_list = self.render_cluster_with_tree(
                cluster_list,
                cx,
                cluster,
                text,
                text_secondary,
                accent,
            );
        }

        // Ungrouped fallback
        if eks_clusters.is_empty() && local_clusters.is_empty() && !visible_clusters.is_empty() {
            for cluster in &visible_clusters {
                cluster_list = self.render_cluster_with_tree(
                    cluster_list,
                    cx,
                    cluster,
                    text,
                    text_secondary,
                    accent,
                );
            }
        }

        // Combine: fixed header + scrollable cluster list
        let tree = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(sidebar_bg)
            .child(header)
            .child(cluster_list);

        // T312: Wrap tree + drag handle in a row container
        let drag_handle = div()
            .id("sidebar-drag-handle")
            .w(px(4.0))
            .h_full()
            .flex_shrink_0()
            .bg(gpui::rgba(0x00000000))
            .hover(|s| s.bg(gpui::rgb(0x3B82F6)))
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                    this.is_dragging_sidebar = true;
                    this.sidebar_drag_start_x = event.position.x.into();
                    this.sidebar_drag_start_width = this.sidebar.sidebar_width;
                }),
            );

        div()
            .flex()
            .flex_row()
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .border_r_1()
            .border_color(border)
            .child(tree)
            .child(drag_handle)
    }

    /// Render the breadcrumb bar for drill-into mode (T311).
    /// Shows a clickable "< All Clusters" link that exits drill-into.
    fn render_drill_into_breadcrumb(
        &self,
        cx: &mut Context<Self>,
        _text_secondary: Rgba,
    ) -> Stateful<Div> {
        let breadcrumb_id = ElementId::Name(SharedString::from("nav-drill-into-back"));
        div()
            .id(breadcrumb_id)
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(gpui::rgb(0x60A5FA))
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.sidebar.exit_drill_into();
            }))
            .child(Icon::new(IconName::ArrowLeft).xsmall())
            .child(SharedString::from("All Clusters"))
    }

    /// Render the cluster context menu as a full-window overlay (backdrop + menu).
    /// This is rendered at the root level to avoid clipping from parent overflow.
    fn render_cluster_context_menu_overlay(&self, cx: &mut Context<Self>) -> Div {
        let Some(menu_id) = self.context_menu_cluster else {
            return div();
        };
        let Some(cluster) = self.sidebar.clusters.iter().find(|c| c.id == menu_id).cloned() else {
            return div();
        };

        // Position: use the stored window Y coordinate from the right-click event.
        let menu_y = self.context_menu_position_y;

        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            // Invisible backdrop — clicking anywhere closes the menu.
            // Uses on_click (not on_mouse_down) so that hit-testing prevents
            // the backdrop from intercepting clicks intended for menu items.
            .child(
                div()
                    .id("cluster-context-menu-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h_full()
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.context_menu_cluster = None;
                        this.context_menu_dismissed_this_frame = true;
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                            this.context_menu_cluster = None;
                            this.context_menu_dismissed_this_frame = true;
                            cx.notify();
                        }),
                    ),
            )
            // The actual menu, positioned at the right-click location
            .child(self.render_cluster_context_menu(cx, &cluster, menu_y))
    }

    /// Render a right-click context menu for a cluster node (T313, FR-054).
    /// Order: Connect/Disconnect, separator, Cluster Settings, Open Dashboard,
    /// Copy Context Name, separator, Remove from List (red).
    fn render_cluster_context_menu(
        &self,
        cx: &mut Context<Self>,
        cluster: &crate::layout::sidebar::ClusterEntry,
        menu_top_y: f32,
    ) -> Div {
        let cluster_id = cluster.id;
        let context_name = cluster.context_name.clone();
        let is_connected = cluster.status == ClusterStatus::Connected;
        let is_connecting = cluster.status == ClusterStatus::Connecting;

        let bg = gpui::rgb(0x1F2937);
        let border = gpui::rgb(0x374151);
        let text_color = gpui::rgb(0xD1D5DB);
        let hover_bg = gpui::rgb(0x374151);
        let separator_color = gpui::rgb(0x374151);

        let mut menu = div()
            .absolute()
            .left(px(40.0))
            .top(px(menu_top_y))
            .w(px(200.0))
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_md()
            .py_1()
            .text_sm()
            .text_color(text_color);

        // 1. Connect or Disconnect
        if is_connected {
            let ctx = context_name.clone();
            let disconnect_id =
                ElementId::Name(SharedString::from(format!("ctx-menu-disconnect-{cluster_id}")));
            menu = menu.child(
                div()
                    .id(disconnect_id)
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.handle_disconnect_cluster(&ctx, cx);
                        this.context_menu_cluster = None;
                        this.context_menu_dismissed_this_frame = true;
                    }))
                    .child("Disconnect"),
            );
        } else if is_connecting {
            // Show a non-clickable "Connecting..." label while role assumption is in progress
            menu = menu.child(
                div().px_3().py_1().text_color(gpui::rgb(0x9CA3AF)).child("Connecting\u{2026}"),
            );
        } else {
            let ctx = context_name.clone();
            let connect_id =
                ElementId::Name(SharedString::from(format!("ctx-menu-connect-{cluster_id}")));
            menu = menu.child(
                div()
                    .id(connect_id)
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.handle_connect_cluster(&ctx, cx);
                        this.context_menu_cluster = None;
                        this.context_menu_dismissed_this_frame = true;
                    }))
                    .child("Connect"),
            );
        }

        // ── separator ──
        menu = menu.child(div().my_1().h(px(1.0)).bg(separator_color));

        // 2. Cluster Settings
        let ctx_settings = context_name.clone();
        let settings_id =
            ElementId::Name(SharedString::from(format!("ctx-menu-settings-{cluster_id}")));
        menu = menu.child(
            div()
                .id(settings_id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    let target =
                        NavigationTarget::ClusterSettings { cluster_context: ctx_settings.clone() };
                    this.workspace.open_tab(target);
                    this.context_menu_cluster = None;
                    this.context_menu_dismissed_this_frame = true;
                    cx.notify();
                }))
                .child("Cluster Settings"),
        );

        // 3. Open Dashboard
        let ctx_dash = context_name.clone();
        let dash_id =
            ElementId::Name(SharedString::from(format!("ctx-menu-dashboard-{cluster_id}")));
        menu = menu.child(
            div()
                .id(dash_id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    let target = NavigationTarget::Dashboard { cluster_context: ctx_dash.clone() };
                    this.workspace.open_tab(target);
                    this.context_menu_cluster = None;
                    this.context_menu_dismissed_this_frame = true;
                    cx.notify();
                }))
                .child("Open Dashboard"),
        );

        // 4. Copy Context Name
        let ctx_copy = context_name.clone();
        let copy_id = ElementId::Name(SharedString::from(format!("ctx-menu-copy-{cluster_id}")));
        menu = menu.child(
            div()
                .id(copy_id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(ctx_copy.clone()));
                    this.context_menu_cluster = None;
                    this.context_menu_dismissed_this_frame = true;
                }))
                .child("Copy Context Name"),
        );

        // ── separator ──
        menu = menu.child(div().my_1().h(px(1.0)).bg(separator_color));

        // 5. Remove from List (red)
        let remove_id =
            ElementId::Name(SharedString::from(format!("ctx-menu-remove-{cluster_id}")));
        let remove_ctx = context_name.clone();
        menu = menu.child(
            div()
                .id(remove_id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .text_color(gpui::rgb(0xEF4444))
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    // Clean up cached kubeconfig file
                    if let Some(path) = this.kubeconfig_paths.remove(&remove_ctx) {
                        if path.contains(".baeus/eks-kubeconfigs") {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                    // Remove from saved EKS connections in preferences
                    this.preferences.saved_eks_connections.retain(|c| {
                        baeus_core::aws_eks::eks_context_name_from_parts(&c.cluster_name, &c.region)
                            != remove_ctx
                    });
                    this.save_preferences();
                    // Remove EKS cluster data
                    this.eks_cluster_data.remove(&remove_ctx);
                    // Remove from sidebar
                    this.sidebar.remove_cluster(cluster_id);
                    this.context_menu_cluster = None;
                    this.context_menu_dismissed_this_frame = true;
                }))
                .child("Remove from List"),
        );

        menu
    }

    /// Persist the current `cluster_appearances` map to a JSON file on disk.
    fn persist_cluster_appearances(&self) {
        if let Some(config_dir) = dirs::config_dir() {
            let dir = config_dir.join("baeus");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("cluster_appearances.json");
            if let Ok(json) = serde_json::to_string_pretty(&self.cluster_appearances) {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!("Failed to save cluster appearances: {e}");
                }
            }
        }
    }

    /// Load cluster appearances from the persisted JSON file.
    fn load_cluster_appearances() -> HashMap<String, ClusterAppearance> {
        let Some(config_dir) = dirs::config_dir() else {
            return HashMap::new();
        };
        let path = config_dir.join("baeus").join("cluster_appearances.json");
        if !path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// The 12-color palette used for cluster icon colors, exported for the color picker.
    const CLUSTER_COLOR_PALETTE: [u32; 12] = [
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

    /// Render the Cluster Settings tab view.
    /// Shows cluster name, colored icon with appearance popup, and kubeconfig path.
    fn render_cluster_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let border = self.theme.colors.border.to_gpui();
        let card_bg = self.theme.colors.surface.to_gpui();

        // Look up the cluster entry for display data.
        let cluster = self.sidebar.clusters.iter().find(|c| c.context_name == cluster_context);

        let display_name =
            cluster.map(|c| c.display_name.clone()).unwrap_or_else(|| cluster_context.to_string());
        let initials = cluster.map(|c| c.initials.clone()).unwrap_or_else(|| "??".to_string());

        // Effective color: custom override or cluster default.
        let effective_color = self
            .cluster_appearances
            .get(cluster_context)
            .and_then(|a| a.custom_color)
            .or_else(|| cluster.map(|c| c.color))
            .unwrap_or(0x3B82F6);

        let kubeconfig_path = self
            .kubeconfig_paths
            .get(cluster_context)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        let ctx_name = cluster_context.to_string();
        let ctx_for_icon_menu = ctx_name.clone();
        let ctx_for_finder = kubeconfig_path.clone();

        // --- Section 1: Cluster Name ---
        let name_section = div()
            .p_4()
            .rounded_lg()
            .bg(card_bg)
            .border_1()
            .border_color(border)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_secondary)
                    .mb_2()
                    .child("CLUSTER"),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text)
                    .child(SharedString::from(display_name)),
            );

        // --- Section 2: Cluster Icon ---
        let icon_preview = div()
            .w(px(48.0))
            .h(px(48.0))
            .rounded_lg()
            .bg(gpui::rgb(effective_color))
            .flex()
            .items_center()
            .justify_center()
            .text_lg()
            .font_weight(FontWeight::BOLD)
            .text_color(gpui::rgb(0xFFFFFF))
            .child(SharedString::from(initials));

        let dots_id =
            ElementId::Name(SharedString::from(format!("cluster-settings-icon-dots-{}", ctx_name)));
        let dots_btn = div()
            .id(dots_id)
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_color(text_secondary)
            .hover(move |s| s.bg(border))
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                if this.cluster_settings_icon_menu.as_deref() == Some(&ctx_for_icon_menu) {
                    this.cluster_settings_icon_menu = None;
                } else {
                    this.cluster_settings_icon_menu = Some(ctx_for_icon_menu.clone());
                }
            }))
            .child(Icon::new(IconName::Ellipsis).small());

        let icon_row =
            div().flex().flex_row().items_center().gap_3().child(icon_preview).child(dots_btn);

        // Icon appearance popup
        let mut icon_section = div()
            .p_4()
            .rounded_lg()
            .bg(card_bg)
            .border_1()
            .border_color(border)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_secondary)
                    .mb_2()
                    .child("ICON"),
            )
            .child(icon_row);

        // Show "..." popup if open
        if self.cluster_settings_icon_menu.as_deref() == Some(cluster_context) {
            let popup_bg = gpui::rgb(0x1F2937);
            let popup_border = gpui::rgb(0x374151);
            let popup_text = gpui::rgb(0xD1D5DB);
            let popup_hover = gpui::rgb(0x374151);

            let ctx_pick = ctx_name.clone();
            let pick_id = ElementId::Name(SharedString::from(format!(
                "cluster-settings-pick-color-{}",
                ctx_name
            )));
            let ctx_clear = ctx_name.clone();
            let clear_id = ElementId::Name(SharedString::from(format!(
                "cluster-settings-clear-icon-{}",
                ctx_name
            )));

            let popup = div()
                .mt_2()
                .w(px(180.0))
                .bg(popup_bg)
                .border_1()
                .border_color(popup_border)
                .rounded_md()
                .py_1()
                .text_sm()
                .text_color(popup_text)
                .child(
                    div()
                        .id(pick_id)
                        .px_3()
                        .py_1()
                        .cursor_pointer()
                        .hover(move |s| s.bg(popup_hover))
                        .on_click(cx.listener(move |this, _event, _window, _cx| {
                            // Toggle color picker visibility
                            if this.cluster_settings_color_picker.as_deref() == Some(&ctx_pick) {
                                this.cluster_settings_color_picker = None;
                            } else {
                                this.cluster_settings_color_picker = Some(ctx_pick.clone());
                            }
                            this.cluster_settings_icon_menu = None;
                        }))
                        .child("Pick Icon Color"),
                )
                .child(
                    div()
                        .id(clear_id)
                        .px_3()
                        .py_1()
                        .cursor_pointer()
                        .hover(move |s| s.bg(popup_hover))
                        .on_click(cx.listener(move |this, _event, _window, _cx| {
                            // Clear custom color and icon
                            this.cluster_appearances.remove(&ctx_clear);
                            // Reset sidebar entry color to auto-generated
                            if let Some(entry) = this
                                .sidebar
                                .clusters
                                .iter_mut()
                                .find(|c| c.context_name == ctx_clear)
                            {
                                entry.color = crate::layout::sidebar::generate_cluster_color(
                                    &entry.context_name,
                                );
                                entry.custom_icon_path = None;
                            }
                            this.persist_cluster_appearances();
                            this.cluster_settings_icon_menu = None;
                        }))
                        .child("Clear Icon"),
                );

            icon_section = icon_section.child(popup);
        }

        // Show color picker if visible
        if self.cluster_settings_color_picker.as_deref() == Some(cluster_context) {
            let mut palette_grid = div().mt_2().flex().flex_row().flex_wrap().gap_2();

            for color in Self::CLUSTER_COLOR_PALETTE {
                let ctx_color = ctx_name.clone();
                let swatch_id = ElementId::Name(SharedString::from(format!(
                    "color-swatch-{}-{:06X}",
                    ctx_name, color
                )));
                let is_selected = effective_color == color;
                let swatch = div()
                    .id(swatch_id)
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_full()
                    .bg(gpui::rgb(color))
                    .cursor_pointer()
                    .when(is_selected, |s| s.border_2().border_color(gpui::rgb(0xFFFFFF)))
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        // Set custom color
                        let appearance =
                            this.cluster_appearances.entry(ctx_color.clone()).or_default();
                        appearance.custom_color = Some(color);
                        // Update sidebar entry color
                        if let Some(entry) =
                            this.sidebar.clusters.iter_mut().find(|c| c.context_name == ctx_color)
                        {
                            entry.color = color;
                        }
                        this.persist_cluster_appearances();
                        this.cluster_settings_color_picker = None;
                    }));
                palette_grid = palette_grid.child(swatch);
            }

            icon_section = icon_section.child(palette_grid);
        }

        // --- Section 3: Kubeconfig ---
        let kubeconfig_id = ElementId::Name(SharedString::from(format!(
            "cluster-settings-kubeconfig-{}",
            ctx_name
        )));
        let kubeconfig_section = div()
            .p_4()
            .rounded_lg()
            .bg(card_bg)
            .border_1()
            .border_color(border)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_secondary)
                    .mb_2()
                    .child("KUBECONFIG"),
            )
            .child(
                div()
                    .id(kubeconfig_id)
                    .text_sm()
                    .text_color(gpui::rgb(0x60A5FA))
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _event, _window, _cx| {
                        // Reveal file in platform file manager
                        #[cfg(target_os = "macos")]
                        {
                            let _ = std::process::Command::new("open")
                                .arg("-R")
                                .arg(&ctx_for_finder)
                                .spawn();
                        }
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer.exe")
                                .args(["/select,", &ctx_for_finder])
                                .spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            if let Some(parent) =
                                std::path::Path::new(ctx_for_finder.as_str()).parent()
                            {
                                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
                            }
                        }
                    }))
                    .child(SharedString::from(kubeconfig_path)),
            );

        // --- Section 4: AWS Profile ---
        let aws_profile_section = {
            // Lazily create per-cluster AWS profile input
            let ctx_key = ctx_name.clone();
            if !self.cluster_aws_profile_inputs.contains_key(&ctx_key) {
                let initial_value = self
                    .preferences
                    .cluster_aws_profiles
                    .get(&ctx_key)
                    .cloned()
                    .unwrap_or_default();
                let input = cx.new(|cx| {
                    let mut state =
                        InputState::new(window, cx).placeholder("Leave blank to use default");
                    state.set_value(initial_value, window, cx);
                    state
                });
                let ctx_for_sub = ctx_key.clone();
                let sub = cx.subscribe(
                    &input,
                    move |this: &mut AppShell, entity, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            let val = entity.read(cx).value().to_string();
                            if val.is_empty() {
                                this.preferences.cluster_aws_profiles.remove(&ctx_for_sub);
                                this.cluster_aws_profiles.remove(&ctx_for_sub);
                            } else {
                                this.preferences
                                    .cluster_aws_profiles
                                    .insert(ctx_for_sub.clone(), val.clone());
                                this.cluster_aws_profiles.insert(ctx_for_sub.clone(), val);
                            }
                            cx.notify();
                        }
                    },
                );
                self.cluster_aws_profile_inputs.insert(ctx_key.clone(), input);
                self._cluster_aws_profile_subscriptions.insert(ctx_key.clone(), sub);
            }

            let mut section =
                div().p_4().rounded_lg().bg(card_bg).border_1().border_color(border).child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_secondary)
                        .mb_2()
                        .child("AWS PROFILE"),
                );

            if let Some(input_entity) = self.cluster_aws_profile_inputs.get(&ctx_key) {
                section = section.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(text_secondary)
                                .min_w(px(80.))
                                .child("Profile:"),
                        )
                        .child(div().flex_1().child(Input::new(input_entity).text_sm().small())),
                );
            }

            section = section.child(
                div()
                    .text_xs()
                    .text_color(text_secondary)
                    .mt_1()
                    .child("Uses the default profile if left blank."),
            );

            section
        };

        // Assemble the settings view
        let scrollable =
            div()
                .id("cluster-settings-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .h_full()
                .p_6()
                .gap_4()
                .overflow_y_scroll()
                .child(
                    div().text_lg().font_weight(FontWeight::BOLD).text_color(text).mb_2().child(
                        SharedString::from(format!("Cluster Settings \u{2014} {}", ctx_name)),
                    ),
                )
                .child(name_section)
                .child(icon_section)
                .child(kubeconfig_section)
                .child(aws_profile_section);

        div().flex().flex_col().flex_1().h_full().bg(bg).child(scrollable)
    }

    /// Render a cluster node plus its expanded tree (if expanded).
    /// Used by the grouped section rendering.
    fn render_cluster_with_tree(
        &self,
        mut parent: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
        cluster: &crate::layout::sidebar::ClusterEntry,
        text: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
    ) -> gpui::Stateful<gpui::Div> {
        parent = parent.child(self.render_navigator_cluster_node(
            cx,
            cluster,
            text,
            text_secondary,
            accent,
        ));

        if cluster.expanded {
            let entries = self.sidebar.flatten_navigator_tree(cluster);
            if !entries.is_empty() {
                let entry_count = entries.len();

                let decoration = NavigatorIndentGuideDecoration::new(
                    entries.clone(),
                    gpui::hsla(0.0, 0.0, 1.0, 0.08),
                );

                let scroll_handle =
                    self.navigator_scroll_handles.get(&cluster.id).cloned().unwrap_or_default();

                let list_id =
                    ElementId::Name(SharedString::from(format!("nav-tree-{}", cluster.id,)));

                let view = cx.entity().downgrade();
                let entries = Rc::new(entries);

                parent = parent.child(
                    uniform_list(list_id, entry_count, {
                        let view = view.clone();
                        let entries = entries.clone();
                        move |range, _window, cx| {
                            view.update(cx, |this, cx| {
                                range
                                    .map(|ix| {
                                        this.render_navigator_flat_entry(
                                            &entries[ix],
                                            ix,
                                            accent,
                                            cx,
                                        )
                                        .into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                        }
                    })
                    .flex_shrink_0()
                    .h(px(entry_count as f32 * 24.0))
                    .with_decoration(decoration)
                    .track_scroll(scroll_handle),
                );
            }
        }

        parent
    }

    /// Render a single top-level cluster node in the Navigator tree (T307).
    /// Shows colored initials icon, display name, status dot, and expand/collapse chevron.
    fn render_navigator_cluster_node(
        &self,
        cx: &mut Context<Self>,
        cluster: &crate::layout::sidebar::ClusterEntry,
        text: Rgba,
        _text_secondary: Rgba,
        _accent: Rgba,
    ) -> Stateful<Div> {
        let cluster_id = cluster.id;
        let initials = SharedString::from(cluster.initials.clone());
        let color = cluster.color;
        let display_name = SharedString::from(cluster.display_name.clone());
        let expanded = cluster.expanded;
        let status = cluster.status.clone();
        let context_name = cluster.context_name.clone();
        let is_disconnected = status == ClusterStatus::Disconnected;

        // Status dot color
        let status_color = match &status {
            ClusterStatus::Connected => gpui::rgb(0x22C55E), // green
            ClusterStatus::Connecting => gpui::rgb(0xFBBF24), // yellow
            ClusterStatus::Disconnected => gpui::rgb(0x6B7280), // gray
            ClusterStatus::Error => gpui::rgb(0xEF4444),     // red
        };

        let chevron_icon = if expanded {
            Icon::new(IconName::ChevronDown).xsmall()
        } else {
            Icon::new(IconName::ChevronRight).xsmall()
        };
        let chevron_id = ElementId::Name(SharedString::from(format!("nav-chevron-{}", cluster_id)));
        let chevron_btn = div()
            .id(chevron_id)
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .text_color(self.theme.colors.text_secondary.to_gpui())
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                if this.has_modal_overlay() {
                    return;
                }
                this.sidebar.toggle_cluster_expand(cluster_id);
            }))
            .child(chevron_icon);

        // Colored initials icon
        let initials_icon = div()
            .w(px(24.0))
            .h(px(24.0))
            .rounded_md()
            .bg(gpui::rgb(color))
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(gpui::rgb(0xFFFFFF))
            .child(initials);

        // Status dot
        let status_dot = div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color);

        // Cluster name — clicking opens dashboard (T309)
        let name_id = ElementId::Name(SharedString::from(format!("nav-cluster-{}", cluster_id)));

        let ctx_for_click = context_name.clone();
        let name_el = div()
            .id(name_id)
            .flex_1()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text)
            .cursor_pointer()
            .on_click(cx.listener(move |this, event, _window, cx| {
                if this.has_modal_overlay() {
                    return;
                }
                // T311: Double-click enters drill-into mode.
                let click_count = match event {
                    ClickEvent::Mouse(m) => m.down.click_count,
                    ClickEvent::Keyboard(_) => 1,
                };
                if click_count >= 2 {
                    this.sidebar.enter_drill_into(cluster_id);
                    return;
                }

                if is_disconnected {
                    // T309: Connect on click if disconnected
                    this.handle_connect_cluster(&ctx_for_click, cx);
                } else {
                    // T309: Open dashboard tab for this cluster
                    let target =
                        NavigationTarget::Dashboard { cluster_context: ctx_for_click.clone() };
                    this.workspace.open_tab(target);
                    // T310: Sync navigator tree to the newly opened tab.
                    this.sync_navigator_to_active_tab();
                }
            }))
            .child(display_name);

        // Clicking the cluster name connects if disconnected (no separate "Connect" label needed)

        // Assemble the cluster row
        div()
            .id(ElementId::Name(SharedString::from(format!("nav-cluster-row-{cluster_id}"))))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .relative()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _window, _cx| {
                    this.context_menu_cluster = Some(cluster_id);
                    this.context_menu_position_y = event.position.y.into();
                }),
            )
            .child(chevron_btn)
            .child(initials_icon)
            .child(name_el)
            .child(status_dot)
    }

    /// Render a single flat entry for the navigator uniform_list.
    fn render_navigator_flat_entry(
        &self,
        entry: &NavigatorFlatEntry,
        _ix: usize,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        match entry {
            NavigatorFlatEntry::Leaf {
                depth,
                label,
                target_kind,
                cluster_id,
                context_name,
                ..
            } => self.render_nav_leaf(
                *depth,
                label,
                target_kind,
                *cluster_id,
                context_name,
                accent,
                cx,
            ),
            NavigatorFlatEntry::CategoryHeader {
                depth,
                label,
                category,
                cluster_id,
                expanded,
                ..
            } => self.render_nav_category_header(
                *depth,
                label,
                *category,
                *cluster_id,
                *expanded,
                cx,
            ),
            NavigatorFlatEntry::ResourceKind {
                depth,
                label,
                kind,
                category,
                cluster_id,
                context_name,
                badge_count,
                ..
            } => self.render_nav_resource_kind(
                *depth,
                label,
                kind,
                *category,
                *cluster_id,
                context_name,
                *badge_count,
                accent,
                cx,
            ),
        }
    }

    /// Render a leaf item (Overview, Nodes, Namespaces, Events) for the navigator uniform_list.
    #[allow(clippy::too_many_arguments)]
    fn render_nav_leaf(
        &self,
        depth: usize,
        label: &str,
        target_kind: &str,
        cluster_id: uuid::Uuid,
        context_name: &str,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let is_dashboard = target_kind == "__Dashboard__";
        let is_cluster_topology = target_kind == "__ClusterTopology__";

        let is_active = if is_dashboard {
            self.workspace
                .active_tab()
                .map(|tab| {
                    matches!(&tab.target, NavigationTarget::Dashboard { cluster_context } if cluster_context == context_name)
                })
                .unwrap_or(false)
        } else if is_cluster_topology {
            self.workspace
                .active_tab()
                .map(|tab| {
                    matches!(&tab.target, NavigationTarget::ClusterTopology { cluster_context } if cluster_context == context_name)
                })
                .unwrap_or(false)
        } else {
            self.sidebar.is_active(target_kind)
                && self
                    .workspace
                    .active_tab()
                    .and_then(|tab| tab.target.cluster_context().map(|c| c == context_name))
                    .unwrap_or(false)
        };

        let item_id =
            ElementId::Name(SharedString::from(format!("nav-leaf-{}-{}", cluster_id, target_kind)));

        let label_str = SharedString::from(label.to_string());
        let target_kind_owned = target_kind.to_string();
        let ctx_owned = context_name.to_string();
        let left_pad = INDENT_OFFSET + depth as f32 * INDENT_STEP;

        let mut item_div = div()
            .id(item_id)
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .pl(px(left_pad))
            .pr(px(8.0))
            .cursor_pointer()
            .text_sm()
            .on_click(cx.listener({
                let kind = target_kind_owned.clone();
                let ctx = ctx_owned.clone();
                move |this, _event, _window, cx| {
                    if this.has_modal_overlay() {
                        return;
                    }
                    if kind == "__Dashboard__" {
                        let target = NavigationTarget::Dashboard { cluster_context: ctx.clone() };
                        this.workspace.open_tab(target);
                    } else if kind == "__ClusterTopology__" {
                        let target =
                            NavigationTarget::ClusterTopology { cluster_context: ctx.clone() };
                        this.workspace.open_tab(target);
                    } else {
                        this.sidebar.navigate_to_kind(&kind, &ctx);
                        if let Some(category) = this.sidebar.find_kind_category(&kind) {
                            this.workspace.open_tab(NavigationTarget::ResourceList {
                                cluster_context: ctx.clone(),
                                category,
                                kind: kind.clone(),
                            });
                        }
                    }
                    this.sync_navigator_to_active_tab();
                    this.trigger_data_loading_for_active_tab(cx);
                }
            }));

        let nav_text = self.theme.colors.text_primary.to_gpui();
        let nav_text_secondary = self.theme.colors.text_secondary.to_gpui();
        let nav_selection_bg = self.theme.colors.selection.to_gpui();
        let nav_hover_bg = Rgba { r: nav_text.r, g: nav_text.g, b: nav_text.b, a: 0.04 };

        if is_active {
            item_div = item_div
                .bg(nav_selection_bg)
                .border_l_2()
                .border_color(accent)
                .text_color(nav_text);
        } else {
            item_div = item_div.text_color(nav_text).hover(move |s| s.bg(nav_hover_bg));
        }

        // Choose icon based on the leaf target kind
        let leaf_icon = if is_dashboard {
            Icon::new(IconName::LayoutDashboard).xsmall().text_color(nav_text_secondary)
        } else if is_cluster_topology {
            Icon::new(IconName::Globe).xsmall().text_color(nav_text_secondary)
        } else {
            Icon::new(ResourceIcon::from_kind(target_kind)).xsmall().text_color(nav_text_secondary)
        };

        item_div = item_div
            .gap_1p5()
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(leaf_icon),
            )
            .child(label_str);
        item_div
    }
    #[allow(clippy::too_many_arguments)]
    fn render_nav_category_header(
        &self,
        depth: usize,
        label: &str,
        category: crate::icons::ResourceCategory,
        cluster_id: uuid::Uuid,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let cat_text = self.theme.colors.text_secondary.to_gpui();
        let cat_hover = Rgba {
            r: self.theme.colors.text_primary.to_gpui().r,
            g: self.theme.colors.text_primary.to_gpui().g,
            b: self.theme.colors.text_primary.to_gpui().b,
            a: 0.04,
        };
        let chevron_icon = if expanded {
            Icon::new(IconName::ChevronDown).xsmall()
        } else {
            Icon::new(IconName::ChevronRight).xsmall()
        };
        let label_str = SharedString::from(label.to_string());
        let left_pad = INDENT_OFFSET + depth as f32 * INDENT_STEP;

        let cat_id = ElementId::Name(SharedString::from(format!(
            "nav-tcat-{}-{}",
            cluster_id,
            category.label()
        )));

        div()
            .id(cat_id)
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .pl(px(left_pad))
            .pr(px(8.0))
            .gap_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cat_text)
            .cursor_pointer()
            .hover(move |s| s.bg(cat_hover))
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                if this.has_modal_overlay() {
                    return;
                }
                this.sidebar.toggle_category_expand(cluster_id, category);
            }))
            .child(div().w(px(12.0)).flex().items_center().justify_center().child(chevron_icon))
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(category).xsmall().text_color(cat_text)),
            )
            .child(label_str)
    }

    /// Render a resource kind item for the navigator uniform_list.
    #[allow(clippy::too_many_arguments)]
    fn render_nav_resource_kind(
        &self,
        depth: usize,
        label: &str,
        kind: &str,
        category: crate::icons::ResourceCategory,
        cluster_id: uuid::Uuid,
        context_name: &str,
        badge: Option<u32>,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let is_active = self.sidebar.is_active(kind)
            && self
                .workspace
                .active_tab()
                .and_then(|tab| tab.target.cluster_context().map(|c| c == context_name))
                .unwrap_or(false);

        let kind_owned = kind.to_string();
        let ctx_owned = context_name.to_string();
        let label_str = SharedString::from(label.to_string());
        let left_pad = INDENT_OFFSET + depth as f32 * INDENT_STEP;

        let item_id = ElementId::Name(SharedString::from(format!(
            "nav-tk-{}-{}-{}",
            cluster_id,
            category.label(),
            kind
        )));

        let mut item_div = div()
            .id(item_id)
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pl(px(left_pad))
            .pr(px(8.0))
            .cursor_pointer()
            .text_sm()
            .on_click(cx.listener({
                let kind_click = kind_owned.clone();
                let ctx_click = ctx_owned.clone();
                move |this, _event, _window, cx| {
                    if this.has_modal_overlay() {
                        return;
                    }
                    this.sidebar.navigate_to_kind(&kind_click, &ctx_click);
                    this.workspace.open_tab(NavigationTarget::ResourceList {
                        cluster_context: ctx_click.clone(),
                        category,
                        kind: kind_click.clone(),
                    });
                    this.sync_navigator_to_active_tab();
                    this.trigger_data_loading_for_active_tab(cx);
                }
            }));

        let rk_text = self.theme.colors.text_primary.to_gpui();
        let rk_text_sec = self.theme.colors.text_secondary.to_gpui();
        let rk_selection = self.theme.colors.selection.to_gpui();
        let rk_hover = Rgba { r: rk_text.r, g: rk_text.g, b: rk_text.b, a: 0.04 };
        let rk_badge_bg = self.theme.colors.surface_hover.to_gpui();

        if is_active {
            item_div =
                item_div.bg(rk_selection).border_l_2().border_color(accent).text_color(rk_text);
        } else {
            item_div = item_div.text_color(rk_text).hover(move |s| s.bg(rk_hover));
        }

        let res_icon = ResourceIcon::from_kind(kind);
        item_div = item_div
            .gap_1p5()
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(res_icon).xsmall().text_color(rk_text_sec)),
            )
            .child(div().flex_1().child(label_str));

        if let Some(count) = badge {
            item_div = item_div.child(
                div()
                    .text_xs()
                    .px_1()
                    .rounded_sm()
                    .bg(rk_badge_bg)
                    .text_color(rk_text_sec)
                    .child(SharedString::from(count.to_string())),
            );
        }

        item_div
    }

    /// Render sidebar sections (shared between cluster-tree and static fallback).
    fn render_sections(
        &self,
        mut container: Div,
        sections: &[crate::layout::sidebar::SidebarSection],
        cluster_context: &str,
        cx: &mut Context<Self>,
        text_secondary: Rgba,
        accent: Rgba,
    ) -> Div {
        for (sec_idx, section) in sections.iter().enumerate() {
            let category_label = section.category.label().to_string();
            let expanded = section.expanded;
            let category = section.category;

            let mut section_div = div().flex().flex_col().py_1().child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_secondary)
                    .cursor_pointer()
                    .child(if expanded { "v " } else { "> " })
                    .child(category_label),
            );

            if expanded {
                for item in &section.items {
                    let is_active = self.sidebar.is_active(&item.kind);
                    let label = item.label.clone();
                    let badge = item.badge_count;
                    let kind = item.kind.clone();
                    let ctx = cluster_context.to_string();

                    let item_id = ElementId::Name(SharedString::from(format!(
                        "sidebar-item-{sec_idx}-{}",
                        kind
                    )));
                    let mut item_div = div()
                        .id(item_id)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_sm()
                        .on_click(cx.listener({
                            let kind = kind.clone();
                            move |this, _event, _window, _cx| {
                                this.sidebar.set_active_kind(&kind);
                                this.workspace.open_tab(NavigationTarget::ResourceList {
                                    cluster_context: ctx.clone(),
                                    category,
                                    kind: kind.clone(),
                                });
                            }
                        }));

                    if is_active {
                        item_div = item_div.bg(accent).text_color(gpui::rgb(0xFFFFFF));
                    }

                    item_div = item_div.child(label);

                    if let Some(count) = badge {
                        item_div = item_div.child(
                            div()
                                .text_xs()
                                .px_1()
                                .rounded_sm()
                                .bg(gpui::rgb(0x374151))
                                .child(count.to_string()),
                        );
                    }

                    section_div = section_div.child(item_div);
                }
            }

            container = container.child(section_div);
        }

        container
    }

    /// T356: Render the status bar at the very bottom of the window.
    ///
    /// Shows: [connection-dot cluster-name] --- spacer --- [k8s version]
    fn render_status_bar(&self, _text: Rgba, text_secondary: Rgba, border: Rgba) -> Div {
        // Determine the active cluster name and its connection status.
        let (cluster_display, cluster_status) = self.status_bar_cluster_info();

        let k8s_version_label = self.k8s_version.as_deref().unwrap_or("K8s version unknown");

        // Status dot color: green=connected, yellow=connecting, gray=disconnected, red=error.
        let dot_color = match cluster_status {
            ClusterStatus::Connected => gpui::rgb(0x22C55E), // green
            ClusterStatus::Connecting => gpui::rgb(0xF59E0B), // yellow
            ClusterStatus::Disconnected => gpui::rgb(0x9CA3AF), // gray
            ClusterStatus::Error => gpui::rgb(0xEF4444),     // red
        };

        // --- Left side: connection indicator + cluster name + context ---
        let context_name = self.active_dashboard_cluster.as_deref().unwrap_or("");
        let left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            // Colored dot
            .child(div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(dot_color))
            // Cluster name
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary)
                    .child(SharedString::from(cluster_display)),
            )
            // Context name (dimmer)
            .when(!context_name.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(Rgba {
                            r: text_secondary.r,
                            g: text_secondary.g,
                            b: text_secondary.b,
                            a: 0.5,
                        })
                        .child(SharedString::from(format!("ctx: {context_name}"))),
                )
            });

        // --- Right side: K8s version ---
        let right = div()
            .text_xs()
            .text_color(text_secondary)
            .child(SharedString::from(k8s_version_label.to_string()));

        // --- Center: Update notification ---
        let update_banner = if let Some((ref version, ref _url)) = self.update_available {
            let msg = format!(
                "Update available: v{version} — Run: curl -sL https://github.com/Craigeous/baeus/releases/latest/download/Baeus-macos-arm64.dmg -o /tmp/Baeus.dmg && open /tmp/Baeus.dmg"
            );
            div()
                .text_xs()
                .text_color(gpui::rgb(0xF59E0B)) // amber
                .child(SharedString::from(msg))
        } else {
            div()
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(24.0))
            .px_4()
            .bg(self.theme.colors.surface.to_gpui())
            .border_t_1()
            .border_color(border)
            .flex_shrink_0()
            .child(left)
            .child(div().flex_1()) // spacer
            .child(update_banner)
            .child(div().flex_1()) // spacer
            .child(right)
    }

    /// T356: Determine the cluster display name and status for the status bar.
    fn status_bar_cluster_info(&self) -> (String, ClusterStatus) {
        // Prefer the active dashboard cluster.
        if let Some(ref ctx) = self.active_dashboard_cluster {
            if let Some(entry) = self.sidebar.clusters.iter().find(|c| c.context_name == *ctx) {
                return (entry.display_name.clone(), entry.status.clone());
            }
        }
        // Fall back to the first connected cluster.
        if let Some(entry) =
            self.sidebar.clusters.iter().find(|c| c.status == ClusterStatus::Connected)
        {
            return (entry.display_name.clone(), entry.status.clone());
        }
        // Fall back to the first cluster, or show a placeholder.
        if let Some(entry) = self.sidebar.clusters.first() {
            return (entry.display_name.clone(), entry.status.clone());
        }
        ("No cluster".to_string(), ClusterStatus::Disconnected)
    }

    /// Render the tab bar with click-to-activate and close buttons.
    fn render_tab_bar(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let tabs = self.workspace.tabs.clone();

        let mut tab_bar = div()
            .id("app-tab-bar")
            .flex()
            .flex_row()
            .items_center()
            .h_9()
            .bg(self.theme.colors.tab_bar_bg.to_gpui())
            .border_b_1()
            .border_color(border)
            .flex_shrink_0()
            .overflow_x_scroll();

        for (tab_idx, tab) in tabs.iter().enumerate() {
            let is_active = self.workspace.active_tab_id == Some(tab.id);
            let tab_id = tab.id;
            let closable = tab.closable;
            let tab_cluster_context = tab.target.cluster_context().map(|s| s.to_string());

            let tab_element_id =
                ElementId::Name(SharedString::from(format!("appshell-tab-{tab_idx}")));

            let tab_active_bg = self.theme.colors.tab_active_bg.to_gpui();

            // Compute display label for the tab
            let tab_label_text = if let Some(ctx) = tab.target.cluster_context() {
                let display = self.sidebar.display_name_for_context(ctx);
                tab.label.replace(ctx, &display)
            } else {
                tab.label.clone()
            };
            let tab_label_shared = SharedString::from(tab_label_text.clone());

            let mut tab_el = div()
                .id(tab_element_id)
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .border_t_2()
                .flex_shrink()
                .min_w(px(if is_active { 80.0 } else { 40.0 }))
                .max_w(px(200.0))
                .overflow_hidden()
                .when(is_active, |el| el.border_color(accent).text_color(text).bg(tab_active_bg))
                .when(!is_active, |el| {
                    el.border_color(gpui::rgba(0x00000000)).text_color(text_secondary)
                })
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.workspace.activate_tab(tab_id);
                    // T310: Sync navigator tree to the newly active tab.
                    this.sync_navigator_to_active_tab();
                    // T332: Trigger data loading for the newly active tab.
                    this.trigger_data_loading_for_active_tab(cx);
                }))
                .child(
                    div().flex_1().overflow_hidden().whitespace_nowrap().child(tab_label_shared),
                );

            if closable {
                let close_id =
                    ElementId::Name(SharedString::from(format!("appshell-tab-close-{tab_idx}")));
                let close_btn = div()
                    .id(close_id)
                    .ml_1()
                    .px_1()
                    .text_xs()
                    .text_color(text_secondary)
                    .cursor_pointer()
                    .flex_shrink_0()
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.workspace.close_tab(tab_id);
                        if let Some(ref ctx) = tab_cluster_context {
                            this.cleanup_cluster_terminal_if_last(ctx);
                        }
                    }))
                    .child(Icon::new(IconName::Close).xsmall());
                tab_el = tab_el.child(close_btn);
            }

            tab_bar = tab_bar.child(tab_el);
        }

        // Wrap in a non-stateful Div for return type compatibility
        div().child(tab_bar)
    }

    // -----------------------------------------------------------------------
    // T330: View routing — render content area based on active tab's NavigationTarget
    // -----------------------------------------------------------------------

    fn render_content_area(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        _border: Rgba,
        _accent: Rgba,
    ) -> Div {
        let bg = self.theme.colors.background.to_gpui();
        let Some(active_tab) = self.workspace.active_tab().cloned() else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("No Tab Selected");
        };

        let target = &active_tab.target;
        let label = target.label();

        // T362: Check if there is an error for the current view before rendering.
        let error_key = target.view_error_key();
        if let Some(error_msg) = self.view_errors.get(&error_key) {
            let target_clone = target.clone();
            return self.render_error_state(cx, &error_key, error_msg, target_clone, bg);
        }

        match target {
            NavigationTarget::ClusterList => {
                self.render_view_placeholder("Cluster List", &label, text, text_secondary, bg)
            }
            NavigationTarget::Dashboard { cluster_context } => {
                self.render_dashboard_content(cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::ResourceList { cluster_context, kind, .. } => self
                .render_resource_list_content(cx, cluster_context, kind, text, text_secondary, bg),
            NavigationTarget::ResourceDetail { cluster_context, kind, name, namespace } => self
                .render_resource_detail_content(
                    cx,
                    cluster_context,
                    kind,
                    name,
                    namespace.as_deref(),
                    text,
                    text_secondary,
                    bg,
                ),
            NavigationTarget::HelmReleases { cluster_context } => {
                self.render_helm_releases_content(cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::HelmInstall { cluster_context } => {
                self.render_helm_install_content(cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::CrdBrowser { cluster_context } => {
                self.render_crd_browser_content(cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::NamespaceMap { .. } => {
                self.render_view_placeholder("Resource Map", &label, text, text_secondary, bg)
            }
            NavigationTarget::PluginManager { .. } => {
                self.render_view_placeholder("Plugin Manager", &label, text, text_secondary, bg)
            }
            NavigationTarget::ClusterSettings { cluster_context } => {
                self.render_cluster_settings(window, cx, cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::ClusterTopology { cluster_context } => {
                self.render_cluster_topology(cx, cluster_context, text, text_secondary, bg)
            }
            NavigationTarget::Preferences => {
                self.render_preferences_content(window, cx, text, text_secondary, bg)
            }
        }
    }

    fn render_view_placeholder(
        &self,
        view_name: &str,
        label: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(bg)
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .child(view_name.to_string()),
            )
            .child(div().text_sm().text_color(text_secondary).child(label.to_string()))
    }

    // -----------------------------------------------------------------------
    // Preferences panel rendering
    // -----------------------------------------------------------------------

    fn render_preferences_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let surface = self.theme.colors.surface.to_gpui();
        let surface_hover = self.theme.colors.surface_hover.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let border = self.theme.colors.border.to_gpui();

        // --- Left sidebar ---
        let mut sidebar = div()
            .w(px(180.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(surface)
            .border_r_1()
            .border_color(border)
            .py_4()
            .child(
                div()
                    .px_4()
                    .pb_3()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("PREFERENCES"),
            );

        for &section in PreferencesSection::all() {
            let is_active = section == self.active_prefs_section;
            let section_id =
                ElementId::Name(SharedString::from(format!("prefs-section-{}", section.label())));

            let item = div()
                .id(section_id)
                .px_4()
                .py_1p5()
                .cursor_pointer()
                .text_sm()
                .text_color(if is_active { text } else { text_secondary })
                .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .when(is_active, |el| el.bg(surface_hover))
                .hover(|el| el.bg(surface_hover))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.active_prefs_section = section;
                    // Auto-fetch AWS identity when switching to Kubernetes section
                    if section == PreferencesSection::Kubernetes
                        && this.aws_caller_identity.is_none()
                        && !this.aws_identity_loading
                    {
                        this.fetch_aws_caller_identity(cx);
                    }
                    cx.notify();
                }))
                .child(section.label());

            sidebar = sidebar.child(item);
        }

        // --- Right content ---
        let content = div()
            .id("prefs-content-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p_6()
            .bg(bg)
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text)
                    .child(self.active_prefs_section.label()),
            )
            .children(self.render_prefs_section_content(
                window,
                cx,
                text,
                text_secondary,
                surface,
                surface_hover,
                accent,
                border,
            ));

        // --- Outer container ---
        div().flex_1().h_full().flex().flex_row().child(sidebar).child(content)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_prefs_section_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        _surface_hover: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> Vec<Div> {
        match self.active_prefs_section {
            PreferencesSection::App => {
                self.render_prefs_app(cx, text, text_secondary, surface, accent, border)
            }
            PreferencesSection::Kubernetes => {
                self.render_prefs_kubernetes(window, cx, text, text_secondary, surface, border)
            }
            PreferencesSection::Terminal => {
                self.render_prefs_terminal(text, text_secondary, surface, border)
            }
            PreferencesSection::About => self.render_prefs_about(text, text_secondary),
        }
    }

    /// Helper: render a settings group label.
    fn prefs_group_label(&self, label: &str, text_secondary: Rgba) -> Div {
        div()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(text_secondary)
            .pb_1()
            .child(label.to_string())
    }

    // --- App section ---
    fn render_prefs_app(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> Vec<Div> {
        let current_mode = self.preferences.theme_mode;
        let font_size = self.preferences.font_size;
        let log_limit = self.preferences.log_line_limit;

        // --- Theme selector ---
        let theme_group = {
            let mut row = div().flex().flex_row().gap_2();
            for &mode in &[ThemeMode::Light, ThemeMode::Dark, ThemeMode::System] {
                let label = match mode {
                    ThemeMode::Light => "Light",
                    ThemeMode::Dark => "Dark",
                    ThemeMode::System => "System",
                };
                let is_selected = mode == current_mode;
                let btn_id = ElementId::Name(SharedString::from(format!("theme-{label}")));
                let btn = div()
                    .id(btn_id)
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .border_1()
                    .border_color(if is_selected { accent } else { border })
                    .text_color(if is_selected { accent } else { text })
                    .font_weight(if is_selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .when(is_selected, |el| {
                        el.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.15 })
                    })
                    .hover(|el| el.bg(surface))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.preferences.theme_mode = mode;
                        this.theme = Theme::for_mode(mode);
                        cx.notify();
                    }))
                    .child(label);
                row = row.child(btn);
            }
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("THEME", text_secondary))
                .child(row)
        };

        // --- Font size stepper ---
        let font_group = {
            let minus_id = ElementId::Name(SharedString::from("font-minus"));
            let plus_id = ElementId::Name(SharedString::from("font-plus"));

            let minus_btn = div()
                .id(minus_id)
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(text)
                .hover(|el| el.bg(surface))
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.preferences.font_size = (this.preferences.font_size - 1.0).max(10.0);
                    cx.notify();
                }))
                .child("\u{2212}"); // minus sign

            let plus_btn = div()
                .id(plus_id)
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(text)
                .hover(|el| el.bg(surface))
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.preferences.font_size = (this.preferences.font_size + 1.0).min(24.0);
                    cx.notify();
                }))
                .child("+");

            let value_display =
                div().px_3().text_sm().text_color(text).child(format!("{}", font_size as u32));

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("FONT SIZE", text_secondary))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(minus_btn)
                        .child(value_display)
                        .child(plus_btn),
                )
        };

        // --- Log line limit ---
        let log_group = {
            let log_minus_id = ElementId::Name(SharedString::from("log-minus"));
            let log_plus_id = ElementId::Name(SharedString::from("log-plus"));

            let minus_btn = div()
                .id(log_minus_id)
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(text)
                .hover(|el| el.bg(surface))
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.preferences.log_line_limit =
                        this.preferences.log_line_limit.saturating_sub(1000).max(1000);
                    cx.notify();
                }))
                .child("\u{2212}");

            let plus_btn = div()
                .id(log_plus_id)
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(border)
                .text_sm()
                .text_color(text)
                .hover(|el| el.bg(surface))
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.preferences.log_line_limit =
                        (this.preferences.log_line_limit + 1000).min(100_000);
                    cx.notify();
                }))
                .child("+");

            let value_display =
                div().px_3().text_sm().text_color(text).child(format!("{}", log_limit));

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("LOG LINE LIMIT", text_secondary))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(minus_btn)
                        .child(value_display)
                        .child(plus_btn),
                )
        };

        // --- Save button ---
        let save_btn = self.render_prefs_save_button(cx, text, accent, border);

        vec![theme_group, font_group, log_group, save_btn]
    }

    // --- Kubernetes section ---
    fn render_prefs_kubernetes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Vec<Div> {
        // Default namespace
        let ns_group = {
            let ns_val =
                self.preferences.default_namespace.as_deref().unwrap_or("(all namespaces)");

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("DEFAULT NAMESPACE", text_secondary))
                .child(
                    div()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(border)
                        .bg(surface)
                        .text_sm()
                        .text_color(text)
                        .child(ns_val.to_string()),
                )
        };

        // Kubeconfig scan dirs
        let dirs_group = {
            let mut list = div().flex().flex_col().gap_1();

            // Always show the default ~/.kube/ entry (non-removable)
            list = list.child(
                div().flex().flex_row().items_center().gap_2().child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(border)
                        .bg(surface)
                        .text_sm()
                        .text_color(text_secondary)
                        .child("~/.kube/ (default)"),
                ),
            );

            // User-added directories with Remove buttons
            for (i, dir) in self.preferences.kubeconfig_scan_dirs.iter().enumerate() {
                let remove_id = ElementId::Name(SharedString::from(format!("remove-scandir-{i}")));
                let row = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(border)
                            .bg(surface)
                            .text_sm()
                            .text_color(text)
                            .child(dir.clone()),
                    )
                    .child(
                        div()
                            .id(remove_id)
                            .cursor_pointer()
                            .text_sm()
                            .text_color(self.theme.colors.error.to_gpui())
                            .hover(|el| el.text_color(gpui::rgb(0xff4444)))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if i < this.preferences.kubeconfig_scan_dirs.len() {
                                    this.preferences.kubeconfig_scan_dirs.remove(i);
                                    cx.notify();
                                }
                            }))
                            .child("Remove"),
                    );
                list = list.child(row);
            }

            // Add Directory button — opens native macOS directory picker
            let add_btn = div()
                .id("add-scandir-btn")
                .cursor_pointer()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(self.theme.colors.accent.to_gpui())
                .text_sm()
                .text_color(gpui::rgb(0xffffff))
                .hover(|el| el.opacity(0.8))
                .on_click(cx.listener(|_this, _event, _window, cx| {
                    let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some("Select kubeconfig directory".into()),
                    });
                    cx.spawn(async move |this, cx: &mut gpui::AsyncApp| {
                        if let Ok(Ok(Some(paths))) = rx.await {
                            if let Some(path) = paths.first() {
                                let path_str = path.to_string_lossy().to_string();
                                this.update(cx, |this, cx| {
                                    if !this.preferences.kubeconfig_scan_dirs.contains(&path_str) {
                                        this.preferences.kubeconfig_scan_dirs.push(path_str);
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                    })
                    .detach();
                }))
                .child("Add Directory");
            list = list.child(add_btn);

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("KUBECONFIG SCAN DIRECTORIES", text_secondary))
                .child(list)
        };

        // AWS Authentication section
        let aws_group = {
            // Lazily create the default AWS profile input
            if self.aws_profile_input.is_none() {
                let initial_value =
                    self.preferences.default_aws_profile.clone().unwrap_or_default();
                let input = cx.new(|cx| {
                    let mut state = InputState::new(window, cx).placeholder("e.g. my-sso-profile");
                    state.set_value(initial_value, window, cx);
                    state
                });
                let sub =
                    cx.subscribe(&input, |this: &mut AppShell, entity, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            let val = entity.read(cx).value().to_string();
                            if val.is_empty() {
                                this.preferences.default_aws_profile = None;
                            } else {
                                this.preferences.default_aws_profile = Some(val);
                            }
                            cx.notify();
                        }
                    });
                self.aws_profile_input = Some(input);
                self._aws_profile_subscription = Some(sub);
            }

            let mut aws_section = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.prefs_group_label("AWS AUTHENTICATION", text_secondary))
                .child(div().text_xs().text_color(text_secondary).child(
                    "Set the AWS profile used for EKS cluster authentication (aws eks get-token).",
                ));

            // Current Identity display
            let identity_row =
                {
                    let mut row = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(border)
                        .bg(surface);

                    let header_row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_secondary)
                                .child("Current Identity"),
                        )
                        .child(
                            div()
                                .id("aws-identity-refresh")
                                .cursor_pointer()
                                .text_xs()
                                .text_color(text_secondary)
                                .hover(|el| el.text_color(gpui::rgb(0x60A5FA)))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.fetch_aws_caller_identity(cx);
                                }))
                                .child(if self.aws_identity_loading {
                                    "loading..."
                                } else {
                                    "refresh"
                                }),
                        );
                    row = row.child(header_row);

                    if let Some(ref identity) = self.aws_caller_identity {
                        row = row
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text)
                                    .child(SharedString::from(identity.arn.clone())),
                            )
                            .child(div().text_xs().text_color(text_secondary).child(
                                SharedString::from(format!("Account: {}", identity.account)),
                            ));
                    } else if self.aws_identity_loading {
                        row = row
                            .child(div().text_sm().text_color(text_secondary).child("Fetching..."));
                    } else {
                        row =
                            row.child(div().text_sm().text_color(text_secondary).child(
                                "Not available — click refresh or check AWS CLI configuration",
                            ));
                    }

                    row
                };
            aws_section = aws_section.child(identity_row);

            // Default AWS Profile editable input
            if let Some(ref input_entity) = self.aws_profile_input {
                aws_section = aws_section.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(text_secondary)
                                .min_w(px(160.))
                                .child("Default AWS Profile:"),
                        )
                        .child(div().flex_1().child(Input::new(input_entity).text_sm().small())),
                );
            }

            // Authenticate button
            let accent = self.theme.colors.accent.to_gpui();
            aws_section = aws_section.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .mt_2()
                    .child(
                        div()
                            .id("aws-sso-authenticate-btn")
                            .cursor_pointer()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(accent)
                            .text_sm()
                            .text_color(gpui::rgb(0xffffff))
                            .font_weight(FontWeight::SEMIBOLD)
                            .hover(|el| el.opacity(0.8))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.run_aws_sso_login(cx);
                            }))
                            .child("Authenticate"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .flex()
                            .items_center()
                            .child("Runs aws sso login in the dock terminal"),
                    ),
            );

            aws_section
        };

        let save_btn =
            self.render_prefs_save_button(cx, text, self.theme.colors.accent.to_gpui(), border);

        vec![ns_group, dirs_group, aws_group, save_btn]
    }

    // --- Terminal section ---
    fn render_prefs_terminal(
        &self,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Vec<Div> {
        let shell_path =
            self.preferences.terminal_shell_path.as_deref().unwrap_or("(system default)");

        let shell_group = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.prefs_group_label("SHELL PATH", text_secondary))
            .child(
                div()
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(surface)
                    .text_sm()
                    .text_color(text)
                    .child(shell_path.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary)
                    .child("Edit ~/.config/baeus/preferences.json to change the shell path"),
            );

        vec![shell_group]
    }

    // --- About section ---
    fn render_prefs_about(&self, text: Rgba, text_secondary: Rgba) -> Vec<Div> {
        let version = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.prefs_group_label("VERSION", text_secondary))
            .child(
                div()
                    .text_sm()
                    .text_color(text)
                    .child(format!("Baeus {}", env!("CARGO_PKG_VERSION"))),
            );

        let config_path = dirs::config_dir()
            .map(|d| d.join("baeus").join("preferences.json").to_string_lossy().to_string())
            .unwrap_or_else(|| "(unknown)".to_string());

        let config_group = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.prefs_group_label("CONFIG FILE", text_secondary))
            .child(div().text_sm().text_color(text).child(config_path));

        vec![version, config_group]
    }

    // --- Save button ---
    fn render_prefs_save_button(
        &self,
        cx: &mut Context<Self>,
        text: Rgba,
        accent: Rgba,
        _border: Rgba,
    ) -> Div {
        let save_id = ElementId::Name(SharedString::from("prefs-save"));
        div().pt_4().child(
            div()
                .id(save_id)
                .px_4()
                .py_1p5()
                .rounded_md()
                .cursor_pointer()
                .bg(accent)
                .text_color(text)
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .hover(|el| el.opacity(0.9))
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.save_preferences();
                    cx.notify();
                }))
                .child("Save"),
        )
    }

    /// Persist current preferences to disk and apply theme.
    pub(crate) fn save_preferences(&mut self) {
        // Apply theme immediately
        self.theme = Theme::for_mode(self.preferences.theme_mode);
        self.layout.sidebar_collapsed = self.preferences.sidebar_collapsed;

        // Sync AWS profile fields to the runtime state used by handle_connect_cluster.
        self.default_aws_profile = self.preferences.default_aws_profile.clone();
        self.cluster_aws_profiles = self.preferences.cluster_aws_profiles.clone();

        // AWS profiles are injected into kubeconfig exec env at connection time;
        // no process-level env var modification needed.

        // Build a JSON value matching the UserPreferences structure and write it.
        let json = serde_json::json!({
            "theme": match self.preferences.theme_mode {
                ThemeMode::Light => "Light",
                ThemeMode::Dark => "Dark",
                ThemeMode::System => "System",
            },
            "default_namespace": self.preferences.default_namespace,
            "favorite_clusters": [],
            "keybindings": {},
            "log_line_limit": self.preferences.log_line_limit,
            "font_size": self.preferences.font_size,
            "sidebar_collapsed": self.preferences.sidebar_collapsed,
            "kubeconfig_scan_dirs": self.preferences.kubeconfig_scan_dirs,
            "terminal_shell_path": self.preferences.terminal_shell_path,
            "default_aws_profile": self.preferences.default_aws_profile,
            "cluster_aws_profiles": self.preferences.cluster_aws_profiles,
            "saved_eks_connections": self.preferences.saved_eks_connections,
        });

        if let Some(home) = dirs::home_dir() {
            let dir = home.join(".baeus");
            if std::fs::create_dir_all(&dir).is_ok() {
                let path = dir.join("preferences.json");
                match serde_json::to_string_pretty(&json) {
                    Ok(contents) => {
                        if let Err(e) = std::fs::write(&path, &contents) {
                            tracing::error!("Failed to save preferences: {e}");
                        } else {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let _ = std::fs::set_permissions(
                                    &path,
                                    std::fs::Permissions::from_mode(0o600),
                                );
                            }
                            tracing::info!("Preferences saved to {}", path.display());
                        }
                    }
                    Err(e) => tracing::error!("Failed to serialize preferences: {e}"),
                }
            }
        }

        // Re-discover clusters using updated scan dirs so sidebar reflects new paths.
        self.rediscover_clusters();
    }

    /// Re-run kubeconfig discovery with current preferences and update the sidebar.
    ///
    /// Adds any newly discovered clusters to the sidebar and kubeconfig_paths map
    /// without removing existing ones (to avoid disrupting active connections).
    fn rediscover_clusters(&mut self) {
        let scan_dirs: Vec<std::path::PathBuf> =
            self.preferences.kubeconfig_scan_dirs.iter().map(std::path::PathBuf::from).collect();
        let discovery =
            baeus_core::kubeconfig::KubeconfigDiscovery::new().with_additional_dirs(scan_dirs);

        let loaded = match discovery.load_all() {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Failed to rediscover kubeconfigs: {e}");
                return;
            }
        };

        let mut found = 0u32;
        for (path, loader) in &loaded {
            let path_str = path.to_string_lossy().to_string();
            for ctx in loader.contexts() {
                // Disambiguate duplicate context names the same way as initial discovery.
                let effective_name = if self.kubeconfig_paths.contains_key(&ctx.name) {
                    if ctx.cluster_name.is_empty() || ctx.cluster_name == ctx.name {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                        format!("{}@{}", ctx.name, stem)
                    } else {
                        format!("{}@{}", ctx.cluster_name, ctx.name)
                    }
                } else {
                    ctx.name.clone()
                };

                // Skip if we already have this effective name.
                if self.kubeconfig_paths.contains_key(&effective_name) {
                    continue;
                }

                let display_name = if ctx.cluster_name.is_empty()
                    || ctx.cluster_name == ctx.name
                    || ctx.cluster_name == effective_name
                {
                    effective_name.clone()
                } else {
                    format!("{}({})", ctx.cluster_name, effective_name)
                };
                self.sidebar.add_cluster(&effective_name, &display_name);
                self.kubeconfig_paths.insert(effective_name.clone(), path_str.clone());
                if effective_name != ctx.name {
                    self.original_context_names.insert(effective_name.clone(), ctx.name.clone());
                }

                // Register in the core cluster manager.
                let conn = ClusterConnection::new(
                    display_name,
                    effective_name,
                    String::new(),
                    AuthMethod::Token,
                );
                self.cluster_manager.add_connection(conn);
                found += 1;
            }
        }

        if found > 0 {
            tracing::info!("Rediscovery added {found} new cluster context(s)");
        }
    }

    /// Fetch the current AWS caller identity via `aws sts get-caller-identity`.
    /// Caches the result in `self.aws_caller_identity` and triggers a re-render.
    fn fetch_aws_caller_identity(&mut self, cx: &mut Context<Self>) {
        if self.aws_identity_loading {
            return;
        }
        self.aws_identity_loading = true;
        cx.notify();

        let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
        cx.spawn(async move |this, cx: &mut gpui::AsyncApp| {
            let result = tokio_handle
                .spawn(async move { baeus_core::aws_sso::get_caller_identity().await })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(identity)) => this.aws_caller_identity = Some(identity),
                    _ => this.aws_caller_identity = None,
                }
                this.aws_identity_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Open a dock terminal and run `aws sso login --profile <profile>`.
    pub(crate) fn run_aws_sso_login(&mut self, cx: &mut Context<Self>) {
        let profile = self.preferences.default_aws_profile.clone();
        self.run_aws_sso_login_with_optional_profile(profile.as_deref(), cx);
    }

    /// Open a dock terminal and run `aws sso login`, optionally with `--profile <name>`.
    pub(crate) fn run_aws_sso_login_with_optional_profile(
        &mut self,
        profile: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let label = profile.unwrap_or("default");
        let kind = DockTabKind::Terminal {
            pod: String::new(),
            container: String::new(),
            cluster: format!("aws-sso-{label}"),
        };
        let tab_id = self.dock.add_tab(kind);
        self.dock.select_tab(tab_id);
        self.dock.collapsed = false;
        // Take up half the window so the SSO output is impossible to miss.
        self.dock.height = 800.0;
        cx.notify();

        // Create a TerminalViewComponent entity.
        let state = TerminalViewState::for_local_shell();
        let theme = self.theme.clone();
        let entity = cx.new(|cx| TerminalViewComponent::new_with_cx(state, theme, cx));
        self.terminal_views.insert(tab_id, entity.clone());

        // Spawn a PTY process with enough rows to display SSO output.
        match PtyProcess::spawn_shell_with_env(40, 120, &[]) {
            Ok(pty) => {
                let output_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
                let reader = pty.reader_handle();

                let buf_clone = Arc::clone(&output_buf);
                std::thread::spawn(move || {
                    let mut tmp = [0u8; 4096];
                    loop {
                        let n = {
                            let mut r = match reader.lock() {
                                Ok(r) => r,
                                Err(_) => break,
                            };
                            match r.read(&mut tmp) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            }
                        };
                        if let Ok(mut buf) = buf_clone.lock() {
                            buf.extend_from_slice(&tmp[..n]);
                        }
                    }
                });

                let pty_arc = Arc::new(Mutex::new(pty));
                self.pty_processes.insert(tab_id, Arc::clone(&pty_arc));
                self.pty_output_buffers.insert(tab_id, Arc::clone(&output_buf));

                entity.update(cx, |view, _cx| {
                    view.state.connection_state =
                        crate::components::terminal_view::TerminalConnectionState::Connected;
                });

                // Write the aws sso login command after a short delay for the shell prompt.
                let cmd = match profile {
                    Some(p) => format!("aws sso login --profile {p}\n"),
                    None => "aws sso login\n".to_string(),
                };
                let writer = pty_arc.lock().ok().map(|p| p.writer_handle());
                if let Some(writer_handle) = writer {
                    let cmd_bytes = cmd.into_bytes();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        if let Ok(mut w) = writer_handle.lock() {
                            use std::io::Write;
                            let _ = w.write_all(&cmd_bytes);
                            let _ = w.flush();
                        }
                    });

                    // Poll loop: drain PTY output → terminal emulator, and keyboard → PTY.
                    // Also monitor for successful login to auto-close the tab.
                    let entity_weak = entity.downgrade();
                    let buf_for_poll = Arc::clone(&output_buf);
                    let writer_for_poll = pty_arc.lock().ok().map(|p| p.writer_handle());
                    cx.spawn(async move |this, cx: &mut gpui::AsyncApp| {
                        let mut accumulated_output = String::new();
                        loop {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(16))
                                .await;

                            // Drain output buffer and feed to emulator.
                            let data = {
                                let mut buf = match buf_for_poll.lock() {
                                    Ok(b) => b,
                                    Err(_) => break,
                                };
                                if buf.is_empty() { Vec::new() } else { std::mem::take(&mut *buf) }
                            };

                            // Track output for success detection.
                            if !data.is_empty() {
                                if let Ok(text) = std::str::from_utf8(&data) {
                                    accumulated_output.push_str(text);
                                    // Keep only the last 2KB to avoid unbounded growth.
                                    if accumulated_output.len() > 2048 {
                                        let start = accumulated_output.len() - 2048;
                                        accumulated_output =
                                            accumulated_output[start..].to_string();
                                    }
                                }
                            }

                            let alive = cx.update(|cx| {
                                entity_weak
                                    .update(cx, |view, cx| {
                                        if !data.is_empty() {
                                            view.process_output(&data);
                                        }
                                        // Drain pending keyboard input and write to PTY.
                                        let input = view.take_pending_input();
                                        if !input.is_empty() {
                                            if let Some(ref writer_h) = writer_for_poll {
                                                if let Ok(mut w) = writer_h.lock() {
                                                    let _ =
                                                        std::io::Write::write_all(&mut *w, &input);
                                                    let _ = std::io::Write::flush(&mut *w);
                                                }
                                            }
                                        }
                                        cx.notify();
                                    })
                                    .is_ok()
                            });

                            match alive {
                                Ok(true) => {}
                                _ => break,
                            }

                            // Check for successful login to auto-close.
                            let lower = accumulated_output.to_lowercase();
                            if lower.contains("successfully logged in")
                                || lower.contains("login successful")
                            {
                                // Wait a moment so the user can see the success message.
                                cx.background_executor()
                                    .timer(std::time::Duration::from_secs(3))
                                    .await;
                                // Close the dock tab.
                                let _ = this.update(cx, |this, cx| {
                                    this.dock.remove_tab(tab_id);
                                    this.terminal_views.remove(&tab_id);
                                    this.pty_processes.remove(&tab_id);
                                    this.pty_output_buffers.remove(&tab_id);
                                    // Refresh the caller identity after successful login.
                                    this.fetch_aws_caller_identity(cx);
                                    cx.notify();
                                });
                                break;
                            }
                        }
                    })
                    .detach();
                }
            }
            Err(e) => {
                tracing::error!("Failed to spawn PTY for AWS SSO login: {e}");
                entity.update(cx, |view, _cx| {
                    view.state.connection_state =
                        crate::components::terminal_view::TerminalConnectionState::Error(format!(
                            "Failed to spawn shell: {e}"
                        ));
                });
            }
        }

        cx.notify();
    }

    // -----------------------------------------------------------------------
    // T362: Error state rendering for all views
    // -----------------------------------------------------------------------

    /// T362: Render an error state panel with an error icon, message, and retry button.
    ///
    /// This is shown instead of the normal view content when `view_errors` contains
    /// an entry for the current NavigationTarget's error key.
    fn render_error_state(
        &self,
        cx: &mut Context<Self>,
        error_key: &str,
        error_message: &str,
        target: NavigationTarget,
        bg: Rgba,
    ) -> Div {
        let error_color = self.theme.colors.error.to_gpui();
        let text_color = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let surface_color = self.theme.colors.surface.to_gpui();
        let accent_color = self.theme.colors.accent.to_gpui();

        let error_msg_shared = SharedString::from(error_message.to_string());
        let error_key_owned = error_key.to_string();

        // Retry button element ID (unique per error key to avoid collisions)
        let retry_id = ElementId::Name(SharedString::from(format!("retry-btn-{}", error_key)));

        let retry_button = div()
            .id(retry_id)
            .px_4()
            .py_2()
            .rounded_md()
            .bg(accent_color)
            .text_color(gpui::rgb(0xFFFFFF))
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(|s| s.bg(gpui::rgb(0x2563EB)))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                // Clear the error and re-trigger the data fetch.
                this.view_errors.remove(&error_key_owned);
                this.retry_data_loading_for_target(&target, cx);
            }))
            .child("Retry");

        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .bg(bg)
            // Error icon: red circle with exclamation mark
            .child(
                div()
                    .w(px(48.0))
                    .h(px(48.0))
                    .rounded_full()
                    .bg(error_color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::rgb(0xFFFFFF))
                    .font_weight(FontWeight::BOLD)
                    .text_xl()
                    .child("!"),
            )
            // "Error" heading
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color)
                    .child("Something went wrong"),
            )
            // Error message in a surface-colored card
            .child(
                div()
                    .max_w(px(500.0))
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(surface_color)
                    .text_sm()
                    .text_color(text_secondary)
                    .overflow_hidden()
                    .child(error_msg_shared),
            )
            // Retry button
            .child(retry_button)
    }

    /// T362: Re-trigger data loading for a given NavigationTarget after clearing an error.
    ///
    /// This dispatches to the appropriate `start_*_loading` method based on the
    /// target variant, matching the logic in `trigger_data_loading_for_active_tab`.
    fn retry_data_loading_for_target(&mut self, target: &NavigationTarget, cx: &mut Context<Self>) {
        match target {
            NavigationTarget::Dashboard { cluster_context } => {
                self.start_dashboard_loading(cluster_context, cx);
            }
            NavigationTarget::ResourceList { cluster_context, kind, .. } => {
                self.start_resource_loading(cluster_context, kind, None, cx);
            }
            NavigationTarget::ResourceDetail { cluster_context, kind, name, namespace } => {
                self.start_resource_detail_loading(
                    cluster_context,
                    kind,
                    name,
                    namespace.as_deref(),
                    cx,
                );
            }
            _ => {
                // Other view types don't have data loading yet.
                tracing::debug!("T362: No retry action for target: {:?}", target);
            }
        }
    }

    /// T362: Public accessor to check if there is a view error for a given key.
    #[allow(dead_code)]
    pub fn get_view_error(&self, key: &str) -> Option<&str> {
        self.view_errors.get(key).map(|s| s.as_str())
    }

    /// T362: Public accessor to set a view error (useful for testing).
    #[allow(dead_code)]
    pub fn set_view_error(&mut self, key: String, message: String) {
        self.view_errors.insert(key, message);
    }

    /// T362: Public accessor to clear a view error.
    #[allow(dead_code)]
    pub fn clear_view_error(&mut self, key: &str) {
        self.view_errors.remove(key);
    }

    /// T362: Clear all view errors (e.g., on cluster disconnect/reconnect).
    #[allow(dead_code)]
    pub fn clear_all_view_errors(&mut self) {
        self.view_errors.clear();
    }

    // -----------------------------------------------------------------------
    // T330: Content view rendering helpers
    // -----------------------------------------------------------------------

    fn render_dashboard_content(
        &self,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        // Check for connection error first.
        if let Some(error_msg) = self.connection_errors.get(cluster_context) {
            let error_color = self.theme.colors.error.to_gpui();
            let surface = self.theme.colors.surface.to_gpui();
            let border = self.theme.colors.border.to_gpui();
            let cluster_display = self.sidebar.display_name_for_context(cluster_context);

            return div()
                .flex_1()
                .flex()
                .flex_col()
                .bg(bg)
                .p_6()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child(SharedString::from(format!(
                            "Cluster Overview \u{2014} {}",
                            cluster_display,
                        ))),
                )
                .child(
                    div()
                        .p_4()
                        .rounded_lg()
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::CircleX)
                                        .small()
                                        .text_color(error_color),
                                )
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(error_color)
                                        .child("Connection Failed"),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(text)
                                .child(SharedString::from(error_msg.clone())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_secondary)
                                .child("Check your kubeconfig, network connectivity, and AWS credentials. You can retry by clicking Connect in the sidebar."),
                        ),
                );
        }

        use crate::components::donut_chart::{
            DonutChart, ResourceDistEntry, ResourceDistributionBar, resource_kind_color,
        };

        if let Some(state) = &self.dashboard_state {
            if self.active_dashboard_cluster.as_deref() == Some(cluster_context) {
                // Use resource counts from dashboard API data (not lazy resource_list_data).
                let rc = &state.resource_counts;

                let bg_color = self.theme.colors.background;
                let surface = self.theme.colors.surface.to_gpui();
                let border = self.theme.colors.border.to_gpui();

                let scroll_id = ElementId::Name(SharedString::from(format!(
                    "dashboard-scroll-{}",
                    cluster_context
                )));

                let mut inner = div().flex().flex_col().p_4().gap_4();

                // Header: Cluster Overview
                let cluster_display = self.sidebar.display_name_for_context(cluster_context);
                inner = inner.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text)
                                .child(format!("Cluster Overview \u{2014} {}", cluster_display)),
                        )
                        .child(div().flex_grow())
                        .child(div().text_sm().text_color(text_secondary).child(format!(
                            "Nodes: {}  |  Namespaces: {}",
                            state.node_count,
                            state.namespaces.len()
                        ))),
                );

                // Resource usage donut row
                let cpu_value = state.cpu_usage_percent.map(|p| p / 100.0).unwrap_or(0.0);
                let mem_value = state.memory_usage_percent.map(|p| p / 100.0).unwrap_or(0.0);
                let pod_total = state.pod_summary.total;
                let pod_running = state.pod_summary.running;
                let pod_value =
                    if pod_total > 0 { pod_running as f32 / pod_total as f32 } else { 0.0 };

                let cpu_used_label =
                    state.cpu_used.map(|v| format!("{:.1} cores", v)).unwrap_or_else(|| {
                        if state.cpu_capacity.is_some() {
                            "No metrics".to_string()
                        } else {
                            "N/A".to_string()
                        }
                    });
                let cpu_total_label = state
                    .cpu_capacity
                    .map(|v| format!("{:.0} cores", v))
                    .unwrap_or_else(|| "N/A".to_string());
                let mem_used_label = state
                    .memory_used
                    .map(|v| format!("{:.1} GiB", v / 1_073_741_824.0))
                    .unwrap_or_else(|| {
                        if state.memory_capacity.is_some() {
                            "No metrics".to_string()
                        } else {
                            "N/A".to_string()
                        }
                    });
                let mem_total_label = state
                    .memory_capacity
                    .map(|v| format!("{:.1} GiB", v / 1_073_741_824.0))
                    .unwrap_or_else(|| "N/A".to_string());

                let cpu_donut = DonutChart {
                    label: "CPU",
                    value: cpu_value,
                    used_label: cpu_used_label,
                    total_label: cpu_total_label,
                    color: self.theme.colors.accent,
                    bg: bg_color,
                };
                let mem_donut = DonutChart {
                    label: "Memory",
                    value: mem_value,
                    used_label: mem_used_label,
                    total_label: mem_total_label,
                    color: self.theme.colors.info,
                    bg: bg_color,
                };
                let pod_donut = DonutChart {
                    label: "Pods",
                    value: pod_value,
                    used_label: format!("{}/{}", pod_running, pod_total),
                    total_label: format!("{} total", pod_total),
                    color: self.theme.colors.success,
                    bg: bg_color,
                };

                inner = inner.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_center()
                        .gap_8()
                        .py_4()
                        .bg(surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(border)
                        .child(cpu_donut.render(text, text_secondary))
                        .child(mem_donut.render(text, text_secondary))
                        .child(pod_donut.render(text, text_secondary)),
                );

                // Resource distribution bar (stacked horizontal bar with legend)
                let dist_entries = vec![
                    ResourceDistEntry {
                        kind: "Pods",
                        count: rc.pods,
                        color: resource_kind_color("Pods"),
                    },
                    ResourceDistEntry {
                        kind: "Deployments",
                        count: rc.deployments,
                        color: resource_kind_color("Deployments"),
                    },
                    ResourceDistEntry {
                        kind: "DaemonSets",
                        count: rc.daemonsets,
                        color: resource_kind_color("DaemonSets"),
                    },
                    ResourceDistEntry {
                        kind: "StatefulSets",
                        count: rc.statefulsets,
                        color: resource_kind_color("StatefulSets"),
                    },
                    ResourceDistEntry {
                        kind: "ReplicaSets",
                        count: rc.replicasets,
                        color: resource_kind_color("ReplicaSets"),
                    },
                    ResourceDistEntry {
                        kind: "Jobs",
                        count: rc.jobs,
                        color: resource_kind_color("Jobs"),
                    },
                    ResourceDistEntry {
                        kind: "CronJobs",
                        count: rc.cronjobs,
                        color: resource_kind_color("CronJobs"),
                    },
                ];

                inner = inner.child(
                    div().bg(surface).rounded_lg().border_1().border_color(border).p_3().child(
                        ResourceDistributionBar::render(&dist_entries, text, text_secondary, bg),
                    ),
                );

                // Pod summary stats
                inner = inner.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .py_2()
                        .bg(surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(border)
                        .px_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .flex_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(self.theme.colors.success.to_gpui())
                                        .child(state.pod_summary.running.to_string()),
                                )
                                .child(div().text_xs().text_color(text_secondary).child("Running")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .flex_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(self.theme.colors.warning.to_gpui())
                                        .child(state.pod_summary.pending.to_string()),
                                )
                                .child(div().text_xs().text_color(text_secondary).child("Pending")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .flex_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(self.theme.colors.error.to_gpui())
                                        .child(state.pod_summary.failed.to_string()),
                                )
                                .child(div().text_xs().text_color(text_secondary).child("Failed")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .flex_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(text)
                                        .child(state.pod_summary.succeeded.to_string()),
                                )
                                .child(
                                    div().text_xs().text_color(text_secondary).child("Succeeded"),
                                ),
                        ),
                );

                // Events table (compact layout)
                if !state.recent_events.is_empty() {
                    let total_events = state.recent_events.len();
                    let display_count = total_events.min(10);
                    let header_text = format!("Events ({} of {})", display_count, total_events);

                    let mut events_section = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .bg(surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(border)
                        .p_3();

                    events_section = events_section.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .pb_2()
                            .child(header_text),
                    );

                    // Column header row
                    events_section = events_section
                        .child(self.render_dashboard_events_header(text_secondary, border));

                    let warning_color = self.theme.colors.warning.to_gpui();
                    for event in state.recent_events.iter().take(10) {
                        events_section = events_section.child(self.render_dashboard_event_row(
                            event,
                            text,
                            text_secondary,
                            border,
                            warning_color,
                        ));
                    }

                    inner = inner.child(events_section);
                }

                // Workloads Summary section
                {
                    let workload_kinds = [
                        "Deployment",
                        "StatefulSet",
                        "DaemonSet",
                        "ReplicaSet",
                        "Job",
                        "CronJob",
                        "Pod",
                    ];
                    let mut workload_section =
                        div().flex().flex_col().p_4().gap_2().bg(surface).rounded(px(8.0));

                    workload_section = workload_section.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text)
                            .child("Workloads Summary"),
                    );

                    for wk in &workload_kinds {
                        let list_key = ResourceListKey {
                            cluster_context: cluster_context.to_string(),
                            kind: wk.to_string(),
                            namespace: None,
                        };
                        let count = self
                            .resource_list_data
                            .get(&list_key)
                            .map(|items| items.len())
                            .unwrap_or(0);

                        // Count "ready" items for a basic status bar
                        let ready = self
                            .resource_list_data
                            .get(&list_key)
                            .map(|items| {
                                items
                                    .iter()
                                    .filter(|item| {
                                        // For Pods: phase == Running
                                        if *wk == "Pod" {
                                            return item
                                                .pointer("/status/phase")
                                                .and_then(|v| v.as_str())
                                                == Some("Running");
                                        }
                                        // For Deployments/StatefulSets/etc: readyReplicas == replicas
                                        let desired = item
                                            .pointer("/spec/replicas")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);
                                        let ready = item
                                            .pointer("/status/readyReplicas")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);
                                        if desired > 0 { ready >= desired } else { true }
                                    })
                                    .count()
                            })
                            .unwrap_or(0);

                        let bar_fraction =
                            if count > 0 { ready as f32 / count as f32 } else { 0.0 };

                        let bar_color = if bar_fraction >= 1.0 {
                            self.theme.colors.success.to_gpui()
                        } else if bar_fraction > 0.5 {
                            self.theme.colors.warning.to_gpui()
                        } else if count > 0 {
                            self.theme.colors.error.to_gpui()
                        } else {
                            text_secondary
                        };

                        workload_section = workload_section.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(100.0))
                                        .text_xs()
                                        .text_color(text)
                                        .child(SharedString::from(wk.to_string())),
                                )
                                .child(
                                    div()
                                        .w(px(60.0))
                                        .text_xs()
                                        .text_color(text_secondary)
                                        .child(SharedString::from(format!("{ready}/{count}"))),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(6.0))
                                        .rounded(px(3.0))
                                        .bg(border)
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .h_full()
                                                .rounded(px(3.0))
                                                .bg(bar_color)
                                                .w(gpui::relative(bar_fraction)),
                                        ),
                                ),
                        );
                    }

                    inner = inner.child(workload_section);
                }

                return div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .bg(bg)
                    .overflow_hidden()
                    .child(div().id(scroll_id).flex_1().overflow_y_scroll().child(inner));
            }
        }

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .bg(bg)
            .text_color(text_secondary)
            .text_sm()
            .child(format!("Loading dashboard for {}...", cluster_context))
    }

    fn render_resource_list_content(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        kind: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            namespace: None,
        };

        if let Some(table_state) = self.resource_table_states.get(&key) {
            let border = self.theme.colors.border.to_gpui();
            let surface = self.theme.colors.surface.to_gpui();
            let selection = self.theme.colors.selection.to_gpui();
            let accent = self.theme.colors.accent.to_gpui();

            // Filter rows by namespace selector
            let ns_selector = self.namespace_selectors.get(cluster_context);
            let filter_query = table_state.filter_text.to_lowercase();
            let filtered_rows: Vec<&TableRow> = table_state
                .rows
                .iter()
                .filter(|row| {
                    // Namespace filter
                    let ns_match = match (&row.namespace, ns_selector) {
                        (Some(ns), Some(sel)) => sel.matches_namespace(ns),
                        _ => true,
                    };
                    if !ns_match {
                        return false;
                    }
                    // Text search filter
                    if filter_query.is_empty() {
                        return true;
                    }
                    if row.name.to_lowercase().contains(&filter_query) {
                        return true;
                    }
                    if let Some(ns) = &row.namespace {
                        if ns.to_lowercase().contains(&filter_query) {
                            return true;
                        }
                    }
                    row.cells.iter().any(|cell| cell.to_lowercase().contains(&filter_query))
                })
                .collect();
            let count = filtered_rows.len();

            let mut content =
                div().flex_1().min_h(px(0.0)).flex().flex_col().bg(bg).overflow_hidden();

            // Header bar with kind name + count badge + namespace dropdown
            content = content.child(self.render_resource_list_header(
                cx,
                cluster_context,
                kind,
                count,
                text,
                text_secondary,
                surface,
                border,
            ));

            // Search / filter bar
            content = content.child(self.render_resource_filter_bar(
                cx,
                &key,
                text,
                text_secondary,
                surface,
                border,
            ));

            // Column headers
            content = content.child(self.render_resource_table_headers(
                cx,
                &table_state.columns,
                cluster_context,
                kind,
                text_secondary,
                surface,
                border,
            ));

            // Table body with namespace-filtered rows
            content = content.child(self.render_resource_table_body_filtered(
                cx,
                &filtered_rows,
                &table_state.columns,
                cluster_context,
                text,
                text_secondary,
                bg,
                border,
                selection,
                accent,
            ));

            // Context menu popup overlay (if a row "..." is active)
            if let Some(&menu_row_idx) = self.context_menu_row.get(&key) {
                if let Some(row) = filtered_rows.get(menu_row_idx) {
                    content = content.child(self.render_row_context_menu(
                        cx,
                        cluster_context,
                        &row.kind,
                        &row.name,
                        row.namespace.as_deref(),
                        menu_row_idx,
                        text,
                        text_secondary,
                        surface,
                        border,
                    ));
                }
            }

            content
        } else if self.resource_list_data.contains_key(&key) {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child(format!("Building table for {}...", kind))
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child(format!("Loading {} list for {}...", kind, cluster_context))
        }
    }

    /// Render the header bar for a resource list: kind name + count badge + namespace dropdown.
    #[allow(clippy::too_many_arguments)]
    fn render_resource_list_header(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        kind: &str,
        count: usize,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        let badge = SharedString::from(count.to_string());
        let kind_label = SharedString::from(format!("{}s", kind));

        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(
                div().font_weight(FontWeight::BOLD).text_color(text).text_base().child(kind_label),
            )
            .child(
                div()
                    .text_xs()
                    .px_2()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(gpui::rgb(0x374151))
                    .text_color(text_secondary)
                    .child(badge),
            )
            // Spacer pushes namespace dropdown to the right
            .child(div().flex_grow());

        // Namespace dropdown button + panel
        if let Some(ns_sel) = self.namespace_selectors.get(cluster_context) {
            let display = SharedString::from(ns_sel.display_label());
            let arrow: SharedString =
                if ns_sel.is_dropdown_open { "\u{25B2}".into() } else { "\u{25BC}".into() };
            let ctx = cluster_context.to_string();

            let button = div()
                .id(ElementId::Name(SharedString::from(format!("ns-dropdown-btn-{}", kind))))
                .flex()
                .items_center()
                .gap(px(4.0))
                .px_3()
                .py_1()
                .rounded(px(6.0))
                .bg(gpui::rgb(0x374151))
                .text_sm()
                .text_color(gpui::rgb(0xD1D5DB))
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0x4B5563)))
                .on_click(cx.listener(move |this, _evt, window, cx| {
                    if let Some(sel) = this.namespace_selectors.get_mut(&ctx) {
                        sel.toggle_dropdown();
                        if sel.is_dropdown_open {
                            // Create a fresh InputState for the search box
                            let ctx_for_sub = ctx.clone();
                            let input = cx.new(|cx| {
                                InputState::new(window, cx).placeholder("Filter namespaces...")
                            });
                            let sub = cx.subscribe(
                                &input,
                                move |this: &mut AppShell, entity, event: &InputEvent, cx| {
                                    if matches!(event, InputEvent::Change) {
                                        let val = entity.read(cx).value().to_string();
                                        if let Some(sel) =
                                            this.namespace_selectors.get_mut(&ctx_for_sub)
                                        {
                                            sel.search_query = val;
                                        }
                                        cx.notify();
                                    }
                                },
                            );
                            // Focus the input
                            let fh = input.read(cx).focus_handle(cx);
                            fh.focus(window);
                            this.ns_search_input = Some(input);
                            this._ns_search_subscription = Some(sub);
                        } else {
                            // Closing: clear search and drop the input entity
                            sel.search_query.clear();
                            this.ns_search_input = None;
                            this._ns_search_subscription = None;
                        }
                    }
                }))
                .child(display)
                .child(div().text_xs().text_color(gpui::rgb(0x9CA3AF)).child(arrow));

            let wrapper = div().relative().child(button);

            header = header.child(wrapper);
        }

        header
    }

    /// Render the search/filter bar for a resource list.
    #[allow(clippy::too_many_arguments)]
    fn render_resource_filter_bar(
        &self,
        cx: &mut Context<Self>,
        key: &ResourceListKey,
        _text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        let filter_input = self.resource_filter_inputs.get(key).cloned();
        let key_for_create = key.clone();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_1()
            .bg(surface)
            .border_b_1()
            .border_color(border);

        // Search icon label
        bar = bar.child(
            div().text_xs().text_color(text_secondary).child(SharedString::from("\u{1F50D}")),
        );

        if let Some(input_entity) = filter_input {
            bar = bar.child(div().flex_1().child(Input::new(&input_entity).text_sm().small()));
        } else {
            // Create the input on first render via an on_click on the placeholder
            bar = bar.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "filter-placeholder-{}-{}",
                        key_for_create.cluster_context, key_for_create.kind
                    ))))
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(text_secondary)
                    .cursor_pointer()
                    .child(SharedString::from("Search resources..."))
                    .on_click(cx.listener(move |this, _evt, window, cx| {
                        let k = key_for_create.clone();
                        let k2 = k.clone();
                        let input = cx.new(|cx| {
                            InputState::new(window, cx).placeholder("Search resources...")
                        });
                        let sub = cx.subscribe(
                            &input,
                            move |this: &mut AppShell, entity, event: &InputEvent, cx| {
                                if matches!(event, InputEvent::Change) {
                                    let val = entity.read(cx).value().to_string();
                                    if let Some(ts) = this.resource_table_states.get_mut(&k2) {
                                        ts.filter_text = val;
                                    }
                                    cx.notify();
                                }
                            },
                        );
                        let fh = input.read(cx).focus_handle(cx);
                        fh.focus(window);
                        this.resource_filter_inputs.insert(k.clone(), input);
                        this._resource_filter_subscriptions.insert(k, sub);
                    })),
            );
        }

        bar
    }

    /// Render the namespace dropdown as a root-level overlay (if any dropdown is open).
    /// Returns an Option so it can be used with `.children()`.
    fn render_namespace_dropdown_overlay(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (ctx, ns_sel) = self.namespace_selectors.iter().find(|(_, s)| s.is_dropdown_open)?;

        // Position the dropdown at the top-right of the content area.
        // header(48) + tab_bar(36) + resource_list_header(~40) = ~124px from top
        let top_offset = px(126.0);

        Some(
            self.render_namespace_dropdown_panel(cx, ctx, ns_sel)
                .absolute()
                .top(top_offset)
                .right(px(16.0)),
        )
    }

    /// Render the namespace dropdown panel (search + "All" row + checkbox rows).
    fn render_namespace_dropdown_panel(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        ns_sel: &EnhancedNamespaceSelector,
    ) -> Stateful<Div> {
        let filtered = ns_sel.filtered_namespaces();
        let ctx_all = cluster_context.to_string();
        let ctx_dismiss = cluster_context.to_string();

        let mut panel = div()
            .id(ElementId::Name(SharedString::from(format!(
                "ns-dropdown-panel-{}",
                cluster_context
            ))))
            .occlude()
            .w(px(260.0))
            .max_h(px(360.0))
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x1F2937))
            .border_1()
            .border_color(gpui::rgb(0x4B5563))
            .rounded(px(6.0))
            .shadow_lg()
            .overflow_hidden();

        // Search input using gpui_component::Input for proper text entry
        if let Some(input_entity) = &self.ns_search_input {
            panel = panel.child(
                div().border_b_1().border_color(gpui::rgb(0x4B5563)).px_1().py_1().child(
                    Input::new(input_entity)
                        .prefix(Icon::new(IconName::Search).size(px(14.0)))
                        .cleanable(true)
                        .with_size(gpui_component::Size::Small),
                ),
            );
        }

        // "All namespaces" row
        let all_selected = ns_sel.selected_namespaces.is_empty();
        panel = panel.child(
            div()
                .id(ElementId::Name(SharedString::from("ns-row-all")))
                .flex()
                .items_center()
                .gap(px(8.0))
                .w_full()
                .px_3()
                .py(px(6.0))
                .border_b_1()
                .border_color(gpui::rgb(0x374151))
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0x374151)))
                .when(all_selected, |el| el.bg(Rgba { r: 0.118, g: 0.227, b: 0.373, a: 0.3 }))
                .on_click(cx.listener(move |this, _evt, _window, _cx| {
                    if let Some(sel) = this.namespace_selectors.get_mut(&ctx_all) {
                        sel.clear_selection();
                    }
                }))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::rgb(0xD1D5DB))
                        .font_weight(FontWeight::MEDIUM)
                        .child("All Namespaces"),
                )
                .child(div().flex_grow())
                .when(all_selected, |el| {
                    el.child(
                        div()
                            .text_color(gpui::rgb(0x60A5FA))
                            .child(Icon::new(IconName::Check).size(px(14.0))),
                    )
                }),
        );

        // Scrollable namespace rows
        let mut rows_container = div()
            .id(ElementId::Name(SharedString::from("ns-rows-scroll")))
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .max_h(px(260.0));

        for (idx, ns) in filtered.iter().enumerate() {
            let ns_string = ns.to_string();
            let ctx_toggle = cluster_context.to_string();
            let is_selected = ns_sel.is_namespace_selected(ns);
            let ns_label = SharedString::from(ns_string.clone());
            let row_id = ElementId::Name(SharedString::from(format!("ns-row-{idx}")));

            let row = div()
                .id(row_id)
                .flex()
                .items_center()
                .gap(px(8.0))
                .w_full()
                .px_3()
                .py(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0x374151)))
                .on_click(cx.listener(move |this, _evt, _window, _cx| {
                    if let Some(sel) = this.namespace_selectors.get_mut(&ctx_toggle) {
                        sel.toggle_namespace(&ns_string);
                    }
                }))
                .child(
                    div()
                        .flex_none()
                        .text_color(gpui::rgb(0x6B7280))
                        .child(Icon::new(IconName::Folder).size(px(14.0))),
                )
                .child(div().flex_1().text_sm().text_color(gpui::rgb(0xD1D5DB)).child(ns_label))
                .when(is_selected, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .text_color(gpui::rgb(0x60A5FA))
                            .child(Icon::new(IconName::Check).size(px(14.0))),
                    )
                });

            rows_container = rows_container.child(row);
        }

        panel = panel.child(rows_container);
        panel.on_mouse_down_out(cx.listener(move |this, _evt: &MouseDownEvent, _window, _cx| {
            if let Some(sel) = this.namespace_selectors.get_mut(&ctx_dismiss) {
                sel.is_dropdown_open = false;
                sel.search_query.clear();
            }
            this.ns_search_input = None;
            this._ns_search_subscription = None;
        }))
    }

    /// Render the table body from a pre-filtered slice of rows.
    #[allow(clippy::too_many_arguments)]
    fn render_resource_table_body_filtered(
        &self,
        cx: &mut Context<Self>,
        rows: &[&TableRow],
        columns: &[crate::components::resource_table::ColumnDef],
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
        border: Rgba,
        selection: Rgba,
        _accent: Rgba,
    ) -> Div {
        let table_id = ElementId::Name(SharedString::from(format!(
            "resource-table-body-{}",
            cluster_context,
        )));
        let mut inner = div()
            .id(table_id)
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .bg(bg);

        if rows.is_empty() {
            inner = inner.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("No resources found"),
            );
        } else {
            for (idx, row) in rows.iter().take(200).enumerate() {
                inner = inner.child(self.render_resource_table_row(
                    cx,
                    row,
                    idx,
                    cluster_context,
                    columns,
                    text,
                    bg,
                    border,
                    selection,
                ));
            }

            if rows.len() > 200 {
                inner =
                    inner.child(div().text_xs().text_color(text_secondary).px_4().py_2().child(
                        SharedString::from(format!("Showing 200 of {} resources", rows.len())),
                    ));
            }
        }

        // Wrap Stateful<Div> in a plain Div to match return type
        div().flex().flex_col().w_full().flex_1().min_h(px(0.0)).child(inner)
    }

    /// Create a base div for a table cell with a fixed pixel width.
    /// Used when column_widths are available from ResourceTableState.
    fn table_cell_px(width: f32) -> Div {
        div().px_2().flex_shrink_0().w(px(width)).overflow_hidden()
    }

    /// Create a base div for a table cell with proportional width based on column name.
    /// "Name" gets double flex, narrow metric columns get fixed widths, others get flex_1.
    /// Fallback when no column_widths are available.
    fn table_cell_base(col_label: &str, weight: f32) -> Div {
        let base = div().px_2();
        match col_label {
            "Name" => {
                let mut d = base.flex_shrink().flex_basis(px(0.0)).min_w(px(120.0));
                d.style().flex_grow = Some(2.0);
                d
            }
            "Namespace" | "Controlled By" | "Node" => base.flex_1().min_w(px(80.0)),
            "CPU" | "Memory" | "Restarts" | "QoS" | "Age" | "Status" => {
                base.flex_shrink_0().w(px(80.0))
            }
            "Containers" | "Ports" | "Type" => base.flex_shrink_0().w(px(90.0)),
            _ => base.flex_1().min_w(px(weight * 50.0)),
        }
    }

    /// Render column header row for the resource table.
    /// Sortable headers are clickable with direction arrows.
    /// Includes draggable resize handles between columns.
    #[allow(clippy::too_many_arguments)]
    fn render_resource_table_headers(
        &self,
        cx: &mut Context<Self>,
        columns: &[crate::components::resource_table::ColumnDef],
        cluster_context: &str,
        kind: &str,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        use crate::components::resource_table::SortDirection;

        let mut header =
            div().flex().flex_row().w_full().border_b_1().border_color(border).bg(surface).px_2();

        // Look up current sort state and column widths for this table
        let list_key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            namespace: None,
        };
        let table_state = self.resource_table_states.get(&list_key);
        let current_sort = table_state
            .and_then(|ts| ts.sort.as_ref())
            .map(|s| (s.column_id.as_str(), &s.direction));
        let column_widths: Vec<f32> =
            table_state.map(|ts| ts.column_widths.clone()).unwrap_or_default();

        for (col_idx, col) in columns.iter().enumerate() {
            // Determine sort indicator
            let sort_indicator = match current_sort {
                Some((id, dir)) if id == col.id => match dir {
                    SortDirection::Ascending => " \u{25B2}",  // ▲
                    SortDirection::Descending => " \u{25BC}", // ▼
                },
                _ => "",
            };

            let label_with_sort = SharedString::from(format!("{}{}", col.label, sort_indicator));

            // Use stored column width if available, otherwise fallback
            let col_width = column_widths.get(col_idx).copied();

            if col.sortable {
                let col_id = col.id.clone();
                let ctx = cluster_context.to_string();
                let k = kind.to_string();
                let header_id =
                    ElementId::Name(SharedString::from(format!("hdr-{}-{}-{}", ctx, k, col_idx)));
                let cell = if let Some(w) = col_width {
                    Self::table_cell_px(w)
                } else {
                    Self::table_cell_base(&col.label, col.width_weight)
                }
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .id(header_id)
                .cursor_pointer()
                .hover(|s| s.bg(border))
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    let key = ResourceListKey {
                        cluster_context: ctx.clone(),
                        kind: k.clone(),
                        namespace: None,
                    };
                    if let Some(ts) = this.resource_table_states.get_mut(&key) {
                        ts.sort_by(&col_id);
                    }
                }))
                .child(label_with_sort);
                header = header.child(cell);
            } else {
                let cell = if let Some(w) = col_width {
                    Self::table_cell_px(w)
                } else {
                    Self::table_cell_base(&col.label, col.width_weight)
                }
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(label_with_sort);
                header = header.child(cell);
            }

            // Resize handle between columns
            let current_width = col_width.unwrap_or(col.width_weight * 100.0);
            let drag_ctx = cluster_context.to_string();
            let drag_kind = kind.to_string();
            let handle_id = ElementId::Name(SharedString::from(format!(
                "col-resize-{}-{}-{}",
                drag_ctx, drag_kind, col_idx
            )));
            let resize_handle = div()
                .id(handle_id)
                .w(px(4.0))
                .h_full()
                .cursor_col_resize()
                .hover(|s| s.bg(border))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _window, _cx| {
                        this.is_dragging_column = true;
                        this.column_drag_index = Some(col_idx);
                        this.column_drag_start_x = event.position.x.into();
                        this.column_drag_start_width = current_width;
                        this.column_drag_table_key = Some(ResourceListKey {
                            cluster_context: drag_ctx.clone(),
                            kind: drag_kind.clone(),
                            namespace: None,
                        });
                    }),
                );
            header = header.child(resize_handle);
        }

        // Empty column header for the "..." dots column
        header = header.child(div().w(px(32.0)).flex_shrink_0());

        header
    }

    /// Render the dashboard events column header row (8-column layout).
    fn render_dashboard_events_header(&self, text_secondary: Rgba, border: Rgba) -> Div {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(text_secondary)
            .border_b_1()
            .border_color(border)
            .pb_1()
            .child(div().w(px(60.0)).child("Type"))
            .child(
                div().flex_grow().flex_basis(Pixels::ZERO).flex_shrink().min_w_0().child("Message"),
            )
            .child(div().w(px(80.0)).child("Namespace"))
            .child(
                div()
                    .flex_grow()
                    .flex_basis(Pixels::ZERO)
                    .flex_shrink()
                    .min_w_0()
                    .child("Involved Object"),
            )
            .child(div().w(px(90.0)).child("Source"))
            .child(div().w(px(40.0)).child("Count"))
            .child(div().w(px(50.0)).child("Age"))
            .child(div().w(px(60.0)).child("Last Seen"))
    }

    /// Render a single dashboard event row (8-column layout).
    fn render_dashboard_event_row(
        &self,
        event: &DashboardEvent,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        warning_color: Rgba,
    ) -> Div {
        use crate::views::dashboard::human_age_from_datetime;

        let type_label = if event.is_warning { "Warning" } else { "Normal" };
        let type_color = if event.is_warning { warning_color } else { text_secondary };

        let age_str = human_age_from_datetime(event.timestamp);
        let last_seen_str =
            event.last_seen.map(human_age_from_datetime).unwrap_or_else(|| "—".to_string());

        let involved = event.involved_object_display();
        let ns = event.namespace.clone().unwrap_or_default();
        let source = event.source.clone().unwrap_or_else(|| "—".to_string());

        let mut row =
            div().flex().flex_row().gap_2().text_xs().py(px(2.0)).border_b_1().border_color(border);

        // Subtle amber left border accent for warnings
        if event.is_warning {
            let w = warning_color;
            row = row.border_l_2().border_color(Rgba { r: w.r, g: w.g, b: w.b, a: 0.5 });
        }

        row.child(div().w(px(60.0)).text_color(type_color).child(type_label.to_string()))
            .child(
                div()
                    .flex_grow()
                    .flex_basis(Pixels::ZERO)
                    .flex_shrink()
                    .min_w_0()
                    .text_color(text)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(SharedString::from(event.message.clone())),
            )
            .child(
                div()
                    .w(px(80.0))
                    .text_color(text_secondary)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(SharedString::from(ns)),
            )
            .child(
                div()
                    .flex_grow()
                    .flex_basis(Pixels::ZERO)
                    .flex_shrink()
                    .min_w_0()
                    .text_color(text_secondary)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(SharedString::from(involved)),
            )
            .child(
                div()
                    .w(px(90.0))
                    .text_color(text_secondary)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(SharedString::from(source)),
            )
            .child(
                div()
                    .w(px(40.0))
                    .text_color(text_secondary)
                    .child(SharedString::from(event.count.to_string())),
            )
            .child(div().w(px(50.0)).text_color(text_secondary).child(SharedString::from(age_str)))
            .child(
                div()
                    .w(px(60.0))
                    .text_color(text_secondary)
                    .child(SharedString::from(last_seen_str)),
            )
    }

    /// Clicking a row opens a ResourceDetail tab for that resource.
    #[allow(clippy::too_many_arguments)]
    fn render_resource_table_row(
        &self,
        cx: &mut Context<Self>,
        row: &TableRow,
        idx: usize,
        cluster_context: &str,
        columns: &[crate::components::resource_table::ColumnDef],
        text: Rgba,
        bg: Rgba,
        border: Rgba,
        selection: Rgba,
    ) -> Stateful<Div> {
        let row_bg = if idx % 2 == 0 { bg } else { self.theme.colors.table_stripe.to_gpui() };

        // Tint Warning event rows with subtle amber background
        let row_bg = if row.kind == "Event" {
            let is_warning = row.cells.first().map(|c| c == "Warning").unwrap_or(false);
            if is_warning {
                let w = self.theme.colors.warning.to_gpui();
                Rgba { r: w.r, g: w.g, b: w.b, a: 0.08 }
            } else {
                row_bg
            }
        } else {
            row_bg
        };

        let row_id = ElementId::Name(SharedString::from(format!(
            "tbl-row-{}-{}-{}",
            cluster_context, row.kind, idx
        )));

        let kind_for_click = row.kind.clone();
        let name_for_click = row.name.clone();
        let ns_for_click = row.namespace.clone();
        let ctx_for_click = cluster_context.to_string();

        // Look up column widths for this table
        let list_key = ResourceListKey {
            cluster_context: cluster_context.to_string(),
            kind: row.kind.clone(),
            namespace: None,
        };
        let column_widths: Vec<f32> = self
            .resource_table_states
            .get(&list_key)
            .map(|ts| ts.column_widths.clone())
            .unwrap_or_default();

        let mut row_div = div()
            .id(row_id)
            .flex()
            .flex_row()
            .w_full()
            .items_center()
            .bg(row_bg)
            .border_b_1()
            .border_color(border)
            .cursor_pointer()
            .px_2()
            .hover(|s| s.bg(selection))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                let target = NavigationTarget::ResourceDetail {
                    cluster_context: ctx_for_click.clone(),
                    kind: kind_for_click.clone(),
                    name: name_for_click.clone(),
                    namespace: ns_for_click.clone(),
                };
                this.workspace.open_tab(target);
                this.sync_navigator_to_active_tab();
                this.trigger_data_loading_for_active_tab(cx);
            }));

        // Look up metrics override for CPU/Memory columns
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let metrics_data = self.get_metrics_for_row(
            cluster_context,
            &row.kind,
            &row.name,
            row.namespace.as_deref(),
        );

        // Render cells matching column order
        for (col_idx, cell_val) in row.cells.iter().enumerate() {
            let (col_id, col_label, col_weight) = columns
                .get(col_idx)
                .map(|c| (c.id.as_str(), c.label.as_str(), c.width_weight))
                .unwrap_or(("", "", 1.0));
            let col_width = column_widths.get(col_idx).copied();

            // Phase 3: Container bricks for Pod "containers" column
            if col_id == "containers" && !row.container_statuses.is_empty() {
                let brick_cell = self.render_container_bricks_cell(
                    &row.container_statuses,
                    cell_val,
                    col_label,
                    col_weight,
                    col_width,
                );
                row_div = row_div.child(brick_cell);
                continue;
            }

            // Phase 4: Condition badges for "conditions" columns
            if col_id == "conditions" && !row.conditions.is_empty() {
                let cond_cell =
                    self.render_conditions_cell(&row.conditions, col_label, col_weight, col_width);
                row_div = row_div.child(cond_cell);
                continue;
            }

            // Metrics bar for CPU/Memory columns
            if let Some(ref md) = metrics_data {
                if col_id == "cpu" || col_id == "memory" {
                    let (display_text, usage_pct) = if col_id == "cpu" {
                        (&md.cpu_display, md.cpu_percent)
                    } else {
                        (&md.mem_display, md.mem_percent)
                    };
                    let bar_cell = self.render_metrics_bar_cell(
                        usage_pct,
                        display_text,
                        col_label,
                        col_weight,
                        col_width,
                        text_secondary,
                    );
                    row_div = row_div.child(bar_cell);
                    continue;
                }
            }

            // Phase 1: Status/Pods cell coloring + Event type coloring
            let cell_color = if col_id == "status" {
                json_extract::status_color(cell_val, &self.theme).unwrap_or(text)
            } else if col_id == "pods" || col_id == "containers" {
                json_extract::pods_color(cell_val, &self.theme).unwrap_or(text)
            } else if col_id == "type" && row.kind == "Event" {
                if cell_val == "Warning" {
                    self.theme.colors.warning.to_gpui()
                } else {
                    text_secondary
                }
            } else {
                text
            };

            let cell_text = SharedString::from(cell_val.clone());
            let cell = if let Some(w) = col_width {
                Self::table_cell_px(w)
            } else {
                Self::table_cell_base(col_label, col_weight)
            }
            .py_1()
            .text_sm()
            .text_color(cell_color)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(cell_text);
            row_div = row_div.child(cell);
        }

        // "..." dots button for context menu
        let dots_id = ElementId::Name(SharedString::from(format!(
            "tbl-dots-{}-{}-{}",
            cluster_context, row.kind, idx
        )));
        let dots_ctx = cluster_context.to_string();
        let dots_kind = row.kind.clone();
        let dots_idx = idx;
        row_div = row_div.child(
            div()
                .id(dots_id)
                .w(px(32.0))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .items_center()
                .cursor_pointer()
                .text_sm()
                .text_color(text)
                .hover(|s| s.text_color(gpui::rgb(0xFFFFFF)))
                .child("\u{22EF}") // ⋯ horizontal ellipsis
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    let key = ResourceListKey {
                        cluster_context: dots_ctx.clone(),
                        kind: dots_kind.clone(),
                        namespace: None,
                    };
                    // Toggle: if already open for this row, close it
                    if this.context_menu_row.get(&key) == Some(&dots_idx) {
                        this.context_menu_row.remove(&key);
                    } else {
                        this.context_menu_row.insert(key, dots_idx);
                    }
                    cx.notify();
                })),
        );

        row_div
    }

    /// Render a context menu popup for a table row.
    #[allow(clippy::too_many_arguments)]
    fn render_row_context_menu(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
        _row_idx: usize,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        let ctx = cluster_context.to_string();
        let k = kind.to_string();
        let n = name.to_string();
        let ns = namespace.map(|s| s.to_string());

        let is_scalable = matches!(kind, "Deployment" | "StatefulSet" | "ReplicaSet");
        let is_pod = kind == "Pod";

        let dismiss_ctx = ctx.clone();
        let dismiss_kind = k.clone();

        // Click-away dismiss backdrop
        let backdrop =
            div().id("ctx-menu-backdrop").absolute().top_0().left_0().w_full().h_full().on_click(
                cx.listener(move |this, _event, _window, cx| {
                    let key = ResourceListKey {
                        cluster_context: dismiss_ctx.clone(),
                        kind: dismiss_kind.clone(),
                        namespace: None,
                    };
                    this.context_menu_row.remove(&key);
                    cx.notify();
                }),
            );

        // Menu items
        let mut menu = div()
            .absolute()
            .right(px(8.0))
            .top(px(40.0))
            .w(px(180.0))
            .bg(surface)
            .rounded(px(6.0))
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .flex()
            .flex_col();

        // "Edit YAML" item
        let edit_ctx = ctx.clone();
        let edit_kind = k.clone();
        let edit_name = n.clone();
        let edit_ns = ns.clone();
        let dismiss_ctx2 = ctx.clone();
        let dismiss_kind2 = k.clone();
        menu = menu.child(
            div()
                .id("ctx-edit-yaml")
                .px_3()
                .py_2()
                .cursor_pointer()
                .text_sm()
                .text_color(text)
                .hover(|s| s.bg(gpui::rgb(0x1F2937)))
                .child("Edit YAML")
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    // Close menu
                    let menu_key = ResourceListKey {
                        cluster_context: dismiss_ctx2.clone(),
                        kind: dismiss_kind2.clone(),
                        namespace: None,
                    };
                    this.context_menu_row.remove(&menu_key);
                    // Navigate to detail + YAML tab
                    let target = NavigationTarget::ResourceDetail {
                        cluster_context: edit_ctx.clone(),
                        kind: edit_kind.clone(),
                        name: edit_name.clone(),
                        namespace: edit_ns.clone(),
                    };
                    this.workspace.open_tab(target);
                    let detail_key = ResourceDetailKey {
                        cluster_context: edit_ctx.clone(),
                        kind: edit_kind.clone(),
                        name: edit_name.clone(),
                        namespace: edit_ns.clone(),
                    };
                    this.detail_active_tab.insert(detail_key, DetailTabMode::Yaml);
                    this.sync_navigator_to_active_tab();
                    this.trigger_data_loading_for_active_tab(cx);
                })),
        );

        // "Delete" item
        let del_ctx = ctx.clone();
        let del_kind = k.clone();
        let del_name = n.clone();
        let del_ns = ns.clone();
        let dismiss_ctx3 = ctx.clone();
        let dismiss_kind3 = k.clone();
        let error_color = self.theme.colors.error.to_gpui();
        menu = menu.child(
            div()
                .id("ctx-delete")
                .px_3()
                .py_2()
                .cursor_pointer()
                .text_sm()
                .text_color(error_color)
                .hover(|s| s.bg(gpui::rgb(0x1F2937)))
                .child("Delete")
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    let menu_key = ResourceListKey {
                        cluster_context: dismiss_ctx3.clone(),
                        kind: dismiss_kind3.clone(),
                        namespace: None,
                    };
                    this.context_menu_row.remove(&menu_key);
                    use crate::components::confirm_dialog::ConfirmDialogState;
                    let dialog = ConfirmDialogState::delete_resource(&del_kind, &del_name);
                    this.confirm_dialog = Some(ConfirmDialogContext {
                        dialog,
                        action: PendingAction::DeleteResource {
                            cluster_context: del_ctx.clone(),
                            kind: del_kind.clone(),
                            name: del_name.clone(),
                            namespace: del_ns.clone(),
                        },
                    });
                    cx.notify();
                })),
        );

        // "Scale" item (Deployment/StatefulSet/ReplicaSet only)
        if is_scalable {
            let scale_ctx = ctx.clone();
            let scale_kind = k.clone();
            let scale_name = n.clone();
            let scale_ns = ns.clone();
            let dismiss_ctx4 = ctx.clone();
            let dismiss_kind4 = k.clone();
            menu = menu.child(
                div()
                    .id("ctx-scale")
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(text)
                    .hover(|s| s.bg(gpui::rgb(0x1F2937)))
                    .child("Scale")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let menu_key = ResourceListKey {
                            cluster_context: dismiss_ctx4.clone(),
                            kind: dismiss_kind4.clone(),
                            namespace: None,
                        };
                        this.context_menu_row.remove(&menu_key);
                        use crate::components::confirm_dialog::ConfirmDialogState;
                        let dialog =
                            ConfirmDialogState::scale_resource(&scale_kind, &scale_name, 1);
                        this.confirm_dialog = Some(ConfirmDialogContext {
                            dialog,
                            action: PendingAction::ScaleResource {
                                cluster_context: scale_ctx.clone(),
                                kind: scale_kind.clone(),
                                name: scale_name.clone(),
                                namespace: scale_ns.clone(),
                                replicas: 1,
                            },
                        });
                        cx.notify();
                    })),
            );
        }

        // "Logs" item (Pod only)
        if is_pod {
            let logs_ctx = ctx.clone();
            let logs_name = n.clone();
            let dismiss_ctx5 = ctx.clone();
            let dismiss_kind5 = k.clone();
            menu = menu.child(
                div()
                    .id("ctx-logs")
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(text_secondary)
                    .hover(|s| s.bg(gpui::rgb(0x1F2937)))
                    .child("Logs")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let menu_key = ResourceListKey {
                            cluster_context: dismiss_ctx5.clone(),
                            kind: dismiss_kind5.clone(),
                            namespace: None,
                        };
                        this.context_menu_row.remove(&menu_key);
                        // Open logs dock tab
                        this.dock.add_tab(DockTabKind::LogViewer {
                            pod: logs_name.clone(),
                            container: String::new(),
                            cluster: logs_ctx.clone(),
                        });
                        this.dock.collapsed = false;
                        cx.notify();
                    })),
            );

            // "Terminal" item (Pod only)
            let term_ctx = ctx.clone();
            let term_name = n.clone();
            let dismiss_ctx6 = ctx.clone();
            let dismiss_kind6 = k.clone();
            menu = menu.child(
                div()
                    .id("ctx-terminal")
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(text_secondary)
                    .hover(|s| s.bg(gpui::rgb(0x1F2937)))
                    .child("Terminal")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let menu_key = ResourceListKey {
                            cluster_context: dismiss_ctx6.clone(),
                            kind: dismiss_kind6.clone(),
                            namespace: None,
                        };
                        this.context_menu_row.remove(&menu_key);
                        this.dock.add_tab(DockTabKind::Terminal {
                            pod: term_name.clone(),
                            container: String::new(),
                            cluster: term_ctx.clone(),
                        });
                        this.dock.collapsed = false;
                        cx.notify();
                    })),
            );
        }

        div().relative().w_full().h_0().child(backdrop).child(menu)
    }

    /// Look up metrics for a table row from `self.cluster_metrics`.
    ///
    /// Returns display strings and optional usage percentages for CPU/Memory.
    /// For Nodes: returns percentage-based values (bar chart with %).
    /// For Pods: returns absolute usage values (bar with text, no percentage bar).
    fn get_metrics_for_row(
        &self,
        cluster_context: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Option<RowMetrics> {
        let metrics = self.cluster_metrics.get(cluster_context)?;
        if !metrics.is_available() {
            return None;
        }

        match kind {
            "Pod" => {
                let pm = metrics.find_pod_metrics(name, namespace.unwrap_or(""))?;
                let cpu = baeus_core::client::format_cpu_millicores(pm.total_cpu_millicores());
                let mem = baeus_core::client::format_memory_bytes(pm.total_memory_bytes());
                Some(RowMetrics {
                    cpu_display: cpu,
                    cpu_percent: None, // Pods don't have capacity for percentage
                    mem_display: mem,
                    mem_percent: None,
                })
            }
            "Node" => {
                let nm = metrics.find_node_metrics(name)?;
                let cpu_pct = nm.cpu_usage_percent();
                let mem_pct = nm.memory_usage_percent();
                Some(RowMetrics {
                    cpu_display: format!("{}%", cpu_pct as u32),
                    cpu_percent: Some(cpu_pct as f32 / 100.0),
                    mem_display: format!("{}%", mem_pct as u32),
                    mem_percent: Some(mem_pct as f32 / 100.0),
                })
            }
            _ => None,
        }
    }

    /// Render an inline metrics bar cell for CPU/Memory columns.
    ///
    /// When `usage_percent` is Some, renders a colored bar proportional to usage.
    /// When None, just shows the display text (for Pod absolute values).
    fn render_metrics_bar_cell(
        &self,
        usage_percent: Option<f32>,
        display_text: &str,
        col_label: &str,
        col_weight: f32,
        col_width: Option<f32>,
        text_secondary: Rgba,
    ) -> Div {
        let cell_base = if let Some(w) = col_width {
            Self::table_cell_px(w)
        } else {
            Self::table_cell_base(col_label, col_weight)
        };

        if let Some(pct) = usage_percent {
            // Node metrics: bar + text
            let bar_color = if pct > 0.9 {
                self.theme.colors.error.to_gpui()
            } else if pct > 0.7 {
                self.theme.colors.warning.to_gpui()
            } else {
                self.theme.colors.success.to_gpui()
            };

            let bar_bg = Rgba { r: bar_color.r, g: bar_color.g, b: bar_color.b, a: 0.15 };

            cell_base
                .py_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .bg(bar_bg)
                        .overflow_hidden()
                        .child(div().h_full().w(relative(pct)).bg(bar_color).rounded(px(3.0))),
                )
                .child(div().text_xs().text_color(text_secondary).child(display_text.to_string()))
        } else {
            // Pod metrics: just the value text (styled differently from "—")
            let accent = self.theme.colors.accent.to_gpui();
            cell_base
                .py_1()
                .text_sm()
                .text_color(accent)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(display_text.to_string())
        }
    }

    /// Phase 3: Render container status bricks (colored 8×8px squares) for Pod rows.
    fn render_container_bricks_cell(
        &self,
        statuses: &[json_extract::ContainerBrickStatus],
        fallback_text: &str,
        col_label: &str,
        col_weight: f32,
        col_width: Option<f32>,
    ) -> Div {
        use json_extract::ContainerBrickStatus;

        let mut cell = if let Some(w) = col_width {
            Self::table_cell_px(w)
        } else {
            Self::table_cell_base(col_label, col_weight)
        }
        .py_1()
        .flex()
        .flex_row()
        .items_center()
        .gap_1();

        for (i, status) in statuses.iter().enumerate() {
            let (bg_color, border_color) = match status {
                ContainerBrickStatus::Running => (self.theme.colors.success.to_gpui(), None),
                ContainerBrickStatus::Waiting => {
                    let c = self.theme.colors.warning.to_gpui();
                    let bg = Rgba { r: c.r, g: c.g, b: c.b, a: 0.3 };
                    (bg, Some(c))
                }
                ContainerBrickStatus::Terminated => {
                    let c = self.theme.colors.text_muted.to_gpui();
                    let bg = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
                    (bg, Some(c))
                }
                ContainerBrickStatus::Failed => (self.theme.colors.error.to_gpui(), None),
                ContainerBrickStatus::Creating => (self.theme.colors.info.to_gpui(), None),
                ContainerBrickStatus::Restarted => {
                    let green = self.theme.colors.success.to_gpui();
                    let orange = self.theme.colors.warning.to_gpui();
                    (green, Some(orange))
                }
            };

            let mut brick = div()
                .id(ElementId::Name(SharedString::from(format!("brick-{col_label}-{i}"))))
                .w(px(8.0))
                .h(px(8.0))
                .rounded(px(1.0))
                .bg(bg_color);

            if let Some(bc) = border_color {
                brick = brick.border_1().border_color(bc);
            }

            cell = cell.child(brick);
        }

        // Show count text after bricks
        let count_text = SharedString::from(fallback_text.to_string());
        cell = cell.child(
            div()
                .text_xs()
                .text_color(self.theme.colors.text_muted.to_gpui())
                .ml_1()
                .child(count_text),
        );

        cell
    }

    /// Phase 4: Render condition badges as colored inline text.
    fn render_conditions_cell(
        &self,
        conditions: &[(String, bool)],
        col_label: &str,
        col_weight: f32,
        col_width: Option<f32>,
    ) -> Div {
        let mut cell = if let Some(w) = col_width {
            Self::table_cell_px(w)
        } else {
            Self::table_cell_base(col_label, col_weight)
        }
        .py_1()
        .flex()
        .flex_row()
        .items_center()
        .overflow_hidden();

        for (i, (ctype, is_true)) in conditions.iter().enumerate() {
            if i > 0 {
                cell = cell.child(
                    div().text_xs().text_color(self.theme.colors.text_muted.to_gpui()).child(" · "),
                );
            }

            let color = if *is_true {
                self.theme.colors.success.to_gpui()
            } else {
                self.theme.colors.error.to_gpui()
            };

            cell = cell
                .child(div().text_xs().text_color(color).child(SharedString::from(ctype.clone())));
        }

        cell
    }

    #[allow(clippy::too_many_arguments)]
    fn render_resource_detail_content(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let key = ResourceDetailKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };

        if let Some(json) = self.resource_detail_data.get(&key) {
            let border = self.theme.colors.border.to_gpui();
            let surface = self.theme.colors.surface.to_gpui();
            let accent = self.theme.colors.accent.to_gpui();

            let mut content = div().flex_1().min_h(px(0.0)).flex().flex_col().bg(bg);

            // Title bar with action buttons
            content = content.child(self.render_detail_title_bar(
                cx,
                kind,
                name,
                cluster_context,
                namespace,
                text,
                surface,
                border,
                accent,
            ));

            // Overview | YAML tab bar
            let active_tab =
                self.detail_active_tab.get(&key).copied().unwrap_or(DetailTabMode::Overview);
            content = content.child(self.render_detail_tab_bar(
                cx,
                &key,
                active_tab,
                text,
                text_secondary,
                surface,
                border,
                accent,
            ));

            // If YAML tab is active, render the YAML editor
            if active_tab == DetailTabMode::Yaml {
                // Lazily initialize the YAML editor if needed
                if !self.yaml_editors.contains_key(&key) {
                    let yaml_text = serde_yaml_ng::to_string(json).unwrap_or_default();
                    let rv = json
                        .get("metadata")
                        .and_then(|m| m.get("resourceVersion"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let editor = crate::components::editor_view::EditorViewState::new(
                        &yaml_text,
                        kind,
                        name,
                        namespace.map(|s| s.to_string()),
                        &rv,
                    );
                    self.yaml_editors.insert(key.clone(), editor);
                    let fh = cx.focus_handle();
                    self.yaml_editor_focus_handles.insert(key.clone(), fh);
                }
                content = content.child(self.render_yaml_editor_content(
                    cx,
                    &key,
                    text,
                    text_secondary,
                    bg,
                ));
                return content;
            }

            // If Events tab is active, render the events list
            if active_tab == DetailTabMode::Events {
                content = content.child(self.render_resource_events_tab(
                    kind,
                    name,
                    namespace,
                    json,
                    text,
                    text_secondary,
                    bg,
                    border,
                    accent,
                ));
                return content;
            }

            // If Topology tab is active, render the topology view
            if active_tab == DetailTabMode::Topology {
                content =
                    content.child(self.render_topology_tab(cx, &key, text, text_secondary, bg));
                return content;
            }

            // Pod-specific rich detail view
            if kind == "Pod" {
                content =
                    content.child(self.render_pod_detail_body(cx, json, text, text_secondary, bg));
                return content;
            }

            // Node-specific rich detail view
            if kind == "Node" {
                content =
                    content.child(self.render_node_detail_body(cx, json, text, text_secondary, bg));
                return content;
            }

            // Generic detail view for non-Pod resources — collapsible sections
            let mut body = div()
                .id("generic-detail-body")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .p_4()
                .gap_3();

            // Properties section — collapsible
            let props = json_extract::extract_detail_properties(kind, json);
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Info,
                &format!("{kind}-properties"),
                &props,
                text,
                text_secondary,
                border,
                accent,
                |this: &Self, _cx, props, text, text_secondary, border, _accent| {
                    this.render_detail_properties_body(props, text, text_secondary, border)
                },
            ));

            // Labels section — collapsible
            let labels = json_extract::extract_labels(json);
            if !labels.is_empty() {
                body = body.child(self.render_pod_section(
                    cx,
                    SectionIcon::Labels,
                    &format!("{kind}-labels"),
                    &labels,
                    text,
                    text_secondary,
                    border,
                    accent,
                    |this: &Self, _cx, labels, _text, _text_secondary, _border, _accent| {
                        this.render_detail_label_badges_body(labels, surface)
                    },
                ));
            }

            // Annotations section — collapsible
            let annotations = json_extract::extract_annotations(json);
            if !annotations.is_empty() {
                body = body.child(self.render_pod_section(
                    cx,
                    SectionIcon::Annotations,
                    &format!("{kind}-annotations"),
                    &annotations,
                    text,
                    text_secondary,
                    border,
                    accent,
                    |this: &Self, _cx, annotations, _text, _text_secondary, _border, _accent| {
                        this.render_detail_annotations_body(annotations, surface)
                    },
                ));
            }

            // Conditions section — collapsible
            let conditions = json_extract::extract_conditions(json);
            if !conditions.is_empty() {
                body = body.child(self.render_pod_section(
                    cx,
                    SectionIcon::Conditions,
                    &format!("{kind}-conditions"),
                    &conditions,
                    text,
                    text_secondary,
                    border,
                    accent,
                    |this: &Self, _cx, conditions, _text, text_secondary, _border, _accent| {
                        this.render_detail_conditions_body(
                            conditions,
                            text_secondary,
                            surface,
                            border,
                        )
                    },
                ));
            }

            // Secret Data section — show keys with masked values and eye toggle
            if kind == "Secret" {
                if let Some(data_obj) = json.get("data").and_then(|d| d.as_object()) {
                    let data_keys: Vec<(String, String)> = data_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                    if !data_keys.is_empty() {
                        let section_id = format!("{kind}-data");
                        let is_collapsed = self.detail_collapsed_sections.contains(&section_id);
                        let section_id_toggle = section_id.clone();

                        let mut section = div().flex().flex_col();

                        // Section header
                        section = section.child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!(
                                    "section-hdr-{section_id}"
                                ))))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    if this.detail_collapsed_sections.contains(&section_id_toggle) {
                                        this.detail_collapsed_sections.remove(&section_id_toggle);
                                    } else {
                                        this.detail_collapsed_sections
                                            .insert(section_id_toggle.clone());
                                    }
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_secondary)
                                        .child(if is_collapsed { "▶" } else { "▼" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text)
                                        .child("Data"),
                                )
                                .child(div().text_xs().text_color(text_secondary).child(
                                    SharedString::from(format!("({} keys)", data_keys.len())),
                                )),
                        );

                        if !is_collapsed {
                            let mut rows = div().flex().flex_col().px_3().gap_1();
                            let secret_name = name;
                            let secret_ns = namespace.unwrap_or("");
                            for (key, b64_value) in &data_keys {
                                let reveal_id =
                                    format!("{cluster_context}:{secret_ns}:{secret_name}:{key}");
                                let is_revealed = self.revealed_secret_keys.contains(&reveal_id);

                                // Decode base64 for display when revealed
                                let decoded = if is_revealed {
                                    use base64::Engine;
                                    base64::engine::general_purpose::STANDARD
                                        .decode(b64_value)
                                        .ok()
                                        .and_then(|bytes| String::from_utf8(bytes).ok())
                                        .unwrap_or_else(|| b64_value.clone())
                                } else {
                                    "•".repeat(b64_value.len().min(40))
                                };

                                let reveal_id_click = reveal_id.clone();
                                let eye_icon = if is_revealed {
                                    Icon::new(IconName::Eye).xsmall()
                                } else {
                                    Icon::new(IconName::EyeOff).xsmall()
                                };

                                rows = rows.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .py_1()
                                        .border_b_1()
                                        .border_color(Rgba {
                                            r: border.r,
                                            g: border.g,
                                            b: border.b,
                                            a: 0.2,
                                        })
                                        .child(
                                            div()
                                                .w(px(160.0))
                                                .flex_shrink_0()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(text_secondary)
                                                .child(SharedString::from(key.clone())),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_xs()
                                                .text_color(if is_revealed {
                                                    text
                                                } else {
                                                    Rgba { r: text.r, g: text.g, b: text.b, a: 0.4 }
                                                })
                                                .child(SharedString::from(decoded)),
                                        )
                                        .child(
                                            div()
                                                .id(ElementId::Name(SharedString::from(format!(
                                                    "eye-{reveal_id}"
                                                ))))
                                                .w(px(24.0))
                                                .h(px(20.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .rounded(px(3.0))
                                                .text_xs()
                                                .hover(|s| {
                                                    s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.1 })
                                                })
                                                .child(eye_icon)
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        if this
                                                            .revealed_secret_keys
                                                            .contains(&reveal_id_click)
                                                        {
                                                            this.revealed_secret_keys
                                                                .remove(&reveal_id_click);
                                                        } else {
                                                            this.revealed_secret_keys
                                                                .insert(reveal_id_click.clone());
                                                        }
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                );
                            }
                            section = section.child(rows);
                        }

                        body = body.child(section);
                    }
                }
            }

            content = content.child(body);
            content
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child(format!("Loading {kind}/{name} from {cluster_context}..."))
        }
    }

    /// Render the Overview | YAML tab bar for the resource detail view.
    #[allow(clippy::too_many_arguments)]
    fn render_detail_tab_bar(
        &self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        active: DetailTabMode,
        text: Rgba,
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let key_overview = key.clone();
        let key_yaml = key.clone();
        let key_events = key.clone();
        let key_topology = key.clone();

        let overview_color = if active == DetailTabMode::Overview { text } else { text_secondary };
        let yaml_color = if active == DetailTabMode::Yaml { text } else { text_secondary };
        let events_color = if active == DetailTabMode::Events { text } else { text_secondary };
        let topology_color = if active == DetailTabMode::Topology { text } else { text_secondary };

        let mut bar =
            div().flex().flex_row().w_full().border_b_1().border_color(border).bg(surface);

        // Overview tab
        let mut overview_tab = div()
            .id("detail-tab-overview")
            .px_3()
            .py_2()
            .cursor_pointer()
            .text_sm()
            .text_color(overview_color)
            .child("Overview");
        if active == DetailTabMode::Overview {
            overview_tab = overview_tab.border_b_2().border_color(accent);
        }
        bar = bar.child(overview_tab.on_click(cx.listener(move |this, _event, _window, cx| {
            this.detail_active_tab.insert(key_overview.clone(), DetailTabMode::Overview);
            this.topology_data.remove(&key_overview);
            cx.notify();
        })));

        // YAML tab
        let mut yaml_tab = div()
            .id("detail-tab-yaml")
            .px_3()
            .py_2()
            .cursor_pointer()
            .text_sm()
            .text_color(yaml_color)
            .child("YAML");
        if active == DetailTabMode::Yaml {
            yaml_tab = yaml_tab.border_b_2().border_color(accent);
        }
        bar = bar.child(yaml_tab.on_click(cx.listener(move |this, _event, _window, cx| {
            this.detail_active_tab.insert(key_yaml.clone(), DetailTabMode::Yaml);
            this.topology_data.remove(&key_yaml);
            cx.notify();
        })));

        // Events tab
        let mut events_tab = div()
            .id("detail-tab-events")
            .px_3()
            .py_2()
            .cursor_pointer()
            .text_sm()
            .text_color(events_color)
            .child("Events");
        if active == DetailTabMode::Events {
            events_tab = events_tab.border_b_2().border_color(accent);
        }
        bar = bar.child(events_tab.on_click(cx.listener(move |this, _event, _window, cx| {
            this.detail_active_tab.insert(key_events.clone(), DetailTabMode::Events);
            this.topology_data.remove(&key_events);
            cx.notify();
        })));

        // Topology tab
        let mut topology_tab = div()
            .id("detail-tab-topology")
            .px_3()
            .py_2()
            .cursor_pointer()
            .text_sm()
            .text_color(topology_color)
            .child("Topology");
        if active == DetailTabMode::Topology {
            topology_tab = topology_tab.border_b_2().border_color(accent);
        }
        bar = bar.child(topology_tab.on_click(cx.listener(move |this, _event, _window, cx| {
            this.detail_active_tab.insert(key_topology.clone(), DetailTabMode::Topology);
            // Start loading topology data if not already loaded
            if !this.topology_data.contains_key(&key_topology) {
                this.start_topology_loading(&key_topology, cx);
            }
            cx.notify();
        })));

        bar
    }

    /// Render the title bar for a resource detail view with action buttons.
    #[allow(clippy::too_many_arguments)]
    fn render_detail_title_bar(
        &self,
        cx: &mut Context<Self>,
        kind: &str,
        name: &str,
        cluster_context: &str,
        namespace: Option<&str>,
        text: Rgba,
        surface: Rgba,
        border: Rgba,
        accent: Rgba,
    ) -> Div {
        let title = SharedString::from(format!("{kind}: {name}"));
        let error_color = self.theme.colors.error.to_gpui();
        let is_scalable = matches!(kind, "Deployment" | "StatefulSet" | "ReplicaSet");

        let key_for_yaml = ResourceDetailKey {
            cluster_context: cluster_context.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };
        let key_for_scale = key_for_yaml.clone();
        let key_for_restart = key_for_yaml.clone();
        let key_for_cordon = key_for_yaml.clone();
        let key_for_uncordon = key_for_yaml.clone();

        let kind_del = kind.to_string();
        let name_del = name.to_string();
        let ctx_del = cluster_context.to_string();
        let ns_del = namespace.map(|s| s.to_string());

        let kind_scale = kind.to_string();
        let name_scale = name.to_string();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_3()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(div().font_weight(FontWeight::BOLD).text_color(text).text_base().child(title))
            // Spacer
            .child(div().flex_1());

        // Pod-specific action buttons: Logs and Shell
        if kind == "Pod" {
            let logs_ctx = cluster_context.to_string();
            let logs_name = name.to_string();
            let logs_ns = namespace.map(|s| s.to_string());

            // Extract owner reference and container names from pod JSON
            let detail_key = ResourceDetailKey {
                cluster_context: cluster_context.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                namespace: namespace.map(|s| s.to_string()),
            };
            let pod_json = self.resource_detail_data.get(&detail_key);
            let owner: Option<(String, String)> = pod_json.and_then(|json| {
                let refs = json.pointer("/metadata/ownerReferences")?;
                let first = refs.as_array()?.first()?;
                let ok = first.get("kind")?.as_str()?.to_string();
                let on = first.get("name")?.as_str()?.to_string();
                Some((ok, on))
            });
            let mut containers: Vec<String> = pod_json
                .and_then(|json| json.pointer("/spec/containers"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.get("name")?.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // Also include init containers
            if let Some(init_arr) = pod_json
                .and_then(|json| json.pointer("/spec/initContainers"))
                .and_then(|v| v.as_array())
            {
                for c in init_arr {
                    if let Some(name) = c.get("name").and_then(|n| n.as_str()) {
                        containers.push(format!("init:{name}"));
                    }
                }
            }
            // Pick first container as default
            let default_container = containers.first().cloned().unwrap_or_default();

            // Find sibling pods from the same owner
            let sibling_pods: Vec<String> = if let Some((ref ok, ref on)) = owner {
                // Try both namespace-scoped and unscoped keys
                let ns_str = namespace.map(|s| s.to_string());
                let keys = [
                    ResourceListKey {
                        cluster_context: cluster_context.to_string(),
                        kind: "Pod".to_string(),
                        namespace: ns_str.clone(),
                    },
                    ResourceListKey {
                        cluster_context: cluster_context.to_string(),
                        kind: "Pod".to_string(),
                        namespace: None,
                    },
                ];
                let mut found = Vec::new();
                for key in &keys {
                    if let Some(pods) = self.resource_list_data.get(key) {
                        found = pods
                            .iter()
                            .filter_map(|pj| {
                                let pname = pj.pointer("/metadata/name")?.as_str()?;
                                if let Some(ns) = &ns_str {
                                    let pns =
                                        pj.pointer("/metadata/namespace").and_then(|v| v.as_str());
                                    if pns != Some(ns) {
                                        return None;
                                    }
                                }
                                let refs = pj.pointer("/metadata/ownerReferences")?.as_array()?;
                                let first = refs.first()?;
                                let rk = first.get("kind")?.as_str()?;
                                let rn = first.get("name")?.as_str()?;
                                if rk == ok.as_str() && rn == on.as_str() {
                                    Some(pname.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !found.is_empty() {
                            break;
                        }
                    }
                }
                found
            } else {
                Vec::new()
            };

            let owner_for_click = owner.clone();
            let containers_for_click = containers.clone();
            let container_for_click = default_container.clone();
            let sibling_pods_for_click = sibling_pods;
            bar = bar.child(
                div()
                    .id("detail-pod-logs-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.08 }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(Icon::new(IconName::File).xsmall())
                            .child("Logs"),
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let owner_ref =
                            owner_for_click.as_ref().map(|(k, n)| (k.as_str(), n.as_str()));
                        this.open_logs_in_dock_with_owner(
                            &logs_name,
                            &container_for_click,
                            &logs_ctx,
                            logs_ns.as_deref(),
                            owner_ref,
                            cx,
                        );
                        // Set container filter and sibling pods on the newly created log viewer
                        if let Some(tab_id) = this.dock.active_tab_id {
                            if let Some(entity) = this.log_viewer_views.get(&tab_id) {
                                entity.update(cx, |view, _cx| {
                                    if !containers_for_click.is_empty() {
                                        view.state
                                            .set_container_filter(containers_for_click.clone());
                                    }
                                    if !sibling_pods_for_click.is_empty() {
                                        view.state.sibling_pods = sibling_pods_for_click.clone();
                                    }
                                });
                            }
                        }
                    })),
            );

            let shell_ctx = cluster_context.to_string();
            let shell_name = name.to_string();
            let shell_ns = namespace.map(|s| s.to_string());
            bar = bar.child(
                div()
                    .id("detail-pod-shell-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.08 }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(Icon::new(IconName::SquareTerminal).xsmall())
                            .child("Shell"),
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let cluster = shell_ctx.clone();
                        let pod = shell_name.clone();
                        let ns = shell_ns.clone();
                        // Ensure cluster terminal exists (no-op if already open)
                        this.spawn_terminal_for_cluster(&cluster, cx);
                        // Select the terminal tab and expand dock
                        if let Some(&tab_id) = this.cluster_terminals.get(&cluster) {
                            this.dock.select_tab(tab_id);
                            if this.dock.collapsed {
                                this.dock.toggle_collapsed();
                            }
                            // Send kubectl exec command via PTY with delay
                            // (wait for terminal init + context switch)
                            if let Some(pty) = this.pty_processes.get(&tab_id) {
                                let exec_cmd = if let Some(ns) = ns {
                                    format!("kubectl exec -it -n {} {} -- sh\n", ns, pod)
                                } else {
                                    format!("kubectl exec -it {} -- sh\n", pod)
                                };
                                let cmd_bytes = exec_cmd.into_bytes();
                                let pty_clone = pty.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_millis(2000));
                                    if let Ok(p) = pty_clone.lock() {
                                        let _ = p.write_input(&cmd_bytes);
                                    }
                                });
                            }
                        }
                        this.dock.collapsed = false;
                        cx.notify();
                    })),
            );
        }

        // "Edit YAML" button
        bar = bar.child(
            div()
                .id("detail-edit-yaml-btn")
                .px_3()
                .py_1()
                .rounded(px(4.0))
                .bg(accent)
                .cursor_pointer()
                .text_xs()
                .text_color(gpui::rgb(0xFFFFFF))
                .child("Edit YAML")
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.detail_active_tab.insert(key_for_yaml.clone(), DetailTabMode::Yaml);
                    cx.notify();
                })),
        );

        // "Scale" button (only for scalable resources)
        if is_scalable {
            bar = bar.child(
                div()
                    .id("detail-scale-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .child("Scale")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        use crate::components::confirm_dialog::ConfirmDialogState;
                        let dialog =
                            ConfirmDialogState::scale_resource(&kind_scale, &name_scale, 1);
                        this.confirm_dialog = Some(ConfirmDialogContext {
                            dialog,
                            action: PendingAction::ScaleResource {
                                cluster_context: key_for_scale.cluster_context.clone(),
                                kind: key_for_scale.kind.clone(),
                                name: key_for_scale.name.clone(),
                                namespace: key_for_scale.namespace.clone(),
                                replicas: 1,
                            },
                        });
                        cx.notify();
                    })),
            );
        }

        // "Restart" button (for Deployment, StatefulSet, DaemonSet)
        let is_restartable = matches!(kind, "Deployment" | "StatefulSet" | "DaemonSet");
        if is_restartable {
            let name_restart = name.to_string();
            bar = bar.child(
                div()
                    .id("detail-restart-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .child("Restart")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        use crate::components::confirm_dialog::ConfirmDialogState;
                        use crate::components::confirm_dialog::DialogSeverity;
                        let dialog = ConfirmDialogState::new(
                            "Restart Resource",
                            &format!(
                                "Restart {}? This will trigger a rolling update.",
                                name_restart
                            ),
                            DialogSeverity::Warning,
                        )
                        .with_confirm_label("Restart");
                        this.confirm_dialog = Some(ConfirmDialogContext {
                            dialog,
                            action: PendingAction::RestartResource {
                                cluster_context: key_for_restart.cluster_context.clone(),
                                kind: key_for_restart.kind.clone(),
                                name: key_for_restart.name.clone(),
                                namespace: key_for_restart.namespace.clone(),
                            },
                        });
                        cx.notify();
                    })),
            );
        }

        // "Port Forward" button (for Pod and Service)
        if matches!(kind, "Pod" | "Service") {
            let pf_name = name.to_string();
            let pf_ns = namespace.map(|s| s.to_string());
            bar = bar.child(
                div()
                    .id("detail-port-forward-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .child("Port Forward")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.start_port_forward(
                            &pf_name,
                            pf_ns.as_deref().unwrap_or("default"),
                            0, // placeholder — user would pick ports in real flow
                            0,
                            cx,
                        );
                    })),
            );
        }

        // "Cordon" / "Uncordon" buttons (for Node)
        if kind == "Node" {
            let name_cordon = name.to_string();
            let name_uncordon = name.to_string();
            bar = bar.child(
                div()
                    .id("detail-cordon-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .child("Cordon")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        use crate::components::confirm_dialog::ConfirmDialogState;
                        use crate::components::confirm_dialog::DialogSeverity;
                        let dialog = ConfirmDialogState::new(
                            "Cordon Node",
                            &format!("Mark node {} as unschedulable?", name_cordon),
                            DialogSeverity::Warning,
                        )
                        .with_confirm_label("Cordon");
                        this.confirm_dialog = Some(ConfirmDialogContext {
                            dialog,
                            action: PendingAction::CordonNode {
                                cluster_context: key_for_cordon.cluster_context.clone(),
                                name: key_for_cordon.name.clone(),
                            },
                        });
                        cx.notify();
                    })),
            );
            bar = bar.child(
                div()
                    .id("detail-uncordon-btn")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(text)
                    .child("Uncordon")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        use crate::components::confirm_dialog::ConfirmDialogState;
                        use crate::components::confirm_dialog::DialogSeverity;
                        let dialog = ConfirmDialogState::new(
                            "Uncordon Node",
                            &format!("Mark node {} as schedulable?", name_uncordon),
                            DialogSeverity::Info,
                        )
                        .with_confirm_label("Uncordon");
                        this.confirm_dialog = Some(ConfirmDialogContext {
                            dialog,
                            action: PendingAction::UncordonNode {
                                cluster_context: key_for_uncordon.cluster_context.clone(),
                                name: key_for_uncordon.name.clone(),
                            },
                        });
                        cx.notify();
                    })),
            );
        }

        // "Delete" button
        bar = bar.child(
            div()
                .id("detail-delete-btn")
                .px_3()
                .py_1()
                .rounded(px(4.0))
                .bg(error_color)
                .cursor_pointer()
                .text_xs()
                .text_color(gpui::rgb(0xFFFFFF))
                .child("Delete")
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    use crate::components::confirm_dialog::ConfirmDialogState;
                    let dialog = ConfirmDialogState::delete_resource(&kind_del, &name_del);
                    this.confirm_dialog = Some(ConfirmDialogContext {
                        dialog,
                        action: PendingAction::DeleteResource {
                            cluster_context: ctx_del.clone(),
                            kind: kind_del.clone(),
                            name: name_del.clone(),
                            namespace: ns_del.clone(),
                        },
                    });
                    cx.notify();
                })),
        );

        bar
    }

    /// Render a single property row: label on left, value on right.
    fn render_detail_property_row(
        &self,
        label: &str,
        value: &str,
        _text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
    ) -> Div {
        let label_s = SharedString::from(label.to_string());
        let value_s = SharedString::from(value.to_string());
        div()
            .flex()
            .flex_row()
            .items_center()
            .border_b_1()
            .border_color(border)
            .py_1()
            .child(
                div()
                    .w(px(160.0))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child(label_s),
            )
            .child(div().flex_1().text_xs().text_color(gpui::rgb(0xE5E7EB)).child(value_s))
    }

    /// Render property rows only (no section header).
    /// Used inside `render_pod_section` closures for collapsible sections.
    pub(crate) fn render_detail_properties_body(
        &self,
        props: &[(String, String)],
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
    ) -> Div {
        let mut body = div().flex().flex_col().gap_1();
        for (label, value) in props {
            body = body.child(self.render_detail_property_row(
                label,
                value,
                text,
                text_secondary,
                border,
            ));
        }
        body
    }

    /// Render label badges only (no section header).
    pub(crate) fn render_detail_label_badges_body(
        &self,
        labels: &[(String, String)],
        surface: Rgba,
    ) -> Div {
        let mut badges_row = div().flex().flex_row().flex_wrap().gap_1();
        for (key, value) in labels {
            let badge_text = SharedString::from(format!("{key}={value}"));
            let badge = div()
                .px_2()
                .py(px(2.0))
                .rounded_sm()
                .bg(surface)
                .text_xs()
                .text_color(gpui::rgb(0xD1D5DB))
                .child(badge_text);
            badges_row = badges_row.child(badge);
        }
        badges_row
    }

    /// Render annotation badges only (no section header).
    pub(crate) fn render_detail_annotations_body(
        &self,
        annotations: &[(String, String)],
        surface: Rgba,
    ) -> Div {
        let mut badges_row = div().flex().flex_row().flex_wrap().gap_1();
        for (key, value) in annotations {
            let badge_text = SharedString::from(format!("{key}={value}"));
            let badge = div()
                .px_2()
                .py(px(2.0))
                .rounded_sm()
                .bg(surface)
                .text_xs()
                .text_color(gpui::rgb(0xD1D5DB))
                .child(badge_text);
            badges_row = badges_row.child(badge);
        }
        badges_row
    }

    /// Render conditions table only (no section header).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_detail_conditions_body(
        &self,
        conditions: &[(String, String, String, String, String)],
        text_secondary: Rgba,
        surface: Rgba,
        border: Rgba,
    ) -> Div {
        let mut body = div().flex().flex_col().gap_1();

        // Header
        let cond_header = div()
            .flex()
            .flex_row()
            .w_full()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .w(px(120.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Type"),
            )
            .child(
                div()
                    .w(px(60.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Status"),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Reason"),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Message"),
            );
        body = body.child(cond_header);

        // Rows
        for (ctype, status, reason, message, _last_transition) in conditions {
            let status_color =
                if status == "True" { gpui::rgb(0x22C55E) } else { gpui::rgb(0xEF4444) };
            let cond_row = div()
                .flex()
                .flex_row()
                .w_full()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(120.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(gpui::rgb(0xE5E7EB))
                        .child(SharedString::from(ctype.clone())),
                )
                .child(
                    div()
                        .w(px(60.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(status_color)
                        .child(SharedString::from(status.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(reason.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .child(SharedString::from(message.clone())),
                );
            body = body.child(cond_row);
        }

        body
    }

    // -----------------------------------------------------------------------
    // T355: Navigation history
    // -----------------------------------------------------------------------

    /// T355: Navigate backward in history.
    fn navigate_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(target) = self.navigation_history.get(self.history_index).cloned() {
                self.layout.navigate(target.clone());
                self.workspace.open_tab(target);
            }
        }
    }

    /// T355: Navigate forward in history.
    fn navigate_forward(&mut self) {
        if self.history_index + 1 < self.navigation_history.len() {
            self.history_index += 1;
            if let Some(target) = self.navigation_history.get(self.history_index).cloned() {
                self.layout.navigate(target.clone());
                self.workspace.open_tab(target);
            }
        }
    }

    /// T355: Push a navigation target onto the history stack.
    #[allow(dead_code)]
    pub fn push_navigation_history(&mut self, target: NavigationTarget) {
        if !self.navigation_history.is_empty()
            && self.history_index + 1 < self.navigation_history.len()
        {
            self.navigation_history.truncate(self.history_index + 1);
        }
        self.navigation_history.push(target);
        // Cap history to prevent unbounded growth.
        const MAX_HISTORY: usize = 1000;
        if self.navigation_history.len() > MAX_HISTORY {
            let excess = self.navigation_history.len() - MAX_HISTORY;
            self.navigation_history.drain(..excess);
        }
        self.history_index = self.navigation_history.len() - 1;
    }

    /// Evict oldest entries from resource data caches when they exceed their cap.
    /// Called after inserting into resource_list_data or resource_detail_data.
    fn evict_data_caches(&mut self) {
        const MAX_LIST_CACHE: usize = 200;
        const MAX_DETAIL_CACHE: usize = 500;
        if self.resource_list_data.len() > MAX_LIST_CACHE {
            let excess = self.resource_list_data.len() - MAX_LIST_CACHE;
            let keys: Vec<_> = self.resource_list_data.keys().take(excess).cloned().collect();
            for k in keys {
                self.resource_list_data.remove(&k);
                self.resource_table_states.remove(&k);
            }
        }
        if self.resource_detail_data.len() > MAX_DETAIL_CACHE {
            let excess = self.resource_detail_data.len() - MAX_DETAIL_CACHE;
            let keys: Vec<_> = self.resource_detail_data.keys().take(excess).cloned().collect();
            for k in keys {
                self.resource_detail_data.remove(&k);
            }
        }
    }

    // -----------------------------------------------------------------------
    // T317: Dock panel rendering helpers
    // -----------------------------------------------------------------------

    /// Top-level dock panel renderer.
    /// Returns an empty div when there are no tabs, a collapsed strip when
    /// collapsed, or the full expanded dock panel otherwise.
    fn render_dock_panel(&self, cx: &mut Context<Self>) -> Div {
        if self.dock.tabs.is_empty() {
            return self.render_dock_empty_bar(cx);
        }
        if self.dock.collapsed {
            self.render_dock_collapsed(cx)
        } else {
            self.render_dock_expanded(cx)
        }
    }

    /// Render a minimal dock bar when no tabs are open, with a Terminal button.
    fn render_dock_empty_bar(&self, cx: &mut Context<Self>) -> Div {
        let dock_bg = self.theme.colors.surface.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let text_dim = self.theme.colors.text_secondary.to_gpui();

        let btn_id = ElementId::Name(SharedString::from("dock-open-terminal-btn"));
        let terminal_btn = div()
            .id(btn_id)
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .text_xs()
            .text_color(text_dim)
            .cursor_pointer()
            .hover(|s| s.text_color(gpui::rgb(0x60A5FA)))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.spawn_local_terminal(cx);
            }))
            .child(Icon::new(IconName::SquareTerminal).xsmall())
            .child(SharedString::from("Terminal"));

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(28.0))
            .bg(dock_bg)
            .border_t_1()
            .border_color(border_color)
            .px_2()
            .gap_1()
            .flex_shrink_0()
            .child(terminal_btn)
    }

    /// Render the collapsed dock: a thin 28px tab bar header with an expand button.
    fn render_dock_collapsed(&self, cx: &mut Context<Self>) -> Div {
        let dock_bg = self.theme.colors.surface.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let active_color = self.theme.colors.accent.to_gpui();
        let text_dim = self.theme.colors.text_secondary.to_gpui();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(28.0))
            .bg(dock_bg)
            .border_t_1()
            .border_color(border_color)
            .px_2()
            .gap_1()
            .flex_shrink_0();

        bar = self.render_dock_tab_labels(bar, cx, active_color, text_dim);

        // Expand button (chevron up)
        let expand_id = ElementId::Name(SharedString::from("dock-expand-btn"));
        let expand_btn = div()
            .id(expand_id)
            .ml_auto()
            .px_2()
            .text_xs()
            .text_color(text_dim)
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.dock.toggle_collapsed();
            }))
            .child(Icon::new(IconName::ChevronUp).xsmall());

        bar = bar.child(expand_btn);
        bar
    }

    /// Render the fully expanded dock panel with drag handle, tab bar, content,
    /// and collapse button.
    fn render_dock_expanded(&self, cx: &mut Context<Self>) -> Div {
        let dock_bg = self.theme.colors.surface.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let active_color = self.theme.colors.accent.to_gpui();
        let text_dim = self.theme.colors.text_secondary.to_gpui();
        let handle_color = self.theme.colors.border.to_gpui();
        let handle_hover = self.theme.colors.accent.to_gpui();

        div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(self.dock.height))
            .min_h(px(self.dock.height))
            .bg(dock_bg)
            .border_t_1()
            .border_color(border_color)
            .flex_shrink_0()
            // Drag handle at top
            .child(self.render_dock_drag_handle(handle_color, handle_hover, cx))
            // Tab bar with tabs + collapse button
            .child(self.render_dock_expanded_tab_bar(cx, active_color, text_dim, border_color))
            // Cluster context info bar
            .child(self.render_dock_context_bar(text_dim, border_color))
            // Active tab content area
            .child(self.render_dock_content(text_dim, cx))
    }

    /// Render the 6px drag handle at the top of the expanded dock.
    fn render_dock_drag_handle(
        &self,
        color: Rgba,
        hover_color: Rgba,
        cx: &mut Context<Self>,
    ) -> Div {
        div().child(
            div()
                .id("dock-drag-handle")
                .w_full()
                .h(px(6.0))
                .bg(color)
                .cursor(CursorStyle::ResizeUpDown)
                .hover(|s| s.bg(hover_color))
                .flex_shrink_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                        this.is_dragging_dock = true;
                        this.dock_drag_start_y = event.position.y.into();
                        this.dock_drag_start_height = this.dock.height;
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                    if this.is_dragging_dock {
                        let y: f32 = event.position.y.into();
                        let delta = this.dock_drag_start_y - y;
                        let new_height = (this.dock_drag_start_height + delta).clamp(100.0, 1200.0);
                        this.dock.height = new_height;
                        cx.notify();
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                        this.is_dragging_dock = false;
                    }),
                ),
        )
    }

    /// Render the cluster context info bar shown between tab bar and content.
    fn render_dock_context_bar(&self, text_dim: Rgba, border_color: Rgba) -> Div {
        let cluster_name = self.active_dashboard_cluster.as_deref().unwrap_or("none");
        let context_text = format!("Kubernetes cluster {cluster_name} in context.");
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(24.0))
            .border_b_1()
            .border_color(border_color)
            .px_2()
            .text_xs()
            .text_color(text_dim)
            .flex_shrink_0()
            .child(context_text)
    }

    /// Toggle between default dock height and near-fullscreen.
    fn toggle_dock_fullscreen(&mut self) {
        // Toggle between default 250px and expanded 800px.
        if self.dock.height > 500.0 {
            self.dock.height = 250.0;
        } else {
            self.dock.height = 800.0;
        }
    }

    /// Render the tab bar inside the expanded dock (tabs + collapse button).
    fn render_dock_expanded_tab_bar(
        &self,
        cx: &mut Context<Self>,
        active_color: Rgba,
        text_dim: Rgba,
        border_color: Rgba,
    ) -> Div {
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(28.0))
            .border_b_1()
            .border_color(border_color)
            .px_2()
            .gap_1()
            .flex_shrink_0();

        bar = self.render_dock_tab_items(bar, cx, active_color, text_dim);

        // "+" new terminal button
        let add_id = ElementId::Name(SharedString::from("dock-add-tab-btn"));
        let add_btn = div()
            .id(add_id)
            .px_1()
            .text_xs()
            .text_color(text_dim)
            .cursor_pointer()
            .hover(|s| s.text_color(gpui::rgb(0x60A5FA)))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.spawn_local_terminal(cx);
            }))
            .child(Icon::new(IconName::Plus).xsmall());
        bar = bar.child(add_btn);

        // Spacer to push right-side buttons
        bar = bar.child(div().flex_1());

        // Fullscreen toggle button
        let fullscreen_id = ElementId::Name(SharedString::from("dock-fullscreen-btn"));
        let fullscreen_btn = div()
            .id(fullscreen_id)
            .px_2()
            .text_xs()
            .text_color(text_dim)
            .cursor_pointer()
            .hover(|s| s.text_color(gpui::rgb(0x60A5FA)))
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.toggle_dock_fullscreen();
            }))
            .child(Icon::new(IconName::Maximize).xsmall());
        bar = bar.child(fullscreen_btn);

        // Collapse button (chevron down)
        let collapse_id = ElementId::Name(SharedString::from("dock-collapse-btn"));
        let collapse_btn = div()
            .id(collapse_id)
            .px_2()
            .text_xs()
            .text_color(text_dim)
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, _cx| {
                this.dock.toggle_collapsed();
            }))
            .child(Icon::new(IconName::ChevronDown).xsmall());

        bar = bar.child(collapse_btn);
        bar
    }

    /// Append clickable tab labels to the collapsed dock bar (no close buttons).
    fn render_dock_tab_labels(
        &self,
        mut bar: Div,
        cx: &mut Context<Self>,
        active_color: Rgba,
        text_dim: Rgba,
    ) -> Div {
        let active_id = self.dock.active_tab_id;
        for (idx, tab) in self.dock.tabs.iter().enumerate() {
            let is_active = Some(tab.id) == active_id;
            let tab_uuid = tab.id;
            let label_id = ElementId::Name(SharedString::from(format!("dock-tab-collapsed-{idx}")));
            let label_el = div()
                .id(label_id)
                .text_xs()
                .px_2()
                .py(px(2.0))
                .cursor_pointer()
                .rounded_sm()
                .when(is_active, |el| el.text_color(active_color))
                .when(!is_active, |el| el.text_color(text_dim))
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    this.dock.select_tab(tab_uuid);
                    if this.dock.collapsed {
                        this.dock.toggle_collapsed();
                    }
                }))
                .child(Self::dock_tab_icon(&tab.kind))
                .child(tab.label.clone());
            bar = bar.child(label_el);
        }
        bar
    }

    /// Return the appropriate icon for a dock tab kind.
    fn dock_tab_icon(kind: &DockTabKind) -> Icon {
        match kind {
            DockTabKind::Terminal { .. } => Icon::new(IconName::SquareTerminal).xsmall(),
            DockTabKind::LogViewer { .. } => Icon::new(IconName::File).xsmall(),
            DockTabKind::PortForwardManager => Icon::new(IconName::Globe).xsmall(),
        }
    }

    /// Append tab items with close buttons to the expanded dock tab bar.
    fn render_dock_tab_items(
        &self,
        mut bar: Div,
        cx: &mut Context<Self>,
        active_color: Rgba,
        text_dim: Rgba,
    ) -> Div {
        let active_id = self.dock.active_tab_id;
        for (idx, tab) in self.dock.tabs.iter().enumerate() {
            let is_active = Some(tab.id) == active_id;
            let tab_uuid = tab.id;

            let tab_el_id = ElementId::Name(SharedString::from(format!("dock-tab-{idx}")));

            let close_id = ElementId::Name(SharedString::from(format!("dock-tab-close-{idx}")));

            let close_btn = div()
                .id(close_id)
                .ml_1()
                .text_xs()
                .text_color(text_dim)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    this.dock.remove_tab(tab_uuid);
                    this.terminal_views.remove(&tab_uuid);
                    this.log_viewer_views.remove(&tab_uuid);
                    // Kill PTY process and clean up buffers.
                    if let Some(pty) = this.pty_processes.remove(&tab_uuid) {
                        if let Ok(mut p) = pty.lock() {
                            p.kill();
                        }
                    }
                    this.pty_output_buffers.remove(&tab_uuid);
                    // Remove from cluster_terminals mapping so a fresh terminal
                    // can be auto-spawned when the cluster tab is re-activated.
                    this.cluster_terminals.retain(|_ctx, &mut dock_id| dock_id != tab_uuid);
                }))
                .child(Icon::new(IconName::Close).xsmall());

            let tab_el = div()
                .id(tab_el_id)
                .flex()
                .flex_row()
                .items_center()
                .text_xs()
                .px_2()
                .py(px(2.0))
                .cursor_pointer()
                .rounded_sm()
                .when(is_active, |el| el.text_color(active_color))
                .when(!is_active, |el| el.text_color(text_dim))
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    this.dock.select_tab(tab_uuid);
                }))
                .child(Self::dock_tab_icon(&tab.kind))
                .child(tab.label.clone())
                .child(close_btn);

            bar = bar.child(tab_el);
        }
        bar
    }

    /// Render the active tab content area with the actual terminal or log viewer component.
    fn render_dock_content(&self, text_dim: Rgba, cx: &mut Context<Self>) -> Div {
        let active_tab =
            self.dock.active_tab_id.and_then(|id| self.dock.tabs.iter().find(|t| t.id == id));

        let Some(tab) = active_tab else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_dim)
                .text_sm()
                .child("No active tab");
        };

        let tab_id = tab.id;

        match &tab.kind {
            DockTabKind::Terminal { .. } => {
                if let Some(entity) = self.terminal_views.get(&tab_id) {
                    div().flex_1().size_full().overflow_hidden().child(entity.clone())
                } else {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(text_dim)
                        .text_sm()
                        .child("Terminal initializing...")
                }
            }
            DockTabKind::LogViewer { .. } => {
                if let Some(entity) = self.log_viewer_views.get(&tab_id) {
                    div().flex_1().size_full().overflow_hidden().child(entity.clone())
                } else {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(text_dim)
                        .text_sm()
                        .child("Log viewer initializing...")
                }
            }
            DockTabKind::PortForwardManager => self.render_port_forward_panel(cx),
        }
    }
}

// ---------------------------------------------------------------------------
// CRD Browser view rendering
// ---------------------------------------------------------------------------

impl AppShell {
    /// Render the CRD browser view for a cluster.
    fn render_crd_browser_content(
        &self,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        use baeus_core::crd::CrdScope;

        let border = self.theme.colors.border.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let success = self.theme.colors.success.to_gpui();

        let state = self.crd_browser.get(cluster_context);

        let mut container = div().flex().flex_col().flex_1().bg(bg);

        let crds = state.map(|s| s.filtered_crds()).unwrap_or_default();
        let total = state.map(|s| s.total_count()).unwrap_or(0);
        let namespaced = crds.iter().filter(|c| c.scope == CrdScope::Namespaced).count();
        let cluster_scoped = crds.len().saturating_sub(namespaced);

        // Toolbar
        container = container.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .px_4()
                .py_2()
                .gap(px(8.0))
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child("Custom Resource Definitions"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(format!("Total: {total}"))),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(accent)
                        .child(SharedString::from(format!("Namespaced: {namespaced}"))),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(success)
                        .child(SharedString::from(format!("Cluster: {cluster_scoped}"))),
                ),
        );

        // Loading / error states
        if let Some(state) = state {
            if state.loading {
                container = container.child(
                    div()
                        .flex()
                        .justify_center()
                        .py_8()
                        .text_sm()
                        .text_color(text_secondary)
                        .child("Discovering custom resource definitions..."),
                );
                return container;
            }
            if let Some(ref err) = state.error {
                let error_color = self.theme.colors.error.to_gpui();
                container = container.child(
                    div()
                        .flex()
                        .justify_center()
                        .py_8()
                        .text_sm()
                        .text_color(error_color)
                        .child(SharedString::from(format!("Error: {err}"))),
                );
                return container;
            }
        }

        if crds.is_empty() {
            container = container.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("No custom resource definitions found"),
            );
            return container;
        }

        // Table header
        container = container.child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .px_4()
                .py_1()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(200.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Kind"),
                )
                .child(
                    div()
                        .w(px(200.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Group"),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Version"),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Scope"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Full Name"),
                ),
        );

        // CRD rows
        for crd in &crds {
            let scope_label =
                if crd.scope == CrdScope::Namespaced { "Namespaced" } else { "Cluster" };
            let version =
                crd.preferred_version().map(|v| v.to_string()).unwrap_or_else(|| "—".to_string());

            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .px_4()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .w(px(200.0))
                            .text_xs()
                            .text_color(text)
                            .child(SharedString::from(crd.kind.clone())),
                    )
                    .child(
                        div()
                            .w(px(200.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(crd.group.clone())),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(version)),
                    )
                    .child(
                        div().w(px(100.0)).text_xs().text_color(text_secondary).child(scope_label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(crd.name.clone())),
                    ),
            );
        }

        container
    }
}

// ---------------------------------------------------------------------------
// Helm Releases view rendering
// ---------------------------------------------------------------------------

impl AppShell {
    /// Render the Helm Releases list view for a cluster.
    fn render_helm_releases_content(
        &self,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        use crate::views::helm_releases::HelmReleasesViewComponent;

        let border = self.theme.colors.border.to_gpui();
        let success = self.theme.colors.success.to_gpui();
        let error_color = self.theme.colors.error.to_gpui();
        let warning = self.theme.colors.warning.to_gpui();

        let state = self.helm_releases.get(cluster_context);

        let mut container = div().flex().flex_col().flex_1().bg(bg);

        // Toolbar with health badges
        let releases = state.map(|s| &s.releases[..]).unwrap_or(&[]);
        let healthy = releases.iter().filter(|r| r.status.is_healthy()).count();
        let unhealthy = releases.len().saturating_sub(healthy);

        container = container.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .px_4()
                .py_2()
                .gap(px(8.0))
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child("Helm Releases"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(success)
                        .child(SharedString::from(format!("Healthy: {healthy}"))),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(if unhealthy > 0 { error_color } else { text_secondary })
                        .child(SharedString::from(format!("Unhealthy: {unhealthy}"))),
                ),
        );

        // Loading / error / empty states
        if let Some(state) = state {
            if state.loading {
                container = container.child(
                    div()
                        .flex()
                        .justify_center()
                        .py_8()
                        .text_sm()
                        .text_color(text_secondary)
                        .child("Loading Helm releases..."),
                );
                return container;
            }
            if let Some(ref err) = state.error {
                container = container.child(
                    div()
                        .flex()
                        .justify_center()
                        .py_8()
                        .text_sm()
                        .text_color(error_color)
                        .child(SharedString::from(format!("Error: {err}"))),
                );
                return container;
            }
        }

        if releases.is_empty() {
            container = container.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("No Helm releases found in this cluster"),
            );
            return container;
        }

        // Table header
        container = container.child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .px_4()
                .py_1()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(150.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Name"),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Namespace"),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Chart"),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Version"),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("App Version"),
                )
                .child(
                    div()
                        .w(px(50.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Rev"),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Status"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary)
                        .child("Last Deployed"),
                ),
        );

        // Release rows
        for release in releases {
            let status_label = HelmReleasesViewComponent::status_label(&release.status);
            let status_color = match &release.status {
                baeus_helm::HelmReleaseStatus::Deployed => success,
                baeus_helm::HelmReleaseStatus::Failed => error_color,
                baeus_helm::HelmReleaseStatus::PendingInstall
                | baeus_helm::HelmReleaseStatus::PendingUpgrade
                | baeus_helm::HelmReleaseStatus::PendingRollback => warning,
                _ => text_secondary,
            };
            let deployed = release.last_deployed.format("%Y-%m-%d %H:%M").to_string();

            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .px_4()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .w(px(150.0))
                            .text_xs()
                            .text_color(text)
                            .child(SharedString::from(release.name.clone())),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(release.namespace.clone())),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(release.chart_name.clone())),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(release.chart_version.clone())),
                    )
                    .child(div().w(px(80.0)).text_xs().text_color(text_secondary).child(
                        SharedString::from(
                            release.app_version.as_deref().unwrap_or("—").to_string(),
                        ),
                    ))
                    .child(
                        div()
                            .w(px(50.0))
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(release.revision.to_string())),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .text_color(status_color)
                            .child(SharedString::from(status_label.to_string())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(text_secondary)
                            .child(SharedString::from(deployed)),
                    ),
            );
        }

        container
    }

    /// Render the Helm Install view for a cluster (chart search + install form).
    fn render_helm_install_content(
        &self,
        _cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        // For now, render a functional placeholder with guidance
        div().flex().flex_col().flex_1().bg(bg).child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py_8()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child("Install Helm Chart"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(text_secondary)
                        .child("Search for a chart to install"),
                ),
        )
    }
}

// ---------------------------------------------------------------------------
// Port Forward panel rendering + actions
// ---------------------------------------------------------------------------

impl AppShell {
    /// Render the port forward management panel for the dock.
    fn render_port_forward_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        use crate::components::port_forward::PortForwardDisplayState;

        let bg = self.theme.colors.background.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let text = self.theme.colors.text_primary.to_gpui();
        let text_dim = self.theme.colors.text_muted.to_gpui();
        let success = self.theme.colors.success.to_gpui();
        let error = self.theme.colors.error.to_gpui();

        let active_count = self.port_forward_panel.active_count();
        let count_text = SharedString::from(format!("{active_count} active"));

        // Header
        let header = div()
            .flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text)
                            .child("Port Forwards"),
                    )
                    .child(div().text_xs().text_color(text_dim).child(count_text)),
            );

        // Entries list
        let mut entries_div =
            div().id("pf-entries-scroll").flex().flex_col().w_full().flex_1().overflow_y_scroll();

        if self.port_forward_panel.entries.is_empty() {
            entries_div = entries_div.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(text_dim)
                    .child("No active port forwards"),
            );
        } else {
            for (idx, entry) in self.port_forward_panel.entries.iter().enumerate() {
                let port_text = SharedString::from(entry.port_display());
                let pod_text =
                    SharedString::from(format!("{}/{}", entry.namespace, entry.pod_name));
                let state_label = SharedString::from(entry.state.label().to_string());
                let state_color = match entry.state {
                    PortForwardDisplayState::Active => success,
                    PortForwardDisplayState::Error => error,
                    PortForwardDisplayState::Stopped => text_dim,
                    PortForwardDisplayState::Starting => accent,
                };

                let mut row = div()
                    .id(ElementId::Name(SharedString::from(format!("pf-entry-{idx}"))))
                    .flex()
                    .items_center()
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(div().text_sm().text_color(text).child(port_text))
                            .child(div().text_xs().text_color(text_dim).child(pod_text)),
                    )
                    .child(div().px_2().text_xs().text_color(state_color).child(state_label));

                if entry.is_active() || entry.state == PortForwardDisplayState::Starting {
                    let stop_id = ElementId::Name(SharedString::from(format!("pf-stop-{idx}")));
                    let entry_id = entry.id.clone();
                    row = row.child(
                        div()
                            .id(stop_id)
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(error)
                            .text_xs()
                            .text_color(gpui::rgb(0xFFFFFF))
                            .cursor_pointer()
                            .child("Stop")
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.port_forward_panel.stop_entry(&entry_id);
                                cx.notify();
                            })),
                    );
                }

                if entry.is_error() {
                    if let Some(ref msg) = entry.error_message {
                        row = row.child(
                            div()
                                .w_full()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(error)
                                .child(SharedString::from(msg.clone())),
                        );
                    }
                }

                entries_div = entries_div.child(row);
            }
        }

        div().flex().flex_col().w_full().h_full().bg(bg).child(header).child(entries_div)
    }

    /// Open the port forward manager in the dock panel.
    pub fn open_port_forward_in_dock(&mut self, cx: &mut Context<Self>) {
        let kind = DockTabKind::PortForwardManager;
        let tab_id = self.dock.add_tab(kind);
        self.dock.select_tab(tab_id);
        if self.dock.collapsed {
            self.dock.toggle_collapsed();
        }
        cx.notify();
    }

    /// Start a port forward for a pod and add it to the panel.
    pub fn start_port_forward(
        &mut self,
        pod_name: &str,
        namespace: &str,
        local_port: u16,
        remote_port: u16,
        cx: &mut Context<Self>,
    ) {
        use crate::components::port_forward::{PortForwardDisplayState, PortForwardEntry};

        let entry = PortForwardEntry {
            id: uuid::Uuid::new_v4().to_string(),
            pod_name: pod_name.to_string(),
            namespace: namespace.to_string(),
            local_port,
            remote_port,
            state: PortForwardDisplayState::Active,
            error_message: None,
        };
        self.port_forward_panel.add_entry(entry);
        self.open_port_forward_in_dock(cx);
    }
}

// ---------------------------------------------------------------------------
// T319 / T320: Wire terminal and log actions to the dock panel
// ---------------------------------------------------------------------------

impl AppShell {
    /// T319: Open a terminal exec session in the dock panel.
    ///
    /// Creates a `DockTabKind::Terminal` tab, selects it, and auto-expands
    /// the dock if it is currently collapsed. Called from resource detail
    /// "Terminal" / "Shell" actions.
    pub fn open_terminal_in_dock(
        &mut self,
        pod: &str,
        container: &str,
        cluster_context: &str,
        cx: &mut Context<Self>,
    ) {
        let kind = DockTabKind::Terminal {
            pod: pod.to_string(),
            container: container.to_string(),
            cluster: cluster_context.to_string(),
        };
        let tab_id = self.dock.add_tab(kind);
        self.dock.select_tab(tab_id);
        if self.dock.collapsed {
            self.dock.toggle_collapsed();
        }

        // Create a TerminalViewComponent entity for this tab.
        let state = TerminalViewState::for_pod_exec(
            uuid::Uuid::nil(), // placeholder cluster UUID
            "",                // namespace not tracked in DockTabKind
            pod,
            Some(container),
        );
        let theme = self.theme.clone();
        let entity = cx.new(|cx| TerminalViewComponent::new_with_cx(state, theme, cx));
        self.terminal_views.insert(tab_id, entity);
    }

    /// T320: Open a log viewer session in the dock panel.
    ///
    /// Creates a `DockTabKind::LogViewer` tab, selects it, and auto-expands
    /// the dock if it is currently collapsed. Called from resource detail
    /// "Logs" actions.
    pub fn open_logs_in_dock(
        &mut self,
        pod: &str,
        container: &str,
        cluster_context: &str,
        namespace: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.open_logs_in_dock_with_owner(pod, container, cluster_context, namespace, None, cx);
    }

    /// Open logs in dock with optional owner metadata.
    pub fn open_logs_in_dock_with_owner(
        &mut self,
        pod: &str,
        container: &str,
        cluster_context: &str,
        namespace: Option<&str>,
        owner: Option<(&str, &str)>, // (kind, name)
        cx: &mut Context<Self>,
    ) {
        let kind = DockTabKind::LogViewer {
            pod: pod.to_string(),
            container: container.to_string(),
            cluster: cluster_context.to_string(),
        };
        let tab_id = self.dock.add_tab(kind);
        self.dock.select_tab(tab_id);
        if self.dock.collapsed {
            self.dock.toggle_collapsed();
        }

        // Create a LogViewerView entity for this tab with metadata.
        let mut log_state = LogViewerState::new(10_000);
        let ns = namespace
            .map(|s| s.to_string())
            .or_else(|| self.header.namespace_selector.active_namespace.clone());
        log_state.namespace = ns.clone().unwrap_or_default();
        log_state.pod_name = pod.to_string();
        if let Some((ok, on)) = owner {
            log_state.owner_kind = Some(ok.to_string());
            log_state.owner_name = Some(on.to_string());
        }
        let theme = self.theme.clone();
        let entity = cx.new(|_cx| {
            let mut view = LogViewerView::new(log_state, theme);
            view.container_name = container.to_string();
            view.cluster_context = cluster_context.to_string();
            view
        });
        self.log_viewer_views.insert(tab_id, entity.clone());

        // Start streaming logs from the kube API with polling
        if let Some(client) = self.active_clients.get(cluster_context).cloned() {
            let mut pod_name = pod.to_string();
            let container_name = container.to_string();
            let tokio_handle = cx.global::<GpuiTokioHandle>().0.clone();
            let entity_for_stream = entity.downgrade();

            cx.spawn(async move |_this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
                use baeus_core::logs::{LogLine, LogStreamState, parse_k8s_log_timestamp};
                use k8s_openapi::api::core::v1::Pod;
                use kube::api::{Api, LogParams};

                let api: Api<Pod> = if let Some(ref ns) = ns {
                    Api::namespaced(client, ns)
                } else {
                    Api::default_namespaced(client)
                };

                // Initial fetch: tail 500 lines with timestamps
                let mut params = LogParams {
                    follow: false,
                    tail_lines: Some(500),
                    timestamps: true,
                    ..Default::default()
                };
                if !container_name.is_empty() {
                    params.container = Some(container_name.clone());
                }

                let result = tokio_handle
                    .spawn({
                        let api = api.clone();
                        let pod_name = pod_name.clone();
                        async move { api.logs(&pod_name, &params).await }
                    })
                    .await;

                let mut last_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

                match result {
                    Ok(Ok(logs)) => {
                        // Parse log lines with timestamps
                        let parsed_lines: Vec<(Option<chrono::DateTime<chrono::Utc>>, String)> =
                            logs.lines()
                                .map(|line| {
                                    if let Some(ts) = parse_k8s_log_timestamp(line) {
                                        let space_idx = line.find(' ').unwrap_or(0);
                                        (Some(ts), line[space_idx + 1..].to_string())
                                    } else {
                                        (None, line.to_string())
                                    }
                                })
                                .collect();

                        // Get last timestamp for polling
                        for (ts, _) in parsed_lines.iter().rev() {
                            if let Some(t) = ts {
                                last_timestamp = Some(*t);
                                break;
                            }
                        }

                        entity_for_stream
                            .update(cx, |view: &mut LogViewerView, _cx| {
                                for (ts, content) in &parsed_lines {
                                    view.state.push_line(LogLine {
                                        content: content.clone(),
                                        container_name: container_name.clone(),
                                        pod_name: pod_name.clone(),
                                        timestamp: *ts,
                                        source_color_index: 0,
                                    });
                                }
                                if let Some(lts) = last_timestamp {
                                    view.state.last_fetch_time = Some(lts.to_rfc3339());
                                }
                                view.state.set_stream_state(LogStreamState::Streaming);
                            })
                            .ok();
                    }
                    _ => {
                        entity_for_stream
                            .update(cx, |view: &mut LogViewerView, _cx| {
                                view.state.set_stream_state(LogStreamState::Error);
                            })
                            .ok();
                        return;
                    }
                }

                // Polling loop: fetch new lines every 10 seconds
                loop {
                    // Check if entity is still alive and read flags
                    let check = entity_for_stream.update(cx, |view: &mut LogViewerView, _cx| {
                        let refetch = view.state.needs_refetch;
                        let previous = view.state.previous_container;
                        let new_pod = view.state.switch_to_pod.take();
                        if refetch {
                            view.state.needs_refetch = false;
                        }
                        (refetch, previous, new_pod)
                    });
                    let Ok((needs_refetch, previous, new_pod)) = check else {
                        break;
                    };

                    // If switching to a different pod, update the pod_name for fetch
                    if let Some(ref np) = new_pod {
                        pod_name = np.clone();
                    }

                    if needs_refetch {
                        // Full re-fetch (e.g. previous container toggled)
                        let refetch_result = tokio_handle
                            .spawn({
                                let api = api.clone();
                                let pod_name = pod_name.clone();
                                let container_name = container_name.clone();
                                async move {
                                    let mut params = LogParams {
                                        follow: false,
                                        tail_lines: Some(500),
                                        timestamps: true,
                                        previous,
                                        ..Default::default()
                                    };
                                    if !container_name.is_empty() {
                                        params.container = Some(container_name);
                                    }
                                    api.logs(&pod_name, &params).await
                                }
                            })
                            .await;

                        match refetch_result {
                            Ok(Ok(logs)) => {
                                let parsed_lines: Vec<(
                                    Option<chrono::DateTime<chrono::Utc>>,
                                    String,
                                )> = logs
                                    .lines()
                                    .map(|line| {
                                        if let Some(ts) = parse_k8s_log_timestamp(line) {
                                            let space_idx = line.find(' ').unwrap_or(0);
                                            (Some(ts), line[space_idx + 1..].to_string())
                                        } else {
                                            (None, line.to_string())
                                        }
                                    })
                                    .collect();

                                for (ts, _) in parsed_lines.iter().rev() {
                                    if let Some(t) = ts {
                                        last_timestamp = Some(*t);
                                        break;
                                    }
                                }

                                let cn = container_name.clone();
                                let pn = pod_name.clone();
                                entity_for_stream
                                    .update(cx, |view: &mut LogViewerView, _cx| {
                                        view.state.clear();
                                        for (ts, content) in &parsed_lines {
                                            view.state.push_line(LogLine {
                                                content: content.clone(),
                                                container_name: cn.clone(),
                                                pod_name: pn.clone(),
                                                timestamp: *ts,
                                                source_color_index: 0,
                                            });
                                        }
                                        if previous {
                                            view.state.set_stream_state(LogStreamState::Stopped);
                                        } else {
                                            view.state.set_stream_state(LogStreamState::Streaming);
                                        }
                                    })
                                    .ok();

                                // If showing previous container, don't continue polling
                                if previous {
                                    break;
                                }
                                continue;
                            }
                            _ => {
                                entity_for_stream
                                    .update(cx, |view: &mut LogViewerView, _cx| {
                                        view.state.set_stream_state(LogStreamState::Error);
                                    })
                                    .ok();
                                continue;
                            }
                        }
                    }

                    let since_time = last_timestamp;
                    // Sleep + fetch inside tokio runtime (can't use tokio::time::sleep on main thread)
                    let poll_result = tokio_handle
                        .spawn({
                            let api = api.clone();
                            let pod_name = pod_name.clone();
                            let container_name = container_name.clone();
                            async move {
                                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                                let mut params = LogParams {
                                    follow: false,
                                    timestamps: true,
                                    since_time,
                                    ..Default::default()
                                };
                                if !container_name.is_empty() {
                                    params.container = Some(container_name);
                                }
                                api.logs(&pod_name, &params).await
                            }
                        })
                        .await;

                    match poll_result {
                        Ok(Ok(logs)) if !logs.is_empty() => {
                            let parsed_lines: Vec<(Option<chrono::DateTime<chrono::Utc>>, String)> =
                                logs.lines()
                                    .filter(|line| !line.is_empty())
                                    .map(|line| {
                                        if let Some(ts) = parse_k8s_log_timestamp(line) {
                                            let space_idx = line.find(' ').unwrap_or(0);
                                            (Some(ts), line[space_idx + 1..].to_string())
                                        } else {
                                            (None, line.to_string())
                                        }
                                    })
                                    .collect();

                            if parsed_lines.is_empty() {
                                continue;
                            }

                            // Update last_timestamp for next poll
                            for (ts, _) in parsed_lines.iter().rev() {
                                if let Some(t) = ts {
                                    last_timestamp = Some(*t);
                                    break;
                                }
                            }

                            let container_name = container_name.clone();
                            let pod_name = pod_name.clone();
                            let lts = last_timestamp;
                            entity_for_stream
                                .update(cx, |view: &mut LogViewerView, _cx| {
                                    // Skip the first line if it duplicates the last line
                                    // (sinceTime is inclusive)
                                    let skip = if !parsed_lines.is_empty() {
                                        if let Some(last) = view.state.buffer.lines().last() {
                                            parsed_lines[0].1 == last.content
                                                && parsed_lines[0].0 == last.timestamp
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };
                                    for (i, (ts, content)) in parsed_lines.iter().enumerate() {
                                        if i == 0 && skip {
                                            continue;
                                        }
                                        view.state.push_line(LogLine {
                                            content: content.clone(),
                                            container_name: container_name.clone(),
                                            pod_name: pod_name.clone(),
                                            timestamp: *ts,
                                            source_color_index: 0,
                                        });
                                    }
                                    if let Some(lts) = lts {
                                        view.state.last_fetch_time = Some(lts.to_rfc3339());
                                    }
                                })
                                .ok();
                        }
                        _ => {
                            // Non-fatal: just skip this poll cycle
                        }
                    }
                }
            })
            .detach();
        }
    }

    /// Open a new local shell terminal in the dock panel.
    ///
    /// Delegates to `spawn_terminal_for_cluster` using the active dashboard cluster.
    pub fn spawn_local_terminal(&mut self, cx: &mut Context<Self>) {
        let cluster = self.active_dashboard_cluster.clone().unwrap_or_default();
        if cluster.is_empty() {
            return;
        }
        self.spawn_terminal_for_cluster(&cluster, cx);
    }

    /// Spawn a per-cluster terminal in the dock. If a terminal already exists
    /// for this cluster, this is a no-op.
    /// Write a temporary kubeconfig file for an EKS cluster so terminals can use it.
    /// Returns the path to the generated kubeconfig file.
    fn generate_eks_kubeconfig_file_with_role(
        &self,
        context_name: &str,
        cluster: &baeus_core::aws_eks::EksCluster,
        role_arn: Option<&str>,
    ) -> Result<String, String> {
        // Use ~/.baeus/eks-kubeconfigs/ to avoid spaces in path (macOS "Application Support" breaks shell)
        let config_dir = dirs::home_dir()
            .ok_or_else(|| "No home directory".to_string())?
            .join(".baeus")
            .join("eks-kubeconfigs");
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create EKS kubeconfig dir: {e}"))?;

        let safe_name: String = context_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let path = config_dir.join(format!("{safe_name}.yaml"));

        let ca_data = cluster.certificate_authority_data.as_deref().unwrap_or("");

        // Build args — include --role-arn if a role was specified
        let role_args = if let Some(arn) = role_arn {
            format!("      - --role-arn\n      - {arn}")
        } else {
            String::new()
        };

        let kubeconfig_yaml = format!(
            r#"apiVersion: v1
kind: Config
current-context: {context_name}
clusters:
- name: {name}
  cluster:
    server: {endpoint}
    certificate-authority-data: {ca_data}
contexts:
- name: {context_name}
  context:
    cluster: {name}
    user: {context_name}-user
users:
- name: {context_name}-user
  user:
    exec:
      apiVersion: client.authentication.k8s.io/v1beta1
      command: aws
      args:
      - eks
      - get-token
      - --cluster-name
      - {name}
      - --region
      - {region}
{role_args}
"#,
            context_name = context_name,
            name = cluster.name,
            endpoint = cluster.endpoint,
            ca_data = ca_data,
            region = cluster.region,
            role_args = role_args,
        );

        std::fs::write(&path, &kubeconfig_yaml)
            .map_err(|e| format!("Failed to write EKS kubeconfig: {e}"))?;

        // Restrict permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(path.to_string_lossy().to_string())
    }

    fn spawn_terminal_for_cluster(&mut self, cluster_context: &str, cx: &mut Context<Self>) {
        // Skip if terminal already exists for this cluster.
        if self.cluster_terminals.contains_key(cluster_context) {
            return;
        }

        // For EKS clusters, write a temp kubeconfig so the terminal can use it.
        if cluster_context.starts_with("eks:")
            && !self.kubeconfig_paths.contains_key(cluster_context)
        {
            if let Some((cluster, _creds, role_arn)) = self.eks_cluster_data.get(cluster_context) {
                if let Ok(path) = self.generate_eks_kubeconfig_file_with_role(
                    cluster_context,
                    cluster,
                    role_arn.as_deref(),
                ) {
                    self.kubeconfig_paths.insert(cluster_context.to_string(), path);
                }
            }
        }

        let kubeconfig_paths_for_terminal = self.kubeconfig_paths.clone();

        let cluster = cluster_context.to_string();
        let kind = DockTabKind::Terminal {
            pod: String::new(),
            container: String::new(),
            cluster: cluster.clone(),
        };
        let tab_id = self.dock.add_tab(kind);
        self.dock.select_tab(tab_id);
        if self.dock.collapsed {
            self.dock.toggle_collapsed();
        }

        // Record the cluster → dock terminal mapping.
        self.cluster_terminals.insert(cluster.clone(), tab_id);

        // Create a TerminalViewComponent entity for this tab.
        let state = TerminalViewState::for_local_shell();
        let theme = self.theme.clone();
        let entity = cx.new(|cx| TerminalViewComponent::new_with_cx(state, theme, cx));
        self.terminal_views.insert(tab_id, entity.clone());

        // Build env vars for the shell: set KUBECONFIG to the config file
        // containing the active cluster's context.
        let mut env_vars: Vec<(String, String)> = Vec::new();
        if !cluster.is_empty() {
            if let Some(config_path) = self.kubeconfig_paths.get(&cluster) {
                env_vars.push(("KUBECONFIG".to_string(), config_path.clone()));
            }
        }

        // Spawn a real PTY process with cluster-scoped env vars.
        let env_refs: Vec<(&str, &str)> =
            env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        match PtyProcess::spawn_shell_with_env(24, 80, &env_refs) {
            Ok(pty) => {
                let output_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
                let reader = pty.reader_handle();
                let writer = pty.writer_handle();

                // Background thread: read PTY output into shared buffer.
                let buf_clone = Arc::clone(&output_buf);
                std::thread::spawn(move || {
                    let mut tmp = [0u8; 4096];
                    loop {
                        let n = {
                            let mut r = match reader.lock() {
                                Ok(r) => r,
                                Err(_) => break,
                            };
                            match r.read(&mut tmp) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            }
                        };
                        if let Ok(mut buf) = buf_clone.lock() {
                            buf.extend_from_slice(&tmp[..n]);
                        }
                    }
                });

                let pty_arc = Arc::new(Mutex::new(pty));
                self.pty_processes.insert(tab_id, Arc::clone(&pty_arc));
                self.pty_output_buffers.insert(tab_id, Arc::clone(&output_buf));

                // Set connection state to Connected.
                entity.update(cx, |view, _cx| {
                    view.state.connection_state =
                        crate::components::terminal_view::TerminalConnectionState::Connected;
                });

                // If an active cluster context is set, switch kubectl to that context.
                // For EKS wizard clusters (eks:region:name), write a temp kubeconfig
                // since the context doesn't exist in the user's kubeconfig files.
                if !cluster.is_empty() {
                    let switch_cmd = if cluster.starts_with("eks:") {
                        // EKS cluster — set KUBECONFIG to temp file if it exists
                        if let Some(path) = kubeconfig_paths_for_terminal.get(&cluster) {
                            format!(
                                "export KUBECONFIG='{}' && kubectl config use-context '{}' && clear\n",
                                path, cluster
                            )
                        } else {
                            format!(
                                "echo 'EKS cluster {} — no kubeconfig available (reconnect via EKS wizard)' && clear\n",
                                cluster
                            )
                        }
                    } else {
                        format!("kubectl config use-context {} && clear\n", cluster)
                    };
                    let cmd_writer = pty_arc.clone();
                    // Slight delay so the shell prompt has time to initialize.
                    let cmd_bytes = switch_cmd.into_bytes();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if let Ok(pty) = cmd_writer.lock() {
                            let _ = pty.write_input(&cmd_bytes);
                        }
                    });
                }

                // Poll loop: every 16ms drain output buffer → process_output,
                // and drain pending keyboard input → pty.write_input.
                let entity_weak = entity.downgrade();
                let buf_for_poll = Arc::clone(&output_buf);
                let writer_for_poll = writer;
                cx.spawn(async move |_this, cx| {
                    loop {
                        cx.background_executor().timer(std::time::Duration::from_millis(16)).await;

                        // Drain output buffer and feed to emulator.
                        let data = {
                            let mut buf = match buf_for_poll.lock() {
                                Ok(b) => b,
                                Err(_) => break,
                            };
                            if buf.is_empty() { Vec::new() } else { std::mem::take(&mut *buf) }
                        };

                        let alive = cx.update(|cx| {
                            entity_weak
                                .update(cx, |view, cx| {
                                    if !data.is_empty() {
                                        view.process_output(&data);
                                    }
                                    // Drain pending keyboard input and write to PTY.
                                    let input = view.take_pending_input();
                                    if !input.is_empty() {
                                        if let Ok(mut w) = writer_for_poll.lock() {
                                            let _ = std::io::Write::write_all(&mut *w, &input);
                                            let _ = std::io::Write::flush(&mut *w);
                                        }
                                    }
                                    cx.notify();
                                })
                                .is_ok()
                        });

                        match alive {
                            Ok(true) => {}
                            _ => break,
                        }
                    }
                })
                .detach();
            }
            Err(e) => {
                tracing::error!("Failed to spawn PTY: {e}");
                entity.update(cx, |view, _cx| {
                    view.state.connection_state =
                        crate::components::terminal_view::TerminalConnectionState::Error(format!(
                            "Failed to spawn shell: {e}"
                        ));
                });
            }
        }
    }

    /// Tear down the terminal for a cluster if no workspace tabs remain for it.
    fn cleanup_cluster_terminal_if_last(&mut self, cluster_context: &str) {
        // Check if any remaining workspace tabs belong to this cluster.
        let has_remaining = self
            .workspace
            .tabs
            .iter()
            .any(|tab| tab.target.cluster_context() == Some(cluster_context));
        if has_remaining {
            return;
        }

        // No more tabs for this cluster — kill its terminal.
        if let Some(dock_id) = self.cluster_terminals.remove(cluster_context) {
            self.dock.remove_tab(dock_id);
            self.terminal_views.remove(&dock_id);
            self.log_viewer_views.remove(&dock_id);
            if let Some(pty) = self.pty_processes.remove(&dock_id) {
                if let Ok(mut p) = pty.lock() {
                    p.kill();
                }
            }
            self.pty_output_buffers.remove(&dock_id);
        }
    }
}
