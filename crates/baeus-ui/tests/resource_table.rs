// T103a-T103c: Tests for Service, Ingress, and PVC specific detail rendering

use std::collections::HashMap;

use baeus_ui::views::resource_detail::{
    IngressDetail, IngressPath, IngressRule, PvcAccessMode, PvcDetail, PvcStatus, ServiceDetail,
    ServicePort, ServiceType, TlsConfig,
};

// ============================================================================
// T103a: Service-specific detail rendering tests
// ============================================================================

#[test]
fn test_service_detail_cluster_ip_type() {
    let detail = ServiceDetail {
        service_type: ServiceType::ClusterIP,
        cluster_ip: Some("10.96.0.1".to_string()),
        external_ips: vec![],
        ports: vec![ServicePort {
            name: Some("http".to_string()),
            protocol: "TCP".to_string(),
            port: 80,
            target_port: "8080".to_string(),
            node_port: None,
        }],
        selectors: HashMap::from([("app".to_string(), "nginx".to_string())]),
    };

    assert_eq!(detail.service_type, ServiceType::ClusterIP);
    assert_eq!(detail.cluster_ip.as_deref(), Some("10.96.0.1"));
    assert!(detail.external_ips.is_empty());
    assert_eq!(detail.ports.len(), 1);
    assert_eq!(detail.ports[0].port, 80);
    assert_eq!(detail.ports[0].target_port, "8080");
    assert!(detail.ports[0].node_port.is_none());
    assert_eq!(detail.selectors.get("app").unwrap(), "nginx");
}

#[test]
fn test_service_detail_node_port_type() {
    let detail = ServiceDetail {
        service_type: ServiceType::NodePort,
        cluster_ip: Some("10.96.0.42".to_string()),
        external_ips: vec![],
        ports: vec![ServicePort {
            name: Some("http".to_string()),
            protocol: "TCP".to_string(),
            port: 80,
            target_port: "8080".to_string(),
            node_port: Some(30080),
        }],
        selectors: HashMap::from([("app".to_string(), "web".to_string())]),
    };

    assert_eq!(detail.service_type, ServiceType::NodePort);
    assert_eq!(detail.ports[0].node_port, Some(30080));
}

#[test]
fn test_service_detail_load_balancer_type() {
    let detail = ServiceDetail {
        service_type: ServiceType::LoadBalancer,
        cluster_ip: Some("10.96.1.10".to_string()),
        external_ips: vec!["203.0.113.1".to_string(), "203.0.113.2".to_string()],
        ports: vec![
            ServicePort {
                name: Some("http".to_string()),
                protocol: "TCP".to_string(),
                port: 80,
                target_port: "8080".to_string(),
                node_port: Some(30080),
            },
            ServicePort {
                name: Some("https".to_string()),
                protocol: "TCP".to_string(),
                port: 443,
                target_port: "8443".to_string(),
                node_port: Some(30443),
            },
        ],
        selectors: HashMap::new(),
    };

    assert_eq!(detail.service_type, ServiceType::LoadBalancer);
    assert_eq!(detail.external_ips.len(), 2);
    assert_eq!(detail.external_ips[0], "203.0.113.1");
    assert_eq!(detail.ports.len(), 2);
    assert_eq!(detail.ports[1].port, 443);
}

#[test]
fn test_service_detail_external_name_type() {
    let detail = ServiceDetail {
        service_type: ServiceType::ExternalName,
        cluster_ip: None,
        external_ips: vec![],
        ports: vec![],
        selectors: HashMap::new(),
    };

    assert_eq!(detail.service_type, ServiceType::ExternalName);
    assert!(detail.cluster_ip.is_none());
    assert!(detail.ports.is_empty());
    assert!(detail.selectors.is_empty());
}

#[test]
fn test_service_detail_multiple_selectors() {
    let detail = ServiceDetail {
        service_type: ServiceType::ClusterIP,
        cluster_ip: Some("10.96.0.1".to_string()),
        external_ips: vec![],
        ports: vec![],
        selectors: HashMap::from([
            ("app".to_string(), "web".to_string()),
            ("tier".to_string(), "frontend".to_string()),
            ("version".to_string(), "v2".to_string()),
        ]),
    };

    assert_eq!(detail.selectors.len(), 3);
    assert_eq!(detail.selectors.get("tier").unwrap(), "frontend");
    assert_eq!(detail.selectors.get("version").unwrap(), "v2");
}

#[test]
fn test_service_port_without_name() {
    let port = ServicePort {
        name: None,
        protocol: "TCP".to_string(),
        port: 9090,
        target_port: "9090".to_string(),
        node_port: None,
    };

    assert!(port.name.is_none());
    assert_eq!(port.protocol, "TCP");
    assert_eq!(port.port, 9090);
}

#[test]
fn test_service_port_udp_protocol() {
    let port = ServicePort {
        name: Some("dns".to_string()),
        protocol: "UDP".to_string(),
        port: 53,
        target_port: "53".to_string(),
        node_port: None,
    };

    assert_eq!(port.protocol, "UDP");
}

#[test]
fn test_service_type_debug_clone() {
    let st = ServiceType::ClusterIP;
    let cloned = st.clone();
    assert_eq!(st, cloned);
    assert_eq!(format!("{:?}", st), "ClusterIP");
}

#[test]
fn test_service_detail_debug_clone() {
    let detail = ServiceDetail {
        service_type: ServiceType::ClusterIP,
        cluster_ip: Some("10.96.0.1".to_string()),
        external_ips: vec![],
        ports: vec![],
        selectors: HashMap::new(),
    };
    let cloned = detail.clone();
    assert_eq!(cloned.service_type, detail.service_type);
    assert!(!format!("{:?}", detail).is_empty());
}

// ============================================================================
// T103b: Ingress-specific detail rendering tests
// ============================================================================

#[test]
fn test_ingress_detail_single_rule() {
    let detail = IngressDetail {
        rules: vec![IngressRule {
            host: Some("example.com".to_string()),
            paths: vec![IngressPath {
                path: "/".to_string(),
                path_type: "Prefix".to_string(),
                backend_service: "web-svc".to_string(),
                backend_port: 80,
            }],
        }],
        default_backend: None,
        tls: vec![],
    };

    assert_eq!(detail.rules.len(), 1);
    assert_eq!(detail.rules[0].host.as_deref(), Some("example.com"));
    assert_eq!(detail.rules[0].paths.len(), 1);
    assert_eq!(detail.rules[0].paths[0].path, "/");
    assert_eq!(detail.rules[0].paths[0].path_type, "Prefix");
    assert_eq!(detail.rules[0].paths[0].backend_service, "web-svc");
    assert_eq!(detail.rules[0].paths[0].backend_port, 80);
    assert!(detail.default_backend.is_none());
    assert!(detail.tls.is_empty());
}

#[test]
fn test_ingress_detail_multiple_rules_and_paths() {
    let detail = IngressDetail {
        rules: vec![
            IngressRule {
                host: Some("api.example.com".to_string()),
                paths: vec![
                    IngressPath {
                        path: "/v1".to_string(),
                        path_type: "Prefix".to_string(),
                        backend_service: "api-v1-svc".to_string(),
                        backend_port: 8080,
                    },
                    IngressPath {
                        path: "/v2".to_string(),
                        path_type: "Prefix".to_string(),
                        backend_service: "api-v2-svc".to_string(),
                        backend_port: 8080,
                    },
                ],
            },
            IngressRule {
                host: Some("web.example.com".to_string()),
                paths: vec![IngressPath {
                    path: "/".to_string(),
                    path_type: "Prefix".to_string(),
                    backend_service: "web-svc".to_string(),
                    backend_port: 80,
                }],
            },
        ],
        default_backend: None,
        tls: vec![],
    };

    assert_eq!(detail.rules.len(), 2);
    assert_eq!(detail.rules[0].paths.len(), 2);
    assert_eq!(detail.rules[0].paths[1].backend_service, "api-v2-svc");
    assert_eq!(detail.rules[1].host.as_deref(), Some("web.example.com"));
}

#[test]
fn test_ingress_detail_with_default_backend() {
    let detail = IngressDetail {
        rules: vec![],
        default_backend: Some("default-svc:80".to_string()),
        tls: vec![],
    };

    assert!(detail.rules.is_empty());
    assert_eq!(detail.default_backend.as_deref(), Some("default-svc:80"));
}

#[test]
fn test_ingress_detail_with_tls() {
    let detail = IngressDetail {
        rules: vec![IngressRule {
            host: Some("secure.example.com".to_string()),
            paths: vec![IngressPath {
                path: "/".to_string(),
                path_type: "Prefix".to_string(),
                backend_service: "secure-svc".to_string(),
                backend_port: 443,
            }],
        }],
        default_backend: None,
        tls: vec![TlsConfig {
            hosts: vec!["secure.example.com".to_string()],
            secret_name: Some("tls-secret".to_string()),
        }],
    };

    assert_eq!(detail.tls.len(), 1);
    assert_eq!(detail.tls[0].hosts, vec!["secure.example.com"]);
    assert_eq!(detail.tls[0].secret_name.as_deref(), Some("tls-secret"));
}

#[test]
fn test_ingress_detail_tls_multiple_hosts() {
    let tls = TlsConfig {
        hosts: vec!["api.example.com".to_string(), "web.example.com".to_string()],
        secret_name: Some("wildcard-tls".to_string()),
    };

    assert_eq!(tls.hosts.len(), 2);
}

#[test]
fn test_ingress_detail_tls_no_secret() {
    let tls = TlsConfig { hosts: vec!["example.com".to_string()], secret_name: None };

    assert!(tls.secret_name.is_none());
}

#[test]
fn test_ingress_rule_no_host() {
    let rule = IngressRule {
        host: None,
        paths: vec![IngressPath {
            path: "/".to_string(),
            path_type: "Prefix".to_string(),
            backend_service: "catch-all-svc".to_string(),
            backend_port: 80,
        }],
    };

    assert!(rule.host.is_none());
    assert_eq!(rule.paths.len(), 1);
}

#[test]
fn test_ingress_path_exact_type() {
    let path = IngressPath {
        path: "/api/v1/users".to_string(),
        path_type: "Exact".to_string(),
        backend_service: "users-svc".to_string(),
        backend_port: 8080,
    };

    assert_eq!(path.path_type, "Exact");
}

#[test]
fn test_ingress_detail_debug_clone() {
    let detail = IngressDetail { rules: vec![], default_backend: None, tls: vec![] };
    let cloned = detail.clone();
    assert_eq!(cloned.rules.len(), detail.rules.len());
    assert!(!format!("{:?}", detail).is_empty());
}

// ============================================================================
// T103c: PVC-specific detail rendering tests
// ============================================================================

#[test]
fn test_pvc_detail_bound() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("10Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: Some("standard".to_string()),
        volume_name: Some("pv-vol-001".to_string()),
    };

    assert_eq!(detail.status, PvcStatus::Bound);
    assert_eq!(detail.capacity.as_deref(), Some("10Gi"));
    assert_eq!(detail.access_modes.len(), 1);
    assert_eq!(detail.access_modes[0], PvcAccessMode::ReadWriteOnce);
    assert_eq!(detail.storage_class_name.as_deref(), Some("standard"));
    assert_eq!(detail.volume_name.as_deref(), Some("pv-vol-001"));
}

#[test]
fn test_pvc_detail_pending() {
    let detail = PvcDetail {
        status: PvcStatus::Pending,
        capacity: None,
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: Some("fast-ssd".to_string()),
        volume_name: None,
    };

    assert_eq!(detail.status, PvcStatus::Pending);
    assert!(detail.capacity.is_none());
    assert!(detail.volume_name.is_none());
}

#[test]
fn test_pvc_detail_lost() {
    let detail = PvcDetail {
        status: PvcStatus::Lost,
        capacity: Some("5Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: Some("standard".to_string()),
        volume_name: Some("pv-deleted-vol".to_string()),
    };

    assert_eq!(detail.status, PvcStatus::Lost);
}

#[test]
fn test_pvc_detail_multiple_access_modes() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("100Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce, PvcAccessMode::ReadOnlyMany],
        storage_class_name: Some("nfs".to_string()),
        volume_name: Some("pv-nfs-001".to_string()),
    };

    assert_eq!(detail.access_modes.len(), 2);
    assert_eq!(detail.access_modes[0], PvcAccessMode::ReadWriteOnce);
    assert_eq!(detail.access_modes[1], PvcAccessMode::ReadOnlyMany);
}

#[test]
fn test_pvc_detail_read_write_many_mode() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("50Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteMany],
        storage_class_name: Some("efs".to_string()),
        volume_name: Some("pv-efs-001".to_string()),
    };

    assert_eq!(detail.access_modes[0], PvcAccessMode::ReadWriteMany);
}

#[test]
fn test_pvc_detail_no_storage_class() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("1Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: None,
        volume_name: Some("pv-manual-001".to_string()),
    };

    assert!(detail.storage_class_name.is_none());
}

#[test]
fn test_pvc_status_debug_clone() {
    let status = PvcStatus::Bound;
    let cloned = status.clone();
    assert_eq!(status, cloned);
    assert_eq!(format!("{:?}", status), "Bound");
}

#[test]
fn test_pvc_access_mode_debug_clone() {
    let mode = PvcAccessMode::ReadWriteOnce;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
    assert_eq!(format!("{:?}", mode), "ReadWriteOnce");
}

#[test]
fn test_pvc_detail_debug_clone() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("10Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: Some("standard".to_string()),
        volume_name: Some("pv-001".to_string()),
    };
    let cloned = detail.clone();
    assert_eq!(cloned.status, detail.status);
    assert_eq!(cloned.capacity, detail.capacity);
    assert!(!format!("{:?}", detail).is_empty());
}

#[test]
fn test_pvc_access_mode_all_variants() {
    let modes =
        [PvcAccessMode::ReadWriteOnce, PvcAccessMode::ReadOnlyMany, PvcAccessMode::ReadWriteMany];
    // Verify they are all distinct
    assert_ne!(modes[0], modes[1]);
    assert_ne!(modes[1], modes[2]);
    assert_ne!(modes[0], modes[2]);
}

#[test]
fn test_pvc_status_all_variants() {
    let statuses = [PvcStatus::Bound, PvcStatus::Pending, PvcStatus::Lost];
    assert_ne!(statuses[0], statuses[1]);
    assert_ne!(statuses[1], statuses[2]);
    assert_ne!(statuses[0], statuses[2]);
}
