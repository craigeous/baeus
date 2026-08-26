mod app;
mod assets;
mod settings;

use baeus_ui::layout::app_shell::{GpuiTokioHandle, OpenPreferencesAction};
use baeus_ui::views::preferences::PreferencesState;
use gpui::{AppContext as _, Menu, MenuItem, OsAction, actions, px};
use settings::UserPreferences;

// Menu bar actions
actions!(baeus, [QuitApp, Undo, Redo, Cut, Copy, Paste, SelectAll]);

fn quit_app(_: &QuitApp, cx: &mut gpui::App) {
    cx.quit();
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Baeus - Kubernetes Cluster Management UI");

    // Install rustls CryptoProvider early — must happen before any TLS connections.
    // Both kube-rs and AWS SDK use rustls, and having rustls as a direct dependency
    // disables auto-detection, requiring explicit provider installation.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // macOS .app bundles launched from Finder/Spotlight get a minimal environment
    // without the user's shell PATH. Credential plugins (aws, gcloud, etc.) that
    // kube-rs invokes won't be found. Fix by sourcing the user's login shell env.
    setup_macos_environment();

    // Spawn Tokio runtime on a background thread for async K8s API calls.
    // The GPUI event loop owns the main thread; Tokio runs in parallel.
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("baeus-tokio")
        .build()
        .expect("Failed to create Tokio runtime");

    // Keep a handle so UI code can spawn async tasks on the Tokio runtime.
    let tokio_handle = tokio_runtime.handle().clone();

    // Discover clusters from kubeconfig files before opening the window.
    let clusters = discover_clusters();

    // Load user preferences to pass into AppShell.
    let prefs = UserPreferences::load().unwrap_or_default();
    let prefs_state = PreferencesState {
        theme_mode: match prefs.theme {
            settings::Theme::Light => baeus_ui::theme::ThemeMode::Light,
            settings::Theme::Dark => baeus_ui::theme::ThemeMode::Dark,
            settings::Theme::System => baeus_ui::theme::ThemeMode::System,
        },
        font_size: prefs.font_size,
        log_line_limit: prefs.log_line_limit,
        default_namespace: prefs.default_namespace.clone(),
        kubeconfig_scan_dirs: prefs
            .kubeconfig_scan_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        terminal_shell_path: prefs.terminal_shell_path.clone(),
        sidebar_collapsed: prefs.sidebar_collapsed,
        default_aws_profile: prefs.default_aws_profile.clone(),
        cluster_aws_profiles: prefs.cluster_aws_profiles.clone(),
        saved_eks_connections: prefs
            .saved_eks_connections
            .iter()
            .map(|c| baeus_ui::views::preferences::SavedEksConnectionInfo {
                cluster_name: c.cluster_name.clone(),
                cluster_arn: c.cluster_arn.clone(),
                endpoint: c.endpoint.clone(),
                region: c.region.clone(),
                certificate_authority_data: c.certificate_authority_data.clone(),
                auth_method: match c.auth_method {
                    settings::SavedEksAuthMethod::Sso => "Sso".to_string(),
                    settings::SavedEksAuthMethod::AccessKey => "AccessKey".to_string(),
                    settings::SavedEksAuthMethod::AssumeRole => "AssumeRole".to_string(),
                },
                sso_start_url: c.sso_start_url.clone(),
                sso_region: c.sso_region.clone(),
                role_arn: c.role_arn.clone(),
            })
            .collect(),
    };

    let app = gpui::Application::new().with_assets(assets::BaeusAssets);

    app.run(move |cx: &mut gpui::App| {
        // Store the Tokio handle as a GPUI global so views can access it.
        cx.set_global(GpuiTokioHandle(tokio_handle.clone()));

        gpui_component::init(cx);

        // Activate app so the menu bar appears in the foreground.
        cx.activate(true);

        // Register global action handlers.
        cx.on_action(quit_app);

        // Set up macOS menu bar.
        cx.set_menus(vec![
            Menu {
                name: "Baeus".into(),
                items: vec![
                    MenuItem::action("Preferences...", OpenPreferencesAction),
                    MenuItem::separator(),
                    MenuItem::action("Quit Baeus", QuitApp),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Undo", Undo, OsAction::Undo),
                    MenuItem::os_action("Redo", Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action("Cut", Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                ],
            },
        ]);

        // Quit the app when the last window is closed.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let clusters_for_window = clusters.clone();
        let prefs_for_window = prefs_state.clone();
        cx.open_window(
            gpui::WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Baeus \u{2014} Kubernetes Cluster Management".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: if cfg!(target_os = "macos") {
                        Some(gpui::point(px(13.), px(13.)))
                    } else {
                        None
                    },
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    baeus_ui::layout::app_shell::AppShell::new(
                        clusters_for_window,
                        prefs_for_window,
                        window,
                        cx,
                    )
                });
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .expect("Failed to open main window");
    });

    // Shutdown the Tokio runtime gracefully after the GPUI event loop exits.
    tokio_runtime.shutdown_background();
}

/// Discover cluster contexts from all kubeconfig files (default + user-configured dirs).
/// Returns (effective_name, display_name, kubeconfig_path, original_context_name) tuples.
/// `effective_name` is unique across all kubeconfigs (disambiguated for duplicates).
/// `original_context_name` is the name as it appears in the kubeconfig file.
fn discover_clusters() -> Vec<(String, String, String, String)> {
    let prefs = UserPreferences::load().unwrap_or_default();

    let discovery = baeus_core::kubeconfig::KubeconfigDiscovery::new()
        .with_additional_dirs(prefs.kubeconfig_scan_dirs);

    let loaded = match discovery.load_all() {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("Failed to discover kubeconfigs: {e}");
            return Vec::new();
        }
    };

    let mut clusters = Vec::new();
    let mut seen_contexts = std::collections::HashSet::new();
    for (path, loader) in &loaded {
        let path_str = path.to_string_lossy().to_string();
        for ctx in loader.contexts() {
            // Use the original context name if unique, otherwise disambiguate
            // with the cluster name. This handles cloud-generated kubeconfig
            // files that all use "default" as the context name.
            let effective_name = if seen_contexts.contains(&ctx.name) {
                // Disambiguate: use "cluster@context" or just "cluster" if context is generic
                if ctx.cluster_name.is_empty() || ctx.cluster_name == ctx.name {
                    // Use filename stem as disambiguator
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    format!("{}@{}", ctx.name, stem)
                } else {
                    format!("{}@{}", ctx.cluster_name, ctx.name)
                }
            } else {
                ctx.name.clone()
            };

            if !seen_contexts.insert(effective_name.clone()) {
                tracing::debug!(
                    "Skipping duplicate context '{}' from {}",
                    effective_name,
                    path_str,
                );
                continue;
            }

            let display_name = effective_name.clone();
            clusters.push((effective_name, display_name, path_str.clone(), ctx.name.clone()));
        }
    }

    tracing::info!("Discovered {} cluster context(s)", clusters.len());
    clusters
}

/// Ensure the process has the user's full shell environment.
///
/// macOS .app bundles launched from Finder/Spotlight/Launchpad inherit a minimal
/// environment (no user PATH, no KUBECONFIG, etc.). This means credential plugins
/// like `aws-iam-authenticator` or `gke-gcloud-auth-plugin` that kube-rs needs to
/// execute won't be found.
///
/// We fix this by asking the user's login shell to print its environment, then
/// importing key variables into the current process.
fn setup_macos_environment() {
    // Only needed on macOS; on Linux the desktop session usually inherits the shell env.
    if !cfg!(target_os = "macos") {
        return;
    }

    // If PATH already looks rich (contains /usr/local or homebrew), we were likely
    // launched from a terminal and don't need to do anything.
    if let Ok(path) = std::env::var("PATH") {
        if path.contains("/usr/local") || path.contains("homebrew") || path.contains(".cargo") {
            tracing::debug!("PATH already includes user paths, skipping shell env import");
            return;
        }
    }

    tracing::info!("Importing user shell environment for macOS .app bundle");

    // Determine the user's login shell.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    // Run `env` inside a login shell to capture the full environment.
    let output = std::process::Command::new(&shell).args(["-l", "-c", "env"]).output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            // Truncate stderr to avoid leaking credentials from shell profiles.
            let stderr = String::from_utf8_lossy(&o.stderr);
            let truncated: String = stderr.chars().take(100).collect();
            tracing::warn!(
                "Shell env import exited with {}: {}{}",
                o.status,
                truncated,
                if stderr.len() > 100 { "..." } else { "" },
            );
            return;
        }
        Err(e) => {
            tracing::warn!("Failed to run {shell} for env import: {e}");
            return;
        }
    };

    let env_str = String::from_utf8_lossy(&output.stdout);

    // Variables that are important for Kubernetes credential plugins and TLS.
    let important_vars = [
        "PATH",
        "KUBECONFIG",
        "HOME",
        "USER",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "AWS_PROFILE",
        "AWS_DEFAULT_REGION",
        "AWS_REGION",
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "CLOUDSDK_CONFIG",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "AZURE_CONFIG_DIR",
        "GOPATH",
    ];

    for line in env_str.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if important_vars.contains(&key) {
                // Only set if not already present or if it's PATH (always override).
                if key == "PATH" || std::env::var(key).is_err() {
                    // SAFETY: This runs single-threaded at the very start of main(),
                    // before the Tokio runtime or any other threads are spawned.
                    // IMPORTANT: Do NOT move Tokio runtime creation or any thread
                    // spawning before this point — set_var is not thread-safe.
                    unsafe { std::env::set_var(key, value) };
                    tracing::debug!("Imported {key}");
                }
            }
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        tracing::info!("PATH after import: {path}");
    }
}
