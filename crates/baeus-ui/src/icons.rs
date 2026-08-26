use crate::theme::Color;
use gpui::SharedString;
use gpui_component::IconNamed;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceIcon {
    Pod,
    Deployment,
    StatefulSet,
    DaemonSet,
    ReplicaSet,
    Job,
    CronJob,
    Service,
    Ingress,
    ConfigMap,
    Secret,
    PersistentVolume,
    PersistentVolumeClaim,
    StorageClass,
    Namespace,
    Node,
    ServiceAccount,
    Role,
    ClusterRole,
    RoleBinding,
    ClusterRoleBinding,
    NetworkPolicy,
    Endpoints,
    ResourceQuota,
    LimitRange,
    HorizontalPodAutoscaler,
    PodDisruptionBudget,
    PriorityClass,
    Lease,
    ValidatingWebhookConfiguration,
    MutatingWebhookConfiguration,
    EndpointSlice,
    IngressClass,
    HelmRelease,
    HelmChart,
    Event,
    Plugin,
    Application,
    ApplicationSet,
    AppProject,
    CustomResource,
    Unknown,
}

impl ResourceIcon {
    pub fn from_kind(kind: &str) -> Self {
        match kind {
            "Pod" => Self::Pod,
            "Deployment" => Self::Deployment,
            "StatefulSet" => Self::StatefulSet,
            "DaemonSet" => Self::DaemonSet,
            "ReplicaSet" => Self::ReplicaSet,
            "Job" => Self::Job,
            "CronJob" => Self::CronJob,
            "Service" => Self::Service,
            "Ingress" => Self::Ingress,
            "ConfigMap" => Self::ConfigMap,
            "Secret" => Self::Secret,
            "PersistentVolume" => Self::PersistentVolume,
            "PersistentVolumeClaim" => Self::PersistentVolumeClaim,
            "StorageClass" => Self::StorageClass,
            "Namespace" => Self::Namespace,
            "Node" => Self::Node,
            "ServiceAccount" => Self::ServiceAccount,
            "Role" => Self::Role,
            "ClusterRole" => Self::ClusterRole,
            "RoleBinding" => Self::RoleBinding,
            "ClusterRoleBinding" => Self::ClusterRoleBinding,
            "NetworkPolicy" => Self::NetworkPolicy,
            "Endpoints" => Self::Endpoints,
            "ResourceQuota" => Self::ResourceQuota,
            "LimitRange" => Self::LimitRange,
            "HorizontalPodAutoscaler" => Self::HorizontalPodAutoscaler,
            "PodDisruptionBudget" => Self::PodDisruptionBudget,
            "PriorityClass" => Self::PriorityClass,
            "Lease" => Self::Lease,
            "ValidatingWebhookConfiguration" => Self::ValidatingWebhookConfiguration,
            "MutatingWebhookConfiguration" => Self::MutatingWebhookConfiguration,
            "EndpointSlice" => Self::EndpointSlice,
            "IngressClass" => Self::IngressClass,
            "Event" => Self::Event,
            "Plugin" => Self::Plugin,
            "Application" => Self::Application,
            "ApplicationSet" => Self::ApplicationSet,
            "AppProject" => Self::AppProject,
            _ => Self::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pod => "Pod",
            Self::Deployment => "Deployment",
            Self::StatefulSet => "StatefulSet",
            Self::DaemonSet => "DaemonSet",
            Self::ReplicaSet => "ReplicaSet",
            Self::Job => "Job",
            Self::CronJob => "CronJob",
            Self::Service => "Service",
            Self::Ingress => "Ingress",
            Self::ConfigMap => "ConfigMap",
            Self::Secret => "Secret",
            Self::PersistentVolume => "PersistentVolume",
            Self::PersistentVolumeClaim => "PersistentVolumeClaim",
            Self::StorageClass => "StorageClass",
            Self::Namespace => "Namespace",
            Self::Node => "Node",
            Self::ServiceAccount => "ServiceAccount",
            Self::Role => "Role",
            Self::ClusterRole => "ClusterRole",
            Self::RoleBinding => "RoleBinding",
            Self::ClusterRoleBinding => "ClusterRoleBinding",
            Self::NetworkPolicy => "NetworkPolicy",
            Self::Endpoints => "Endpoints",
            Self::ResourceQuota => "ResourceQuota",
            Self::LimitRange => "LimitRange",
            Self::HorizontalPodAutoscaler => "HorizontalPodAutoscaler",
            Self::PodDisruptionBudget => "PodDisruptionBudget",
            Self::PriorityClass => "PriorityClass",
            Self::Lease => "Lease",
            Self::ValidatingWebhookConfiguration => "ValidatingWebhookConfiguration",
            Self::MutatingWebhookConfiguration => "MutatingWebhookConfiguration",
            Self::EndpointSlice => "EndpointSlice",
            Self::IngressClass => "IngressClass",
            Self::HelmRelease => "HelmRelease",
            Self::HelmChart => "HelmChart",
            Self::Event => "Event",
            Self::Plugin => "Plugin",
            Self::Application => "Application",
            Self::ApplicationSet => "ApplicationSet",
            Self::AppProject => "AppProject",
            Self::CustomResource => "CustomResource",
            Self::Unknown => "Unknown",
        }
    }

    pub fn category(&self) -> ResourceCategory {
        match self {
            Self::Pod
            | Self::Deployment
            | Self::StatefulSet
            | Self::DaemonSet
            | Self::ReplicaSet
            | Self::Job
            | Self::CronJob
            | Self::HorizontalPodAutoscaler => ResourceCategory::Workloads,
            Self::Service
            | Self::Ingress
            | Self::NetworkPolicy
            | Self::Endpoints
            | Self::EndpointSlice
            | Self::IngressClass => ResourceCategory::Network,
            Self::ConfigMap | Self::Secret | Self::ResourceQuota | Self::LimitRange => {
                ResourceCategory::Configuration
            }
            Self::PersistentVolume | Self::PersistentVolumeClaim | Self::StorageClass => {
                ResourceCategory::Storage
            }
            Self::Namespace
            | Self::Node
            | Self::PriorityClass
            | Self::Lease
            | Self::PodDisruptionBudget
            | Self::ValidatingWebhookConfiguration
            | Self::MutatingWebhookConfiguration => ResourceCategory::Cluster,
            Self::ServiceAccount
            | Self::Role
            | Self::ClusterRole
            | Self::RoleBinding
            | Self::ClusterRoleBinding => ResourceCategory::Rbac,
            Self::HelmRelease | Self::HelmChart => ResourceCategory::Helm,
            Self::Event => ResourceCategory::Monitoring,
            Self::Plugin => ResourceCategory::Plugins,
            Self::Application | Self::ApplicationSet | Self::AppProject => ResourceCategory::ArgoCD,
            Self::CustomResource | Self::Unknown => ResourceCategory::Custom,
        }
    }
}

impl IconNamed for ResourceIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Pod => "icons/res-pod.svg",
            Self::Deployment => "icons/res-deployment.svg",
            Self::StatefulSet => "icons/res-statefulset.svg",
            Self::DaemonSet => "icons/res-daemonset.svg",
            Self::ReplicaSet => "icons/res-replicaset.svg",
            Self::Job => "icons/res-job.svg",
            Self::CronJob => "icons/res-cronjob.svg",
            Self::Service => "icons/res-service.svg",
            Self::Ingress => "icons/res-ingress.svg",
            Self::ConfigMap => "icons/res-configmap.svg",
            Self::Secret => "icons/res-secret.svg",
            Self::PersistentVolume => "icons/res-pv.svg",
            Self::PersistentVolumeClaim => "icons/res-pvc.svg",
            Self::StorageClass => "icons/res-storageclass.svg",
            Self::Namespace => "icons/res-namespace.svg",
            Self::Node => "icons/res-node.svg",
            Self::ServiceAccount => "icons/res-serviceaccount.svg",
            Self::Role => "icons/res-role.svg",
            Self::ClusterRole => "icons/res-clusterrole.svg",
            Self::RoleBinding => "icons/res-rolebinding.svg",
            Self::ClusterRoleBinding => "icons/res-clusterrolebinding.svg",
            Self::NetworkPolicy => "icons/res-networkpolicy.svg",
            Self::Endpoints => "icons/res-endpoints.svg",
            Self::ResourceQuota => "icons/res-resourcequota.svg",
            Self::LimitRange => "icons/res-limitrange.svg",
            Self::HorizontalPodAutoscaler => "icons/res-hpa.svg",
            Self::PodDisruptionBudget => "icons/res-pdb.svg",
            Self::PriorityClass => "icons/res-priorityclass.svg",
            Self::Lease => "icons/res-lease.svg",
            Self::ValidatingWebhookConfiguration => "icons/res-webhook.svg",
            Self::MutatingWebhookConfiguration => "icons/res-webhook.svg",
            Self::EndpointSlice => "icons/res-endpoints.svg",
            Self::IngressClass => "icons/res-ingress.svg",
            Self::HelmRelease => "icons/res-helmrelease.svg",
            Self::HelmChart => "icons/res-helmchart.svg",
            Self::Event => "icons/res-event.svg",
            Self::Plugin => "icons/res-plugin.svg",
            Self::Application => "icons/res-application.svg",
            Self::ApplicationSet => "icons/res-applicationset.svg",
            Self::AppProject => "icons/res-appproject.svg",
            Self::CustomResource => "icons/res-customresource.svg",
            Self::Unknown => "icons/res-unknown.svg",
        }
        .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceCategory {
    Workloads,
    Network,
    Configuration,
    Storage,
    Cluster,
    Rbac,
    Helm,
    Monitoring,
    ArgoCD,
    Plugins,
    Custom,
}

impl ResourceCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Workloads => "Workloads",
            Self::Network => "Network",
            Self::Configuration => "Configuration",
            Self::Storage => "Storage",
            Self::Cluster => "Cluster",
            Self::Rbac => "RBAC",
            Self::Helm => "Helm",
            Self::Monitoring => "Monitoring",
            Self::ArgoCD => "ArgoCD",
            Self::Plugins => "Plugins",
            Self::Custom => "Custom Resources",
        }
    }

    pub fn all() -> &'static [ResourceCategory] {
        &[
            Self::Cluster,
            Self::Workloads,
            Self::Network,
            Self::Configuration,
            Self::Storage,
            Self::Rbac,
            Self::Helm,
            Self::Monitoring,
            Self::ArgoCD,
            Self::Plugins,
            Self::Custom,
        ]
    }
}

impl IconNamed for ResourceCategory {
    fn path(self) -> SharedString {
        match self {
            Self::Cluster => "icons/cat-cluster.svg",
            Self::Workloads => "icons/cat-workloads.svg",
            Self::Network => "icons/cat-network.svg",
            Self::Configuration => "icons/cat-configuration.svg",
            Self::Storage => "icons/cat-storage.svg",
            Self::Rbac => "icons/cat-rbac.svg",
            Self::Helm => "icons/cat-helm.svg",
            Self::Monitoring => "icons/cat-monitoring.svg",
            Self::ArgoCD => "icons/cat-argocd.svg",
            Self::Plugins => "icons/cat-plugins.svg",
            Self::Custom => "icons/cat-custom.svg",
        }
        .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusIndicator {
    Healthy,
    Warning,
    Error,
    Unknown,
    Pending,
    Terminating,
}

impl StatusIndicator {
    pub fn color(&self, is_dark: bool) -> Color {
        if is_dark {
            match self {
                Self::Healthy => Color::rgb(74, 222, 128),
                Self::Warning => Color::rgb(251, 191, 36),
                Self::Error => Color::rgb(248, 113, 113),
                Self::Unknown => Color::rgb(156, 163, 175),
                Self::Pending => Color::rgb(96, 165, 250),
                Self::Terminating => Color::rgb(251, 191, 36),
            }
        } else {
            match self {
                Self::Healthy => Color::rgb(34, 197, 94),
                Self::Warning => Color::rgb(245, 158, 11),
                Self::Error => Color::rgb(239, 68, 68),
                Self::Unknown => Color::rgb(156, 163, 175),
                Self::Pending => Color::rgb(59, 130, 246),
                Self::Terminating => Color::rgb(245, 158, 11),
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Unknown => "Unknown",
            Self::Pending => "Pending",
            Self::Terminating => "Terminating",
        }
    }
}

/// Section icons for detail view sections (Lucide SVGs).
/// Uses `IconNamed` from gpui-component to work with `Icon::new(SectionIcon::Containers)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionIcon {
    /// Overview / Properties — uses built-in info icon
    Info,
    /// Events — uses built-in bell icon
    Events,
    /// Containers
    Containers,
    /// Init Containers
    InitContainers,
    /// Volumes
    Volumes,
    /// Labels
    Labels,
    /// Annotations
    Annotations,
    /// Conditions
    Conditions,
    /// Tolerations
    Tolerations,
    /// Affinity
    Affinity,
    /// Node Selector
    NodeSelector,
    /// Probes
    Probes,
    /// Security Context
    Security,
    /// Resources
    Resources,
    /// Command / Args
    Terminal,
    /// Image
    Image,
    /// Controlled By
    ControlledBy,
    /// Ports
    Ports,
    /// Env Variables
    EnvVars,
    /// Volume Mounts
    VolumeMounts,
    /// Capacity (Node capacity/allocatable)
    Capacity,
    /// Allocatable (Node allocatable resources)
    Allocatable,
    /// Addresses (Node addresses)
    Addresses,
    /// Container Images (Node images)
    Images,
}

impl SectionIcon {
    /// Human-readable label for this section.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Overview",
            Self::Events => "Events",
            Self::Containers => "Containers",
            Self::InitContainers => "Init Containers",
            Self::Volumes => "Volumes",
            Self::Labels => "Labels",
            Self::Annotations => "Annotations",
            Self::Conditions => "Conditions",
            Self::Tolerations => "Tolerations",
            Self::Affinity => "Affinity",
            Self::NodeSelector => "Node Selector",
            Self::Probes => "Probes",
            Self::Security => "Security Context",
            Self::Resources => "Resources",
            Self::Terminal => "Command / Args",
            Self::Image => "Image",
            Self::ControlledBy => "Controlled By",
            Self::Ports => "Ports",
            Self::EnvVars => "Environment Variables",
            Self::VolumeMounts => "Volume Mounts",
            Self::Capacity => "Capacity",
            Self::Allocatable => "Allocatable",
            Self::Addresses => "Addresses",
            Self::Images => "Images",
        }
    }
}

impl IconNamed for SectionIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Info => "icons/info.svg",
            Self::Events => "icons/bell.svg",
            Self::Containers => "icons/section-box.svg",
            Self::InitContainers => "icons/section-play.svg",
            Self::Volumes => "icons/section-hard-drive.svg",
            Self::Labels => "icons/section-tag.svg",
            Self::Annotations => "icons/section-sticky-note.svg",
            Self::Conditions => "icons/section-list-checks.svg",
            Self::Tolerations => "icons/section-shield.svg",
            Self::Affinity => "icons/section-magnet.svg",
            Self::NodeSelector => "icons/section-server.svg",
            Self::Probes => "icons/section-heart-pulse.svg",
            Self::Security => "icons/section-lock.svg",
            Self::Resources => "icons/section-gauge.svg",
            Self::Terminal => "icons/section-terminal.svg",
            Self::Image => "icons/section-layers.svg",
            Self::ControlledBy => "icons/section-git-fork.svg",
            Self::Ports => "icons/section-network.svg",
            Self::EnvVars => "icons/section-variable.svg",
            Self::VolumeMounts => "icons/section-folder-symlink.svg",
            Self::Capacity => "icons/section-cpu.svg",
            Self::Allocatable => "icons/section-cpu.svg",
            Self::Addresses => "icons/section-globe.svg",
            Self::Images => "icons/section-container.svg",
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_icon_from_kind() {
        assert_eq!(ResourceIcon::from_kind("Pod"), ResourceIcon::Pod);
        assert_eq!(ResourceIcon::from_kind("Deployment"), ResourceIcon::Deployment);
        assert_eq!(ResourceIcon::from_kind("Service"), ResourceIcon::Service);
        assert_eq!(ResourceIcon::from_kind("UnknownKind"), ResourceIcon::Unknown);
    }

    #[test]
    fn test_resource_icon_category() {
        assert_eq!(ResourceIcon::Pod.category(), ResourceCategory::Workloads);
        assert_eq!(ResourceIcon::Service.category(), ResourceCategory::Network);
        assert_eq!(ResourceIcon::ConfigMap.category(), ResourceCategory::Configuration);
        assert_eq!(ResourceIcon::PersistentVolume.category(), ResourceCategory::Storage);
        assert_eq!(ResourceIcon::Node.category(), ResourceCategory::Cluster);
        assert_eq!(ResourceIcon::Role.category(), ResourceCategory::Rbac);
    }

    #[test]
    fn test_resource_category_all() {
        let categories = ResourceCategory::all();
        assert_eq!(categories.len(), 11);
    }

    #[test]
    fn test_status_indicator_colors_differ_by_theme() {
        let dark_color = StatusIndicator::Healthy.color(true);
        let light_color = StatusIndicator::Healthy.color(false);
        assert_ne!(dark_color, light_color);
    }

    #[test]
    fn test_status_indicator_labels() {
        assert_eq!(StatusIndicator::Healthy.label(), "Healthy");
        assert_eq!(StatusIndicator::Error.label(), "Error");
        assert_eq!(StatusIndicator::Terminating.label(), "Terminating");
    }
}
