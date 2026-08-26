// T056: Resource watch integration via ResourceWatchBridge

use crate::informer::{InformerConfig, InformerManager, InformerState};
use crate::resource::Resource;
use std::collections::HashMap;
use uuid::Uuid;

/// Bridges the informer cache to provide a unified view of resources.
/// Used to keep the UI's resource tables up-to-date from informer-managed caches.
pub struct ResourceWatchBridge {
    /// Informer manager providing the cache.
    informer_manager: InformerManager,
    /// Maps (cluster_id, kind) to the informer id for that watcher.
    watcher_ids: HashMap<(Uuid, String), Uuid>,
}

impl ResourceWatchBridge {
    /// Create a new bridge wrapping the given informer manager.
    pub fn new(informer_manager: InformerManager) -> Self {
        Self { informer_manager, watcher_ids: HashMap::new() }
    }

    /// Register a watcher (informer) for a specific resource kind on a cluster.
    /// Returns the informer ID that can be used to reference this watcher.
    pub fn register_watcher(
        &mut self,
        cluster_id: Uuid,
        kind: &str,
        api_version: &str,
        namespace: Option<&str>,
    ) -> Uuid {
        let config = InformerConfig {
            cluster_id,
            resource_kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
            api_version: api_version.to_string(),
        };

        let id = self.informer_manager.register(config);
        // Mark the informer as Running immediately since this is the "start watching" call.
        self.informer_manager.set_state(&id, InformerState::Running);
        self.watcher_ids.insert((cluster_id, kind.to_string()), id);
        id
    }

    /// Get cached resources from the informer for a specific kind on a cluster.
    /// Returns an empty slice if no cache entry exists.
    pub fn cached_resources(&self, cluster_id: &Uuid, kind: &str) -> Vec<&Resource> {
        match self.informer_manager.cached_resources(cluster_id, kind) {
            Some(resources) => resources.iter().collect(),
            None => Vec::new(),
        }
    }

    /// Update the informer cache for a specific resource kind on a cluster.
    pub fn refresh_cache(&mut self, cluster_id: Uuid, kind: &str, resources: Vec<Resource>) {
        self.informer_manager.update_cache(cluster_id, kind, resources);
    }

    /// Stop watching a specific resource kind on a cluster. Removes the informer
    /// and clears the corresponding cache entry.
    pub fn stop_watching(&mut self, cluster_id: &Uuid, kind: &str) {
        let key = (*cluster_id, kind.to_string());
        if let Some(informer_id) = self.watcher_ids.remove(&key) {
            self.informer_manager.set_state(&informer_id, InformerState::Stopped);
            self.informer_manager.unregister(&informer_id);
        }
        // Clear the cache for this specific kind by replacing with empty vec,
        // then rely on the informer manager's cache cleanup.
        // We update the cache to empty to signal no resources, then the informer
        // is already unregistered.
        self.informer_manager.update_cache(*cluster_id, kind, Vec::new());
    }

    /// Return a sorted list of resource kinds currently being watched for a cluster.
    pub fn watched_kinds(&self, cluster_id: &Uuid) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .watcher_ids
            .iter()
            .filter(|((cid, _), _)| cid == cluster_id)
            .map(|((_, kind), _)| kind.clone())
            .collect();
        kinds.sort();
        kinds
    }

    /// Get a reference to the underlying informer manager.
    pub fn informer_manager(&self) -> &InformerManager {
        &self.informer_manager
    }

    /// Get a mutable reference to the underlying informer manager.
    pub fn informer_manager_mut(&mut self) -> &mut InformerManager {
        &mut self.informer_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge() -> ResourceWatchBridge {
        ResourceWatchBridge::new(InformerManager::new())
    }

    fn test_cluster_id() -> Uuid {
        Uuid::new_v4()
    }

    // ===================================================================
    // T056: ResourceWatchBridge construction tests
    // ===================================================================

    #[test]
    fn test_new_bridge_has_empty_informer_manager() {
        let bridge = make_bridge();
        assert_eq!(bridge.informer_manager().total_count(), 0);
        assert_eq!(bridge.informer_manager().active_count(), 0);
    }

    #[test]
    fn test_new_bridge_with_existing_manager() {
        let mut mgr = InformerManager::new();
        let cluster = test_cluster_id();
        mgr.register_standard_watchers(cluster);

        let bridge = ResourceWatchBridge::new(mgr);
        // The standard watchers are still registered in the manager
        assert_eq!(bridge.informer_manager().total_count(), 4);
    }

    // ===================================================================
    // T056: register_watcher tests
    // ===================================================================

    #[test]
    fn test_register_watcher_returns_uuid() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        let id = bridge.register_watcher(cluster, "Pod", "v1", Some("default"));
        // UUID should be valid (non-nil)
        assert_ne!(id, Uuid::nil());
    }

    #[test]
    fn test_register_watcher_creates_informer() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        bridge.register_watcher(cluster, "Pod", "v1", Some("default"));
        assert_eq!(bridge.informer_manager().total_count(), 1);
    }

    #[test]
    fn test_register_watcher_sets_state_running() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        let id = bridge.register_watcher(cluster, "Pod", "v1", Some("default"));
        assert_eq!(bridge.informer_manager().state(&id), Some(&InformerState::Running));
        assert_eq!(bridge.informer_manager().active_count(), 1);
    }

    #[test]
    fn test_register_watcher_stores_config_correctly() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        let id = bridge.register_watcher(cluster, "Deployment", "apps/v1", Some("kube-system"));

        let config = bridge.informer_manager().config(&id).unwrap();
        assert_eq!(config.cluster_id, cluster);
        assert_eq!(config.resource_kind, "Deployment");
        assert_eq!(config.api_version, "apps/v1");
        assert_eq!(config.namespace.as_deref(), Some("kube-system"));
    }

    #[test]
    fn test_register_watcher_cluster_scoped() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        let id = bridge.register_watcher(cluster, "Node", "v1", None);

        let config = bridge.informer_manager().config(&id).unwrap();
        assert!(config.namespace.is_none());
    }

    #[test]
    fn test_register_multiple_watchers() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        let id1 = bridge.register_watcher(cluster, "Pod", "v1", None);
        let id2 = bridge.register_watcher(cluster, "Deployment", "apps/v1", None);
        let id3 = bridge.register_watcher(cluster, "Service", "v1", None);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(bridge.informer_manager().total_count(), 3);
        assert_eq!(bridge.informer_manager().active_count(), 3);
    }

    #[test]
    fn test_register_watchers_on_different_clusters() {
        let mut bridge = make_bridge();
        let cluster_a = test_cluster_id();
        let cluster_b = test_cluster_id();

        bridge.register_watcher(cluster_a, "Pod", "v1", None);
        bridge.register_watcher(cluster_b, "Pod", "v1", None);

        assert_eq!(bridge.informer_manager().total_count(), 2);
    }

    // ===================================================================
    // T056: cached_resources tests
    // ===================================================================

    #[test]
    fn test_cached_resources_empty_initially() {
        let bridge = make_bridge();
        let cluster = test_cluster_id();
        let result = bridge.cached_resources(&cluster, "Pod");
        assert!(result.is_empty());
    }

    #[test]
    fn test_cached_resources_after_refresh() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        bridge.register_watcher(cluster, "Pod", "v1", None);

        let pods = vec![
            Resource::new("pod-1", "default", "Pod", "v1", cluster),
            Resource::new("pod-2", "default", "Pod", "v1", cluster),
        ];
        bridge.refresh_cache(cluster, "Pod", pods);

        let cached = bridge.cached_resources(&cluster, "Pod");
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].name, "pod-1");
        assert_eq!(cached[1].name, "pod-2");
    }

    #[test]
    fn test_cached_resources_different_kinds_independent() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        bridge.register_watcher(cluster, "Deployment", "apps/v1", None);

        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster),
                Resource::new("pod-2", "default", "Pod", "v1", cluster),
            ],
        );
        bridge.refresh_cache(
            cluster,
            "Deployment",
            vec![Resource::new("deploy-1", "default", "Deployment", "apps/v1", cluster)],
        );

        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 2);
        assert_eq!(bridge.cached_resources(&cluster, "Deployment").len(), 1);
        assert!(bridge.cached_resources(&cluster, "Service").is_empty());
    }

    #[test]
    fn test_cached_resources_different_clusters_independent() {
        let mut bridge = make_bridge();
        let cluster_a = test_cluster_id();
        let cluster_b = test_cluster_id();

        bridge.register_watcher(cluster_a, "Pod", "v1", None);
        bridge.register_watcher(cluster_b, "Pod", "v1", None);

        bridge.refresh_cache(
            cluster_a,
            "Pod",
            vec![Resource::new("pod-a", "default", "Pod", "v1", cluster_a)],
        );
        bridge.refresh_cache(
            cluster_b,
            "Pod",
            vec![
                Resource::new("pod-b1", "default", "Pod", "v1", cluster_b),
                Resource::new("pod-b2", "default", "Pod", "v1", cluster_b),
            ],
        );

        assert_eq!(bridge.cached_resources(&cluster_a, "Pod").len(), 1);
        assert_eq!(bridge.cached_resources(&cluster_b, "Pod").len(), 2);
    }

    // ===================================================================
    // T056: refresh_cache tests
    // ===================================================================

    #[test]
    fn test_refresh_cache_overwrites_previous() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        bridge.register_watcher(cluster, "Pod", "v1", None);

        // First population
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![Resource::new("pod-1", "default", "Pod", "v1", cluster)],
        );
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 1);

        // Overwrite with more pods
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster),
                Resource::new("pod-2", "default", "Pod", "v1", cluster),
                Resource::new("pod-3", "default", "Pod", "v1", cluster),
            ],
        );
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 3);
    }

    #[test]
    fn test_refresh_cache_with_empty_vec_clears() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        bridge.register_watcher(cluster, "Pod", "v1", None);

        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![Resource::new("pod-1", "default", "Pod", "v1", cluster)],
        );
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 1);

        bridge.refresh_cache(cluster, "Pod", Vec::new());
        assert!(bridge.cached_resources(&cluster, "Pod").is_empty());
    }

    #[test]
    fn test_refresh_cache_without_registered_watcher() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();
        // No watcher registered, but we can still populate the cache
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![Resource::new("pod-1", "default", "Pod", "v1", cluster)],
        );
        // The cache should still work through the informer manager
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 1);
    }

    // ===================================================================
    // T056: stop_watching tests
    // ===================================================================

    #[test]
    fn test_stop_watching_removes_watcher() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Pod"]);

        bridge.stop_watching(&cluster, "Pod");
        assert!(bridge.watched_kinds(&cluster).is_empty());
    }

    #[test]
    fn test_stop_watching_unregisters_informer() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        let id = bridge.register_watcher(cluster, "Pod", "v1", None);
        assert_eq!(bridge.informer_manager().total_count(), 1);

        bridge.stop_watching(&cluster, "Pod");
        // The informer should be fully unregistered
        assert_eq!(bridge.informer_manager().total_count(), 0);
        assert!(bridge.informer_manager().state(&id).is_none());
    }

    #[test]
    fn test_stop_watching_clears_cache() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster),
                Resource::new("pod-2", "default", "Pod", "v1", cluster),
            ],
        );
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 2);

        bridge.stop_watching(&cluster, "Pod");
        // Cache is cleared (empty vec)
        assert!(bridge.cached_resources(&cluster, "Pod").is_empty());
    }

    #[test]
    fn test_stop_watching_does_not_affect_other_kinds() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        bridge.register_watcher(cluster, "Deployment", "apps/v1", None);

        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![Resource::new("pod-1", "default", "Pod", "v1", cluster)],
        );
        bridge.refresh_cache(
            cluster,
            "Deployment",
            vec![Resource::new("deploy-1", "default", "Deployment", "apps/v1", cluster)],
        );

        bridge.stop_watching(&cluster, "Pod");

        // Pod cache cleared, Deployment unaffected
        assert!(bridge.cached_resources(&cluster, "Pod").is_empty());
        assert_eq!(bridge.cached_resources(&cluster, "Deployment").len(), 1);
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Deployment"]);
    }

    #[test]
    fn test_stop_watching_does_not_affect_other_clusters() {
        let mut bridge = make_bridge();
        let cluster_a = test_cluster_id();
        let cluster_b = test_cluster_id();

        bridge.register_watcher(cluster_a, "Pod", "v1", None);
        bridge.register_watcher(cluster_b, "Pod", "v1", None);

        bridge.refresh_cache(
            cluster_a,
            "Pod",
            vec![Resource::new("pod-a", "default", "Pod", "v1", cluster_a)],
        );
        bridge.refresh_cache(
            cluster_b,
            "Pod",
            vec![Resource::new("pod-b", "default", "Pod", "v1", cluster_b)],
        );

        bridge.stop_watching(&cluster_a, "Pod");

        assert!(bridge.cached_resources(&cluster_a, "Pod").is_empty());
        assert_eq!(bridge.cached_resources(&cluster_b, "Pod").len(), 1);
    }

    #[test]
    fn test_stop_watching_nonexistent_kind_is_noop() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        // Stopping a kind that was never registered should not panic
        bridge.stop_watching(&cluster, "ConfigMap");
        // Pod watcher should still be there
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Pod"]);
    }

    // ===================================================================
    // T056: watched_kinds tests
    // ===================================================================

    #[test]
    fn test_watched_kinds_empty() {
        let bridge = make_bridge();
        let cluster = test_cluster_id();
        assert!(bridge.watched_kinds(&cluster).is_empty());
    }

    #[test]
    fn test_watched_kinds_returns_sorted() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Service", "v1", None);
        bridge.register_watcher(cluster, "Deployment", "apps/v1", None);
        bridge.register_watcher(cluster, "Pod", "v1", None);

        let kinds = bridge.watched_kinds(&cluster);
        assert_eq!(kinds, vec!["Deployment", "Pod", "Service"]);
    }

    #[test]
    fn test_watched_kinds_per_cluster() {
        let mut bridge = make_bridge();
        let cluster_a = test_cluster_id();
        let cluster_b = test_cluster_id();

        bridge.register_watcher(cluster_a, "Pod", "v1", None);
        bridge.register_watcher(cluster_a, "Deployment", "apps/v1", None);
        bridge.register_watcher(cluster_b, "Service", "v1", None);

        assert_eq!(bridge.watched_kinds(&cluster_a), vec!["Deployment", "Pod"]);
        assert_eq!(bridge.watched_kinds(&cluster_b), vec!["Service"]);
    }

    #[test]
    fn test_watched_kinds_after_stop() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        bridge.register_watcher(cluster, "Pod", "v1", None);
        bridge.register_watcher(cluster, "Deployment", "apps/v1", None);
        bridge.register_watcher(cluster, "Service", "v1", None);

        bridge.stop_watching(&cluster, "Deployment");
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Pod", "Service"]);
    }

    #[test]
    fn test_watched_kinds_nonexistent_cluster() {
        let bridge = make_bridge();
        let unknown_cluster = test_cluster_id();
        assert!(bridge.watched_kinds(&unknown_cluster).is_empty());
    }

    // ===================================================================
    // T056: Integration / lifecycle tests
    // ===================================================================

    #[test]
    fn test_full_watch_lifecycle() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        // 1. Register watchers
        let pod_id = bridge.register_watcher(cluster, "Pod", "v1", None);
        let deploy_id = bridge.register_watcher(cluster, "Deployment", "apps/v1", None);

        assert_eq!(bridge.informer_manager().active_count(), 2);
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Deployment", "Pod"]);

        // 2. Populate caches (simulating informer receiving data)
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster),
                Resource::new("pod-2", "kube-system", "Pod", "v1", cluster),
            ],
        );
        bridge.refresh_cache(
            cluster,
            "Deployment",
            vec![Resource::new("deploy-1", "default", "Deployment", "apps/v1", cluster)],
        );

        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 2);
        assert_eq!(bridge.cached_resources(&cluster, "Deployment").len(), 1);

        // 3. Refresh cache with updated data
        bridge.refresh_cache(
            cluster,
            "Pod",
            vec![
                Resource::new("pod-1", "default", "Pod", "v1", cluster),
                Resource::new("pod-2", "kube-system", "Pod", "v1", cluster),
                Resource::new("pod-3", "default", "Pod", "v1", cluster),
            ],
        );
        assert_eq!(bridge.cached_resources(&cluster, "Pod").len(), 3);

        // 4. Stop watching one kind
        bridge.stop_watching(&cluster, "Pod");
        assert!(bridge.cached_resources(&cluster, "Pod").is_empty());
        assert_eq!(bridge.cached_resources(&cluster, "Deployment").len(), 1);
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Deployment"]);

        // The pod informer should be gone but the deploy informer remains
        assert!(bridge.informer_manager().state(&pod_id).is_none());
        assert_eq!(bridge.informer_manager().state(&deploy_id), Some(&InformerState::Running));

        // 5. Stop watching the last kind
        bridge.stop_watching(&cluster, "Deployment");
        assert!(bridge.watched_kinds(&cluster).is_empty());
        assert_eq!(bridge.informer_manager().total_count(), 0);
    }

    #[test]
    fn test_watch_bridge_with_pre_populated_manager() {
        let mut mgr = InformerManager::new();
        let cluster = test_cluster_id();

        // Pre-populate the informer manager with some standard watchers and cache
        let ids = mgr.register_standard_watchers(cluster);
        for id in &ids {
            mgr.set_state(id, InformerState::Running);
        }
        mgr.update_cache(
            cluster,
            "Pod",
            vec![Resource::new("existing-pod", "default", "Pod", "v1", cluster)],
        );

        let mut bridge = ResourceWatchBridge::new(mgr);

        // The bridge should see the pre-populated cache
        let pods = bridge.cached_resources(&cluster, "Pod");
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].name, "existing-pod");

        // watched_kinds won't show the pre-registered informers since they
        // weren't registered through the bridge. But we can add new ones.
        assert!(bridge.watched_kinds(&cluster).is_empty());

        // Register a new watcher through the bridge
        bridge.register_watcher(cluster, "ConfigMap", "v1", None);
        assert_eq!(bridge.watched_kinds(&cluster), vec!["ConfigMap"]);

        // Total informers = 4 standard + 1 newly registered
        assert_eq!(bridge.informer_manager().total_count(), 5);
    }

    #[test]
    fn test_register_same_kind_overwrites_watcher_id() {
        let mut bridge = make_bridge();
        let cluster = test_cluster_id();

        let id1 = bridge.register_watcher(cluster, "Pod", "v1", Some("default"));
        let id2 = bridge.register_watcher(cluster, "Pod", "v1", Some("kube-system"));

        // The second registration overwrites the watcher_id mapping
        assert_ne!(id1, id2);
        // Both informers exist in the manager
        assert_eq!(bridge.informer_manager().total_count(), 2);
        // But watched_kinds only shows one entry since the map key is (cluster, kind)
        assert_eq!(bridge.watched_kinds(&cluster), vec!["Pod"]);
    }
}
