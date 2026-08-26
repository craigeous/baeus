use chrono::{DateTime, Utc};
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uid: String,
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub api_version: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub creation_timestamp: DateTime<Utc>,
    pub resource_version: String,
    pub owner_references: Vec<OwnerReference>,
    pub spec: Value,
    pub status: Option<Value>,
    pub conditions: Vec<Condition>,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerReference {
    pub uid: String,
    pub kind: String,
    pub name: String,
    pub api_version: String,
    pub controller: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Condition {
    pub type_name: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub last_transition: DateTime<Utc>,
}

impl Condition {
    pub fn is_true(&self) -> bool {
        self.status == "True"
    }

    pub fn is_false(&self) -> bool {
        self.status == "False"
    }

    pub fn is_unknown(&self) -> bool {
        self.status == "Unknown"
    }
}

impl Resource {
    /// Create a new Resource with minimal required fields. Useful for tests and cache population.
    pub fn new(
        name: impl Into<String>,
        namespace: impl Into<String>,
        kind: impl Into<String>,
        api_version: impl Into<String>,
        cluster_id: Uuid,
    ) -> Self {
        let ns: String = namespace.into();
        Self {
            uid: Uuid::new_v4().to_string(),
            name: name.into(),
            namespace: if ns.is_empty() { None } else { Some(ns) },
            kind: kind.into(),
            api_version: api_version.into(),
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            creation_timestamp: Utc::now(),
            resource_version: "1".to_string(),
            owner_references: Vec::new(),
            spec: Value::Null,
            status: None,
            conditions: Vec::new(),
            cluster_id,
        }
    }

    pub fn is_namespaced(&self) -> bool {
        self.namespace.is_some()
    }

    pub fn has_owner(&self) -> bool {
        !self.owner_references.is_empty()
    }

    pub fn controller_owner(&self) -> Option<&OwnerReference> {
        self.owner_references.iter().find(|o| o.controller)
    }

    pub fn is_ready(&self) -> bool {
        self.conditions.iter().any(|c| c.type_name == "Ready" && c.is_true())
    }

    /// Builder method to add a label to the resource.
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// Builder method to set the spec on the resource.
    pub fn with_spec(mut self, spec: Value) -> Self {
        self.spec = spec;
        self
    }
}

/// Errors that can occur during resource CRUD operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ResourceError {
    #[error("resource not found: {kind}/{name}")]
    NotFound { kind: String, name: String },

    #[error("resource already exists: {kind}/{name}")]
    AlreadyExists { kind: String, name: String },

    #[error("version conflict: expected {expected}, actual {actual}")]
    VersionConflict { expected: String, actual: String },
}

/// An in-memory resource store for managing resources within a cluster.
pub struct ResourceService {
    pub resources: Vec<Resource>,
}

impl ResourceService {
    /// Create a new empty ResourceService.
    pub fn new() -> Self {
        Self { resources: Vec::new() }
    }

    /// List resources by kind, optionally filtered by namespace.
    pub fn list(&self, kind: &str, namespace: Option<&str>) -> Vec<&Resource> {
        self.resources
            .iter()
            .filter(|r| r.kind == kind)
            .filter(|r| match namespace {
                Some(ns) => r.namespace.as_deref() == Some(ns),
                None => true,
            })
            .collect()
    }

    /// Get a specific resource by kind, name, and optional namespace.
    pub fn get(&self, kind: &str, name: &str, namespace: Option<&str>) -> Option<&Resource> {
        self.resources
            .iter()
            .find(|r| r.kind == kind && r.name == name && r.namespace.as_deref() == namespace)
    }

    /// Create a new resource. Returns an error if a resource with the same
    /// kind, name, and namespace already exists.
    pub fn create(&mut self, resource: Resource) -> Result<(), ResourceError> {
        if self.get(&resource.kind, &resource.name, resource.namespace.as_deref()).is_some() {
            return Err(ResourceError::AlreadyExists {
                kind: resource.kind.clone(),
                name: resource.name.clone(),
            });
        }
        self.resources.push(resource);
        Ok(())
    }

    /// Update an existing resource by uid. Returns an error if the resource is
    /// not found or if the resource_version does not match (optimistic
    /// concurrency).
    pub fn update(&mut self, resource: Resource) -> Result<(), ResourceError> {
        let idx = self.resources.iter().position(|r| r.uid == resource.uid).ok_or_else(|| {
            ResourceError::NotFound { kind: resource.kind.clone(), name: resource.name.clone() }
        })?;

        let existing = &self.resources[idx];
        if existing.resource_version != resource.resource_version {
            return Err(ResourceError::VersionConflict {
                expected: resource.resource_version.clone(),
                actual: existing.resource_version.clone(),
            });
        }

        self.resources[idx] = resource;
        Ok(())
    }

    /// Delete a resource by kind, name, and optional namespace. Returns the
    /// removed resource or an error if not found.
    pub fn delete(
        &mut self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Resource, ResourceError> {
        let idx = self
            .resources
            .iter()
            .position(|r| r.kind == kind && r.name == name && r.namespace.as_deref() == namespace)
            .ok_or_else(|| ResourceError::NotFound {
                kind: kind.to_string(),
                name: name.to_string(),
            })?;

        Ok(self.resources.remove(idx))
    }

    /// Count resources of a given kind.
    pub fn count(&self, kind: &str) -> usize {
        self.resources.iter().filter(|r| r.kind == kind).count()
    }

    /// Return a sorted, deduplicated list of resource kinds in the store.
    pub fn kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self.resources.iter().map(|r| r.kind.clone()).collect();
        kinds.sort();
        kinds.dedup();
        kinds
    }
}

impl Default for ResourceService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// T055: KubeResourceClient trait and InMemoryKubeClient
// ---------------------------------------------------------------------------

/// Errors from Kubernetes API operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum KubeApiError {
    #[error("API error: {status_code} {message}")]
    ApiError { status_code: u16, message: String },
    #[error("not found: {kind}/{name}")]
    NotFound { kind: String, name: String },
    #[error("conflict: resource version mismatch")]
    Conflict,
    #[error("forbidden: {message}")]
    Forbidden { message: String },
    #[error("connection error: {message}")]
    ConnectionError { message: String },
}

/// Trait abstracting Kubernetes API operations for any resource kind.
/// This enables testing with mock implementations.
pub trait KubeResourceClient: Send + Sync {
    /// List resources of a given kind, optionally filtered by namespace.
    fn list(
        &self,
        kind: &str,
        api_version: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<Resource>, KubeApiError>;

    /// Get a specific resource.
    fn get(
        &self,
        kind: &str,
        api_version: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Resource, KubeApiError>;

    /// Create a resource from JSON/YAML.
    fn create(
        &self,
        kind: &str,
        api_version: &str,
        namespace: Option<&str>,
        data: &serde_json::Value,
    ) -> Result<Resource, KubeApiError>;

    /// Update (replace) a resource.
    fn update(
        &self,
        kind: &str,
        api_version: &str,
        name: &str,
        namespace: Option<&str>,
        data: &serde_json::Value,
    ) -> Result<Resource, KubeApiError>;

    /// Delete a resource.
    fn delete(
        &self,
        kind: &str,
        api_version: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<(), KubeApiError>;

    /// Scale a deployment/statefulset.
    fn scale(
        &self,
        kind: &str,
        api_version: &str,
        name: &str,
        namespace: &str,
        replicas: u32,
    ) -> Result<(), KubeApiError>;

    /// Restart a workload by patching the pod template annotation.
    fn restart(
        &self,
        kind: &str,
        api_version: &str,
        name: &str,
        namespace: &str,
    ) -> Result<(), KubeApiError>;
}

/// An in-memory implementation of [`KubeResourceClient`] backed by [`ResourceService`].
/// Intended for testing without a real Kubernetes cluster.
pub struct InMemoryKubeClient {
    /// The cluster ID that this client targets.
    pub cluster_id: Uuid,
    /// The underlying resource store, wrapped in `Arc<RwLock<...>>` so the
    /// client can be `Send + Sync` while still allowing mutation.
    store: Arc<RwLock<ResourceService>>,
}

impl InMemoryKubeClient {
    /// Create a new in-memory client targeting the given cluster.
    pub fn new(cluster_id: Uuid) -> Self {
        Self { cluster_id, store: Arc::new(RwLock::new(ResourceService::new())) }
    }

    /// Create a new in-memory client with a pre-populated store.
    pub fn with_store(cluster_id: Uuid, store: Arc<RwLock<ResourceService>>) -> Self {
        Self { cluster_id, store }
    }

    /// Get a reference to the underlying store (useful for test assertions).
    pub fn store(&self) -> &Arc<RwLock<ResourceService>> {
        &self.store
    }
}

impl KubeResourceClient for InMemoryKubeClient {
    fn list(
        &self,
        kind: &str,
        _api_version: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<Resource>, KubeApiError> {
        let store = self.store.read().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;
        Ok(store.list(kind, namespace).into_iter().cloned().collect())
    }

    fn get(
        &self,
        kind: &str,
        _api_version: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Resource, KubeApiError> {
        let store = self.store.read().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;
        store.get(kind, name, namespace).cloned().ok_or_else(|| KubeApiError::NotFound {
            kind: kind.to_string(),
            name: name.to_string(),
        })
    }

    fn create(
        &self,
        kind: &str,
        api_version: &str,
        namespace: Option<&str>,
        data: &serde_json::Value,
    ) -> Result<Resource, KubeApiError> {
        let name = data
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        let ns = namespace
            .map(|s| s.to_string())
            .or_else(|| {
                data.get("metadata")
                    .and_then(|m| m.get("namespace"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();

        let resource = Resource::new(name, &ns, kind, api_version, self.cluster_id)
            .with_spec(data.get("spec").cloned().unwrap_or(Value::Null));

        let mut store = self.store.write().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;

        store.create(resource.clone()).map_err(|e| match e {
            ResourceError::AlreadyExists { kind, name } => KubeApiError::ApiError {
                status_code: 409,
                message: format!("resource already exists: {kind}/{name}"),
            },
            _ => KubeApiError::ApiError { status_code: 500, message: e.to_string() },
        })?;

        Ok(resource)
    }

    fn update(
        &self,
        kind: &str,
        _api_version: &str,
        name: &str,
        namespace: Option<&str>,
        data: &serde_json::Value,
    ) -> Result<Resource, KubeApiError> {
        let mut store = self.store.write().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;

        // Find the existing resource to get its uid and resource_version.
        let existing = store.get(kind, name, namespace).cloned().ok_or_else(|| {
            KubeApiError::NotFound { kind: kind.to_string(), name: name.to_string() }
        })?;

        let mut updated = existing.clone();
        if let Some(spec) = data.get("spec") {
            updated.spec = spec.clone();
        }
        if let Some(labels) =
            data.get("metadata").and_then(|m| m.get("labels")).and_then(|l| l.as_object())
        {
            for (k, v) in labels {
                if let Some(val) = v.as_str() {
                    updated.labels.insert(k.clone(), val.to_string());
                }
            }
        }

        store.update(updated.clone()).map_err(|e| match e {
            ResourceError::VersionConflict { .. } => KubeApiError::Conflict,
            ResourceError::NotFound { kind, name } => KubeApiError::NotFound { kind, name },
            _ => KubeApiError::ApiError { status_code: 500, message: e.to_string() },
        })?;

        Ok(updated)
    }

    fn delete(
        &self,
        kind: &str,
        _api_version: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<(), KubeApiError> {
        let mut store = self.store.write().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;

        store.delete(kind, name, namespace).map_err(|e| match e {
            ResourceError::NotFound { kind, name } => KubeApiError::NotFound { kind, name },
            _ => KubeApiError::ApiError { status_code: 500, message: e.to_string() },
        })?;

        Ok(())
    }

    fn scale(
        &self,
        kind: &str,
        _api_version: &str,
        name: &str,
        namespace: &str,
        replicas: u32,
    ) -> Result<(), KubeApiError> {
        let mut store = self.store.write().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;

        let existing = store.get(kind, name, Some(namespace)).cloned().ok_or_else(|| {
            KubeApiError::NotFound { kind: kind.to_string(), name: name.to_string() }
        })?;

        let scalable = matches!(kind, "Deployment" | "StatefulSet" | "ReplicaSet");
        if !scalable {
            return Err(KubeApiError::ApiError {
                status_code: 400,
                message: format!("resource kind {kind} is not scalable"),
            });
        }

        let mut updated = existing;
        // Merge replicas into the spec.
        if let Value::Object(ref mut map) = updated.spec {
            map.insert("replicas".to_string(), Value::from(replicas));
        } else {
            updated.spec = serde_json::json!({ "replicas": replicas });
        }

        store.update(updated).map_err(|e| match e {
            ResourceError::VersionConflict { .. } => KubeApiError::Conflict,
            ResourceError::NotFound { kind, name } => KubeApiError::NotFound { kind, name },
            _ => KubeApiError::ApiError { status_code: 500, message: e.to_string() },
        })?;

        Ok(())
    }

    fn restart(
        &self,
        kind: &str,
        _api_version: &str,
        name: &str,
        namespace: &str,
    ) -> Result<(), KubeApiError> {
        let mut store = self.store.write().map_err(|e| KubeApiError::ConnectionError {
            message: format!("lock poisoned: {e}"),
        })?;

        let existing = store.get(kind, name, Some(namespace)).cloned().ok_or_else(|| {
            KubeApiError::NotFound { kind: kind.to_string(), name: name.to_string() }
        })?;

        let restartable = matches!(kind, "Deployment" | "StatefulSet" | "DaemonSet");
        if !restartable {
            return Err(KubeApiError::ApiError {
                status_code: 400,
                message: format!("resource kind {kind} is not restartable"),
            });
        }

        let mut updated = existing;
        // Simulate restart by adding a restart annotation with current timestamp.
        updated
            .annotations
            .insert("kubectl.kubernetes.io/restartedAt".to_string(), Utc::now().to_rfc3339());

        store.update(updated).map_err(|e| match e {
            ResourceError::VersionConflict { .. } => KubeApiError::Conflict,
            ResourceError::NotFound { kind, name } => KubeApiError::NotFound { kind, name },
            _ => KubeApiError::ApiError { status_code: 500, message: e.to_string() },
        })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// T117: Relationship extraction types and functions
// ---------------------------------------------------------------------------

/// A reference to a Kubernetes resource by kind, name, and optional namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

impl ResourceRef {
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        namespace: Option<String>,
    ) -> Self {
        Self { kind: kind.into(), name: name.into(), namespace }
    }

    /// Produce a stable key for graph deduplication.
    pub fn key(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}/{}", self.kind, ns, self.name),
            None => format!("{}/{}", self.kind, self.name),
        }
    }
}

/// The kind of relationship between two resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationshipKind {
    /// The target owns the source (via ownerReferences).
    OwnerReference,
    /// The source Service selects the target Pods via label selector.
    ServiceSelector,
    /// The source Ingress routes traffic to the target Service backend.
    IngressBackend,
    /// Node schedules Pod (spec.nodeName).
    NodeToPod,
    /// Pod references Secret (imagePullSecrets, volumes).
    PodToSecret,
    /// Pod references ConfigMap (volumes, envFrom).
    PodToConfigMap,
    /// Pod mounts PVC (volumes.persistentVolumeClaim).
    PodToPVC,
    /// PVC binds to PV (spec.volumeName).
    PVCToPV,
    /// PDB selects Pods via label selector.
    PDBSelector,
}

/// A directed relationship between two resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRelationship {
    pub source: ResourceRef,
    pub target: ResourceRef,
    pub kind: RelationshipKind,
}

impl ResourceRelationship {
    pub fn new(source: ResourceRef, target: ResourceRef, kind: RelationshipKind) -> Self {
        Self { source, target, kind }
    }
}

/// Build a list of relationships from a set of resources.
///
/// This extracts three kinds of relationships:
/// 1. **OwnerReference**: When resource A has an ownerReference pointing to resource B,
///    we create a relationship from B (owner) to A (owned).
/// 2. **ServiceSelector**: When a Service's spec.selector matches a Pod's labels,
///    we create a relationship from the Service to the Pod.
/// 3. **IngressBackend**: When an Ingress's spec.rules[].http.paths[].backend.service.name
///    references a Service, we create a relationship from the Ingress to the Service.
pub fn build_relationship_graph(resources: &[Resource]) -> Vec<ResourceRelationship> {
    let mut relationships = Vec::new();

    // Index resources by uid for owner reference resolution
    let uid_map: HashMap<&str, &Resource> = resources.iter().map(|r| (r.uid.as_str(), r)).collect();

    // Pre-index Pods by namespace for O(pods_in_ns) selector matching instead of O(N).
    let mut pods_by_ns: HashMap<Option<&str>, Vec<&Resource>> = HashMap::new();
    for r in resources {
        if r.kind == "Pod" {
            pods_by_ns.entry(r.namespace.as_deref()).or_default().push(r);
        }
    }

    for resource in resources {
        // 1. Owner references: owner -> owned
        for owner_ref in &resource.owner_references {
            let owner_resource_ref = if let Some(owner) = uid_map.get(owner_ref.uid.as_str()) {
                ResourceRef::new(&owner.kind, &owner.name, owner.namespace.clone())
            } else {
                // Owner not in our resource set; use the info from the owner reference itself
                ResourceRef::new(&owner_ref.kind, &owner_ref.name, resource.namespace.clone())
            };

            let owned_ref =
                ResourceRef::new(&resource.kind, &resource.name, resource.namespace.clone());

            relationships.push(ResourceRelationship::new(
                owner_resource_ref,
                owned_ref,
                RelationshipKind::OwnerReference,
            ));
        }

        // 2. Service selector: Service -> matching Pods
        if resource.kind == "Service" {
            if let Some(selector) = resource.spec.get("selector").and_then(|s| s.as_object()) {
                if !selector.is_empty() {
                    let service_ref =
                        ResourceRef::new("Service", &resource.name, resource.namespace.clone());

                    // Use pre-indexed Pods for the same namespace
                    let ns_key = resource.namespace.as_deref();
                    if let Some(ns_pods) = pods_by_ns.get(&ns_key) {
                        for candidate in ns_pods {
                            let all_match = selector.iter().all(|(k, v)| {
                                v.as_str()
                                    .map(|val| {
                                        candidate.labels.get(k).map(|l| l.as_str()) == Some(val)
                                    })
                                    .unwrap_or(false)
                            });

                            if all_match {
                                let pod_ref = ResourceRef::new(
                                    "Pod",
                                    &candidate.name,
                                    candidate.namespace.clone(),
                                );
                                relationships.push(ResourceRelationship::new(
                                    service_ref.clone(),
                                    pod_ref,
                                    RelationshipKind::ServiceSelector,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // 3. Ingress backend: Ingress -> Service
        if resource.kind == "Ingress" {
            if let Some(rules) = resource.spec.get("rules").and_then(|r| r.as_array()) {
                let ingress_ref =
                    ResourceRef::new("Ingress", &resource.name, resource.namespace.clone());

                for rule in rules {
                    if let Some(paths) =
                        rule.get("http").and_then(|h| h.get("paths")).and_then(|p| p.as_array())
                    {
                        for path in paths {
                            if let Some(svc_name) = path
                                .get("backend")
                                .and_then(|b| b.get("service"))
                                .and_then(|s| s.get("name"))
                                .and_then(|n| n.as_str())
                            {
                                let svc_ref = ResourceRef::new(
                                    "Service",
                                    svc_name,
                                    resource.namespace.clone(),
                                );
                                relationships.push(ResourceRelationship::new(
                                    ingress_ref.clone(),
                                    svc_ref,
                                    RelationshipKind::IngressBackend,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // 4. Pod -> Node (via spec.nodeName)
        if resource.kind == "Pod" {
            if let Some(node_name) = resource.spec.get("nodeName").and_then(|n| n.as_str()) {
                let node_ref = ResourceRef::new("Node", node_name, None);
                let pod_ref = ResourceRef::new("Pod", &resource.name, resource.namespace.clone());
                relationships.push(ResourceRelationship::new(
                    node_ref,
                    pod_ref,
                    RelationshipKind::NodeToPod,
                ));
            }
        }

        // 5. Pod -> Secret (imagePullSecrets, volumes)
        if resource.kind == "Pod" {
            let pod_ref = ResourceRef::new("Pod", &resource.name, resource.namespace.clone());
            let mut seen_secrets: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            // imagePullSecrets
            if let Some(ips) = resource.spec.get("imagePullSecrets").and_then(|v| v.as_array()) {
                for s in ips {
                    if let Some(name) = s.get("name").and_then(|n| n.as_str()) {
                        if seen_secrets.insert(name.to_string()) {
                            relationships.push(ResourceRelationship::new(
                                pod_ref.clone(),
                                ResourceRef::new("Secret", name, resource.namespace.clone()),
                                RelationshipKind::PodToSecret,
                            ));
                        }
                    }
                }
            }
            // volumes[].secret.secretName
            if let Some(volumes) = resource.spec.get("volumes").and_then(|v| v.as_array()) {
                for vol in volumes {
                    if let Some(name) =
                        vol.get("secret").and_then(|s| s.get("secretName")).and_then(|n| n.as_str())
                    {
                        if seen_secrets.insert(name.to_string()) {
                            relationships.push(ResourceRelationship::new(
                                pod_ref.clone(),
                                ResourceRef::new("Secret", name, resource.namespace.clone()),
                                RelationshipKind::PodToSecret,
                            ));
                        }
                    }
                }
            }
        }

        // 6. Pod -> ConfigMap (volumes, envFrom)
        if resource.kind == "Pod" {
            let pod_ref = ResourceRef::new("Pod", &resource.name, resource.namespace.clone());
            let mut seen_cms: std::collections::HashSet<String> = std::collections::HashSet::new();

            // volumes[].configMap.name
            if let Some(volumes) = resource.spec.get("volumes").and_then(|v| v.as_array()) {
                for vol in volumes {
                    if let Some(name) =
                        vol.get("configMap").and_then(|cm| cm.get("name")).and_then(|n| n.as_str())
                    {
                        if seen_cms.insert(name.to_string()) {
                            relationships.push(ResourceRelationship::new(
                                pod_ref.clone(),
                                ResourceRef::new("ConfigMap", name, resource.namespace.clone()),
                                RelationshipKind::PodToConfigMap,
                            ));
                        }
                    }
                }
            }
            // containers[].envFrom[].configMapRef.name
            if let Some(containers) = resource.spec.get("containers").and_then(|c| c.as_array()) {
                for container in containers {
                    if let Some(env_from) = container.get("envFrom").and_then(|e| e.as_array()) {
                        for ef in env_from {
                            if let Some(name) = ef
                                .get("configMapRef")
                                .and_then(|r| r.get("name"))
                                .and_then(|n| n.as_str())
                            {
                                if seen_cms.insert(name.to_string()) {
                                    relationships.push(ResourceRelationship::new(
                                        pod_ref.clone(),
                                        ResourceRef::new(
                                            "ConfigMap",
                                            name,
                                            resource.namespace.clone(),
                                        ),
                                        RelationshipKind::PodToConfigMap,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 7. Pod -> PVC (volumes.persistentVolumeClaim.claimName)
        if resource.kind == "Pod" {
            if let Some(volumes) = resource.spec.get("volumes").and_then(|v| v.as_array()) {
                let pod_ref = ResourceRef::new("Pod", &resource.name, resource.namespace.clone());
                for vol in volumes {
                    if let Some(claim) = vol
                        .get("persistentVolumeClaim")
                        .and_then(|p| p.get("claimName"))
                        .and_then(|n| n.as_str())
                    {
                        relationships.push(ResourceRelationship::new(
                            pod_ref.clone(),
                            ResourceRef::new(
                                "PersistentVolumeClaim",
                                claim,
                                resource.namespace.clone(),
                            ),
                            RelationshipKind::PodToPVC,
                        ));
                    }
                }
            }
        }

        // 8. PVC -> PV (spec.volumeName)
        if resource.kind == "PersistentVolumeClaim" {
            if let Some(pv_name) = resource.spec.get("volumeName").and_then(|n| n.as_str()) {
                let pvc_ref = ResourceRef::new(
                    "PersistentVolumeClaim",
                    &resource.name,
                    resource.namespace.clone(),
                );
                relationships.push(ResourceRelationship::new(
                    pvc_ref,
                    ResourceRef::new("PersistentVolume", pv_name, None),
                    RelationshipKind::PVCToPV,
                ));
            }
        }

        // 9. PDB -> Pods (spec.selector.matchLabels)
        if resource.kind == "PodDisruptionBudget" {
            if let Some(selector) = resource
                .spec
                .get("selector")
                .and_then(|s| s.get("matchLabels"))
                .and_then(|m| m.as_object())
            {
                if !selector.is_empty() {
                    let pdb_ref = ResourceRef::new(
                        "PodDisruptionBudget",
                        &resource.name,
                        resource.namespace.clone(),
                    );
                    let ns_key = resource.namespace.as_deref();
                    if let Some(ns_pods) = pods_by_ns.get(&ns_key) {
                        for candidate in ns_pods {
                            let all_match = selector.iter().all(|(k, v)| {
                                v.as_str()
                                    .map(|val| {
                                        candidate.labels.get(k).map(|l| l.as_str()) == Some(val)
                                    })
                                    .unwrap_or(false)
                            });
                            if all_match {
                                relationships.push(ResourceRelationship::new(
                                    pdb_ref.clone(),
                                    ResourceRef::new(
                                        "Pod",
                                        &candidate.name,
                                        candidate.namespace.clone(),
                                    ),
                                    RelationshipKind::PDBSelector,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    relationships
}

/// Build a petgraph `DiGraph` from resource relationships.
///
/// Returns the graph along with a map from `ResourceRef` key to the graph node index.
pub fn build_dag(
    relationships: &[ResourceRelationship],
) -> (DiGraph<ResourceRef, RelationshipKind>, HashMap<String, petgraph::graph::NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut node_indices: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    let get_or_insert = |graph: &mut DiGraph<ResourceRef, RelationshipKind>,
                         node_map: &mut HashMap<String, petgraph::graph::NodeIndex>,
                         resource_ref: &ResourceRef|
     -> petgraph::graph::NodeIndex {
        let key = resource_ref.key();
        if let Some(&idx) = node_map.get(&key) {
            idx
        } else {
            let idx = graph.add_node(resource_ref.clone());
            node_map.insert(key, idx);
            idx
        }
    };

    for rel in relationships {
        let source_idx = get_or_insert(&mut graph, &mut node_indices, &rel.source);
        let target_idx = get_or_insert(&mut graph, &mut node_indices, &rel.target);
        graph.add_edge(source_idx, target_idx, rel.kind);
    }

    (graph, node_indices)
}

/// Extract a subgraph around a focus resource via BFS in both directions.
///
/// Returns the filtered relationships where both endpoints are reachable
/// within `max_depth` hops, plus the focus key for highlighting.
pub fn build_topology_subgraph(
    focus: &ResourceRef,
    all_relationships: &[ResourceRelationship],
    max_depth: usize,
) -> (Vec<ResourceRelationship>, String) {
    use std::collections::{HashSet, VecDeque};

    let focus_key = focus.key();

    // Build bidirectional adjacency
    let mut forward: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for rel in all_relationships {
        let sk = rel.source.key();
        let tk = rel.target.key();
        forward.entry(sk.clone()).or_default().push(tk.clone());
        reverse.entry(tk).or_default().push(sk);
    }

    // BFS from focus in both directions
    let mut reachable: HashSet<String> = HashSet::new();
    reachable.insert(focus_key.clone());
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((focus_key.clone(), 0));

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for neighbor in forward.get(&node).into_iter().flatten() {
            if reachable.insert(neighbor.clone()) {
                queue.push_back((neighbor.clone(), depth + 1));
            }
        }
        for neighbor in reverse.get(&node).into_iter().flatten() {
            if reachable.insert(neighbor.clone()) {
                queue.push_back((neighbor.clone(), depth + 1));
            }
        }
    }

    let filtered: Vec<ResourceRelationship> = all_relationships
        .iter()
        .filter(|rel| {
            reachable.contains(&rel.source.key()) && reachable.contains(&rel.target.key())
        })
        .cloned()
        .collect();

    (filtered, focus_key)
}

/// Convert raw K8s JSON into a `Resource` struct for relationship graph building.
pub fn resource_from_json(json: &Value, cluster_id: Uuid) -> Option<Resource> {
    let metadata = json.get("metadata")?;
    let name = metadata.get("name")?.as_str()?.to_string();
    let namespace = metadata.get("namespace").and_then(|n| n.as_str()).map(String::from);
    let uid = metadata.get("uid").and_then(|u| u.as_str()).unwrap_or("").to_string();
    let kind = json.get("kind").and_then(|k| k.as_str()).unwrap_or("").to_string();
    let api_version = json.get("apiVersion").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let resource_version =
        metadata.get("resourceVersion").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let creation_timestamp = metadata
        .get("creationTimestamp")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<DateTime<Utc>>().ok())
        .unwrap_or_default();

    let labels: BTreeMap<String, String> = metadata
        .get("labels")
        .and_then(|l| l.as_object())
        .map(|obj| {
            obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
        })
        .unwrap_or_default();

    let annotations: BTreeMap<String, String> = metadata
        .get("annotations")
        .and_then(|a| a.as_object())
        .map(|obj| {
            obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
        })
        .unwrap_or_default();

    let owner_references: Vec<OwnerReference> = metadata
        .get("ownerReferences")
        .and_then(|o| o.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|or| {
                    Some(OwnerReference {
                        uid: or.get("uid")?.as_str()?.to_string(),
                        kind: or.get("kind")?.as_str()?.to_string(),
                        name: or.get("name")?.as_str()?.to_string(),
                        api_version: or
                            .get("apiVersion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        controller: or.get("controller").and_then(|c| c.as_bool()).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let spec = json.get("spec").cloned().unwrap_or(Value::Null);
    let status = json.get("status").cloned();

    let conditions: Vec<Condition> = status
        .as_ref()
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    Some(Condition {
                        type_name: c.get("type")?.as_str()?.to_string(),
                        status: c.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                        reason: c.get("reason").and_then(|r| r.as_str()).map(String::from),
                        message: c.get("message").and_then(|m| m.as_str()).map(String::from),
                        last_transition: c
                            .get("lastTransitionTime")
                            .and_then(|t| t.as_str())
                            .and_then(|t| t.parse().ok())
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Resource {
        uid,
        name,
        namespace,
        kind,
        api_version,
        labels,
        annotations,
        creation_timestamp,
        resource_version,
        owner_references,
        spec,
        status,
        conditions,
        cluster_id,
    })
}

// ---------------------------------------------------------------------------
// T133: Global Search Service
// ---------------------------------------------------------------------------

/// A query for global search across clusters, namespaces, and resource kinds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The search text to match against resource names, namespaces, and kinds.
    pub query: String,
    /// Optional filter: restrict results to specific cluster IDs.
    pub clusters: Option<Vec<Uuid>>,
    /// Optional filter: restrict results to specific namespaces.
    pub namespaces: Option<Vec<String>>,
    /// Optional filter: restrict results to specific resource kinds.
    pub kinds: Option<Vec<String>>,
}

impl SearchQuery {
    /// Create a new search query with just a query string.
    pub fn new(query: impl Into<String>) -> Self {
        Self { query: query.into(), clusters: None, namespaces: None, kinds: None }
    }

    /// Builder method to filter by clusters.
    pub fn with_clusters(mut self, clusters: Vec<Uuid>) -> Self {
        self.clusters = Some(clusters);
        self
    }

    /// Builder method to filter by namespaces.
    pub fn with_namespaces(mut self, namespaces: Vec<String>) -> Self {
        self.namespaces = Some(namespaces);
        self
    }

    /// Builder method to filter by kinds.
    pub fn with_kinds(mut self, kinds: Vec<String>) -> Self {
        self.kinds = Some(kinds);
        self
    }
}

/// A single result from the global search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The kind of the matched resource (e.g., "Pod", "Deployment").
    pub kind: String,
    /// The name of the matched resource.
    pub name: String,
    /// The namespace of the matched resource (None for cluster-scoped).
    pub namespace: Option<String>,
    /// The cluster ID where this resource lives.
    pub cluster_id: Uuid,
    /// The relevance score (higher is better).
    pub score: u32,
}

/// State for the global search service.
#[derive(Debug)]
pub struct GlobalSearchState {
    /// The current search results.
    pub results: Vec<SearchResult>,
    /// Whether a search is currently in progress.
    pub loading: bool,
    /// An error message if the last search failed.
    pub error: Option<String>,
    /// Debounce interval in milliseconds for query input.
    pub debounce_ms: u64,
}

impl Default for GlobalSearchState {
    fn default() -> Self {
        Self { results: Vec::new(), loading: false, error: None, debounce_ms: 300 }
    }
}

impl GlobalSearchState {
    /// Create a new global search state with the default debounce interval.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new global search state with a custom debounce interval.
    pub fn with_debounce(debounce_ms: u64) -> Self {
        Self { debounce_ms, ..Default::default() }
    }

    /// Set the search query, marking the state as loading and clearing previous errors.
    pub fn set_query(&mut self) {
        self.loading = true;
        self.error = None;
    }

    /// Clear all results and reset state.
    pub fn clear(&mut self) {
        self.results.clear();
        self.loading = false;
        self.error = None;
    }

    /// Set the search results, marking loading as complete.
    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        self.loading = false;
        self.error = None;
    }

    /// Filter results by a specific resource kind.
    pub fn filtered_by_kind(&self, kind: &str) -> Vec<&SearchResult> {
        self.results.iter().filter(|r| r.kind == kind).collect()
    }

    /// Return the top N results by score.
    pub fn top_results(&self, n: usize) -> Vec<&SearchResult> {
        self.results.iter().take(n).collect()
    }
}

/// Perform a global search across a set of resources using the given query.
///
/// This function applies the filters from `SearchQuery` (clusters, namespaces,
/// kinds) and then fuzzy-matches the query text against resource names. Results
/// are returned sorted by score descending.
pub fn global_search(query: &SearchQuery, resources: &[Resource]) -> Vec<SearchResult> {
    if query.query.is_empty() {
        return Vec::new();
    }

    let query_lower = query.query.to_lowercase();

    let mut results: Vec<SearchResult> = resources
        .iter()
        .filter(|r| {
            // Apply cluster filter
            if let Some(ref clusters) = query.clusters {
                if !clusters.contains(&r.cluster_id) {
                    return false;
                }
            }
            // Apply namespace filter
            if let Some(ref namespaces) = query.namespaces {
                match &r.namespace {
                    Some(ns) => {
                        if !namespaces.contains(ns) {
                            return false;
                        }
                    }
                    None => return false, // cluster-scoped resources excluded when filtering by namespace
                }
            }
            // Apply kind filter
            if let Some(ref kinds) = query.kinds {
                if !kinds.contains(&r.kind) {
                    return false;
                }
            }
            true
        })
        .filter_map(|r| {
            // Fuzzy match against name
            let name_lower = r.name.to_lowercase();
            let score = if name_lower == query_lower {
                Some(200)
            } else if name_lower.starts_with(&query_lower) {
                Some(100)
            } else if name_lower.contains(&query_lower) {
                Some(50)
            } else {
                // Subsequence match
                let mut chars = query_lower.chars().peekable();
                let mut matched = true;
                let mut name_chars = name_lower.chars();
                for qc in &mut chars {
                    let mut found = false;
                    for nc in name_chars.by_ref() {
                        if nc == qc {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        matched = false;
                        break;
                    }
                }
                if matched { Some(10) } else { None }
            };

            score.map(|s| SearchResult {
                kind: r.kind.clone(),
                name: r.name.clone(),
                namespace: r.namespace.clone(),
                cluster_id: r.cluster_id,
                score: s,
            })
        })
        .collect();

    results.sort_by_key(|a| std::cmp::Reverse(a.score));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_resource(namespace: Option<&str>, owners: Vec<OwnerReference>) -> Resource {
        Resource {
            uid: "uid-1".to_string(),
            name: "test-resource".to_string(),
            namespace: namespace.map(|s| s.to_string()),
            kind: "Pod".to_string(),
            api_version: "v1".to_string(),
            labels: BTreeMap::from([("app".to_string(), "test".to_string())]),
            annotations: BTreeMap::new(),
            creation_timestamp: Utc::now(),
            resource_version: "12345".to_string(),
            owner_references: owners,
            spec: serde_json::json!({}),
            status: Some(serde_json::json!({"phase": "Running"})),
            conditions: vec![],
            cluster_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn test_resource_is_namespaced() {
        let namespaced = make_resource(Some("default"), vec![]);
        assert!(namespaced.is_namespaced());

        let cluster_scoped = make_resource(None, vec![]);
        assert!(!cluster_scoped.is_namespaced());
    }

    #[test]
    fn test_resource_owner_references() {
        let owner = OwnerReference {
            uid: "owner-uid".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "my-rs".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        };

        let resource = make_resource(Some("default"), vec![owner.clone()]);
        assert!(resource.has_owner());
        assert_eq!(resource.controller_owner(), Some(&owner));

        let orphan = make_resource(Some("default"), vec![]);
        assert!(!orphan.has_owner());
        assert!(orphan.controller_owner().is_none());
    }

    #[test]
    fn test_resource_non_controller_owner() {
        let owner = OwnerReference {
            uid: "owner-uid".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "my-rs".to_string(),
            api_version: "apps/v1".to_string(),
            controller: false,
        };

        let resource = make_resource(Some("default"), vec![owner]);
        assert!(resource.has_owner());
        assert!(resource.controller_owner().is_none());
    }

    #[test]
    fn test_condition_status_helpers() {
        let ready = Condition {
            type_name: "Ready".to_string(),
            status: "True".to_string(),
            reason: None,
            message: None,
            last_transition: Utc::now(),
        };
        assert!(ready.is_true());
        assert!(!ready.is_false());
        assert!(!ready.is_unknown());

        let not_ready = Condition {
            type_name: "Ready".to_string(),
            status: "False".to_string(),
            reason: Some("CrashLoopBackOff".to_string()),
            message: Some("Container exited with code 1".to_string()),
            last_transition: Utc::now(),
        };
        assert!(not_ready.is_false());
        assert!(!not_ready.is_true());

        let unknown = Condition {
            type_name: "Ready".to_string(),
            status: "Unknown".to_string(),
            reason: None,
            message: None,
            last_transition: Utc::now(),
        };
        assert!(unknown.is_unknown());
    }

    #[test]
    fn test_resource_is_ready() {
        let mut resource = make_resource(Some("default"), vec![]);
        assert!(!resource.is_ready());

        resource.conditions.push(Condition {
            type_name: "Ready".to_string(),
            status: "True".to_string(),
            reason: None,
            message: None,
            last_transition: Utc::now(),
        });
        assert!(resource.is_ready());
    }

    #[test]
    fn test_resource_serialization() {
        let resource = make_resource(Some("kube-system"), vec![]);
        let json = serde_json::to_string(&resource).unwrap();
        let deserialized: Resource = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.uid, resource.uid);
        assert_eq!(deserialized.name, resource.name);
        assert_eq!(deserialized.namespace, resource.namespace);
        assert_eq!(deserialized.kind, resource.kind);
    }

    #[test]
    fn test_owner_reference_serialization() {
        let owner = OwnerReference {
            uid: "uid-123".to_string(),
            kind: "Deployment".to_string(),
            name: "my-deploy".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        };

        let json = serde_json::to_string(&owner).unwrap();
        let deserialized: OwnerReference = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, owner);
    }

    // ---------------------------------------------------------------
    // Builder method tests
    // ---------------------------------------------------------------

    #[test]
    fn test_with_label_adds_label() {
        let cluster = Uuid::new_v4();
        let resource = Resource::new("my-pod", "default", "Pod", "v1", cluster)
            .with_label("app", "web")
            .with_label("env", "production");

        assert_eq!(resource.labels.get("app").unwrap(), "web");
        assert_eq!(resource.labels.get("env").unwrap(), "production");
        assert_eq!(resource.labels.len(), 2);
    }

    #[test]
    fn test_with_label_overwrites_existing() {
        let cluster = Uuid::new_v4();
        let resource = Resource::new("my-pod", "default", "Pod", "v1", cluster)
            .with_label("app", "web")
            .with_label("app", "api");

        assert_eq!(resource.labels.get("app").unwrap(), "api");
        assert_eq!(resource.labels.len(), 1);
    }

    #[test]
    fn test_with_spec_sets_spec() {
        let cluster = Uuid::new_v4();
        let spec = serde_json::json!({"replicas": 3, "selector": {"app": "web"}});
        let resource = Resource::new("my-deploy", "default", "Deployment", "apps/v1", cluster)
            .with_spec(spec.clone());

        assert_eq!(resource.spec, spec);
    }

    #[test]
    fn test_with_spec_overwrites_default_null() {
        let cluster = Uuid::new_v4();
        let resource = Resource::new("my-pod", "default", "Pod", "v1", cluster);
        assert_eq!(resource.spec, Value::Null);

        let resource = resource.with_spec(serde_json::json!({"containers": []}));
        assert_ne!(resource.spec, Value::Null);
    }

    #[test]
    fn test_builder_chaining() {
        let cluster = Uuid::new_v4();
        let spec = serde_json::json!({"image": "nginx"});
        let resource = Resource::new("nginx-pod", "prod", "Pod", "v1", cluster)
            .with_label("app", "nginx")
            .with_label("tier", "frontend")
            .with_spec(spec.clone());

        assert_eq!(resource.name, "nginx-pod");
        assert_eq!(resource.namespace, Some("prod".to_string()));
        assert_eq!(resource.labels.len(), 2);
        assert_eq!(resource.spec, spec);
    }

    // ---------------------------------------------------------------
    // ResourceService tests
    // ---------------------------------------------------------------

    fn test_cluster_id() -> Uuid {
        Uuid::new_v4()
    }

    fn make_service_with_resources() -> (ResourceService, Uuid) {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(Resource::new("pod-a", "default", "Pod", "v1", cluster)).unwrap();
        svc.create(Resource::new("pod-b", "default", "Pod", "v1", cluster)).unwrap();
        svc.create(Resource::new("pod-c", "kube-system", "Pod", "v1", cluster)).unwrap();
        svc.create(Resource::new("deploy-a", "default", "Deployment", "apps/v1", cluster)).unwrap();
        svc.create(Resource::new("node-1", "", "Node", "v1", cluster)).unwrap();

        (svc, cluster)
    }

    // --- list tests ---

    #[test]
    fn test_list_all_pods() {
        let (svc, _) = make_service_with_resources();
        let pods = svc.list("Pod", None);
        assert_eq!(pods.len(), 3);
    }

    #[test]
    fn test_list_pods_by_namespace() {
        let (svc, _) = make_service_with_resources();
        let pods = svc.list("Pod", Some("default"));
        assert_eq!(pods.len(), 2);

        let pods = svc.list("Pod", Some("kube-system"));
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].name, "pod-c");
    }

    #[test]
    fn test_list_pods_in_nonexistent_namespace() {
        let (svc, _) = make_service_with_resources();
        let pods = svc.list("Pod", Some("nonexistent"));
        assert!(pods.is_empty());
    }

    #[test]
    fn test_list_nonexistent_kind() {
        let (svc, _) = make_service_with_resources();
        let result = svc.list("ConfigMap", None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_deployments() {
        let (svc, _) = make_service_with_resources();
        let deploys = svc.list("Deployment", None);
        assert_eq!(deploys.len(), 1);
        assert_eq!(deploys[0].name, "deploy-a");
    }

    #[test]
    fn test_list_cluster_scoped_resources() {
        let (svc, _) = make_service_with_resources();
        // Node is cluster-scoped (namespace is None)
        let nodes = svc.list("Node", None);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "node-1");
        assert!(!nodes[0].is_namespaced());
    }

    // --- get tests ---

    #[test]
    fn test_get_existing_resource() {
        let (svc, _) = make_service_with_resources();
        let result = svc.get("Pod", "pod-a", Some("default"));
        assert!(result.is_some());
        let pod = result.unwrap();
        assert_eq!(pod.name, "pod-a");
        assert_eq!(pod.kind, "Pod");
        assert_eq!(pod.namespace, Some("default".to_string()));
    }

    #[test]
    fn test_get_nonexistent_resource() {
        let (svc, _) = make_service_with_resources();
        let result = svc.get("Pod", "no-such-pod", Some("default"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_wrong_namespace() {
        let (svc, _) = make_service_with_resources();
        // pod-a is in "default", not "kube-system"
        let result = svc.get("Pod", "pod-a", Some("kube-system"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_wrong_kind() {
        let (svc, _) = make_service_with_resources();
        let result = svc.get("Deployment", "pod-a", Some("default"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_cluster_scoped_resource() {
        let (svc, _) = make_service_with_resources();
        let result = svc.get("Node", "node-1", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "node-1");
    }

    #[test]
    fn test_get_cluster_scoped_with_namespace_returns_none() {
        let (svc, _) = make_service_with_resources();
        // Node is cluster-scoped, searching with a namespace should find nothing
        let result = svc.get("Node", "node-1", Some("default"));
        assert!(result.is_none());
    }

    // --- create tests ---

    #[test]
    fn test_create_new_resource() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        let result = svc.create(Resource::new("my-pod", "default", "Pod", "v1", cluster));
        assert!(result.is_ok());
        assert_eq!(svc.resources.len(), 1);
        assert_eq!(svc.resources[0].name, "my-pod");
    }

    #[test]
    fn test_create_duplicate_resource_errors() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(Resource::new("my-pod", "default", "Pod", "v1", cluster)).unwrap();
        let result = svc.create(Resource::new("my-pod", "default", "Pod", "v1", cluster));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ResourceError::AlreadyExists { kind: "Pod".to_string(), name: "my-pod".to_string() }
        );
    }

    #[test]
    fn test_create_same_name_different_namespace_ok() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(Resource::new("my-pod", "default", "Pod", "v1", cluster)).unwrap();
        let result = svc.create(Resource::new("my-pod", "kube-system", "Pod", "v1", cluster));
        assert!(result.is_ok());
        assert_eq!(svc.resources.len(), 2);
    }

    #[test]
    fn test_create_same_name_different_kind_ok() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(Resource::new("my-resource", "default", "Pod", "v1", cluster)).unwrap();
        let result = svc.create(Resource::new("my-resource", "default", "Service", "v1", cluster));
        assert!(result.is_ok());
        assert_eq!(svc.resources.len(), 2);
    }

    // --- update tests ---

    #[test]
    fn test_update_existing_resource() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(
            Resource::new("my-pod", "default", "Pod", "v1", cluster).with_label("version", "1"),
        )
        .unwrap();

        let existing = svc.get("Pod", "my-pod", Some("default")).unwrap();
        let uid = existing.uid.clone();
        let version = existing.resource_version.clone();

        let mut updated =
            Resource::new("my-pod", "default", "Pod", "v1", cluster).with_label("version", "2");
        updated.uid = uid;
        updated.resource_version = version;

        let result = svc.update(updated);
        assert!(result.is_ok());

        let fetched = svc.get("Pod", "my-pod", Some("default")).unwrap();
        assert_eq!(fetched.labels.get("version").unwrap(), "2");
    }

    #[test]
    fn test_update_nonexistent_resource_errors() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        let resource = Resource::new("ghost-pod", "default", "Pod", "v1", cluster);
        let result = svc.update(resource);

        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceError::NotFound { kind, name } => {
                assert_eq!(kind, "Pod");
                assert_eq!(name, "ghost-pod");
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_update_with_version_conflict_errors() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        svc.create(Resource::new("my-pod", "default", "Pod", "v1", cluster)).unwrap();

        let existing = svc.get("Pod", "my-pod", Some("default")).unwrap();
        let uid = existing.uid.clone();

        let mut updated = Resource::new("my-pod", "default", "Pod", "v1", cluster);
        updated.uid = uid;
        updated.resource_version = "wrong-version".to_string();

        let result = svc.update(updated);
        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceError::VersionConflict { expected, actual } => {
                assert_eq!(expected, "wrong-version");
                assert_eq!(actual, "1");
            }
            other => panic!("expected VersionConflict, got {:?}", other),
        }
    }

    // --- delete tests ---

    #[test]
    fn test_delete_existing_resource() {
        let (mut svc, _) = make_service_with_resources();
        let initial_count = svc.resources.len();

        let result = svc.delete("Pod", "pod-a", Some("default"));
        assert!(result.is_ok());
        let deleted = result.unwrap();
        assert_eq!(deleted.name, "pod-a");
        assert_eq!(deleted.kind, "Pod");
        assert_eq!(svc.resources.len(), initial_count - 1);

        // Verify it's truly gone
        assert!(svc.get("Pod", "pod-a", Some("default")).is_none());
    }

    #[test]
    fn test_delete_nonexistent_resource_errors() {
        let (mut svc, _) = make_service_with_resources();
        let result = svc.delete("Pod", "no-such-pod", Some("default"));

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ResourceError::NotFound { kind: "Pod".to_string(), name: "no-such-pod".to_string() }
        );
    }

    #[test]
    fn test_delete_wrong_namespace_errors() {
        let (mut svc, _) = make_service_with_resources();
        let result = svc.delete("Pod", "pod-a", Some("kube-system"));
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_cluster_scoped_resource() {
        let (mut svc, _) = make_service_with_resources();
        let result = svc.delete("Node", "node-1", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "node-1");
        assert!(svc.get("Node", "node-1", None).is_none());
    }

    // --- count tests ---

    #[test]
    fn test_count_by_kind() {
        let (svc, _) = make_service_with_resources();
        assert_eq!(svc.count("Pod"), 3);
        assert_eq!(svc.count("Deployment"), 1);
        assert_eq!(svc.count("Node"), 1);
        assert_eq!(svc.count("ConfigMap"), 0);
    }

    #[test]
    fn test_count_after_create_and_delete() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        assert_eq!(svc.count("Pod"), 0);

        svc.create(Resource::new("pod-1", "default", "Pod", "v1", cluster)).unwrap();
        assert_eq!(svc.count("Pod"), 1);

        svc.create(Resource::new("pod-2", "default", "Pod", "v1", cluster)).unwrap();
        assert_eq!(svc.count("Pod"), 2);

        svc.delete("Pod", "pod-1", Some("default")).unwrap();
        assert_eq!(svc.count("Pod"), 1);
    }

    // --- kinds tests ---

    #[test]
    fn test_kinds_returns_unique_sorted() {
        let (svc, _) = make_service_with_resources();
        let kinds = svc.kinds();
        assert_eq!(kinds, vec!["Deployment", "Node", "Pod"]);
    }

    #[test]
    fn test_kinds_empty_service() {
        let svc = ResourceService::new();
        assert!(svc.kinds().is_empty());
    }

    #[test]
    fn test_kinds_single_kind() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();
        svc.create(Resource::new("pod-1", "default", "Pod", "v1", cluster)).unwrap();
        svc.create(Resource::new("pod-2", "default", "Pod", "v1", cluster)).unwrap();
        assert_eq!(svc.kinds(), vec!["Pod"]);
    }

    // --- ResourceService::new() test ---

    #[test]
    fn test_resource_service_new_is_empty() {
        let svc = ResourceService::new();
        assert!(svc.resources.is_empty());
        assert_eq!(svc.count("Pod"), 0);
        assert!(svc.kinds().is_empty());
    }

    // --- ResourceError display tests ---

    #[test]
    fn test_resource_error_display() {
        let not_found =
            ResourceError::NotFound { kind: "Pod".to_string(), name: "my-pod".to_string() };
        assert_eq!(not_found.to_string(), "resource not found: Pod/my-pod");

        let already_exists =
            ResourceError::AlreadyExists { kind: "Pod".to_string(), name: "my-pod".to_string() };
        assert_eq!(already_exists.to_string(), "resource already exists: Pod/my-pod");

        let conflict =
            ResourceError::VersionConflict { expected: "2".to_string(), actual: "1".to_string() };
        assert_eq!(conflict.to_string(), "version conflict: expected 2, actual 1");
    }

    // --- Integration-style tests ---

    #[test]
    fn test_full_crud_lifecycle() {
        let cluster = test_cluster_id();
        let mut svc = ResourceService::new();

        // Create
        let resource = Resource::new("lifecycle-pod", "default", "Pod", "v1", cluster)
            .with_label("phase", "created")
            .with_spec(serde_json::json!({"containers": [{"name": "app", "image": "nginx"}]}));
        svc.create(resource).unwrap();

        // Read
        let fetched = svc.get("Pod", "lifecycle-pod", Some("default")).unwrap();
        assert_eq!(fetched.labels.get("phase").unwrap(), "created");
        assert_eq!(fetched.spec["containers"][0]["image"], "nginx");
        let uid = fetched.uid.clone();
        let version = fetched.resource_version.clone();

        // Update
        let mut updated = Resource::new("lifecycle-pod", "default", "Pod", "v1", cluster)
            .with_label("phase", "updated")
            .with_spec(
                serde_json::json!({"containers": [{"name": "app", "image": "nginx:latest"}]}),
            );
        updated.uid = uid;
        updated.resource_version = version;
        svc.update(updated).unwrap();

        let fetched = svc.get("Pod", "lifecycle-pod", Some("default")).unwrap();
        assert_eq!(fetched.labels.get("phase").unwrap(), "updated");
        assert_eq!(fetched.spec["containers"][0]["image"], "nginx:latest");

        // Delete
        let deleted = svc.delete("Pod", "lifecycle-pod", Some("default")).unwrap();
        assert_eq!(deleted.name, "lifecycle-pod");
        assert!(svc.get("Pod", "lifecycle-pod", Some("default")).is_none());
        assert_eq!(svc.count("Pod"), 0);
    }

    // ===================================================================
    // T055: KubeApiError display tests
    // ===================================================================

    #[test]
    fn test_kube_api_error_display_api_error() {
        let err = KubeApiError::ApiError {
            status_code: 500,
            message: "internal server error".to_string(),
        };
        assert_eq!(err.to_string(), "API error: 500 internal server error");
    }

    #[test]
    fn test_kube_api_error_display_not_found() {
        let err = KubeApiError::NotFound { kind: "Pod".to_string(), name: "my-pod".to_string() };
        assert_eq!(err.to_string(), "not found: Pod/my-pod");
    }

    #[test]
    fn test_kube_api_error_display_conflict() {
        let err = KubeApiError::Conflict;
        assert_eq!(err.to_string(), "conflict: resource version mismatch");
    }

    #[test]
    fn test_kube_api_error_display_forbidden() {
        let err = KubeApiError::Forbidden { message: "access denied".to_string() };
        assert_eq!(err.to_string(), "forbidden: access denied");
    }

    #[test]
    fn test_kube_api_error_display_connection() {
        let err = KubeApiError::ConnectionError { message: "timeout".to_string() };
        assert_eq!(err.to_string(), "connection error: timeout");
    }

    // ===================================================================
    // T055: InMemoryKubeClient tests
    // ===================================================================

    fn make_kube_client() -> (InMemoryKubeClient, Uuid) {
        let cluster = Uuid::new_v4();
        (InMemoryKubeClient::new(cluster), cluster)
    }

    fn make_kube_client_with_data() -> (InMemoryKubeClient, Uuid) {
        let cluster = Uuid::new_v4();
        let store = ResourceService::new();
        let store = Arc::new(RwLock::new(store));
        let client = InMemoryKubeClient::with_store(cluster, store.clone());

        // Pre-populate with some resources
        {
            let mut s = store.write().unwrap();
            s.create(Resource::new("pod-a", "default", "Pod", "v1", cluster)).unwrap();
            s.create(Resource::new("pod-b", "default", "Pod", "v1", cluster)).unwrap();
            s.create(Resource::new("pod-c", "kube-system", "Pod", "v1", cluster)).unwrap();
            s.create(
                Resource::new("deploy-a", "default", "Deployment", "apps/v1", cluster)
                    .with_spec(serde_json::json!({"replicas": 1})),
            )
            .unwrap();
            s.create(Resource::new("node-1", "", "Node", "v1", cluster)).unwrap();
        }

        (client, cluster)
    }

    // --- KubeResourceClient::list tests ---

    #[test]
    fn test_kube_client_list_all_pods() {
        let (client, _) = make_kube_client_with_data();
        let pods = client.list("Pod", "v1", None).unwrap();
        assert_eq!(pods.len(), 3);
    }

    #[test]
    fn test_kube_client_list_pods_by_namespace() {
        let (client, _) = make_kube_client_with_data();
        let pods = client.list("Pod", "v1", Some("default")).unwrap();
        assert_eq!(pods.len(), 2);
    }

    #[test]
    fn test_kube_client_list_empty_result() {
        let (client, _) = make_kube_client_with_data();
        let result = client.list("ConfigMap", "v1", None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_kube_client_list_nonexistent_namespace() {
        let (client, _) = make_kube_client_with_data();
        let result = client.list("Pod", "v1", Some("nonexistent")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_kube_client_list_on_empty_store() {
        let (client, _) = make_kube_client();
        let result = client.list("Pod", "v1", None).unwrap();
        assert!(result.is_empty());
    }

    // --- KubeResourceClient::get tests ---

    #[test]
    fn test_kube_client_get_existing() {
        let (client, _) = make_kube_client_with_data();
        let pod = client.get("Pod", "v1", "pod-a", Some("default")).unwrap();
        assert_eq!(pod.name, "pod-a");
        assert_eq!(pod.kind, "Pod");
        assert_eq!(pod.namespace, Some("default".to_string()));
    }

    #[test]
    fn test_kube_client_get_not_found() {
        let (client, _) = make_kube_client_with_data();
        let result = client.get("Pod", "v1", "no-such", Some("default"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KubeApiError::NotFound { .. }));
    }

    #[test]
    fn test_kube_client_get_wrong_namespace() {
        let (client, _) = make_kube_client_with_data();
        let result = client.get("Pod", "v1", "pod-a", Some("kube-system"));
        assert!(result.is_err());
    }

    #[test]
    fn test_kube_client_get_cluster_scoped() {
        let (client, _) = make_kube_client_with_data();
        let node = client.get("Node", "v1", "node-1", None).unwrap();
        assert_eq!(node.name, "node-1");
        assert!(!node.is_namespaced());
    }

    // --- KubeResourceClient::create tests ---

    #[test]
    fn test_kube_client_create_resource() {
        let (client, _) = make_kube_client();
        let data = serde_json::json!({
            "metadata": {
                "name": "new-pod",
                "namespace": "default"
            },
            "spec": {
                "containers": [{"name": "app", "image": "nginx"}]
            }
        });
        let result = client.create("Pod", "v1", Some("default"), &data);
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.name, "new-pod");
        assert_eq!(created.namespace, Some("default".to_string()));
        assert_eq!(created.kind, "Pod");
        assert_eq!(created.spec["containers"][0]["image"], "nginx");
    }

    #[test]
    fn test_kube_client_create_duplicate_errors() {
        let (client, _) = make_kube_client_with_data();
        let data = serde_json::json!({
            "metadata": {"name": "pod-a", "namespace": "default"},
            "spec": {}
        });
        let result = client.create("Pod", "v1", Some("default"), &data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KubeApiError::ApiError { status_code: 409, .. }));
    }

    #[test]
    fn test_kube_client_create_uses_namespace_param_over_data() {
        let (client, _) = make_kube_client();
        let data = serde_json::json!({
            "metadata": {"name": "test-pod", "namespace": "from-data"},
            "spec": {}
        });
        // Explicit namespace parameter should take precedence
        let result = client.create("Pod", "v1", Some("from-param"), &data);
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.namespace, Some("from-param".to_string()));
    }

    #[test]
    fn test_kube_client_create_extracts_namespace_from_data() {
        let (client, _) = make_kube_client();
        let data = serde_json::json!({
            "metadata": {"name": "test-pod", "namespace": "from-data"},
            "spec": {}
        });
        let result = client.create("Pod", "v1", None, &data);
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created.namespace, Some("from-data".to_string()));
    }

    #[test]
    fn test_kube_client_create_persists_in_store() {
        let (client, _) = make_kube_client();
        let data = serde_json::json!({
            "metadata": {"name": "stored-pod"},
            "spec": {"containers": []}
        });
        client.create("Pod", "v1", Some("test-ns"), &data).unwrap();

        // Verify we can get it back
        let fetched = client.get("Pod", "v1", "stored-pod", Some("test-ns")).unwrap();
        assert_eq!(fetched.name, "stored-pod");
    }

    // --- KubeResourceClient::update tests ---

    #[test]
    fn test_kube_client_update_existing() {
        let (client, _) = make_kube_client_with_data();
        let data = serde_json::json!({
            "spec": {"containers": [{"name": "updated-app", "image": "nginx:latest"}]}
        });
        let result = client.update("Pod", "v1", "pod-a", Some("default"), &data);
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.spec["containers"][0]["image"], "nginx:latest");
    }

    #[test]
    fn test_kube_client_update_not_found() {
        let (client, _) = make_kube_client_with_data();
        let data = serde_json::json!({"spec": {}});
        let result = client.update("Pod", "v1", "ghost", Some("default"), &data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KubeApiError::NotFound { .. }));
    }

    #[test]
    fn test_kube_client_update_merges_labels() {
        let (client, _) = make_kube_client_with_data();
        let data = serde_json::json!({
            "metadata": {
                "labels": {"env": "production", "version": "2"}
            }
        });
        let result = client.update("Pod", "v1", "pod-a", Some("default"), &data);
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.labels.get("env").unwrap(), "production");
        assert_eq!(updated.labels.get("version").unwrap(), "2");
    }

    // --- KubeResourceClient::delete tests ---

    #[test]
    fn test_kube_client_delete_existing() {
        let (client, _) = make_kube_client_with_data();
        let result = client.delete("Pod", "v1", "pod-a", Some("default"));
        assert!(result.is_ok());

        // Verify it is gone
        let get_result = client.get("Pod", "v1", "pod-a", Some("default"));
        assert!(get_result.is_err());
    }

    #[test]
    fn test_kube_client_delete_not_found() {
        let (client, _) = make_kube_client_with_data();
        let result = client.delete("Pod", "v1", "ghost", Some("default"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KubeApiError::NotFound { .. }));
    }

    #[test]
    fn test_kube_client_delete_cluster_scoped() {
        let (client, _) = make_kube_client_with_data();
        let result = client.delete("Node", "v1", "node-1", None);
        assert!(result.is_ok());
        assert!(client.get("Node", "v1", "node-1", None).is_err());
    }

    // --- KubeResourceClient::scale tests ---

    #[test]
    fn test_kube_client_scale_deployment() {
        let (client, _) = make_kube_client_with_data();
        let result = client.scale("Deployment", "apps/v1", "deploy-a", "default", 5);
        assert!(result.is_ok());

        let fetched = client.get("Deployment", "apps/v1", "deploy-a", Some("default")).unwrap();
        assert_eq!(fetched.spec["replicas"], 5);
    }

    #[test]
    fn test_kube_client_scale_to_zero() {
        let (client, _) = make_kube_client_with_data();
        let result = client.scale("Deployment", "apps/v1", "deploy-a", "default", 0);
        assert!(result.is_ok());

        let fetched = client.get("Deployment", "apps/v1", "deploy-a", Some("default")).unwrap();
        assert_eq!(fetched.spec["replicas"], 0);
    }

    #[test]
    fn test_kube_client_scale_not_found() {
        let (client, _) = make_kube_client_with_data();
        let result = client.scale("Deployment", "apps/v1", "ghost-deploy", "default", 3);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KubeApiError::NotFound { .. }));
    }

    #[test]
    fn test_kube_client_scale_non_scalable_kind() {
        let (client, _) = make_kube_client_with_data();
        // Pods are not scalable
        let result = client.scale("Pod", "v1", "pod-a", "default", 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KubeApiError::ApiError { status_code: 400, .. }));
    }

    // --- KubeResourceClient::restart tests ---

    #[test]
    fn test_kube_client_restart_deployment() {
        let (client, _) = make_kube_client_with_data();
        let result = client.restart("Deployment", "apps/v1", "deploy-a", "default");
        assert!(result.is_ok());

        let fetched = client.get("Deployment", "apps/v1", "deploy-a", Some("default")).unwrap();
        assert!(fetched.annotations.contains_key("kubectl.kubernetes.io/restartedAt"));
    }

    #[test]
    fn test_kube_client_restart_not_found() {
        let (client, _) = make_kube_client_with_data();
        let result = client.restart("Deployment", "apps/v1", "ghost-deploy", "default");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KubeApiError::NotFound { .. }));
    }

    #[test]
    fn test_kube_client_restart_non_restartable_kind() {
        let (client, _) = make_kube_client_with_data();
        // Pods are not restartable via the workload restart mechanism
        let result = client.restart("Pod", "v1", "pod-a", "default");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KubeApiError::ApiError { status_code: 400, .. }));
    }

    // --- InMemoryKubeClient constructor tests ---

    #[test]
    fn test_in_memory_client_new_has_empty_store() {
        let cluster = Uuid::new_v4();
        let client = InMemoryKubeClient::new(cluster);
        assert_eq!(client.cluster_id, cluster);
        let store = client.store().read().unwrap();
        assert!(store.resources.is_empty());
    }

    #[test]
    fn test_in_memory_client_with_store_shares_state() {
        let cluster = Uuid::new_v4();
        let store = Arc::new(RwLock::new(ResourceService::new()));
        let client = InMemoryKubeClient::with_store(cluster, store.clone());

        // Add through the store directly
        {
            let mut s = store.write().unwrap();
            s.create(Resource::new("shared-pod", "default", "Pod", "v1", cluster)).unwrap();
        }

        // Verify the client can see it
        let result = client.get("Pod", "v1", "shared-pod", Some("default"));
        assert!(result.is_ok());
    }

    // --- KubeResourceClient trait object tests ---

    #[test]
    fn test_kube_client_trait_object() {
        let cluster = Uuid::new_v4();
        let client: Box<dyn KubeResourceClient> = Box::new(InMemoryKubeClient::new(cluster));

        // Should be usable as a trait object
        let data = serde_json::json!({
            "metadata": {"name": "trait-pod"},
            "spec": {}
        });
        let result = client.create("Pod", "v1", Some("default"), &data);
        assert!(result.is_ok());

        let listed = client.list("Pod", "v1", None).unwrap();
        assert_eq!(listed.len(), 1);
    }

    // --- Full CRUD lifecycle through KubeResourceClient ---

    #[test]
    fn test_kube_client_full_crud_lifecycle() {
        let (client, _) = make_kube_client();

        // Create
        let data = serde_json::json!({
            "metadata": {"name": "lifecycle-pod"},
            "spec": {"containers": [{"name": "app", "image": "nginx:1.0"}]}
        });
        let created = client.create("Pod", "v1", Some("default"), &data).unwrap();
        assert_eq!(created.name, "lifecycle-pod");

        // List
        let pods = client.list("Pod", "v1", Some("default")).unwrap();
        assert_eq!(pods.len(), 1);

        // Get
        let fetched = client.get("Pod", "v1", "lifecycle-pod", Some("default")).unwrap();
        assert_eq!(fetched.spec["containers"][0]["image"], "nginx:1.0");

        // Update
        let update_data = serde_json::json!({
            "spec": {"containers": [{"name": "app", "image": "nginx:2.0"}]}
        });
        let updated =
            client.update("Pod", "v1", "lifecycle-pod", Some("default"), &update_data).unwrap();
        assert_eq!(updated.spec["containers"][0]["image"], "nginx:2.0");

        // Delete
        client.delete("Pod", "v1", "lifecycle-pod", Some("default")).unwrap();

        // Verify deletion
        let pods = client.list("Pod", "v1", None).unwrap();
        assert!(pods.is_empty());
    }

    // ===================================================================
    // T115: Relationship extraction tests
    // ===================================================================

    fn make_resource_with_uid(
        uid: &str,
        name: &str,
        namespace: &str,
        kind: &str,
        api_version: &str,
        cluster_id: Uuid,
    ) -> Resource {
        let mut r = Resource::new(name, namespace, kind, api_version, cluster_id);
        r.uid = uid.to_string();
        r
    }

    // --- ResourceRef tests ---

    #[test]
    fn test_resource_ref_key_namespaced() {
        let r = ResourceRef::new("Pod", "nginx", Some("default".to_string()));
        assert_eq!(r.key(), "Pod/default/nginx");
    }

    #[test]
    fn test_resource_ref_key_cluster_scoped() {
        let r = ResourceRef::new("Node", "node-1", None);
        assert_eq!(r.key(), "Node/node-1");
    }

    #[test]
    fn test_resource_ref_equality() {
        let a = ResourceRef::new("Pod", "nginx", Some("default".to_string()));
        let b = ResourceRef::new("Pod", "nginx", Some("default".to_string()));
        assert_eq!(a, b);
    }

    #[test]
    fn test_resource_ref_inequality_different_namespace() {
        let a = ResourceRef::new("Pod", "nginx", Some("default".to_string()));
        let b = ResourceRef::new("Pod", "nginx", Some("kube-system".to_string()));
        assert_ne!(a, b);
    }

    // --- RelationshipKind tests ---

    #[test]
    fn test_relationship_kind_equality() {
        assert_eq!(RelationshipKind::OwnerReference, RelationshipKind::OwnerReference);
        assert_ne!(RelationshipKind::OwnerReference, RelationshipKind::ServiceSelector);
    }

    // --- Owner reference relationship tests ---

    #[test]
    fn test_build_relationships_from_owner_references() {
        let cluster = Uuid::new_v4();

        let deployment = make_resource_with_uid(
            "deploy-uid-1",
            "my-deploy",
            "default",
            "Deployment",
            "apps/v1",
            cluster,
        );

        let mut rs = make_resource_with_uid(
            "rs-uid-1",
            "my-deploy-abc123",
            "default",
            "ReplicaSet",
            "apps/v1",
            cluster,
        );
        rs.owner_references.push(OwnerReference {
            uid: "deploy-uid-1".to_string(),
            kind: "Deployment".to_string(),
            name: "my-deploy".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });

        let mut pod = make_resource_with_uid(
            "pod-uid-1",
            "my-deploy-abc123-xyz",
            "default",
            "Pod",
            "v1",
            cluster,
        );
        pod.owner_references.push(OwnerReference {
            uid: "rs-uid-1".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "my-deploy-abc123".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });

        let resources = vec![deployment, rs, pod];
        let rels = build_relationship_graph(&resources);

        // Should have 2 relationships: Deployment -> ReplicaSet, ReplicaSet -> Pod
        let owner_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::OwnerReference).collect();
        assert_eq!(owner_rels.len(), 2);

        // Deployment -> ReplicaSet
        assert!(owner_rels.iter().any(|r| {
            r.source.kind == "Deployment"
                && r.source.name == "my-deploy"
                && r.target.kind == "ReplicaSet"
                && r.target.name == "my-deploy-abc123"
        }));

        // ReplicaSet -> Pod
        assert!(owner_rels.iter().any(|r| {
            r.source.kind == "ReplicaSet"
                && r.source.name == "my-deploy-abc123"
                && r.target.kind == "Pod"
                && r.target.name == "my-deploy-abc123-xyz"
        }));
    }

    #[test]
    fn test_owner_reference_with_missing_owner_in_set() {
        let cluster = Uuid::new_v4();

        // Pod with owner reference to a ReplicaSet not in the resource set
        let mut pod =
            make_resource_with_uid("pod-uid-1", "orphan-pod", "default", "Pod", "v1", cluster);
        pod.owner_references.push(OwnerReference {
            uid: "missing-rs-uid".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "missing-rs".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });

        let resources = vec![pod];
        let rels = build_relationship_graph(&resources);

        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source.kind, "ReplicaSet");
        assert_eq!(rels[0].source.name, "missing-rs");
        // Falls back to the owned resource's namespace
        assert_eq!(rels[0].source.namespace, Some("default".to_string()));
        assert_eq!(rels[0].target.kind, "Pod");
        assert_eq!(rels[0].target.name, "orphan-pod");
    }

    #[test]
    fn test_no_owner_references_yields_no_owner_relationships() {
        let cluster = Uuid::new_v4();
        let pod =
            make_resource_with_uid("pod-uid-1", "standalone-pod", "default", "Pod", "v1", cluster);
        let resources = vec![pod];
        let rels = build_relationship_graph(&resources);
        let owner_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::OwnerReference).collect();
        assert!(owner_rels.is_empty());
    }

    // --- Service selector relationship tests ---

    #[test]
    fn test_build_relationships_from_service_selector() {
        let cluster = Uuid::new_v4();

        let svc =
            make_resource_with_uid("svc-uid-1", "my-service", "default", "Service", "v1", cluster)
                .with_spec(serde_json::json!({
                    "selector": {"app": "web"}
                }));

        let pod_match =
            make_resource_with_uid("pod-uid-1", "web-pod", "default", "Pod", "v1", cluster)
                .with_label("app", "web");

        let pod_no_match =
            make_resource_with_uid("pod-uid-2", "api-pod", "default", "Pod", "v1", cluster)
                .with_label("app", "api");

        let resources = vec![svc, pod_match, pod_no_match];
        let rels = build_relationship_graph(&resources);

        let svc_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::ServiceSelector).collect();
        assert_eq!(svc_rels.len(), 1);
        assert_eq!(svc_rels[0].source.kind, "Service");
        assert_eq!(svc_rels[0].source.name, "my-service");
        assert_eq!(svc_rels[0].target.kind, "Pod");
        assert_eq!(svc_rels[0].target.name, "web-pod");
    }

    #[test]
    fn test_service_selector_multi_label_match() {
        let cluster = Uuid::new_v4();

        let svc =
            make_resource_with_uid("svc-uid-1", "my-service", "default", "Service", "v1", cluster)
                .with_spec(serde_json::json!({
                    "selector": {"app": "web", "tier": "frontend"}
                }));

        let pod_full_match =
            make_resource_with_uid("pod-uid-1", "frontend-pod", "default", "Pod", "v1", cluster)
                .with_label("app", "web")
                .with_label("tier", "frontend");

        // Partial match should not count
        let pod_partial =
            make_resource_with_uid("pod-uid-2", "partial-pod", "default", "Pod", "v1", cluster)
                .with_label("app", "web");

        let resources = vec![svc, pod_full_match, pod_partial];
        let rels = build_relationship_graph(&resources);

        let svc_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::ServiceSelector).collect();
        assert_eq!(svc_rels.len(), 1);
        assert_eq!(svc_rels[0].target.name, "frontend-pod");
    }

    #[test]
    fn test_service_selector_cross_namespace_no_match() {
        let cluster = Uuid::new_v4();

        let svc =
            make_resource_with_uid("svc-uid-1", "my-service", "default", "Service", "v1", cluster)
                .with_spec(serde_json::json!({
                    "selector": {"app": "web"}
                }));

        // Pod in a different namespace should not match
        let pod_other_ns =
            make_resource_with_uid("pod-uid-1", "web-pod", "kube-system", "Pod", "v1", cluster)
                .with_label("app", "web");

        let resources = vec![svc, pod_other_ns];
        let rels = build_relationship_graph(&resources);

        let svc_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::ServiceSelector).collect();
        assert!(svc_rels.is_empty());
    }

    #[test]
    fn test_service_with_empty_selector() {
        let cluster = Uuid::new_v4();

        let svc = make_resource_with_uid(
            "svc-uid-1",
            "no-selector-svc",
            "default",
            "Service",
            "v1",
            cluster,
        )
        .with_spec(serde_json::json!({
            "selector": {}
        }));

        let pod = make_resource_with_uid("pod-uid-1", "any-pod", "default", "Pod", "v1", cluster)
            .with_label("app", "web");

        let resources = vec![svc, pod];
        let rels = build_relationship_graph(&resources);

        let svc_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::ServiceSelector).collect();
        assert!(svc_rels.is_empty());
    }

    // --- Ingress backend relationship tests ---

    #[test]
    fn test_build_relationships_from_ingress_backend() {
        let cluster = Uuid::new_v4();

        let ingress = make_resource_with_uid(
            "ing-uid-1",
            "my-ingress",
            "default",
            "Ingress",
            "networking.k8s.io/v1",
            cluster,
        )
        .with_spec(serde_json::json!({
            "rules": [{
                "host": "example.com",
                "http": {
                    "paths": [{
                        "path": "/",
                        "backend": {
                            "service": {
                                "name": "my-service",
                                "port": { "number": 80 }
                            }
                        }
                    }]
                }
            }]
        }));

        let svc =
            make_resource_with_uid("svc-uid-1", "my-service", "default", "Service", "v1", cluster);

        let resources = vec![ingress, svc];
        let rels = build_relationship_graph(&resources);

        let ingress_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::IngressBackend).collect();
        assert_eq!(ingress_rels.len(), 1);
        assert_eq!(ingress_rels[0].source.kind, "Ingress");
        assert_eq!(ingress_rels[0].source.name, "my-ingress");
        assert_eq!(ingress_rels[0].target.kind, "Service");
        assert_eq!(ingress_rels[0].target.name, "my-service");
    }

    #[test]
    fn test_ingress_with_multiple_backends() {
        let cluster = Uuid::new_v4();

        let ingress = make_resource_with_uid(
            "ing-uid-1",
            "multi-ingress",
            "default",
            "Ingress",
            "networking.k8s.io/v1",
            cluster,
        )
        .with_spec(serde_json::json!({
            "rules": [{
                "host": "example.com",
                "http": {
                    "paths": [
                        {
                            "path": "/api",
                            "backend": {
                                "service": { "name": "api-service", "port": { "number": 80 } }
                            }
                        },
                        {
                            "path": "/web",
                            "backend": {
                                "service": { "name": "web-service", "port": { "number": 80 } }
                            }
                        }
                    ]
                }
            }]
        }));

        let resources = vec![ingress];
        let rels = build_relationship_graph(&resources);

        let ingress_rels: Vec<_> =
            rels.iter().filter(|r| r.kind == RelationshipKind::IngressBackend).collect();
        assert_eq!(ingress_rels.len(), 2);

        let target_names: Vec<&str> = ingress_rels.iter().map(|r| r.target.name.as_str()).collect();
        assert!(target_names.contains(&"api-service"));
        assert!(target_names.contains(&"web-service"));
    }

    // --- DAG construction tests ---

    #[test]
    fn test_build_dag_from_relationships() {
        let cluster = Uuid::new_v4();

        let deployment = make_resource_with_uid(
            "deploy-uid",
            "my-deploy",
            "default",
            "Deployment",
            "apps/v1",
            cluster,
        );
        let mut rs =
            make_resource_with_uid("rs-uid", "my-rs", "default", "ReplicaSet", "apps/v1", cluster);
        rs.owner_references.push(OwnerReference {
            uid: "deploy-uid".to_string(),
            kind: "Deployment".to_string(),
            name: "my-deploy".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });
        let mut pod = make_resource_with_uid("pod-uid", "my-pod", "default", "Pod", "v1", cluster);
        pod.owner_references.push(OwnerReference {
            uid: "rs-uid".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "my-rs".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });

        let resources = vec![deployment, rs, pod];
        let rels = build_relationship_graph(&resources);
        let (graph, node_map) = build_dag(&rels);

        // 3 unique nodes
        assert_eq!(graph.node_count(), 3);
        // 2 edges: Deployment->RS, RS->Pod
        assert_eq!(graph.edge_count(), 2);

        assert!(node_map.contains_key("Deployment/default/my-deploy"));
        assert!(node_map.contains_key("ReplicaSet/default/my-rs"));
        assert!(node_map.contains_key("Pod/default/my-pod"));
    }

    #[test]
    fn test_build_dag_deduplicates_nodes() {
        // Two relationships pointing to the same service
        let rels = vec![
            ResourceRelationship::new(
                ResourceRef::new("Ingress", "ing-1", Some("default".to_string())),
                ResourceRef::new("Service", "svc-1", Some("default".to_string())),
                RelationshipKind::IngressBackend,
            ),
            ResourceRelationship::new(
                ResourceRef::new("Service", "svc-1", Some("default".to_string())),
                ResourceRef::new("Pod", "pod-1", Some("default".to_string())),
                RelationshipKind::ServiceSelector,
            ),
        ];

        let (graph, _node_map) = build_dag(&rels);

        // 3 unique nodes: Ingress, Service, Pod
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn test_build_dag_empty_relationships() {
        let (graph, node_map) = build_dag(&[]);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert!(node_map.is_empty());
    }

    // --- Full pipeline: Ingress -> Service -> Pods ---

    #[test]
    fn test_full_relationship_pipeline() {
        let cluster = Uuid::new_v4();

        let ingress = make_resource_with_uid(
            "ing-uid",
            "my-ingress",
            "default",
            "Ingress",
            "networking.k8s.io/v1",
            cluster,
        )
        .with_spec(serde_json::json!({
            "rules": [{
                "host": "example.com",
                "http": {
                    "paths": [{
                        "path": "/",
                        "backend": { "service": { "name": "my-service", "port": { "number": 80 } } }
                    }]
                }
            }]
        }));

        let svc =
            make_resource_with_uid("svc-uid", "my-service", "default", "Service", "v1", cluster)
                .with_spec(serde_json::json!({ "selector": {"app": "web"} }));

        let pod1 =
            make_resource_with_uid("pod-uid-1", "web-pod-1", "default", "Pod", "v1", cluster)
                .with_label("app", "web");

        let pod2 =
            make_resource_with_uid("pod-uid-2", "web-pod-2", "default", "Pod", "v1", cluster)
                .with_label("app", "web");

        let resources = vec![ingress, svc, pod1, pod2];
        let rels = build_relationship_graph(&resources);

        // 1 Ingress->Service + 2 Service->Pod
        assert_eq!(rels.len(), 3);

        let (graph, _node_map) = build_dag(&rels);
        // Ingress, Service, Pod1, Pod2 = 4 nodes
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn test_empty_resources_yields_empty_relationships() {
        let rels = build_relationship_graph(&[]);
        assert!(rels.is_empty());
    }

    // ===================================================================
    // T133: Global Search Service tests
    // ===================================================================

    // --- SearchQuery tests ---

    #[test]
    fn test_search_query_new() {
        let q = SearchQuery::new("nginx");
        assert_eq!(q.query, "nginx");
        assert!(q.clusters.is_none());
        assert!(q.namespaces.is_none());
        assert!(q.kinds.is_none());
    }

    #[test]
    fn test_search_query_with_clusters() {
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let q = SearchQuery::new("test").with_clusters(vec![c1, c2]);
        assert_eq!(q.clusters.as_ref().unwrap().len(), 2);
        assert!(q.clusters.as_ref().unwrap().contains(&c1));
    }

    #[test]
    fn test_search_query_with_namespaces() {
        let q = SearchQuery::new("test")
            .with_namespaces(vec!["default".to_string(), "kube-system".to_string()]);
        assert_eq!(q.namespaces.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_search_query_with_kinds() {
        let q =
            SearchQuery::new("test").with_kinds(vec!["Pod".to_string(), "Deployment".to_string()]);
        assert_eq!(q.kinds.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_search_query_builder_chaining() {
        let cluster = Uuid::new_v4();
        let q = SearchQuery::new("nginx")
            .with_clusters(vec![cluster])
            .with_namespaces(vec!["default".to_string()])
            .with_kinds(vec!["Pod".to_string()]);
        assert_eq!(q.query, "nginx");
        assert_eq!(q.clusters.unwrap().len(), 1);
        assert_eq!(q.namespaces.unwrap().len(), 1);
        assert_eq!(q.kinds.unwrap().len(), 1);
    }

    #[test]
    fn test_search_query_serialization() {
        let q = SearchQuery::new("test");
        let json = serde_json::to_string(&q).unwrap();
        let deserialized: SearchQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.query, "test");
        assert!(deserialized.clusters.is_none());
    }

    // --- SearchResult tests ---

    #[test]
    fn test_search_result_serialization() {
        let cluster = Uuid::new_v4();
        let result = SearchResult {
            kind: "Pod".to_string(),
            name: "nginx".to_string(),
            namespace: Some("default".to_string()),
            cluster_id: cluster,
            score: 100,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.kind, "Pod");
        assert_eq!(deserialized.name, "nginx");
        assert_eq!(deserialized.namespace, Some("default".to_string()));
        assert_eq!(deserialized.cluster_id, cluster);
        assert_eq!(deserialized.score, 100);
    }

    // --- GlobalSearchState tests ---

    #[test]
    fn test_global_search_state_default() {
        let state = GlobalSearchState::new();
        assert!(state.results.is_empty());
        assert!(!state.loading);
        assert!(state.error.is_none());
        assert_eq!(state.debounce_ms, 300);
    }

    #[test]
    fn test_global_search_state_custom_debounce() {
        let state = GlobalSearchState::with_debounce(500);
        assert_eq!(state.debounce_ms, 500);
        assert!(state.results.is_empty());
    }

    #[test]
    fn test_global_search_state_set_query() {
        let mut state = GlobalSearchState::new();
        state.error = Some("old error".to_string());
        state.set_query();
        assert!(state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_global_search_state_clear() {
        let cluster = Uuid::new_v4();
        let mut state = GlobalSearchState::new();
        state.results.push(SearchResult {
            kind: "Pod".to_string(),
            name: "test".to_string(),
            namespace: None,
            cluster_id: cluster,
            score: 50,
        });
        state.loading = true;
        state.error = Some("error".to_string());

        state.clear();
        assert!(state.results.is_empty());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_global_search_state_set_results() {
        let cluster = Uuid::new_v4();
        let mut state = GlobalSearchState::new();
        state.loading = true;

        let results = vec![
            SearchResult {
                kind: "Pod".to_string(),
                name: "nginx".to_string(),
                namespace: Some("default".to_string()),
                cluster_id: cluster,
                score: 100,
            },
            SearchResult {
                kind: "Deployment".to_string(),
                name: "nginx-deploy".to_string(),
                namespace: Some("default".to_string()),
                cluster_id: cluster,
                score: 50,
            },
        ];

        state.set_results(results);
        assert_eq!(state.results.len(), 2);
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_global_search_state_filtered_by_kind() {
        let cluster = Uuid::new_v4();
        let mut state = GlobalSearchState::new();
        state.set_results(vec![
            SearchResult {
                kind: "Pod".to_string(),
                name: "pod-a".to_string(),
                namespace: Some("default".to_string()),
                cluster_id: cluster,
                score: 100,
            },
            SearchResult {
                kind: "Deployment".to_string(),
                name: "deploy-a".to_string(),
                namespace: Some("default".to_string()),
                cluster_id: cluster,
                score: 80,
            },
            SearchResult {
                kind: "Pod".to_string(),
                name: "pod-b".to_string(),
                namespace: Some("default".to_string()),
                cluster_id: cluster,
                score: 60,
            },
        ]);

        let pods = state.filtered_by_kind("Pod");
        assert_eq!(pods.len(), 2);
        assert!(pods.iter().all(|r| r.kind == "Pod"));

        let deploys = state.filtered_by_kind("Deployment");
        assert_eq!(deploys.len(), 1);

        let svcs = state.filtered_by_kind("Service");
        assert!(svcs.is_empty());
    }

    #[test]
    fn test_global_search_state_top_results() {
        let cluster = Uuid::new_v4();
        let mut state = GlobalSearchState::new();
        state.set_results(vec![
            SearchResult {
                kind: "Pod".to_string(),
                name: "first".to_string(),
                namespace: None,
                cluster_id: cluster,
                score: 100,
            },
            SearchResult {
                kind: "Pod".to_string(),
                name: "second".to_string(),
                namespace: None,
                cluster_id: cluster,
                score: 80,
            },
            SearchResult {
                kind: "Pod".to_string(),
                name: "third".to_string(),
                namespace: None,
                cluster_id: cluster,
                score: 60,
            },
        ]);

        let top2 = state.top_results(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].name, "first");
        assert_eq!(top2[1].name, "second");
    }

    #[test]
    fn test_global_search_state_top_results_more_than_available() {
        let cluster = Uuid::new_v4();
        let mut state = GlobalSearchState::new();
        state.set_results(vec![SearchResult {
            kind: "Pod".to_string(),
            name: "only-one".to_string(),
            namespace: None,
            cluster_id: cluster,
            score: 100,
        }]);

        let top5 = state.top_results(5);
        assert_eq!(top5.len(), 1);
    }

    #[test]
    fn test_global_search_state_top_results_empty() {
        let state = GlobalSearchState::new();
        let top = state.top_results(10);
        assert!(top.is_empty());
    }

    // --- global_search function tests ---

    fn make_search_resources() -> (Vec<Resource>, Uuid, Uuid) {
        let cluster_a = Uuid::new_v4();
        let cluster_b = Uuid::new_v4();
        let resources = vec![
            Resource::new("nginx-pod", "default", "Pod", "v1", cluster_a),
            Resource::new("nginx-deploy", "default", "Deployment", "apps/v1", cluster_a),
            Resource::new("redis-cache", "cache-ns", "Pod", "v1", cluster_a),
            Resource::new("api-gateway", "production", "Service", "v1", cluster_b),
            Resource::new("nginx-pod", "staging", "Pod", "v1", cluster_b),
            Resource::new("node-1", "", "Node", "v1", cluster_a),
        ];
        (resources, cluster_a, cluster_b)
    }

    #[test]
    fn test_global_search_empty_query() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("");
        let results = global_search(&q, &resources);
        assert!(results.is_empty());
    }

    #[test]
    fn test_global_search_by_name() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx");
        let results = global_search(&q, &resources);
        assert!(!results.is_empty());
        // All results should contain "nginx" in their name
        assert!(results.iter().all(|r| r.name.contains("nginx")));
    }

    #[test]
    fn test_global_search_exact_match_scores_highest() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx-pod");
        let results = global_search(&q, &resources);
        assert!(!results.is_empty());
        assert_eq!(results[0].score, 200);
        assert_eq!(results[0].name, "nginx-pod");
    }

    #[test]
    fn test_global_search_prefix_match() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx");
        let results = global_search(&q, &resources);
        // "nginx-pod" and "nginx-deploy" should both match via prefix
        assert!(results.len() >= 2);
        assert!(results.iter().all(|r| r.score >= 100));
    }

    #[test]
    fn test_global_search_filter_by_cluster() {
        let (resources, cluster_a, _) = make_search_resources();
        let q = SearchQuery::new("nginx").with_clusters(vec![cluster_a]);
        let results = global_search(&q, &resources);
        // Only results from cluster_a
        assert!(results.iter().all(|r| r.cluster_id == cluster_a));
    }

    #[test]
    fn test_global_search_filter_by_namespace() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx").with_namespaces(vec!["default".to_string()]);
        let results = global_search(&q, &resources);
        assert!(results.iter().all(|r| r.namespace.as_deref() == Some("default")));
    }

    #[test]
    fn test_global_search_filter_by_kind() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx").with_kinds(vec!["Pod".to_string()]);
        let results = global_search(&q, &resources);
        assert!(results.iter().all(|r| r.kind == "Pod"));
    }

    #[test]
    fn test_global_search_combined_filters() {
        let (resources, cluster_a, _) = make_search_resources();
        let q = SearchQuery::new("nginx")
            .with_clusters(vec![cluster_a])
            .with_namespaces(vec!["default".to_string()])
            .with_kinds(vec!["Pod".to_string()]);
        let results = global_search(&q, &resources);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "nginx-pod");
        assert_eq!(results[0].kind, "Pod");
        assert_eq!(results[0].namespace, Some("default".to_string()));
    }

    #[test]
    fn test_global_search_no_match() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("zzzzzzz");
        let results = global_search(&q, &resources);
        assert!(results.is_empty());
    }

    #[test]
    fn test_global_search_results_sorted_by_score() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("nginx");
        let results = global_search(&q, &resources);
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "Results should be sorted by score descending"
            );
        }
    }

    #[test]
    fn test_global_search_cluster_scoped_excluded_by_namespace_filter() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("node").with_namespaces(vec!["default".to_string()]);
        let results = global_search(&q, &resources);
        // "node-1" is cluster-scoped (no namespace), so it should be excluded
        assert!(results.is_empty());
    }

    #[test]
    fn test_global_search_cluster_scoped_without_namespace_filter() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("node");
        let results = global_search(&q, &resources);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "node-1");
        assert!(results[0].namespace.is_none());
    }

    #[test]
    fn test_global_search_empty_resources() {
        let q = SearchQuery::new("test");
        let results = global_search(&q, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_global_search_substring_match() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("cache");
        let results = global_search(&q, &resources);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "redis-cache");
    }

    #[test]
    fn test_global_search_case_insensitive() {
        let (resources, _, _) = make_search_resources();
        let q = SearchQuery::new("NGINX");
        let results = global_search(&q, &resources);
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.name.to_lowercase().contains("nginx")));
    }
}
