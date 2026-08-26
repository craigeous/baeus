// RBAC permission checking via SelfSubjectAccessReview

use anyhow::{Context, Result};
use kube::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kubernetes RBAC verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RbacVerb {
    Get,
    List,
    Watch,
    Create,
    Update,
    Patch,
    Delete,
}

impl RbacVerb {
    /// Return the lowercase string representation used by the Kubernetes API.
    pub fn as_str(&self) -> &'static str {
        match self {
            RbacVerb::Get => "get",
            RbacVerb::List => "list",
            RbacVerb::Watch => "watch",
            RbacVerb::Create => "create",
            RbacVerb::Update => "update",
            RbacVerb::Patch => "patch",
            RbacVerb::Delete => "delete",
        }
    }
}

/// Describes a single RBAC permission check request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub verb: RbacVerb,
    pub resource: String,
    pub api_group: String,
    pub namespace: Option<String>,
}

impl PermissionCheck {
    pub fn new(
        verb: RbacVerb,
        resource: impl Into<String>,
        api_group: impl Into<String>,
        namespace: Option<String>,
    ) -> Self {
        Self { verb, resource: resource.into(), api_group: api_group.into(), namespace }
    }

    /// Returns `true` if this is a cluster-scoped check (no namespace).
    pub fn is_cluster_scoped(&self) -> bool {
        self.namespace.is_none()
    }
}

/// The result of an RBAC permission check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionResult {
    pub allowed: bool,
    pub reason: Option<String>,
}

impl PermissionResult {
    pub fn allowed() -> Self {
        Self { allowed: true, reason: None }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self { allowed: false, reason: Some(reason.into()) }
    }

    pub fn denied_no_reason() -> Self {
        Self { allowed: false, reason: None }
    }
}

/// In-memory cache for RBAC permission results. Avoids redundant
/// SelfSubjectAccessReview calls for permissions we have already checked.
#[derive(Debug, Default)]
pub struct RbacCache {
    cache: HashMap<PermissionCheck, PermissionResult>,
}

impl RbacCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    /// Look up a cached permission result.
    pub fn check(&self, permission: &PermissionCheck) -> Option<&PermissionResult> {
        self.cache.get(permission)
    }

    /// Record a permission result in the cache.
    pub fn record(&mut self, permission: PermissionCheck, result: PermissionResult) {
        self.cache.insert(permission, result);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Convenience method: check if a specific action is allowed.
    /// Returns `None` if not cached, `Some(true)` if allowed, `Some(false)` if denied.
    pub fn is_allowed(
        &self,
        verb: RbacVerb,
        resource: &str,
        api_group: &str,
        namespace: Option<&str>,
    ) -> Option<bool> {
        let check = PermissionCheck {
            verb,
            resource: resource.to_string(),
            api_group: api_group.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };
        self.cache.get(&check).map(|r| r.allowed)
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns `true` if no entries are cached.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// Builds a SelfSubjectAccessReview resource attributes spec from a PermissionCheck.
/// Returns (resource, verb, api_group, namespace) tuple suitable for the K8s API.
pub fn build_access_review_attrs(check: &PermissionCheck) -> AccessReviewAttributes {
    AccessReviewAttributes {
        verb: check.verb.as_str().to_string(),
        resource: check.resource.clone(),
        group: check.api_group.clone(),
        namespace: check.namespace.clone(),
    }
}

/// Attributes for a SelfSubjectAccessReview request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessReviewAttributes {
    pub verb: String,
    pub resource: String,
    pub group: String,
    pub namespace: Option<String>,
}

/// Manages RBAC permission checking with caching.
///
/// Uses the cache to avoid redundant API calls. Provides methods for building
/// batch permission checks efficiently.
#[derive(Debug, Default)]
pub struct RbacChecker {
    pub cache: RbacCache,
}

impl RbacChecker {
    pub fn new() -> Self {
        Self { cache: RbacCache::new() }
    }

    /// Check a permission, returning the cached result if available.
    /// Returns `None` if the permission is not cached (caller must make API call).
    pub fn check_cached(&self, check: &PermissionCheck) -> Option<&PermissionResult> {
        self.cache.check(check)
    }

    /// Record the result of a permission check in the cache.
    pub fn record_result(&mut self, check: PermissionCheck, result: PermissionResult) {
        self.cache.record(check, result);
    }

    /// Build AccessReviewAttributes for a permission check.
    pub fn build_review(&self, check: &PermissionCheck) -> AccessReviewAttributes {
        build_access_review_attrs(check)
    }

    /// Process a SelfSubjectAccessReview API response.
    /// `allowed` indicates the API server's decision.
    /// `reason` is the optional reason string from the response.
    pub fn process_review_response(
        &mut self,
        check: PermissionCheck,
        allowed: bool,
        reason: Option<String>,
    ) -> PermissionResult {
        let result = if allowed {
            PermissionResult::allowed()
        } else {
            match reason {
                Some(r) => PermissionResult::denied(r),
                None => PermissionResult::denied_no_reason(),
            }
        };
        self.cache.record(check, result.clone());
        result
    }

    /// Return all uncached checks from a batch, along with their review attributes.
    /// Cached results are returned immediately.
    #[allow(clippy::type_complexity)]
    pub fn batch_check<'a>(
        &self,
        checks: &'a [PermissionCheck],
    ) -> (
        Vec<(&'a PermissionCheck, PermissionResult)>,
        Vec<(&'a PermissionCheck, AccessReviewAttributes)>,
    ) {
        let mut cached = Vec::new();
        let mut uncached = Vec::new();

        for check in checks {
            if let Some(result) = self.cache.check(check) {
                cached.push((check, result.clone()));
            } else {
                uncached.push((check, self.build_review(check)));
            }
        }

        (cached, uncached)
    }

    /// Clear the permission cache (e.g., when switching clusters).
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Perform a real SelfSubjectAccessReview API call against the K8s API server (T364b).
///
/// Returns the server's allow/deny decision. Callers should use `RbacChecker` to cache
/// results and avoid redundant API calls.
pub async fn check_permission(
    client: &Client,
    check: &PermissionCheck,
) -> Result<PermissionResult> {
    use k8s_openapi::api::authorization::v1::{
        ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
    };
    use kube::api::PostParams;

    let attrs = ResourceAttributes {
        verb: Some(check.verb.as_str().to_string()),
        resource: Some(check.resource.clone()),
        group: Some(check.api_group.clone()),
        namespace: check.namespace.clone(),
        ..Default::default()
    };

    let review = SelfSubjectAccessReview {
        metadata: Default::default(),
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(attrs),
            non_resource_attributes: None,
        },
        status: None,
    };

    let api: kube::Api<SelfSubjectAccessReview> = kube::Api::all(client.clone());
    let response = api
        .create(&PostParams::default(), &review)
        .await
        .context("Failed to create SelfSubjectAccessReview")?;

    let status = response.status.unwrap_or_default();
    let allowed = status.allowed;
    let reason = status.reason;

    Ok(if allowed {
        PermissionResult::allowed()
    } else {
        match reason {
            Some(r) => PermissionResult::denied(r),
            None => PermissionResult::denied_no_reason(),
        }
    })
}

/// Check multiple permissions in parallel using SelfSubjectAccessReview (T364b).
///
/// Returns a vec of (PermissionCheck, PermissionResult) for all checks.
/// Results are NOT cached — callers should use `RbacChecker::process_review_response`
/// to cache them.
pub async fn batch_check_permissions(
    client: &Client,
    checks: &[PermissionCheck],
) -> Vec<(PermissionCheck, Result<PermissionResult>)> {
    let mut results = Vec::with_capacity(checks.len());
    // Run sequentially to avoid overwhelming the API server
    for check in checks {
        let result = check_permission(client, check).await;
        results.push((check.clone(), result));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pods_list_default() -> PermissionCheck {
        PermissionCheck::new(RbacVerb::List, "pods", "", Some("default".to_string()))
    }

    fn pods_get_default() -> PermissionCheck {
        PermissionCheck::new(RbacVerb::Get, "pods", "", Some("default".to_string()))
    }

    fn nodes_list_cluster() -> PermissionCheck {
        PermissionCheck::new(RbacVerb::List, "nodes", "", None)
    }

    fn deployments_create_kube_system() -> PermissionCheck {
        PermissionCheck::new(
            RbacVerb::Create,
            "deployments",
            "apps",
            Some("kube-system".to_string()),
        )
    }

    // --- RbacVerb tests ---

    #[test]
    fn test_rbac_verb_as_str() {
        assert_eq!(RbacVerb::Get.as_str(), "get");
        assert_eq!(RbacVerb::List.as_str(), "list");
        assert_eq!(RbacVerb::Watch.as_str(), "watch");
        assert_eq!(RbacVerb::Create.as_str(), "create");
        assert_eq!(RbacVerb::Update.as_str(), "update");
        assert_eq!(RbacVerb::Patch.as_str(), "patch");
        assert_eq!(RbacVerb::Delete.as_str(), "delete");
    }

    #[test]
    fn test_rbac_verb_serialization() {
        for verb in [
            RbacVerb::Get,
            RbacVerb::List,
            RbacVerb::Watch,
            RbacVerb::Create,
            RbacVerb::Update,
            RbacVerb::Patch,
            RbacVerb::Delete,
        ] {
            let json = serde_json::to_string(&verb).unwrap();
            let deserialized: RbacVerb = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, verb);
        }
    }

    // --- PermissionCheck tests ---

    #[test]
    fn test_permission_check_new() {
        let check = pods_list_default();
        assert_eq!(check.verb, RbacVerb::List);
        assert_eq!(check.resource, "pods");
        assert_eq!(check.api_group, "");
        assert_eq!(check.namespace.as_deref(), Some("default"));
        assert!(!check.is_cluster_scoped());
    }

    #[test]
    fn test_permission_check_cluster_scoped() {
        let check = nodes_list_cluster();
        assert!(check.is_cluster_scoped());
        assert!(check.namespace.is_none());
    }

    #[test]
    fn test_permission_check_equality() {
        let a = pods_list_default();
        let b = pods_list_default();
        assert_eq!(a, b);

        let c = pods_get_default();
        assert_ne!(a, c);
    }

    #[test]
    fn test_permission_check_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(pods_list_default());
        set.insert(pods_list_default()); // duplicate
        assert_eq!(set.len(), 1);

        set.insert(pods_get_default());
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_permission_check_serialization() {
        let check = deployments_create_kube_system();
        let json = serde_json::to_string(&check).unwrap();
        let deserialized: PermissionCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, check);
    }

    // --- PermissionResult tests ---

    #[test]
    fn test_permission_result_allowed() {
        let result = PermissionResult::allowed();
        assert!(result.allowed);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_permission_result_denied() {
        let result = PermissionResult::denied("RBAC: user lacks permission");
        assert!(!result.allowed);
        assert_eq!(result.reason.as_deref(), Some("RBAC: user lacks permission"));
    }

    #[test]
    fn test_permission_result_denied_no_reason() {
        let result = PermissionResult::denied_no_reason();
        assert!(!result.allowed);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_permission_result_serialization() {
        let result = PermissionResult::denied("forbidden");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PermissionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, result);
    }

    // --- RbacCache tests ---

    #[test]
    fn test_cache_new_is_empty() {
        let cache = RbacCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_default_is_empty() {
        let cache = RbacCache::default();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_record_and_check() {
        let mut cache = RbacCache::new();
        let perm = pods_list_default();

        assert!(cache.check(&perm).is_none());

        cache.record(perm.clone(), PermissionResult::allowed());
        let result = cache.check(&perm).unwrap();
        assert!(result.allowed);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_check_miss() {
        let cache = RbacCache::new();
        assert!(cache.check(&pods_list_default()).is_none());
    }

    #[test]
    fn test_cache_record_overwrites() {
        let mut cache = RbacCache::new();
        let perm = pods_list_default();

        cache.record(perm.clone(), PermissionResult::allowed());
        assert!(cache.check(&perm).unwrap().allowed);

        cache.record(perm.clone(), PermissionResult::denied("revoked"));
        assert!(!cache.check(&perm).unwrap().allowed);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = RbacCache::new();
        cache.record(pods_list_default(), PermissionResult::allowed());
        cache.record(pods_get_default(), PermissionResult::allowed());
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(cache.check(&pods_list_default()).is_none());
    }

    #[test]
    fn test_is_allowed_cached() {
        let mut cache = RbacCache::new();
        cache.record(pods_list_default(), PermissionResult::allowed());

        let result = cache.is_allowed(RbacVerb::List, "pods", "", Some("default"));
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_is_allowed_denied() {
        let mut cache = RbacCache::new();
        cache.record(deployments_create_kube_system(), PermissionResult::denied("forbidden"));

        let result = cache.is_allowed(RbacVerb::Create, "deployments", "apps", Some("kube-system"));
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_is_allowed_not_cached() {
        let cache = RbacCache::new();
        let result = cache.is_allowed(RbacVerb::Delete, "pods", "", Some("default"));
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_allowed_cluster_scoped() {
        let mut cache = RbacCache::new();
        cache.record(nodes_list_cluster(), PermissionResult::allowed());

        assert_eq!(cache.is_allowed(RbacVerb::List, "nodes", "", None), Some(true));
    }

    #[test]
    fn test_cache_distinguishes_namespaces() {
        let mut cache = RbacCache::new();

        let default_check =
            PermissionCheck::new(RbacVerb::List, "pods", "", Some("default".to_string()));
        let kube_system_check =
            PermissionCheck::new(RbacVerb::List, "pods", "", Some("kube-system".to_string()));

        cache.record(default_check.clone(), PermissionResult::allowed());
        cache.record(kube_system_check.clone(), PermissionResult::denied("nope"));

        assert!(cache.check(&default_check).unwrap().allowed);
        assert!(!cache.check(&kube_system_check).unwrap().allowed);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_distinguishes_verbs() {
        let mut cache = RbacCache::new();

        let list_check =
            PermissionCheck::new(RbacVerb::List, "pods", "", Some("default".to_string()));
        let delete_check =
            PermissionCheck::new(RbacVerb::Delete, "pods", "", Some("default".to_string()));

        cache.record(list_check.clone(), PermissionResult::allowed());
        cache.record(delete_check.clone(), PermissionResult::denied("read-only"));

        assert!(cache.check(&list_check).unwrap().allowed);
        assert!(!cache.check(&delete_check).unwrap().allowed);
    }

    #[test]
    fn test_cache_distinguishes_api_groups() {
        let mut cache = RbacCache::new();

        let core_check =
            PermissionCheck::new(RbacVerb::List, "pods", "", Some("default".to_string()));
        let apps_check = PermissionCheck::new(
            RbacVerb::List,
            "deployments",
            "apps",
            Some("default".to_string()),
        );

        cache.record(core_check.clone(), PermissionResult::allowed());
        cache.record(apps_check.clone(), PermissionResult::denied("no apps access"));

        assert!(cache.check(&core_check).unwrap().allowed);
        assert!(!cache.check(&apps_check).unwrap().allowed);
    }

    #[test]
    fn test_multiple_permissions_workflow() {
        let mut cache = RbacCache::new();

        // Simulate checking several permissions for a user session
        let checks = vec![
            (
                PermissionCheck::new(RbacVerb::List, "pods", "", Some("default".to_string())),
                PermissionResult::allowed(),
            ),
            (
                PermissionCheck::new(RbacVerb::Get, "pods", "", Some("default".to_string())),
                PermissionResult::allowed(),
            ),
            (
                PermissionCheck::new(RbacVerb::Watch, "pods", "", Some("default".to_string())),
                PermissionResult::allowed(),
            ),
            (
                PermissionCheck::new(RbacVerb::Delete, "pods", "", Some("default".to_string())),
                PermissionResult::denied("read-only binding"),
            ),
            (
                PermissionCheck::new(
                    RbacVerb::Create,
                    "deployments",
                    "apps",
                    Some("default".to_string()),
                ),
                PermissionResult::denied("not authorized"),
            ),
        ];

        for (check, result) in checks {
            cache.record(check, result);
        }

        assert_eq!(cache.len(), 5);
        assert_eq!(cache.is_allowed(RbacVerb::List, "pods", "", Some("default")), Some(true));
        assert_eq!(cache.is_allowed(RbacVerb::Delete, "pods", "", Some("default")), Some(false));
        assert_eq!(cache.is_allowed(RbacVerb::Patch, "pods", "", Some("default")), None);
    }

    // --- RbacChecker tests (T045) ---

    #[test]
    fn test_rbac_checker_new() {
        let checker = RbacChecker::new();
        assert!(checker.cache.is_empty());
    }

    #[test]
    fn test_build_access_review_attrs() {
        let check = pods_list_default();
        let attrs = build_access_review_attrs(&check);
        assert_eq!(attrs.verb, "list");
        assert_eq!(attrs.resource, "pods");
        assert_eq!(attrs.group, "");
        assert_eq!(attrs.namespace.as_deref(), Some("default"));
    }

    #[test]
    fn test_build_review_cluster_scoped() {
        let checker = RbacChecker::new();
        let check = nodes_list_cluster();
        let attrs = checker.build_review(&check);
        assert_eq!(attrs.verb, "list");
        assert_eq!(attrs.resource, "nodes");
        assert!(attrs.namespace.is_none());
    }

    #[test]
    fn test_process_review_allowed() {
        let mut checker = RbacChecker::new();
        let check = pods_list_default();
        let result = checker.process_review_response(check.clone(), true, None);
        assert!(result.allowed);
        // Verify it was cached
        assert!(checker.check_cached(&check).unwrap().allowed);
    }

    #[test]
    fn test_process_review_denied_with_reason() {
        let mut checker = RbacChecker::new();
        let check = deployments_create_kube_system();
        let result = checker.process_review_response(
            check.clone(),
            false,
            Some("RBAC: forbidden".to_string()),
        );
        assert!(!result.allowed);
        assert_eq!(result.reason.as_deref(), Some("RBAC: forbidden"));
    }

    #[test]
    fn test_batch_check_all_cached() {
        let mut checker = RbacChecker::new();
        let check1 = pods_list_default();
        let check2 = pods_get_default();
        checker.record_result(check1.clone(), PermissionResult::allowed());
        checker.record_result(check2.clone(), PermissionResult::allowed());

        let checks = vec![check1, check2];
        let (cached, uncached) = checker.batch_check(&checks);
        assert_eq!(cached.len(), 2);
        assert!(uncached.is_empty());
    }

    #[test]
    fn test_batch_check_mixed() {
        let mut checker = RbacChecker::new();
        let check1 = pods_list_default();
        checker.record_result(check1.clone(), PermissionResult::allowed());

        let check2 = nodes_list_cluster(); // not cached

        let checks = vec![check1, check2];
        let (cached, uncached) = checker.batch_check(&checks);
        assert_eq!(cached.len(), 1);
        assert_eq!(uncached.len(), 1);
        assert_eq!(uncached[0].1.resource, "nodes");
    }

    #[test]
    fn test_checker_clear() {
        let mut checker = RbacChecker::new();
        checker.record_result(pods_list_default(), PermissionResult::allowed());
        assert!(!checker.cache.is_empty());
        checker.clear();
        assert!(checker.cache.is_empty());
    }
}
