use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrdScope {
    Namespaced,
    Cluster,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdSchema {
    pub name: String,
    pub group: String,
    pub kind: String,
    pub versions: Vec<String>,
    pub scope: CrdScope,
    pub description: Option<String>,
    pub schema_properties: Option<Value>,
    pub schema: Option<Value>,
    pub cluster_id: Uuid,
}

impl CrdSchema {
    pub fn preferred_version(&self) -> Option<&str> {
        self.versions.first().map(|s| s.as_str())
    }

    pub fn api_resource_name(&self) -> String {
        self.name.split('.').next().unwrap_or(&self.name).to_string()
    }

    pub fn is_namespaced(&self) -> bool {
        self.scope == CrdScope::Namespaced
    }

    pub fn full_api_version(&self) -> Option<String> {
        self.preferred_version().map(|v| {
            if self.group.is_empty() { v.to_string() } else { format!("{}/{v}", self.group) }
        })
    }

    /// Returns a display label combining kind and group.
    pub fn display_label(&self) -> String {
        if self.group.is_empty() {
            self.kind.clone()
        } else {
            format!("{} ({})", self.kind, self.group)
        }
    }

    /// Returns all version strings for this CRD.
    pub fn version_list(&self) -> &[String] {
        &self.versions
    }

    /// Returns true if the CRD supports a specific version.
    pub fn supports_version(&self, version: &str) -> bool {
        self.versions.iter().any(|v| v == version)
    }
}

/// Represents an instance of a custom resource (dynamic object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicResourceInstance {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub api_version: String,
    pub data: Value,
    pub cluster_id: Uuid,
}

impl DynamicResourceInstance {
    pub fn is_namespaced(&self) -> bool {
        self.namespace.is_some()
    }
}

#[derive(Debug, Default)]
pub struct CrdRegistry {
    schemas: Vec<CrdSchema>,
}

impl CrdRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, schema: CrdSchema) {
        if let Some(existing) = self
            .schemas
            .iter_mut()
            .find(|s| s.name == schema.name && s.cluster_id == schema.cluster_id)
        {
            *existing = schema;
        } else {
            self.schemas.push(schema);
        }
    }

    pub fn unregister(&mut self, name: &str, cluster_id: Uuid) {
        self.schemas.retain(|s| !(s.name == name && s.cluster_id == cluster_id));
    }

    /// List all registered CRD schemas.
    pub fn list_crds(&self) -> &[CrdSchema] {
        &self.schemas
    }

    /// Find CRDs matching a given kind. Returns the first match or None.
    pub fn find_by_kind(&self, kind: &str) -> Option<&CrdSchema> {
        self.schemas.iter().find(|s| s.kind == kind)
    }

    /// Find all CRDs matching a given kind.
    pub fn find_all_by_kind(&self, kind: &str) -> Vec<&CrdSchema> {
        self.schemas.iter().filter(|s| s.kind == kind).collect()
    }

    pub fn find_by_group(&self, group: &str) -> Vec<&CrdSchema> {
        self.schemas.iter().filter(|s| s.group == group).collect()
    }

    /// Group all CRDs by their API group.
    pub fn group_by_api_group(&self) -> HashMap<String, Vec<&CrdSchema>> {
        let mut groups: HashMap<String, Vec<&CrdSchema>> = HashMap::new();
        for schema in &self.schemas {
            groups.entry(schema.group.clone()).or_default().push(schema);
        }
        groups
    }

    pub fn for_cluster(&self, cluster_id: Uuid) -> Vec<&CrdSchema> {
        self.schemas.iter().filter(|s| s.cluster_id == cluster_id).collect()
    }

    pub fn groups(&self) -> Vec<&str> {
        let mut groups: Vec<&str> = self.schemas.iter().map(|s| s.group.as_str()).collect();
        groups.sort();
        groups.dedup();
        groups
    }

    pub fn count(&self) -> usize {
        self.schemas.len()
    }

    /// Clear all schemas for a given cluster.
    pub fn clear_cluster(&mut self, cluster_id: Uuid) {
        self.schemas.retain(|s| s.cluster_id != cluster_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_crd(name: &str, group: &str, kind: &str) -> CrdSchema {
        CrdSchema {
            name: format!("{}.{group}", name.to_lowercase()),
            group: group.to_string(),
            kind: kind.to_string(),
            versions: vec!["v1".to_string()],
            scope: CrdScope::Namespaced,
            description: None,
            schema_properties: None,
            schema: None,
            cluster_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        }
    }

    fn sample_crd_with_details(
        name: &str,
        group: &str,
        kind: &str,
        versions: Vec<&str>,
        scope: CrdScope,
        description: Option<&str>,
    ) -> CrdSchema {
        CrdSchema {
            name: format!("{}.{group}", name.to_lowercase()),
            group: group.to_string(),
            kind: kind.to_string(),
            versions: versions.into_iter().map(|s| s.to_string()).collect(),
            scope,
            description: description.map(|s| s.to_string()),
            schema_properties: None,
            schema: None,
            cluster_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        }
    }

    // --- T110: CrdSchema type creation and field access ---

    #[test]
    fn test_crd_schema_creation_with_all_fields() {
        let crd = CrdSchema {
            name: "certificates.cert-manager.io".to_string(),
            group: "cert-manager.io".to_string(),
            kind: "Certificate".to_string(),
            versions: vec!["v1".to_string(), "v1alpha1".to_string()],
            scope: CrdScope::Namespaced,
            description: Some("Certificate resource for TLS".to_string()),
            schema_properties: Some(json!({
                "spec": {
                    "type": "object",
                    "properties": {
                        "secretName": { "type": "string" }
                    }
                }
            })),
            schema: None,
            cluster_id: Uuid::new_v4(),
        };

        assert_eq!(crd.name, "certificates.cert-manager.io");
        assert_eq!(crd.group, "cert-manager.io");
        assert_eq!(crd.kind, "Certificate");
        assert_eq!(crd.versions.len(), 2);
        assert_eq!(crd.scope, CrdScope::Namespaced);
        assert_eq!(crd.description.as_deref(), Some("Certificate resource for TLS"));
        assert!(crd.schema_properties.is_some());
    }

    #[test]
    fn test_crd_schema_description_none() {
        let crd = sample_crd("certs", "test.io", "Cert");
        assert!(crd.description.is_none());
    }

    #[test]
    fn test_crd_schema_schema_properties_none() {
        let crd = sample_crd("certs", "test.io", "Cert");
        assert!(crd.schema_properties.is_none());
    }

    #[test]
    fn test_crd_schema_display_label() {
        let crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        assert_eq!(crd.display_label(), "Certificate (cert-manager.io)");
    }

    #[test]
    fn test_crd_schema_display_label_empty_group() {
        let mut crd = sample_crd("things", "test.io", "Thing");
        crd.group = String::new();
        assert_eq!(crd.display_label(), "Thing");
    }

    #[test]
    fn test_crd_schema_preferred_version() {
        let crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        assert_eq!(crd.preferred_version(), Some("v1"));
    }

    #[test]
    fn test_crd_schema_preferred_version_empty() {
        let mut crd = sample_crd("things", "test.io", "Thing");
        crd.versions.clear();
        assert_eq!(crd.preferred_version(), None);
    }

    #[test]
    fn test_crd_schema_api_resource_name() {
        let crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        assert_eq!(crd.api_resource_name(), "certificates");
    }

    #[test]
    fn test_crd_schema_full_api_version() {
        let crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        assert_eq!(crd.full_api_version(), Some("cert-manager.io/v1".to_string()));
    }

    #[test]
    fn test_crd_schema_full_api_version_empty_group() {
        let mut crd = sample_crd("things", "test.io", "Thing");
        crd.group = String::new();
        assert_eq!(crd.full_api_version(), Some("v1".to_string()));
    }

    #[test]
    fn test_crd_schema_is_namespaced() {
        let mut crd = sample_crd("certs", "test.io", "Cert");
        assert!(crd.is_namespaced());

        crd.scope = CrdScope::Cluster;
        assert!(!crd.is_namespaced());
    }

    #[test]
    fn test_crd_schema_version_list() {
        let crd = sample_crd_with_details(
            "certificates",
            "cert-manager.io",
            "Certificate",
            vec!["v1", "v1beta1", "v1alpha1"],
            CrdScope::Namespaced,
            None,
        );
        assert_eq!(crd.version_list(), &["v1", "v1beta1", "v1alpha1"]);
    }

    #[test]
    fn test_crd_schema_supports_version() {
        let crd = sample_crd_with_details(
            "certificates",
            "cert-manager.io",
            "Certificate",
            vec!["v1", "v1beta1"],
            CrdScope::Namespaced,
            None,
        );
        assert!(crd.supports_version("v1"));
        assert!(crd.supports_version("v1beta1"));
        assert!(!crd.supports_version("v2"));
    }

    #[test]
    fn test_crd_schema_serialization() {
        let crd = CrdSchema {
            name: "certs.test.io".to_string(),
            group: "test.io".to_string(),
            kind: "Cert".to_string(),
            versions: vec!["v1".to_string()],
            scope: CrdScope::Namespaced,
            description: Some("Test CRD".to_string()),
            schema_properties: Some(json!({"spec": {"type": "object"}})),
            schema: None,
            cluster_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&crd).unwrap();
        let deserialized: CrdSchema = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, crd.name);
        assert_eq!(deserialized.group, crd.group);
        assert_eq!(deserialized.kind, crd.kind);
        assert_eq!(deserialized.versions, crd.versions);
        assert_eq!(deserialized.scope, crd.scope);
        assert_eq!(deserialized.description, crd.description);
        assert_eq!(deserialized.schema_properties, crd.schema_properties);
    }

    #[test]
    fn test_crd_scope_serialization() {
        assert_eq!(serde_json::to_string(&CrdScope::Namespaced).unwrap(), "\"Namespaced\"");
        assert_eq!(serde_json::to_string(&CrdScope::Cluster).unwrap(), "\"Cluster\"");
    }

    // --- T110: DynamicResourceInstance (DynamicObject listing simulation) ---

    #[test]
    fn test_dynamic_resource_instance_creation() {
        let instance = DynamicResourceInstance {
            name: "my-certificate".to_string(),
            namespace: Some("default".to_string()),
            kind: "Certificate".to_string(),
            api_version: "cert-manager.io/v1".to_string(),
            data: json!({
                "spec": {
                    "secretName": "my-tls-secret",
                    "issuerRef": {
                        "name": "letsencrypt-prod",
                        "kind": "ClusterIssuer"
                    }
                }
            }),
            cluster_id: Uuid::new_v4(),
        };

        assert_eq!(instance.name, "my-certificate");
        assert_eq!(instance.namespace.as_deref(), Some("default"));
        assert_eq!(instance.kind, "Certificate");
        assert!(instance.is_namespaced());
    }

    #[test]
    fn test_dynamic_resource_instance_cluster_scoped() {
        let instance = DynamicResourceInstance {
            name: "my-cluster-issuer".to_string(),
            namespace: None,
            kind: "ClusterIssuer".to_string(),
            api_version: "cert-manager.io/v1".to_string(),
            data: json!({}),
            cluster_id: Uuid::new_v4(),
        };

        assert!(!instance.is_namespaced());
    }

    #[test]
    fn test_dynamic_resource_instance_serialization() {
        let instance = DynamicResourceInstance {
            name: "test-cr".to_string(),
            namespace: Some("test-ns".to_string()),
            kind: "TestKind".to_string(),
            api_version: "test.io/v1".to_string(),
            data: json!({"spec": {"replicas": 3}}),
            cluster_id: Uuid::new_v4(),
        };

        let json_str = serde_json::to_string(&instance).unwrap();
        let deserialized: DynamicResourceInstance = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.name, instance.name);
        assert_eq!(deserialized.namespace, instance.namespace);
        assert_eq!(deserialized.kind, instance.kind);
    }

    #[test]
    fn test_dynamic_resource_listing_simulation() {
        // Simulate listing DynamicObject instances for a CRD
        let instances: Vec<DynamicResourceInstance> = vec![
            DynamicResourceInstance {
                name: "cert-1".to_string(),
                namespace: Some("default".to_string()),
                kind: "Certificate".to_string(),
                api_version: "cert-manager.io/v1".to_string(),
                data: json!({"spec": {"secretName": "cert-1-tls"}}),
                cluster_id: Uuid::new_v4(),
            },
            DynamicResourceInstance {
                name: "cert-2".to_string(),
                namespace: Some("default".to_string()),
                kind: "Certificate".to_string(),
                api_version: "cert-manager.io/v1".to_string(),
                data: json!({"spec": {"secretName": "cert-2-tls"}}),
                cluster_id: Uuid::new_v4(),
            },
            DynamicResourceInstance {
                name: "cert-3".to_string(),
                namespace: Some("monitoring".to_string()),
                kind: "Certificate".to_string(),
                api_version: "cert-manager.io/v1".to_string(),
                data: json!({"spec": {"secretName": "cert-3-tls"}}),
                cluster_id: Uuid::new_v4(),
            },
        ];

        assert_eq!(instances.len(), 3);

        // Filter by namespace
        let default_ns: Vec<&DynamicResourceInstance> =
            instances.iter().filter(|i| i.namespace.as_deref() == Some("default")).collect();
        assert_eq!(default_ns.len(), 2);

        // All are namespaced
        assert!(instances.iter().all(|i| i.is_namespaced()));
    }

    // --- T110/T112: CRD grouping by API group ---

    #[test]
    fn test_group_by_api_group() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));
        registry.register(sample_crd("virtualmachines", "kubevirt.io", "VirtualMachine"));
        registry.register(sample_crd("ingresses", "networking.k8s.io", "Ingress"));

        let grouped = registry.group_by_api_group();
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped["cert-manager.io"].len(), 2);
        assert_eq!(grouped["kubevirt.io"].len(), 1);
        assert_eq!(grouped["networking.k8s.io"].len(), 1);
    }

    #[test]
    fn test_group_by_api_group_empty() {
        let registry = CrdRegistry::new();
        let grouped = registry.group_by_api_group();
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_group_by_api_group_single_group() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));
        registry.register(sample_crd("clusterissuers", "cert-manager.io", "ClusterIssuer"));

        let grouped = registry.group_by_api_group();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped["cert-manager.io"].len(), 3);
    }

    // --- T112: CRD discovery service ---

    #[test]
    fn test_list_crds_empty() {
        let registry = CrdRegistry::new();
        assert!(registry.list_crds().is_empty());
    }

    #[test]
    fn test_list_crds() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));

        let crds = registry.list_crds();
        assert_eq!(crds.len(), 2);
    }

    #[test]
    fn test_find_by_kind_found() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));

        let found = registry.find_by_kind("Certificate");
        assert!(found.is_some());
        assert_eq!(found.unwrap().kind, "Certificate");
    }

    #[test]
    fn test_find_by_kind_not_found() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));

        let found = registry.find_by_kind("NonExistent");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_all_by_kind() {
        let mut registry = CrdRegistry::new();
        let cluster1 = Uuid::new_v4();
        let cluster2 = Uuid::new_v4();

        let mut crd1 = sample_crd("certs", "cert-manager.io", "Certificate");
        crd1.cluster_id = cluster1;
        let mut crd2 = sample_crd("certs", "cert-manager.io", "Certificate");
        crd2.cluster_id = cluster2;

        registry.register(crd1);
        registry.register(crd2);

        let found = registry.find_all_by_kind("Certificate");
        assert_eq!(found.len(), 2);
    }

    // --- T110: CRD version listing ---

    #[test]
    fn test_crd_version_listing_single() {
        let crd = sample_crd("certs", "test.io", "Cert");
        assert_eq!(crd.version_list().len(), 1);
        assert_eq!(crd.version_list()[0], "v1");
    }

    #[test]
    fn test_crd_version_listing_multiple() {
        let crd = sample_crd_with_details(
            "certs",
            "test.io",
            "Cert",
            vec!["v1", "v1beta1", "v1alpha2", "v1alpha1"],
            CrdScope::Namespaced,
            None,
        );
        assert_eq!(crd.version_list().len(), 4);
        assert!(crd.supports_version("v1"));
        assert!(crd.supports_version("v1beta1"));
        assert!(crd.supports_version("v1alpha2"));
        assert!(crd.supports_version("v1alpha1"));
        assert!(!crd.supports_version("v2"));
    }

    #[test]
    fn test_crd_version_listing_empty() {
        let mut crd = sample_crd("certs", "test.io", "Cert");
        crd.versions.clear();
        assert!(crd.version_list().is_empty());
        assert!(!crd.supports_version("v1"));
    }

    // --- Existing registry tests ---

    #[test]
    fn test_crd_registry_register_and_find() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certificates", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));

        assert_eq!(registry.count(), 2);
        assert!(registry.find_by_kind("Certificate").is_some());
        assert_eq!(registry.find_by_group("cert-manager.io").len(), 2);
    }

    #[test]
    fn test_crd_registry_update_existing() {
        let mut registry = CrdRegistry::new();
        let mut crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        registry.register(crd.clone());

        crd.versions = vec!["v1".to_string(), "v1alpha1".to_string()];
        registry.register(crd);

        assert_eq!(registry.count(), 1);
        assert_eq!(registry.find_by_kind("Certificate").unwrap().versions.len(), 2);
    }

    #[test]
    fn test_crd_registry_unregister() {
        let mut registry = CrdRegistry::new();
        let crd = sample_crd("certificates", "cert-manager.io", "Certificate");
        let cluster_id = crd.cluster_id;
        registry.register(crd);

        registry.unregister("certificates.cert-manager.io", cluster_id);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_crd_registry_groups() {
        let mut registry = CrdRegistry::new();
        registry.register(sample_crd("certs", "cert-manager.io", "Certificate"));
        registry.register(sample_crd("issuers", "cert-manager.io", "Issuer"));
        registry.register(sample_crd("virts", "kubevirt.io", "VirtualMachine"));

        let groups = registry.groups();
        assert_eq!(groups.len(), 2);
        assert!(groups.contains(&"cert-manager.io"));
        assert!(groups.contains(&"kubevirt.io"));
    }

    #[test]
    fn test_crd_registry_for_cluster() {
        let mut registry = CrdRegistry::new();
        let cluster1 = Uuid::new_v4();
        let cluster2 = Uuid::new_v4();

        let mut crd1 = sample_crd("certs", "test.io", "Cert");
        crd1.cluster_id = cluster1;
        let mut crd2 = sample_crd("vms", "test.io", "VM");
        crd2.cluster_id = cluster2;

        registry.register(crd1);
        registry.register(crd2);

        assert_eq!(registry.for_cluster(cluster1).len(), 1);
        assert_eq!(registry.for_cluster(cluster2).len(), 1);
    }

    #[test]
    fn test_crd_registry_clear_cluster() {
        let mut registry = CrdRegistry::new();
        let cluster1 = Uuid::new_v4();
        let cluster2 = Uuid::new_v4();

        let mut crd1 = sample_crd("certs", "test.io", "Cert");
        crd1.cluster_id = cluster1;
        let mut crd2 = sample_crd("vms", "test.io", "VM");
        crd2.cluster_id = cluster1;
        let mut crd3 = sample_crd("things", "other.io", "Thing");
        crd3.cluster_id = cluster2;

        registry.register(crd1);
        registry.register(crd2);
        registry.register(crd3);

        assert_eq!(registry.count(), 3);

        registry.clear_cluster(cluster1);
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.for_cluster(cluster2).len(), 1);
    }

    // --- T112: Discovery service integration test ---

    #[test]
    fn test_discovery_service_full_workflow() {
        let mut registry = CrdRegistry::new();
        let cluster_id = Uuid::new_v4();

        // Simulate discovering CRDs from a cluster
        let crds = vec![
            CrdSchema {
                name: "certificates.cert-manager.io".to_string(),
                group: "cert-manager.io".to_string(),
                kind: "Certificate".to_string(),
                versions: vec!["v1".to_string(), "v1beta1".to_string()],
                scope: CrdScope::Namespaced,
                description: Some("Cert-manager Certificate resource".to_string()),
                schema_properties: None,
                schema: None,
                cluster_id,
            },
            CrdSchema {
                name: "issuers.cert-manager.io".to_string(),
                group: "cert-manager.io".to_string(),
                kind: "Issuer".to_string(),
                versions: vec!["v1".to_string()],
                scope: CrdScope::Namespaced,
                description: Some("Cert-manager Issuer".to_string()),
                schema_properties: None,
                schema: None,
                cluster_id,
            },
            CrdSchema {
                name: "clusterissuers.cert-manager.io".to_string(),
                group: "cert-manager.io".to_string(),
                kind: "ClusterIssuer".to_string(),
                versions: vec!["v1".to_string()],
                scope: CrdScope::Cluster,
                description: None,
                schema_properties: None,
                schema: None,
                cluster_id,
            },
            CrdSchema {
                name: "virtualmachines.kubevirt.io".to_string(),
                group: "kubevirt.io".to_string(),
                kind: "VirtualMachine".to_string(),
                versions: vec!["v1".to_string()],
                scope: CrdScope::Namespaced,
                description: Some("KubeVirt Virtual Machine".to_string()),
                schema_properties: None,
                schema: None,
                cluster_id,
            },
        ];

        for crd in crds {
            registry.register(crd);
        }

        // list_crds
        assert_eq!(registry.list_crds().len(), 4);

        // group_by_api_group
        let grouped = registry.group_by_api_group();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["cert-manager.io"].len(), 3);
        assert_eq!(grouped["kubevirt.io"].len(), 1);

        // find_by_kind
        let cert = registry.find_by_kind("Certificate");
        assert!(cert.is_some());
        assert_eq!(cert.unwrap().name, "certificates.cert-manager.io");

        // Version listing
        let cert = registry.find_by_kind("Certificate").unwrap();
        assert_eq!(cert.version_list().len(), 2);
        assert!(cert.supports_version("v1"));
        assert!(cert.supports_version("v1beta1"));

        // Scope check
        let cluster_issuer = registry.find_by_kind("ClusterIssuer").unwrap();
        assert!(!cluster_issuer.is_namespaced());

        // Not found
        assert!(registry.find_by_kind("NonExistent").is_none());
    }
}
