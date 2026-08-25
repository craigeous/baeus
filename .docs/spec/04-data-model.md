# 04 — Data Model

**Status**: Draft (descriptive back-fill, 2026-08-25 — pending Plan Review)

Core entities as designed in the original speckit data model and reflected in
the implementation (`baeus-core` models, `baeus-ui::models`, pod detail typed
structs). Field-level detail is in the historical source
(`baeus-spec:specs/001-k8s-cluster-ui/data-model.md`); this file records the
entity set and relationships.

## Entities

- **ClusterConnection** — a kubeconfig-context-derived cluster connection:
  identity (id, name, context_name, api_server_url), `ConnectionStatus`
  (Disconnected → Connecting → Connected; Error from any state) with optional
  error message, `AuthMethod` (Certificate | Token | OIDC | ExecPlugin),
  TLS always verified, last-connected timestamp, favorite flag.
- **Namespace** — name, parent cluster, phase (Active/Terminating), labels,
  annotations, resource_version.
- **Resource** — generic K8s object (built-in or CRD instance): uid, name,
  optional namespace, kind, api_version, labels, annotations,
  creation_timestamp, resource_version (optimistic concurrency),
  owner_references, opaque spec/status JSON, extracted `Condition`s,
  parent cluster. Identity: (name, namespace, kind, api_version) per cluster.
- **OwnerReference** — uid/kind/name/api_version/controller link forming the
  resource DAG (drives "move-up" log navigation and the relationship map).
- **Condition** — type, True/False/Unknown status, reason, message,
  last_transition.
- **Event** — Normal/Warning type, reason, message, timestamp, count,
  involved-resource reference, parent cluster. UI note: dashboard events use
  `is_warning: bool` and `timestamp: DateTime<Utc>`.
- **HelmRelease** — name, namespace, chart name/version, optional app
  version, status (Deployed, Failed, Uninstalling, PendingInstall,
  PendingUpgrade, PendingRollback, Superseded), revision, last_deployed,
  merged values, parent cluster. Backed by cluster Secrets
  (`sh.helm.release.v1.*`), base64+gzip decoded in `baeus-helm`.
- **HelmRepository** — name, URL, enabled flag.
- **CrdSchema** — name, group, kind, versions, scope (Namespaced|Cluster),
  optional OpenAPI v3 schema, parent cluster; instances are `Resource`s via
  kube-rs `DynamicObject`.
- **Plugin** — id, name, version, description, author, enabled, installed_at,
  config blob, `.dylib` library path.
- **UserPreferences** (persisted locally) — theme (Light/Dark/System),
  default namespace, favorite clusters, keybinding overrides,
  `kubeconfig_scan_dirs` (recursive, depth 3), log line limit (10 000),
  font size, sidebar width/collapse, dock height/collapse; extended after the
  original design with AWS/EKS and updater preferences.
- **PodDetailData** and 10 companion typed structs (`ContainerDetail`, etc.)
  in `baeus-ui::components::pod_detail` — view-model extraction from raw
  resource JSON via `json_extract.rs` (`extract_pod_detail()` plus per-kind
  extraction for tables).

## Relationships

ClusterConnection 1→N Namespace / Event / HelmRelease / CrdSchema;
Namespace 1→N Resource; Resource N→N Resource via owner references (DAG);
Resource 1→N Event; CrdSchema 1→N Resource instances;
HelmRepository 1→N HelmRelease (logical).
