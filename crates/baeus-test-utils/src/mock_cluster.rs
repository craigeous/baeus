use baeus_core::Namespace;
use baeus_core::cluster::{AuthMethod, ClusterConnection, ConnectionStatus};
use baeus_core::resource::Resource;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct MockCluster {
    pub connection: ClusterConnection,
    namespaces: Vec<Namespace>,
    resources: BTreeMap<String, Vec<Resource>>,
}

impl MockCluster {
    pub fn new(name: &str) -> Self {
        Self {
            connection: ClusterConnection::new(
                name.to_string(),
                format!("{name}-context"),
                format!("https://{name}.example.com:6443"),
                AuthMethod::Token,
            ),
            namespaces: Vec::new(),
            resources: BTreeMap::new(),
        }
    }

    pub fn with_status(mut self, status: ConnectionStatus) -> Self {
        self.connection.status = status;
        self
    }

    pub fn with_namespace(mut self, namespace: Namespace) -> Self {
        self.namespaces.push(namespace);
        self
    }

    pub fn with_resources(mut self, kind: &str, resources: Vec<Resource>) -> Self {
        self.resources.insert(kind.to_string(), resources);
        self
    }

    pub fn cluster_id(&self) -> Uuid {
        self.connection.id
    }

    pub fn list_namespaces(&self) -> &[Namespace] {
        &self.namespaces
    }

    pub fn list_resources(&self, kind: &str) -> Vec<&Resource> {
        self.resources.get(kind).map(|r| r.iter().collect()).unwrap_or_default()
    }

    pub fn get_resource(&self, kind: &str, name: &str) -> Option<&Resource> {
        self.resources.get(kind).and_then(|resources| resources.iter().find(|r| r.name == name))
    }
}

#[derive(Debug, Default)]
pub struct MockClusterManager {
    clusters: Vec<MockCluster>,
}

impl MockClusterManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_cluster(&mut self, cluster: MockCluster) {
        self.clusters.push(cluster);
    }

    pub fn clusters(&self) -> &[MockCluster] {
        &self.clusters
    }

    pub fn find_cluster(&self, name: &str) -> Option<&MockCluster> {
        self.clusters.iter().find(|c| c.connection.name == name)
    }

    pub fn connected_clusters(&self) -> Vec<&MockCluster> {
        self.clusters.iter().filter(|c| c.connection.is_connected()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn test_mock_cluster_creation() {
        let cluster = MockCluster::new("test-cluster");
        assert_eq!(cluster.connection.name, "test-cluster");
        assert_eq!(cluster.connection.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_mock_cluster_with_status() {
        let cluster = MockCluster::new("test").with_status(ConnectionStatus::Connected);
        assert!(cluster.connection.is_connected());
    }

    #[test]
    fn test_mock_cluster_with_namespaces() {
        let cluster = MockCluster::new("test")
            .with_namespace(fixtures::sample_namespace("default"))
            .with_namespace(fixtures::sample_namespace("kube-system"));

        assert_eq!(cluster.list_namespaces().len(), 2);
    }

    #[test]
    fn test_mock_cluster_with_resources() {
        let pods = vec![
            fixtures::sample_pod("nginx", "default"),
            fixtures::sample_pod("redis", "default"),
        ];

        let cluster = MockCluster::new("test").with_resources("Pod", pods);

        assert_eq!(cluster.list_resources("Pod").len(), 2);
        assert!(cluster.list_resources("Deployment").is_empty());
    }

    #[test]
    fn test_mock_cluster_get_resource() {
        let pods = vec![fixtures::sample_pod("nginx", "default")];
        let cluster = MockCluster::new("test").with_resources("Pod", pods);

        let found = cluster.get_resource("Pod", "nginx");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "nginx");

        assert!(cluster.get_resource("Pod", "nonexistent").is_none());
    }

    #[test]
    fn test_mock_cluster_manager() {
        let mut manager = MockClusterManager::new();
        manager.add_cluster(MockCluster::new("prod").with_status(ConnectionStatus::Connected));
        manager.add_cluster(MockCluster::new("dev").with_status(ConnectionStatus::Disconnected));

        assert_eq!(manager.clusters().len(), 2);
        assert_eq!(manager.connected_clusters().len(), 1);
        assert!(manager.find_cluster("prod").is_some());
        assert!(manager.find_cluster("staging").is_none());
    }
}
