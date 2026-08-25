use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub node_name: String,
    pub cpu_usage_millicores: u64,
    pub cpu_capacity_millicores: u64,
    pub memory_usage_bytes: u64,
    pub memory_capacity_bytes: u64,
    pub timestamp: DateTime<Utc>,
    pub cluster_id: Uuid,
}

impl NodeMetrics {
    pub fn cpu_usage_percent(&self) -> f64 {
        if self.cpu_capacity_millicores == 0 {
            return 0.0;
        }
        (self.cpu_usage_millicores as f64 / self.cpu_capacity_millicores as f64) * 100.0
    }

    pub fn memory_usage_percent(&self) -> f64 {
        if self.memory_capacity_bytes == 0 {
            return 0.0;
        }
        (self.memory_usage_bytes as f64 / self.memory_capacity_bytes as f64) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodMetrics {
    pub pod_name: String,
    pub namespace: String,
    pub containers: Vec<ContainerMetrics>,
    pub timestamp: DateTime<Utc>,
    pub cluster_id: Uuid,
}

impl PodMetrics {
    pub fn total_cpu_millicores(&self) -> u64 {
        self.containers.iter().map(|c| c.cpu_usage_millicores).sum()
    }

    pub fn total_memory_bytes(&self) -> u64 {
        self.containers.iter().map(|c| c.memory_usage_bytes).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerMetrics {
    pub container_name: String,
    pub cpu_usage_millicores: u64,
    pub memory_usage_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsAvailability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug)]
pub struct MetricsState {
    pub availability: MetricsAvailability,
    pub node_metrics: Vec<NodeMetrics>,
    pub pod_metrics: Vec<PodMetrics>,
    pub last_fetched: Option<DateTime<Utc>>,
}

impl Default for MetricsState {
    fn default() -> Self {
        Self {
            availability: MetricsAvailability::Unknown,
            node_metrics: Vec::new(),
            pod_metrics: Vec::new(),
            last_fetched: None,
        }
    }
}

impl MetricsState {
    pub fn set_available(&mut self, nodes: Vec<NodeMetrics>, pods: Vec<PodMetrics>) {
        self.availability = MetricsAvailability::Available;
        self.node_metrics = nodes;
        self.pod_metrics = pods;
        self.last_fetched = Some(Utc::now());
    }

    pub fn set_unavailable(&mut self) {
        self.availability = MetricsAvailability::Unavailable;
        self.node_metrics.clear();
        self.pod_metrics.clear();
    }

    pub fn find_pod_metrics(&self, name: &str, namespace: &str) -> Option<&PodMetrics> {
        self.pod_metrics.iter().find(|m| m.pod_name == name && m.namespace == namespace)
    }

    pub fn find_node_metrics(&self, name: &str) -> Option<&NodeMetrics> {
        self.node_metrics.iter().find(|m| m.node_name == name)
    }

    pub fn is_available(&self) -> bool {
        self.availability == MetricsAvailability::Available
    }

    pub fn is_unavailable(&self) -> bool {
        self.availability == MetricsAvailability::Unavailable
    }

    /// Aggregate total CPU usage across all nodes (millicores).
    pub fn total_node_cpu_millicores(&self) -> u64 {
        self.node_metrics.iter().map(|n| n.cpu_usage_millicores).sum()
    }

    /// Aggregate total CPU capacity across all nodes (millicores).
    pub fn total_node_cpu_capacity(&self) -> u64 {
        self.node_metrics.iter().map(|n| n.cpu_capacity_millicores).sum()
    }

    /// Aggregate total memory usage across all nodes (bytes).
    pub fn total_node_memory_bytes(&self) -> u64 {
        self.node_metrics.iter().map(|n| n.memory_usage_bytes).sum()
    }

    /// Aggregate total memory capacity across all nodes (bytes).
    pub fn total_node_memory_capacity(&self) -> u64 {
        self.node_metrics.iter().map(|n| n.memory_capacity_bytes).sum()
    }

    /// Get pod metrics for a given namespace.
    pub fn pods_in_namespace(&self, namespace: &str) -> Vec<&PodMetrics> {
        self.pod_metrics.iter().filter(|m| m.namespace == namespace).collect()
    }

    /// Get the top N pods by CPU usage.
    pub fn top_pods_by_cpu(&self, n: usize) -> Vec<&PodMetrics> {
        let mut pods: Vec<&PodMetrics> = self.pod_metrics.iter().collect();
        pods.sort_by_key(|p| std::cmp::Reverse(p.total_cpu_millicores()));
        pods.truncate(n);
        pods
    }

    /// Get the top N pods by memory usage.
    pub fn top_pods_by_memory(&self, n: usize) -> Vec<&PodMetrics> {
        let mut pods: Vec<&PodMetrics> = self.pod_metrics.iter().collect();
        pods.sort_by_key(|p| std::cmp::Reverse(p.total_memory_bytes()));
        pods.truncate(n);
        pods
    }
}

/// Configuration for the metrics polling service.
#[derive(Debug, Clone)]
pub struct MetricsPollingConfig {
    /// Polling interval in seconds.
    pub interval_secs: u64,
    /// Whether to poll node metrics.
    pub poll_nodes: bool,
    /// Whether to poll pod metrics.
    pub poll_pods: bool,
    /// Namespace filter for pod metrics (None = all namespaces).
    pub namespace_filter: Option<String>,
}

impl Default for MetricsPollingConfig {
    fn default() -> Self {
        Self { interval_secs: 30, poll_nodes: true, poll_pods: true, namespace_filter: None }
    }
}

/// Tracks the state of the metrics polling service.
#[derive(Debug)]
pub struct MetricsPollingState {
    pub config: MetricsPollingConfig,
    pub active: bool,
    pub error_count: u32,
    pub max_consecutive_errors: u32,
}

impl Default for MetricsPollingState {
    fn default() -> Self {
        Self {
            config: MetricsPollingConfig::default(),
            active: false,
            error_count: 0,
            max_consecutive_errors: 3,
        }
    }
}

impl MetricsPollingState {
    pub fn start(&mut self) {
        self.active = true;
        self.error_count = 0;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn on_success(&mut self) {
        self.error_count = 0;
    }

    pub fn on_error(&mut self) {
        self.error_count += 1;
    }

    /// Returns true if too many consecutive errors have occurred.
    pub fn should_stop_on_errors(&self) -> bool {
        self.error_count >= self.max_consecutive_errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_node_metrics(name: &str) -> NodeMetrics {
        NodeMetrics {
            node_name: name.to_string(),
            cpu_usage_millicores: 1500,
            cpu_capacity_millicores: 4000,
            memory_usage_bytes: 4_000_000_000,
            memory_capacity_bytes: 16_000_000_000,
            timestamp: Utc::now(),
            cluster_id: Uuid::new_v4(),
        }
    }

    fn sample_pod_metrics(name: &str, ns: &str) -> PodMetrics {
        PodMetrics {
            pod_name: name.to_string(),
            namespace: ns.to_string(),
            containers: vec![
                ContainerMetrics {
                    container_name: "app".to_string(),
                    cpu_usage_millicores: 250,
                    memory_usage_bytes: 128_000_000,
                },
                ContainerMetrics {
                    container_name: "sidecar".to_string(),
                    cpu_usage_millicores: 50,
                    memory_usage_bytes: 32_000_000,
                },
            ],
            timestamp: Utc::now(),
            cluster_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn test_node_metrics_cpu_percent() {
        let metrics = sample_node_metrics("node-1");
        assert!((metrics.cpu_usage_percent() - 37.5).abs() < 0.01);
    }

    #[test]
    fn test_node_metrics_memory_percent() {
        let metrics = sample_node_metrics("node-1");
        assert!((metrics.memory_usage_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_node_metrics_zero_capacity() {
        let metrics = NodeMetrics {
            cpu_capacity_millicores: 0,
            memory_capacity_bytes: 0,
            ..sample_node_metrics("empty")
        };
        assert_eq!(metrics.cpu_usage_percent(), 0.0);
        assert_eq!(metrics.memory_usage_percent(), 0.0);
    }

    #[test]
    fn test_pod_metrics_totals() {
        let metrics = sample_pod_metrics("nginx", "default");
        assert_eq!(metrics.total_cpu_millicores(), 300);
        assert_eq!(metrics.total_memory_bytes(), 160_000_000);
    }

    #[test]
    fn test_metrics_state_default() {
        let state = MetricsState::default();
        assert_eq!(state.availability, MetricsAvailability::Unknown);
        assert!(state.node_metrics.is_empty());
        assert!(state.last_fetched.is_none());
    }

    #[test]
    fn test_metrics_state_set_available() {
        let mut state = MetricsState::default();
        state.set_available(
            vec![sample_node_metrics("node-1")],
            vec![sample_pod_metrics("pod-1", "default")],
        );

        assert_eq!(state.availability, MetricsAvailability::Available);
        assert_eq!(state.node_metrics.len(), 1);
        assert_eq!(state.pod_metrics.len(), 1);
        assert!(state.last_fetched.is_some());
    }

    #[test]
    fn test_metrics_state_set_unavailable() {
        let mut state = MetricsState::default();
        state.set_available(vec![sample_node_metrics("node-1")], vec![]);
        state.set_unavailable();

        assert_eq!(state.availability, MetricsAvailability::Unavailable);
        assert!(state.node_metrics.is_empty());
    }

    #[test]
    fn test_find_pod_metrics() {
        let mut state = MetricsState::default();
        state.set_available(
            vec![],
            vec![sample_pod_metrics("nginx", "default"), sample_pod_metrics("redis", "cache")],
        );

        assert!(state.find_pod_metrics("nginx", "default").is_some());
        assert!(state.find_pod_metrics("redis", "cache").is_some());
        assert!(state.find_pod_metrics("nginx", "cache").is_none());
    }

    #[test]
    fn test_find_node_metrics() {
        let mut state = MetricsState::default();
        state.set_available(vec![sample_node_metrics("worker-1")], vec![]);

        assert!(state.find_node_metrics("worker-1").is_some());
        assert!(state.find_node_metrics("worker-2").is_none());
    }

    // --- T096: Metrics polling service tests ---

    #[test]
    fn test_is_available_unavailable() {
        let mut state = MetricsState::default();
        assert!(!state.is_available());
        assert!(!state.is_unavailable());

        state.set_available(vec![], vec![]);
        assert!(state.is_available());

        state.set_unavailable();
        assert!(state.is_unavailable());
    }

    #[test]
    fn test_total_node_metrics_aggregation() {
        let mut state = MetricsState::default();
        let mut node1 = sample_node_metrics("node-1");
        node1.cpu_usage_millicores = 1000;
        node1.cpu_capacity_millicores = 4000;
        node1.memory_usage_bytes = 2_000_000_000;
        node1.memory_capacity_bytes = 8_000_000_000;

        let mut node2 = sample_node_metrics("node-2");
        node2.cpu_usage_millicores = 2000;
        node2.cpu_capacity_millicores = 4000;
        node2.memory_usage_bytes = 3_000_000_000;
        node2.memory_capacity_bytes = 8_000_000_000;

        state.set_available(vec![node1, node2], vec![]);

        assert_eq!(state.total_node_cpu_millicores(), 3000);
        assert_eq!(state.total_node_cpu_capacity(), 8000);
        assert_eq!(state.total_node_memory_bytes(), 5_000_000_000);
        assert_eq!(state.total_node_memory_capacity(), 16_000_000_000);
    }

    #[test]
    fn test_pods_in_namespace() {
        let mut state = MetricsState::default();
        state.set_available(
            vec![],
            vec![
                sample_pod_metrics("pod-a", "default"),
                sample_pod_metrics("pod-b", "default"),
                sample_pod_metrics("pod-c", "kube-system"),
            ],
        );

        assert_eq!(state.pods_in_namespace("default").len(), 2);
        assert_eq!(state.pods_in_namespace("kube-system").len(), 1);
        assert_eq!(state.pods_in_namespace("nonexistent").len(), 0);
    }

    #[test]
    fn test_top_pods_by_cpu() {
        let mut state = MetricsState::default();
        let cluster = Uuid::new_v4();
        state.set_available(
            vec![],
            vec![
                PodMetrics {
                    pod_name: "low".to_string(),
                    namespace: "default".to_string(),
                    containers: vec![ContainerMetrics {
                        container_name: "app".to_string(),
                        cpu_usage_millicores: 100,
                        memory_usage_bytes: 0,
                    }],
                    timestamp: Utc::now(),
                    cluster_id: cluster,
                },
                PodMetrics {
                    pod_name: "high".to_string(),
                    namespace: "default".to_string(),
                    containers: vec![ContainerMetrics {
                        container_name: "app".to_string(),
                        cpu_usage_millicores: 500,
                        memory_usage_bytes: 0,
                    }],
                    timestamp: Utc::now(),
                    cluster_id: cluster,
                },
                PodMetrics {
                    pod_name: "medium".to_string(),
                    namespace: "default".to_string(),
                    containers: vec![ContainerMetrics {
                        container_name: "app".to_string(),
                        cpu_usage_millicores: 300,
                        memory_usage_bytes: 0,
                    }],
                    timestamp: Utc::now(),
                    cluster_id: cluster,
                },
            ],
        );

        let top2 = state.top_pods_by_cpu(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].pod_name, "high");
        assert_eq!(top2[1].pod_name, "medium");
    }

    #[test]
    fn test_top_pods_by_memory() {
        let mut state = MetricsState::default();
        let cluster = Uuid::new_v4();
        state.set_available(
            vec![],
            vec![
                PodMetrics {
                    pod_name: "small".to_string(),
                    namespace: "default".to_string(),
                    containers: vec![ContainerMetrics {
                        container_name: "app".to_string(),
                        cpu_usage_millicores: 0,
                        memory_usage_bytes: 100_000,
                    }],
                    timestamp: Utc::now(),
                    cluster_id: cluster,
                },
                PodMetrics {
                    pod_name: "large".to_string(),
                    namespace: "default".to_string(),
                    containers: vec![ContainerMetrics {
                        container_name: "app".to_string(),
                        cpu_usage_millicores: 0,
                        memory_usage_bytes: 900_000,
                    }],
                    timestamp: Utc::now(),
                    cluster_id: cluster,
                },
            ],
        );

        let top1 = state.top_pods_by_memory(1);
        assert_eq!(top1.len(), 1);
        assert_eq!(top1[0].pod_name, "large");
    }

    #[test]
    fn test_metrics_polling_config_default() {
        let config = MetricsPollingConfig::default();
        assert_eq!(config.interval_secs, 30);
        assert!(config.poll_nodes);
        assert!(config.poll_pods);
        assert!(config.namespace_filter.is_none());
    }

    #[test]
    fn test_metrics_polling_state_lifecycle() {
        let mut polling = MetricsPollingState::default();
        assert!(!polling.active);
        assert_eq!(polling.error_count, 0);

        polling.start();
        assert!(polling.active);

        polling.on_success();
        assert_eq!(polling.error_count, 0);

        polling.on_error();
        assert_eq!(polling.error_count, 1);
        assert!(!polling.should_stop_on_errors());

        polling.on_error();
        polling.on_error();
        assert!(polling.should_stop_on_errors());

        polling.on_success();
        assert_eq!(polling.error_count, 0);
        assert!(!polling.should_stop_on_errors());

        polling.stop();
        assert!(!polling.active);
    }

    #[test]
    fn test_node_metrics_serialization() {
        let metrics = sample_node_metrics("node-1");
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: NodeMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_name, "node-1");
        assert_eq!(deserialized.cpu_usage_millicores, 1500);
    }

    #[test]
    fn test_pod_metrics_serialization() {
        let metrics = sample_pod_metrics("nginx", "default");
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: PodMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pod_name, "nginx");
        assert_eq!(deserialized.containers.len(), 2);
    }

    #[test]
    fn test_pod_metrics_empty_containers() {
        let metrics = PodMetrics {
            pod_name: "empty".to_string(),
            namespace: "default".to_string(),
            containers: vec![],
            timestamp: Utc::now(),
            cluster_id: Uuid::new_v4(),
        };
        assert_eq!(metrics.total_cpu_millicores(), 0);
        assert_eq!(metrics.total_memory_bytes(), 0);
    }
}
