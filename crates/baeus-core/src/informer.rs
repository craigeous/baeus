// Informer/reflector management and caching

use crate::resource::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Configuration for a single informer/watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InformerConfig {
    /// The cluster this informer targets.
    pub cluster_id: Uuid,
    /// The Kubernetes resource kind to watch (e.g. "Pod", "Deployment").
    pub resource_kind: String,
    /// Optional namespace scope. `None` means cluster-wide.
    pub namespace: Option<String>,
    /// The API version for the resource (e.g. "v1", "apps/v1").
    pub api_version: String,
}

/// Represents the runtime state of an informer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InformerState {
    Idle,
    Running,
    Reconnecting,
    Stopped,
    Error(String),
}

/// Tracks a registered informer: its config, current state, and optional cancellation token.
#[derive(Debug, Clone)]
struct InformerEntry {
    config: InformerConfig,
    state: InformerState,
    /// Cancellation token attached when a watcher task is spawned for this entry.
    /// `None` until `set_cancel_token` is called by the UI after spawning the task.
    cancel: Option<CancellationToken>,
}

/// Key for looking up cached resources: (cluster_id, resource_kind).
type CacheKey = (Uuid, String);

/// Maximum number of resources cached per kind per cluster.
const MAX_RESOURCES_PER_KIND: usize = 50_000;
/// Maximum total cached resources across all kinds and clusters.
const MAX_TOTAL_CACHED_RESOURCES: usize = 200_000;

/// Manages the lifecycle of multiple informers across clusters,
/// including an in-memory resource cache populated by watchers.
#[derive(Debug, Default)]
pub struct InformerManager {
    informers: HashMap<Uuid, InformerEntry>,
    cache: HashMap<CacheKey, Vec<Resource>>,
}

impl InformerManager {
    pub fn new() -> Self {
        Self { informers: HashMap::new(), cache: HashMap::new() }
    }

    /// Register a new informer with the given config. Returns a unique id
    /// that can be used to reference the informer later.
    pub fn register(&mut self, config: InformerConfig) -> Uuid {
        let id = Uuid::new_v4();
        self.informers
            .insert(id, InformerEntry { config, state: InformerState::Idle, cancel: None });
        id
    }

    /// Unregister (remove) an informer by id. Cancels any attached token before removal.
    /// Returns `true` if it existed.
    pub fn unregister(&mut self, id: &Uuid) -> bool {
        if let Some(entry) = self.informers.remove(id) {
            if let Some(token) = entry.cancel {
                token.cancel();
            }
            true
        } else {
            false
        }
    }

    /// Get the current state of an informer.
    pub fn state(&self, id: &Uuid) -> Option<&InformerState> {
        self.informers.get(id).map(|e| &e.state)
    }

    /// Get the config of an informer.
    pub fn config(&self, id: &Uuid) -> Option<&InformerConfig> {
        self.informers.get(id).map(|e| &e.config)
    }

    /// Set the state of an informer. Returns `false` if the id was not found.
    pub fn set_state(&mut self, id: &Uuid, state: InformerState) -> bool {
        if let Some(entry) = self.informers.get_mut(id) {
            entry.state = state;
            true
        } else {
            false
        }
    }

    /// Count informers that are in the `Running` or `Reconnecting` state.
    pub fn active_count(&self) -> usize {
        self.informers
            .values()
            .filter(|e| matches!(e.state, InformerState::Running | InformerState::Reconnecting))
            .count()
    }

    /// Total number of registered informers.
    pub fn total_count(&self) -> usize {
        self.informers.len()
    }

    /// Set all informers to `Stopped`.
    pub fn stop_all(&mut self) {
        for entry in self.informers.values_mut() {
            entry.state = InformerState::Stopped;
        }
    }

    /// Attach a cancellation token to a registered informer.
    ///
    /// Called by the UI when it spawns a watcher task for the corresponding entry.
    /// Returns `true` if the entry exists (and the token was stored), `false` otherwise.
    pub fn set_cancel_token(&mut self, id: &Uuid, token: CancellationToken) -> bool {
        if let Some(entry) = self.informers.get_mut(id) {
            entry.cancel = Some(token);
            true
        } else {
            false
        }
    }

    /// Read the token for a registered informer (for callers that need to clone
    /// it into a spawned task).
    pub fn cancel_token(&self, id: &Uuid) -> Option<&CancellationToken> {
        self.informers.get(id).and_then(|e| e.cancel.as_ref())
    }

    /// Return ids of all informers targeting a given cluster.
    pub fn informers_for_cluster(&self, cluster_id: &Uuid) -> Vec<Uuid> {
        self.informers
            .iter()
            .filter(|(_, e)| &e.config.cluster_id == cluster_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Register the standard set of watchers for a cluster: Namespace, Node, Pod, Event.
    /// Returns the ids of the registered informers.
    pub fn register_standard_watchers(&mut self, cluster_id: Uuid) -> Vec<Uuid> {
        let configs = vec![
            InformerConfig {
                cluster_id,
                resource_kind: "Namespace".to_string(),
                namespace: None,
                api_version: "v1".to_string(),
            },
            InformerConfig {
                cluster_id,
                resource_kind: "Node".to_string(),
                namespace: None,
                api_version: "v1".to_string(),
            },
            InformerConfig {
                cluster_id,
                resource_kind: "Pod".to_string(),
                namespace: None, // watch all namespaces
                api_version: "v1".to_string(),
            },
            InformerConfig {
                cluster_id,
                resource_kind: "Event".to_string(),
                namespace: None,
                api_version: "v1".to_string(),
            },
        ];

        configs.into_iter().map(|c| self.register(c)).collect()
    }

    /// Stop all informers targeting a specific cluster and clear its cache.
    ///
    /// In addition to transitioning state to `Stopped`, cancels every
    /// `CancellationToken` attached via `set_cancel_token`, stopping the
    /// corresponding watcher loops deterministically.
    pub fn stop_for_cluster(&mut self, cluster_id: &Uuid) {
        let ids: Vec<Uuid> = self.informers_for_cluster(cluster_id);
        for id in &ids {
            if let Some(entry) = self.informers.get(id) {
                if let Some(token) = &entry.cancel {
                    token.cancel();
                }
            }
            self.set_state(id, InformerState::Stopped);
        }
        self.clear_cache_for_cluster(cluster_id);
    }

    /// Update the cache for a specific resource kind within a cluster.
    /// Enforces per-kind and total cache size limits to prevent memory exhaustion.
    pub fn update_cache(&mut self, cluster_id: Uuid, kind: &str, mut resources: Vec<Resource>) {
        if resources.len() > MAX_RESOURCES_PER_KIND {
            tracing::warn!(
                "Truncating {} cache for cluster {}: {} resources exceeds limit of {}",
                kind,
                cluster_id,
                resources.len(),
                MAX_RESOURCES_PER_KIND,
            );
            resources.truncate(MAX_RESOURCES_PER_KIND);
        }

        self.cache.insert((cluster_id, kind.to_string()), resources);

        // Check total cache size and warn if approaching limit.
        let total: usize = self.cache.values().map(|v| v.len()).sum();
        if total > MAX_TOTAL_CACHED_RESOURCES {
            tracing::warn!(
                "Total cached resources ({}) exceeds limit ({}); consider reducing watched kinds",
                total,
                MAX_TOTAL_CACHED_RESOURCES,
            );
        }
    }

    /// Get cached resources for a specific kind within a cluster.
    pub fn cached_resources(&self, cluster_id: &Uuid, kind: &str) -> Option<&Vec<Resource>> {
        self.cache.get(&(*cluster_id, kind.to_string()))
    }

    /// Clear all cached resources for a cluster.
    pub fn clear_cache_for_cluster(&mut self, cluster_id: &Uuid) {
        self.cache.retain(|(cid, _), _| cid != cluster_id);
    }

    /// Returns the total number of cached resource entries across all kinds for a cluster.
    pub fn cache_size_for_cluster(&self, cluster_id: &Uuid) -> usize {
        self.cache
            .iter()
            .filter(|((cid, _), _)| cid == cluster_id)
            .map(|(_, resources)| resources.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod_config(cluster_id: Uuid) -> InformerConfig {
        InformerConfig {
            cluster_id,
            resource_kind: "Pod".to_string(),
            namespace: Some("default".to_string()),
            api_version: "v1".to_string(),
        }
    }

    fn deploy_config(cluster_id: Uuid) -> InformerConfig {
        InformerConfig {
            cluster_id,
            resource_kind: "Deployment".to_string(),
            namespace: Some("kube-system".to_string()),
            api_version: "apps/v1".to_string(),
        }
    }

    fn cluster_scoped_config(cluster_id: Uuid) -> InformerConfig {
        InformerConfig {
            cluster_id,
            resource_kind: "Node".to_string(),
            namespace: None,
            api_version: "v1".to_string(),
        }
    }

    #[test]
    fn test_new_manager_is_empty() {
        let mgr = InformerManager::new();
        assert_eq!(mgr.total_count(), 0);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_default_manager_is_empty() {
        let mgr = InformerManager::default();
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn test_register_informer() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(pod_config(cluster_id));

        assert_eq!(mgr.total_count(), 1);
        assert_eq!(mgr.state(&id), Some(&InformerState::Idle));
    }

    #[test]
    fn test_register_returns_unique_ids() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id1 = mgr.register(pod_config(cluster_id));
        let id2 = mgr.register(deploy_config(cluster_id));
        assert_ne!(id1, id2);
        assert_eq!(mgr.total_count(), 2);
    }

    #[test]
    fn test_unregister_informer() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(pod_config(cluster_id));

        assert!(mgr.unregister(&id));
        assert_eq!(mgr.total_count(), 0);
        assert!(mgr.state(&id).is_none());
    }

    #[test]
    fn test_unregister_nonexistent() {
        let mut mgr = InformerManager::new();
        let fake_id = Uuid::new_v4();
        assert!(!mgr.unregister(&fake_id));
    }

    #[test]
    fn test_state_of_nonexistent_informer() {
        let mgr = InformerManager::new();
        assert!(mgr.state(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_get_config() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(pod_config(cluster_id));

        let config = mgr.config(&id).unwrap();
        assert_eq!(config.cluster_id, cluster_id);
        assert_eq!(config.resource_kind, "Pod");
        assert_eq!(config.namespace.as_deref(), Some("default"));
        assert_eq!(config.api_version, "v1");
    }

    #[test]
    fn test_config_of_nonexistent() {
        let mgr = InformerManager::new();
        assert!(mgr.config(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_set_state() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(pod_config(cluster_id));

        assert!(mgr.set_state(&id, InformerState::Running));
        assert_eq!(mgr.state(&id), Some(&InformerState::Running));
    }

    #[test]
    fn test_set_state_nonexistent() {
        let mut mgr = InformerManager::new();
        assert!(!mgr.set_state(&Uuid::new_v4(), InformerState::Running));
    }

    #[test]
    fn test_active_count_running() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let id1 = mgr.register(pod_config(cluster_id));
        let id2 = mgr.register(deploy_config(cluster_id));
        let _id3 = mgr.register(cluster_scoped_config(cluster_id));

        mgr.set_state(&id1, InformerState::Running);
        mgr.set_state(&id2, InformerState::Running);
        // id3 stays Idle

        assert_eq!(mgr.active_count(), 2);
    }

    #[test]
    fn test_active_count_includes_reconnecting() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let id1 = mgr.register(pod_config(cluster_id));
        let id2 = mgr.register(deploy_config(cluster_id));

        mgr.set_state(&id1, InformerState::Running);
        mgr.set_state(&id2, InformerState::Reconnecting);

        assert_eq!(mgr.active_count(), 2);
    }

    #[test]
    fn test_active_count_excludes_stopped_and_error() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let id1 = mgr.register(pod_config(cluster_id));
        let id2 = mgr.register(deploy_config(cluster_id));
        let id3 = mgr.register(cluster_scoped_config(cluster_id));

        mgr.set_state(&id1, InformerState::Stopped);
        mgr.set_state(&id2, InformerState::Error("timeout".to_string()));
        mgr.set_state(&id3, InformerState::Running);

        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn test_stop_all() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let id1 = mgr.register(pod_config(cluster_id));
        let id2 = mgr.register(deploy_config(cluster_id));
        let id3 = mgr.register(cluster_scoped_config(cluster_id));

        mgr.set_state(&id1, InformerState::Running);
        mgr.set_state(&id2, InformerState::Reconnecting);
        // id3 stays Idle

        mgr.stop_all();

        assert_eq!(mgr.state(&id1), Some(&InformerState::Stopped));
        assert_eq!(mgr.state(&id2), Some(&InformerState::Stopped));
        assert_eq!(mgr.state(&id3), Some(&InformerState::Stopped));
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_stop_all_on_empty_manager() {
        let mut mgr = InformerManager::new();
        mgr.stop_all(); // should not panic
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_informers_for_cluster() {
        let mut mgr = InformerManager::new();
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();

        let id1 = mgr.register(pod_config(cluster_a));
        let id2 = mgr.register(deploy_config(cluster_a));
        let id3 = mgr.register(pod_config(cluster_b));

        let ids_a = mgr.informers_for_cluster(&cluster_a);
        assert_eq!(ids_a.len(), 2);
        assert!(ids_a.contains(&id1));
        assert!(ids_a.contains(&id2));

        let ids_b = mgr.informers_for_cluster(&cluster_b);
        assert_eq!(ids_b.len(), 1);
        assert!(ids_b.contains(&id3));
    }

    #[test]
    fn test_informers_for_nonexistent_cluster() {
        let mgr = InformerManager::new();
        let ids = mgr.informers_for_cluster(&Uuid::new_v4());
        assert!(ids.is_empty());
    }

    #[test]
    fn test_full_informer_lifecycle() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        // Register
        let id = mgr.register(pod_config(cluster_id));
        assert_eq!(mgr.state(&id), Some(&InformerState::Idle));

        // Start
        mgr.set_state(&id, InformerState::Running);
        assert_eq!(mgr.state(&id), Some(&InformerState::Running));
        assert_eq!(mgr.active_count(), 1);

        // Reconnect
        mgr.set_state(&id, InformerState::Reconnecting);
        assert_eq!(mgr.state(&id), Some(&InformerState::Reconnecting));
        assert_eq!(mgr.active_count(), 1);

        // Back to running
        mgr.set_state(&id, InformerState::Running);
        assert_eq!(mgr.active_count(), 1);

        // Error
        mgr.set_state(&id, InformerState::Error("watch reset".to_string()));
        assert_eq!(mgr.state(&id), Some(&InformerState::Error("watch reset".to_string())));
        assert_eq!(mgr.active_count(), 0);

        // Stop
        mgr.set_state(&id, InformerState::Stopped);
        assert_eq!(mgr.state(&id), Some(&InformerState::Stopped));

        // Unregister
        assert!(mgr.unregister(&id));
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn test_cluster_scoped_informer_has_no_namespace() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(cluster_scoped_config(cluster_id));

        let config = mgr.config(&id).unwrap();
        assert!(config.namespace.is_none());
        assert_eq!(config.resource_kind, "Node");
    }

    // --- T040: Standard watcher registration and cache population tests ---

    #[test]
    fn test_register_standard_watchers() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let ids = mgr.register_standard_watchers(cluster_id);

        assert_eq!(ids.len(), 4);
        assert_eq!(mgr.total_count(), 4);

        // Verify all 4 standard kinds are registered
        let kinds: Vec<String> =
            ids.iter().map(|id| mgr.config(id).unwrap().resource_kind.clone()).collect();
        assert!(kinds.contains(&"Namespace".to_string()));
        assert!(kinds.contains(&"Node".to_string()));
        assert!(kinds.contains(&"Pod".to_string()));
        assert!(kinds.contains(&"Event".to_string()));
    }

    #[test]
    fn test_standard_watchers_are_cluster_scoped() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let ids = mgr.register_standard_watchers(cluster_id);

        // All standard watchers watch all namespaces (namespace=None)
        for id in &ids {
            let config = mgr.config(id).unwrap();
            assert!(
                config.namespace.is_none(),
                "Standard watcher for {} should be cluster-wide",
                config.resource_kind
            );
        }
    }

    #[test]
    fn test_standard_watchers_all_idle_initially() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let ids = mgr.register_standard_watchers(cluster_id);

        for id in &ids {
            assert_eq!(mgr.state(id), Some(&InformerState::Idle));
        }
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_stop_for_cluster() {
        let mut mgr = InformerManager::new();
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();

        let ids_a = mgr.register_standard_watchers(cluster_a);
        let ids_b = mgr.register_standard_watchers(cluster_b);

        // Start all informers
        for id in ids_a.iter().chain(ids_b.iter()) {
            mgr.set_state(id, InformerState::Running);
        }
        assert_eq!(mgr.active_count(), 8);

        // Stop only cluster A
        mgr.stop_for_cluster(&cluster_a);

        // Cluster A stopped, cluster B still running
        for id in &ids_a {
            assert_eq!(mgr.state(id), Some(&InformerState::Stopped));
        }
        for id in &ids_b {
            assert_eq!(mgr.state(id), Some(&InformerState::Running));
        }
        assert_eq!(mgr.active_count(), 4);
    }

    #[test]
    fn test_cache_update_and_retrieve() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let pods = vec![
            Resource::new("pod-1", "default", "Pod", "v1", cluster_id),
            Resource::new("pod-2", "default", "Pod", "v1", cluster_id),
        ];

        mgr.update_cache(cluster_id, "Pod", pods);

        let cached = mgr.cached_resources(&cluster_id, "Pod").unwrap();
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].name, "pod-1");
        assert_eq!(cached[1].name, "pod-2");
    }

    #[test]
    fn test_cache_miss_returns_none() {
        let mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        assert!(mgr.cached_resources(&cluster_id, "Pod").is_none());
    }

    #[test]
    fn test_cache_update_overwrites() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        let pods_v1 = vec![Resource::new("pod-1", "default", "Pod", "v1", cluster_id)];
        mgr.update_cache(cluster_id, "Pod", pods_v1);
        assert_eq!(mgr.cached_resources(&cluster_id, "Pod").unwrap().len(), 1);

        let pods_v2 = vec![
            Resource::new("pod-1", "default", "Pod", "v1", cluster_id),
            Resource::new("pod-2", "default", "Pod", "v1", cluster_id),
            Resource::new("pod-3", "default", "Pod", "v1", cluster_id),
        ];
        mgr.update_cache(cluster_id, "Pod", pods_v2);
        assert_eq!(mgr.cached_resources(&cluster_id, "Pod").unwrap().len(), 3);
    }

    #[test]
    fn test_cache_clear_for_cluster() {
        let mut mgr = InformerManager::new();
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();

        mgr.update_cache(
            cluster_a,
            "Pod",
            vec![Resource::new("pod-a", "default", "Pod", "v1", cluster_a)],
        );
        mgr.update_cache(
            cluster_a,
            "Node",
            vec![Resource::new("node-a", "", "Node", "v1", cluster_a)],
        );
        mgr.update_cache(
            cluster_b,
            "Pod",
            vec![Resource::new("pod-b", "default", "Pod", "v1", cluster_b)],
        );

        mgr.clear_cache_for_cluster(&cluster_a);

        assert!(mgr.cached_resources(&cluster_a, "Pod").is_none());
        assert!(mgr.cached_resources(&cluster_a, "Node").is_none());
        // Cluster B cache still intact
        assert!(mgr.cached_resources(&cluster_b, "Pod").is_some());
    }

    #[test]
    fn test_cache_size_for_cluster() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();

        assert_eq!(mgr.cache_size_for_cluster(&cluster_id), 0);

        mgr.update_cache(
            cluster_id,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster_id),
                Resource::new("pod-2", "default", "Pod", "v1", cluster_id),
            ],
        );
        mgr.update_cache(
            cluster_id,
            "Node",
            vec![Resource::new("node-1", "", "Node", "v1", cluster_id)],
        );

        assert_eq!(mgr.cache_size_for_cluster(&cluster_id), 3);
    }

    #[test]
    fn test_stop_for_cluster_clears_cache() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        mgr.register_standard_watchers(cluster_id);
        mgr.update_cache(
            cluster_id,
            "Pod",
            vec![Resource::new("pod-1", "default", "Pod", "v1", cluster_id)],
        );

        mgr.stop_for_cluster(&cluster_id);
        assert_eq!(mgr.cache_size_for_cluster(&cluster_id), 0);
    }

    #[test]
    fn test_informer_config_serialization() {
        let config = InformerConfig {
            cluster_id: Uuid::new_v4(),
            resource_kind: "Pod".to_string(),
            namespace: Some("default".to_string()),
            api_version: "v1".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: InformerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.cluster_id, config.cluster_id);
        assert_eq!(deserialized.resource_kind, config.resource_kind);
        assert_eq!(deserialized.namespace, config.namespace);
        assert_eq!(deserialized.api_version, config.api_version);
    }

    #[test]
    fn test_informer_state_serialization() {
        let states = vec![
            InformerState::Idle,
            InformerState::Running,
            InformerState::Reconnecting,
            InformerState::Stopped,
            InformerState::Error("timeout".to_string()),
        ];

        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let deserialized: InformerState = serde_json::from_str(&json).unwrap();
            assert_eq!(&deserialized, state);
        }
    }

    // --- Slice B step 2: cancellation token tests ---

    #[test]
    fn test_set_cancel_token_stores_on_entry() {
        let mut mgr = InformerManager::new();
        let cluster_id = Uuid::new_v4();
        let id = mgr.register(pod_config(cluster_id));

        // Initially no token.
        assert!(mgr.cancel_token(&id).is_none());

        let token = CancellationToken::new();
        assert!(mgr.set_cancel_token(&id, token.clone()));

        // Token is stored and not yet cancelled.
        let stored = mgr.cancel_token(&id).expect("token should be stored");
        assert!(!stored.is_cancelled());
    }

    #[test]
    fn test_stop_for_cluster_cancels_all_tokens_for_cluster() {
        let mut mgr = InformerManager::new();
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();

        let ids_a = mgr.register_standard_watchers(cluster_a);
        let ids_b = mgr.register_standard_watchers(cluster_b);

        // Attach cancellation tokens to all informers.
        let tokens_a: Vec<CancellationToken> =
            ids_a.iter().map(|_| CancellationToken::new()).collect();
        let tokens_b: Vec<CancellationToken> =
            ids_b.iter().map(|_| CancellationToken::new()).collect();

        for (id, tok) in ids_a.iter().zip(tokens_a.iter()) {
            mgr.set_state(id, InformerState::Running);
            mgr.set_cancel_token(id, tok.clone());
        }
        for (id, tok) in ids_b.iter().zip(tokens_b.iter()) {
            mgr.set_state(id, InformerState::Running);
            mgr.set_cancel_token(id, tok.clone());
        }

        // Stop only cluster A.
        mgr.stop_for_cluster(&cluster_a);

        // Every cluster A token must be cancelled.
        for tok in &tokens_a {
            assert!(tok.is_cancelled(), "cluster A token should be cancelled");
        }
        // Every cluster B token must NOT be cancelled.
        for tok in &tokens_b {
            assert!(!tok.is_cancelled(), "cluster B token should not be cancelled");
        }
    }
}
