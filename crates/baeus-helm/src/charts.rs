use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartEntry {
    pub name: String,
    pub version: String,
    pub app_version: Option<String>,
    pub description: Option<String>,
    pub home: Option<String>,
    pub sources: Vec<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartIndex {
    pub api_version: String,
    pub entries: std::collections::BTreeMap<String, Vec<ChartEntry>>,
}

impl ChartIndex {
    pub fn search(&self, query: &str) -> Vec<&ChartEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .values()
            .flat_map(|versions| versions.iter())
            .filter(|entry| {
                entry.name.to_lowercase().contains(&query_lower)
                    || entry
                        .description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    pub fn get_chart(&self, name: &str) -> Option<&Vec<ChartEntry>> {
        self.entries.get(name)
    }

    pub fn latest_version(&self, name: &str) -> Option<&ChartEntry> {
        self.entries.get(name).and_then(|versions| versions.first())
    }

    pub fn chart_names(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_index() -> ChartIndex {
        let mut entries = BTreeMap::new();
        entries.insert(
            "nginx".to_string(),
            vec![
                ChartEntry {
                    name: "nginx".to_string(),
                    version: "15.4.0".to_string(),
                    app_version: Some("1.25.3".to_string()),
                    description: Some("NGINX web server".to_string()),
                    home: Some("https://nginx.org".to_string()),
                    sources: vec![],
                    urls: vec!["https://charts.bitnami.com/bitnami/nginx-15.4.0.tgz".to_string()],
                },
                ChartEntry {
                    name: "nginx".to_string(),
                    version: "15.3.0".to_string(),
                    app_version: Some("1.25.2".to_string()),
                    description: Some("NGINX web server".to_string()),
                    home: None,
                    sources: vec![],
                    urls: vec![],
                },
            ],
        );
        entries.insert(
            "redis".to_string(),
            vec![ChartEntry {
                name: "redis".to_string(),
                version: "18.5.0".to_string(),
                app_version: Some("7.2.4".to_string()),
                description: Some("Redis in-memory database".to_string()),
                home: None,
                sources: vec![],
                urls: vec![],
            }],
        );

        ChartIndex { api_version: "v1".to_string(), entries }
    }

    #[test]
    fn test_search_by_name() {
        let index = sample_index();
        let results = index.search("nginx");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_description() {
        let index = sample_index();
        let results = index.search("database");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "redis");
    }

    #[test]
    fn test_search_case_insensitive() {
        let index = sample_index();
        let results = index.search("NGINX");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_no_results() {
        let index = sample_index();
        assert!(index.search("postgresql").is_empty());
    }

    #[test]
    fn test_get_chart() {
        let index = sample_index();
        let versions = index.get_chart("nginx").unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_latest_version() {
        let index = sample_index();
        let latest = index.latest_version("nginx").unwrap();
        assert_eq!(latest.version, "15.4.0");
    }

    #[test]
    fn test_chart_names() {
        let index = sample_index();
        let names = index.chart_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"nginx"));
        assert!(names.contains(&"redis"));
    }

    // --- T088: Edge-case tests for chart repository ---

    #[test]
    fn test_empty_index() {
        let index = ChartIndex { api_version: "v1".to_string(), entries: BTreeMap::new() };

        assert!(index.search("anything").is_empty());
        assert!(index.get_chart("nginx").is_none());
        assert!(index.latest_version("nginx").is_none());
        assert!(index.chart_names().is_empty());
    }

    #[test]
    fn test_partial_chart_entry_no_description_no_app_version() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "bare-chart".to_string(),
            vec![ChartEntry {
                name: "bare-chart".to_string(),
                version: "0.1.0".to_string(),
                app_version: None,
                description: None,
                home: None,
                sources: vec![],
                urls: vec![],
            }],
        );

        let index = ChartIndex { api_version: "v1".to_string(), entries };

        // Search by name still works
        let results = index.search("bare");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "bare-chart");
        assert!(results[0].app_version.is_none());
        assert!(results[0].description.is_none());

        // Search by description does not match since description is None
        let no_results = index.search("some description text");
        assert!(no_results.is_empty());

        // latest_version works on partial entry
        let latest = index.latest_version("bare-chart").unwrap();
        assert_eq!(latest.version, "0.1.0");
    }

    #[test]
    fn test_partial_chart_entry_empty_urls_and_sources() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "local-chart".to_string(),
            vec![ChartEntry {
                name: "local-chart".to_string(),
                version: "1.0.0".to_string(),
                app_version: Some("1.0.0".to_string()),
                description: Some("A local chart with no download URLs".to_string()),
                home: None,
                sources: vec![],
                urls: vec![],
            }],
        );

        let index = ChartIndex { api_version: "v1".to_string(), entries };

        let chart = index.get_chart("local-chart").unwrap();
        assert_eq!(chart.len(), 1);
        assert!(chart[0].urls.is_empty());
        assert!(chart[0].sources.is_empty());
    }

    #[test]
    fn test_get_chart_nonexistent() {
        let index = sample_index();
        assert!(index.get_chart("does-not-exist").is_none());
    }

    #[test]
    fn test_latest_version_nonexistent() {
        let index = sample_index();
        assert!(index.latest_version("phantom").is_none());
    }

    #[test]
    fn test_search_matches_across_multiple_charts() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "web-server".to_string(),
            vec![ChartEntry {
                name: "web-server".to_string(),
                version: "1.0.0".to_string(),
                app_version: None,
                description: Some("A fast web server".to_string()),
                home: None,
                sources: vec![],
                urls: vec![],
            }],
        );
        entries.insert(
            "web-gateway".to_string(),
            vec![ChartEntry {
                name: "web-gateway".to_string(),
                version: "2.0.0".to_string(),
                app_version: None,
                description: Some("API gateway".to_string()),
                home: None,
                sources: vec![],
                urls: vec![],
            }],
        );

        let index = ChartIndex { api_version: "v1".to_string(), entries };

        // "web" matches both chart names
        let results = index.search("web");
        assert_eq!(results.len(), 2);
    }
}
